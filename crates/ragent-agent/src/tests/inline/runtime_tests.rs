//! Inline tests for `runtime.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use ragent_types::trigger::{
    TriggerActionKind, TriggerEnvelope, TriggerRuleStatus, TriggerSourceKind,
};

fn make_envelope(source: &str, summary: &str, action: &str) -> TriggerEnvelope {
    TriggerEnvelope::new(
        TriggerSourceKind::McpNotification,
        source,
        summary,
        action,
        TriggerActionKind::InjectSummary,
        false,
    )
}

#[test]
fn test_first_envelope_passes() {
    let rt = TriggerRuntime::default();
    let env = make_envelope("server-1", "build done", "report it");
    assert!(rt.process(env).is_some());
}

#[test]
fn test_duplicate_within_window_suppressed() {
    let rt = TriggerRuntime::default();
    let env1 = make_envelope("server-1", "build done", "report it");
    let env2 = make_envelope("server-1", "build done", "report it");
    assert!(rt.process(env1).is_some());
    assert!(rt.process(env2).is_none()); // suppressed
}

#[test]
fn test_different_content_not_suppressed() {
    let rt = TriggerRuntime::default();
    let env1 = make_envelope("server-1", "build done", "report it");
    let env2 = make_envelope("server-1", "build failed", "report it");
    assert!(rt.process(env1).is_some());
    assert!(rt.process(env2).is_some()); // different content
}

#[test]
fn test_different_source_not_suppressed() {
    let rt = TriggerRuntime::default();
    let env1 = make_envelope("server-1", "build done", "report it");
    let env2 = make_envelope("server-2", "build done", "report it");
    assert!(rt.process(env1).is_some());
    assert!(rt.process(env2).is_some());
}

#[test]
fn test_cycle_suppression_after_max_cycles() {
    let config = TriggerRuntimeConfig {
        dedup_window: Duration::from_secs(0), // no dedup window so only cycle matters
        max_cycles: 3,
    };
    let rt = TriggerRuntime::new(config);

    // First 3 firings of the same source+content pass (cycle count 1,2,3)
    for _ in 0..3 {
        let env = make_envelope("server-1", "build done", "report it");
        assert!(
            rt.process(env).is_some(),
            "Should pass within max_cycles boundary"
        );
    }

    // 4th firing is suppressed (consecutive > max_cycles)
    let env = make_envelope("server-1", "build done", "report it");
    assert!(rt.process(env).is_none(), "Should be cycle-suppressed");
}

#[test]
fn test_cycle_resets_on_different_content() {
    let config = TriggerRuntimeConfig {
        dedup_window: Duration::from_secs(0),
        max_cycles: 2,
    };
    let rt = TriggerRuntime::new(config);

    // Fire same content twice
    let env = make_envelope("server-1", "build done", "report it");
    assert!(rt.process(env).is_some());
    let env = make_envelope("server-1", "build done", "report it");
    assert!(rt.process(env).is_some());

    // Different content resets cycle
    let env = make_envelope("server-1", "build failed", "report it");
    assert!(rt.process(env).is_some());

    // Same content as before the reset - should pass (cycle reset)
    let env = make_envelope("server-1", "build done", "report it");
    assert!(rt.process(env).is_some());
}

#[test]
fn test_rule_add_remove_enable_disable() {
    let rt = TriggerRuntime::default();
    let rule = TriggerRule::new("file exists", "print it");
    let id = rt.add_rule(rule);
    assert_eq!(rt.rule_count(), 1);

    assert!(rt.disable_rule(id.as_str()));
    let r = rt.get_rule(id.as_str()).unwrap();
    assert!(!r.enabled);
    assert_eq!(r.status(), TriggerRuleStatus::Disabled);

    assert!(rt.enable_rule(id.as_str()));
    let r = rt.get_rule(id.as_str()).unwrap();
    assert!(r.enabled);
    assert_eq!(r.status(), TriggerRuleStatus::Active);

    assert!(rt.remove_rule(id.as_str()));
    assert_eq!(rt.rule_count(), 0);
    assert!(!rt.remove_rule(id.as_str())); // already gone
}

#[test]
fn test_dynamic_trigger_marks_rule_fired() {
    let rt = TriggerRuntime::default();
    let mut rule = TriggerRule::new("file exists", "print it");
    rule.id = TriggerRuleId::from("rule-test-1");
    rt.add_rule(rule);

    let env = TriggerEnvelope::new(
        TriggerSourceKind::Dynamic,
        "rule-test-1",
        "file exists",
        "print it",
        TriggerActionKind::SubAgent,
        false,
    );
    let fired = rt.process(env).unwrap();
    assert_eq!(fired.rule_id.as_ref().unwrap().as_str(), "rule-test-1");

    let r = rt.get_rule("rule-test-1").unwrap();
    assert!(r.fired_at.is_some());
    assert_eq!(r.status(), TriggerRuleStatus::Fired);
}

#[test]
fn test_purge_expired() {
    let config = TriggerRuntimeConfig {
        dedup_window: Duration::from_millis(10),
        max_cycles: 100,
    };
    let rt = TriggerRuntime::new(config);

    let env = make_envelope("s1", "msg", "act");
    rt.process(env);
    assert_eq!(rt.dedup_cache_size(), 1);

    std::thread::sleep(Duration::from_millis(50));
    let purged = rt.purge_expired();
    assert_eq!(purged, 1);
    assert_eq!(rt.dedup_cache_size(), 0);
}

#[test]
fn test_clear() {
    let rt = TriggerRuntime::default();
    rt.add_rule(TriggerRule::new("cond", "act"));
    let env = make_envelope("s1", "msg", "act");
    rt.process(env);
    assert_eq!(rt.rule_count(), 1);
    assert_eq!(rt.dedup_cache_size(), 1);

    rt.clear();
    assert_eq!(rt.rule_count(), 0);
    assert_eq!(rt.dedup_cache_size(), 0);
    assert_eq!(rt.cycle_tracker_size(), 0);
}

#[test]
fn test_shared_state_via_clone() {
    let rt = TriggerRuntime::default();
    let rt2 = rt.clone();
    let id = rt.add_rule(TriggerRule::new("cond", "act"));
    // The clone shares the same state.
    assert_eq!(rt2.rule_count(), 1);
    assert!(rt2.get_rule(id.as_str()).is_some());
}
