//! Integration tests for trigger envelope types (spec `piegap`, T-001).
//!
//! These tests verify the public API of the trigger types module via the
//! crate's public re-exports, without accessing internal implementation
//! details.

use chrono::Utc;
use ragent_types::{
    TriggerActionKind, TriggerEnvelope, TriggerRule, TriggerRuleId, TriggerRuleStatus,
    TriggerSourceKind,
};

#[test]
fn test_trigger_source_kind_serde_roundtrip() {
    let kind = TriggerSourceKind::Dynamic;
    let json = serde_json::to_string(&kind).expect("serialize");
    assert_eq!(json, r#""dynamic""#);
    let back: TriggerSourceKind = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, TriggerSourceKind::Dynamic);

    let kind = TriggerSourceKind::McpNotification;
    let json = serde_json::to_string(&kind).expect("serialize");
    assert_eq!(json, r#""mcp_notification""#);
}

#[test]
fn test_trigger_action_kind_serde_roundtrip() {
    let kinds = [
        (TriggerActionKind::InjectSummary, "inject_summary"),
        (TriggerActionKind::InjectAndRun, "inject_and_run"),
        (TriggerActionKind::SubAgent, "sub_agent"),
    ];
    for (kind, expected) in kinds {
        let json = serde_json::to_string(&kind).expect("serialize");
        assert_eq!(json, format!(r#""{expected}""#));
        let back: TriggerActionKind = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, kind);
    }
}

#[test]
fn test_trigger_rule_serde_roundtrip() {
    let rule = TriggerRule::new("file exists", "print it");
    let json = serde_json::to_string(&rule).expect("serialize");
    let back: TriggerRule = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back.condition, "file exists");
    assert_eq!(back.action, "print it");
    assert!(back.fire_once);
    assert!(back.enabled);
}

#[test]
fn test_trigger_rule_id_serde_and_display() {
    let id = TriggerRuleId::from("rule-42");
    let json = serde_json::to_string(&id).expect("serialize");
    assert_eq!(json, r#""rule-42""#);
    let back: TriggerRuleId = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, id);
    assert_eq!(id.to_string(), "rule-42");
    assert_eq!(id.as_str(), "rule-42");
}

#[test]
fn test_envelope_creation_populates_fields() {
    let env = TriggerEnvelope::new(
        TriggerSourceKind::McpNotification,
        "mcp-server-1",
        "Build completed",
        "Report the build result",
        TriggerActionKind::InjectSummary,
        false,
    );
    assert_ne!(env.id, String::new());
    assert_eq!(env.source_kind, TriggerSourceKind::McpNotification);
    assert_eq!(env.source_id, "mcp-server-1");
    assert_eq!(env.summary, "Build completed");
    assert_eq!(env.action_kind, TriggerActionKind::InjectSummary);
    assert!(!env.promote_to_chat);
    assert!(env.dedup_hash != 0);
}

#[test]
fn test_envelope_dedup_hash_deterministic() {
    let e1 = TriggerEnvelope::new(
        TriggerSourceKind::Dynamic,
        "rule-1",
        "same summary",
        "same action",
        TriggerActionKind::SubAgent,
        true,
    );
    let e2 = TriggerEnvelope::new(
        TriggerSourceKind::Dynamic,
        "rule-1",
        "same summary",
        "same action",
        TriggerActionKind::SubAgent,
        true,
    );
    assert_eq!(e1.dedup_hash, e2.dedup_hash);
}

#[test]
fn test_envelope_promote_to_chat_flag() {
    let env = TriggerEnvelope::new(
        TriggerSourceKind::Dynamic,
        "rule-1",
        "summary",
        "action",
        TriggerActionKind::SubAgent,
        true,
    );
    assert!(env.promote_to_chat);
}

#[test]
fn test_trigger_rule_status_active() {
    let rule = TriggerRule::new("cond", "act");
    assert_eq!(rule.status(), TriggerRuleStatus::Active);
}

#[test]
fn test_trigger_rule_status_disabled() {
    let mut rule = TriggerRule::new("cond", "act");
    rule.enabled = false;
    assert_eq!(rule.status(), TriggerRuleStatus::Disabled);
}

#[test]
fn test_trigger_rule_status_fired() {
    let mut rule = TriggerRule::new("cond", "act");
    rule.fired_at = Some(Utc::now());
    assert_eq!(rule.status(), TriggerRuleStatus::Fired);
}

#[test]
fn test_envelope_summary_bounded() {
    let long_summary = "A".repeat(2000);
    let env = TriggerEnvelope::new(
        TriggerSourceKind::McpNotification,
        "s",
        &long_summary,
        "act",
        TriggerActionKind::InjectSummary,
        false,
    );
    assert!(env.summary.chars().count() <= TriggerEnvelope::SUMMARY_MAX);
}
