//! Coverage for the automation run-history records (`AutomationTrigger`,
//! `RunOutcome`, `AutomationRun`) - spec `openhands` FR-018.
//!
//! These types live in `ragent-types` and are consumed by `ragent-storage`;
//! the label round-trips must not drift or a corrupt row could read as a
//! success.

use ragent_types::automation::{AutomationRun, AutomationTrigger, RunOutcome};

#[test]
fn trigger_as_str_display_and_from_label_agree() {
    for (trigger, label) in [
        (AutomationTrigger::Webhook, "webhook"),
        (AutomationTrigger::Schedule, "schedule"),
        (AutomationTrigger::Manual, "manual"),
    ] {
        assert_eq!(trigger.as_str(), label);
        assert_eq!(trigger.to_string(), label);
        assert_eq!(AutomationTrigger::from_label(label), trigger);
    }
}

#[test]
fn trigger_from_label_defaults_to_manual_for_unknown_values() {
    assert_eq!(
        AutomationTrigger::from_label("nonsense"),
        AutomationTrigger::Manual
    );
    assert_eq!(AutomationTrigger::from_label(""), AutomationTrigger::Manual);
    assert_eq!(
        AutomationTrigger::from_label("WEBHOOK"),
        AutomationTrigger::Manual
    );
}

#[test]
fn trigger_serializes_snake_case() {
    assert_eq!(
        serde_json::to_string(&AutomationTrigger::Webhook).expect("serialize"),
        "\"webhook\""
    );
    let back: AutomationTrigger = serde_json::from_str("\"schedule\"").expect("deserialize");
    assert_eq!(back, AutomationTrigger::Schedule);
}

#[test]
fn outcome_as_str_display_and_from_label_agree() {
    for (outcome, label) in [
        (RunOutcome::Success, "success"),
        (RunOutcome::Error, "error"),
        (RunOutcome::Cancelled, "cancelled"),
    ] {
        assert_eq!(outcome.as_str(), label);
        assert_eq!(outcome.to_string(), label);
        assert_eq!(RunOutcome::from_label(label), outcome);
    }
}

#[test]
fn outcome_from_label_defaults_to_error_so_a_corrupt_row_never_reads_as_success() {
    assert_eq!(RunOutcome::from_label("garbage"), RunOutcome::Error);
    assert_eq!(RunOutcome::from_label(""), RunOutcome::Error);
    assert_eq!(RunOutcome::from_label("SUCCESS"), RunOutcome::Error);
}

#[test]
fn run_start_is_not_terminal_and_has_no_outcome() {
    let run = AutomationRun::start(
        "run-1".to_string(),
        "auto-7".to_string(),
        AutomationTrigger::Webhook,
        "local".to_string(),
    );
    assert_eq!(run.id, "run-1");
    assert_eq!(run.automation_id, "auto-7");
    assert_eq!(run.trigger, AutomationTrigger::Webhook);
    assert_eq!(run.backend, "local");
    assert!(run.ended_at.is_none());
    assert!(run.outcome.is_none());
    assert!(run.output_ref.is_none());
    assert!(!run.is_terminal());
}

#[test]
fn a_run_with_an_end_time_is_terminal() {
    let mut run = AutomationRun::start(
        "run-2".to_string(),
        "auto-8".to_string(),
        AutomationTrigger::Schedule,
        "podman".to_string(),
    );
    run.ended_at = Some(chrono::Utc::now());
    run.outcome = Some(RunOutcome::Success);
    run.output_ref = Some("child-session".to_string());
    assert!(run.is_terminal());
}

#[test]
fn automation_run_roundtrips_through_serde() {
    let run = AutomationRun::start(
        "run-3".to_string(),
        "auto-9".to_string(),
        AutomationTrigger::Manual,
        "remote".to_string(),
    );
    let json = serde_json::to_string(&run).expect("serialize");
    let back: AutomationRun = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(run, back);
}
