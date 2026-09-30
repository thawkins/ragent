//! Trigger unit tests relocated out of the inline `#[cfg(test)]` block in
//! `src/trigger.rs` (ANTIPAT M2.5/F1). All tested items are public API.
use chrono::Utc;

use ragent_types::trigger::*;
#[test]
fn test_trigger_rule_id_roundtrip() {
    let id = TriggerRuleId::new();
    let s = id.to_string();
    let id2 = TriggerRuleId::from(s);
    assert_eq!(id, id2);
}
#[test]
fn test_trigger_rule_defaults() {
    let rule = TriggerRule::new("file exists", "print it");
    assert!(rule.fire_once);
    assert!(rule.enabled);
    assert!(!rule.promote_to_chat);
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
fn test_envelope_dedup_hash_stable() {
    let e1 = TriggerEnvelope::new(
        TriggerSourceKind::McpNotification,
        "server-1",
        "build done",
        "report it",
        TriggerActionKind::InjectSummary,
        false,
    );
    let e2 = TriggerEnvelope::new(
        TriggerSourceKind::McpNotification,
        "server-1",
        "build done",
        "report it",
        TriggerActionKind::InjectSummary,
        false,
    );
    assert_eq!(e1.dedup_hash, e2.dedup_hash);
}
#[test]
fn test_envelope_dedup_hash_differs_on_content() {
    let e1 = TriggerEnvelope::new(
        TriggerSourceKind::McpNotification,
        "server-1",
        "build done",
        "report it",
        TriggerActionKind::InjectSummary,
        false,
    );
    let e2 = TriggerEnvelope::new(
        TriggerSourceKind::McpNotification,
        "server-1",
        "build failed",
        "report it",
        TriggerActionKind::InjectSummary,
        false,
    );
    assert_ne!(e1.dedup_hash, e2.dedup_hash);
}
#[test]
fn test_envelope_summary_truncation() {
    let long_summary = "x".repeat(1000);
    let e = TriggerEnvelope::new(
        TriggerSourceKind::Dynamic,
        "rule-1",
        &long_summary,
        "act",
        TriggerActionKind::SubAgent,
        false,
    );
    assert!(e.summary.chars().count() <= TriggerEnvelope::SUMMARY_MAX);
}
