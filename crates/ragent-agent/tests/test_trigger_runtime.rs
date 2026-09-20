//! Integration tests for the trigger runtime (spec `piegap`, T-001).
//!
//! These tests verify the trigger runtime's deduplication and cycle
//! suppression logic via the crate's public API.

use ragent_agent::trigger::{TriggerRuntime, TriggerRuntimeConfig};
use ragent_types::trigger::{
    TriggerActionKind, TriggerEnvelope, TriggerRule, TriggerRuleId, TriggerSourceKind,
};
use std::time::Duration;

fn make_mcp_envelope(source: &str, summary: &str, action: &str) -> TriggerEnvelope {
    TriggerEnvelope::new(
        TriggerSourceKind::McpNotification,
        source,
        summary,
        action,
        TriggerActionKind::InjectSummary,
        false,
    )
}

fn make_dynamic_envelope(rule_id: &str, condition: &str, action: &str) -> TriggerEnvelope {
    TriggerEnvelope::new(
        TriggerSourceKind::Dynamic,
        rule_id,
        condition,
        action,
        TriggerActionKind::SubAgent,
        false,
    )
}

// ── Deduplication tests ───────────────────────────────────────────────────

#[test]
fn test_first_envelope_is_dispatched() {
    let rt = TriggerRuntime::default();
    let env = make_mcp_envelope("srv-1", "build done", "report");
    assert!(rt.process(env).is_some());
}

#[test]
fn test_duplicate_envelope_is_suppressed() {
    let rt = TriggerRuntime::default();
    let env1 = make_mcp_envelope("srv-1", "build done", "report");
    let env2 = make_mcp_envelope("srv-1", "build done", "report");
    assert!(rt.process(env1).is_some());
    assert!(rt.process(env2).is_none());
}

#[test]
fn test_different_content_passes_dedup() {
    let rt = TriggerRuntime::default();
    let env1 = make_mcp_envelope("srv-1", "build done", "report");
    let env2 = make_mcp_envelope("srv-1", "build failed", "report");
    assert!(rt.process(env1).is_some());
    assert!(rt.process(env2).is_some());
}

#[test]
fn test_different_source_passes_dedup() {
    let rt = TriggerRuntime::default();
    let env1 = make_mcp_envelope("srv-1", "build done", "report");
    let env2 = make_mcp_envelope("srv-2", "build done", "report");
    assert!(rt.process(env1).is_some());
    assert!(rt.process(env2).is_some());
}

// ── Cycle suppression tests ───────────────────────────────────────────────

#[test]
fn test_cycle_suppression_kicks_in() {
    let config = TriggerRuntimeConfig {
        dedup_window: Duration::from_secs(0),
        max_cycles: 3,
    };
    let rt = TriggerRuntime::new(config);

    for _ in 0..3 {
        let env = make_mcp_envelope("srv-1", "same", "same");
        assert!(rt.process(env).is_some());
    }

    let env = make_mcp_envelope("srv-1", "same", "same");
    assert!(rt.process(env).is_none());
}

#[test]
fn test_cycle_resets_on_content_change() {
    let config = TriggerRuntimeConfig {
        dedup_window: Duration::from_secs(0),
        max_cycles: 2,
    };
    let rt = TriggerRuntime::new(config);

    let env = make_mcp_envelope("srv-1", "A", "act");
    assert!(rt.process(env).is_some());
    let env = make_mcp_envelope("srv-1", "A", "act");
    assert!(rt.process(env).is_some());

    // Different content resets cycle
    let env = make_mcp_envelope("srv-1", "B", "act");
    assert!(rt.process(env).is_some());

    // Original content should pass again after reset
    let env = make_mcp_envelope("srv-1", "A", "act");
    assert!(rt.process(env).is_some());
}

// ── Rule management tests ─────────────────────────────────────────────────

#[test]
fn test_rule_lifecycle() {
    let rt = TriggerRuntime::default();
    let rule = TriggerRule::new("cond", "act");
    let id = rt.add_rule(rule);
    assert_eq!(rt.rule_count(), 1);
    assert!(rt.get_rule(id.as_str()).is_some());

    assert!(rt.disable_rule(id.as_str()));
    assert!(rt.enable_rule(id.as_str()));

    assert!(rt.remove_rule(id.as_str()));
    assert_eq!(rt.rule_count(), 0);
    assert!(!rt.remove_rule(id.as_str()));
}

#[test]
fn test_rule_list_returns_all() {
    let rt = TriggerRuntime::default();
    rt.add_rule(TriggerRule::new("c1", "a1"));
    rt.add_rule(TriggerRule::new("c2", "a2"));
    assert_eq!(rt.list_rules().len(), 2);
}

// ── Dynamic trigger firing tests ─────────────────────────────────────────

#[test]
fn test_dynamic_envelope_marks_rule_fired() {
    let rt = TriggerRuntime::default();
    let mut rule = TriggerRule::new("file exists", "print it");
    rule.id = TriggerRuleId::from("rule-1");
    rt.add_rule(rule);

    let env = make_dynamic_envelope("rule-1", "file exists", "print it");
    let fired = rt.process(env).expect("should fire");
    assert_eq!(fired.rule_id.unwrap().as_str(), "rule-1");

    let r = rt.get_rule("rule-1").unwrap();
    assert!(r.fired_at.is_some());
}

#[test]
fn test_mcp_envelope_has_no_rule_id() {
    let rt = TriggerRuntime::default();
    let env = make_mcp_envelope("srv-1", "msg", "act");
    let fired = rt.process(env).expect("should fire");
    assert!(fired.rule_id.is_none());
}

// ── Maintenance tests ─────────────────────────────────────────────────────

#[test]
fn test_purge_removes_expired_entries() {
    let config = TriggerRuntimeConfig {
        dedup_window: Duration::from_millis(10),
        max_cycles: 100,
    };
    let rt = TriggerRuntime::new(config);

    rt.process(make_mcp_envelope("s", "m", "a"));
    assert_eq!(rt.dedup_cache_size(), 1);

    std::thread::sleep(Duration::from_millis(50));
    let purged = rt.purge_expired();
    assert_eq!(purged, 1);
    assert_eq!(rt.dedup_cache_size(), 0);
}

#[test]
fn test_clear_resets_all_state() {
    let rt = TriggerRuntime::default();
    rt.add_rule(TriggerRule::new("c", "a"));
    rt.process(make_mcp_envelope("s", "m", "a"));

    assert_eq!(rt.rule_count(), 1);
    assert_eq!(rt.dedup_cache_size(), 1);
    assert_eq!(rt.cycle_tracker_size(), 1);

    rt.clear();

    assert_eq!(rt.rule_count(), 0);
    assert_eq!(rt.dedup_cache_size(), 0);
    assert_eq!(rt.cycle_tracker_size(), 0);
}

// ── Shared state (Arc clone) tests ──────────────────────────────────────

#[test]
fn test_clone_shares_state() {
    let rt = TriggerRuntime::default();
    let rt2 = rt.clone();
    let id = rt.add_rule(TriggerRule::new("c", "a"));
    assert_eq!(rt2.rule_count(), 1);
    assert!(rt2.get_rule(id.as_str()).is_some());
}
