//! Storage round-trip tests for the `automation_runs` table (spec `openhands`
//! T-016; FR-018).
//!
//! FR-018 requires the durable run-history record to survive insert -> read with
//! its automation id, trigger kind, start/end times, outcome, backend, and
//! output reference intact.

use ragent_storage::storage::Storage;
use ragent_types::{AutomationRun, AutomationTrigger, RunOutcome};

fn storage() -> Storage {
    Storage::open_in_memory().expect("in-memory storage")
}

#[test]
fn running_record_round_trips_then_finalises() {
    // FR-018: a run is inserted in the running state and later finalised.
    let storage = storage();
    let run = AutomationRun::start(
        "run-1".to_string(),
        "on-issue".to_string(),
        AutomationTrigger::Webhook,
        "local".to_string(),
    );
    storage.insert_automation_run(&run).expect("insert");

    let read = storage
        .get_automation_run("run-1")
        .expect("get")
        .expect("present");
    assert_eq!(read.automation_id, "on-issue");
    assert_eq!(read.trigger, AutomationTrigger::Webhook);
    assert_eq!(read.backend, "local");
    assert!(read.ended_at.is_none());
    assert!(read.outcome.is_none());
    assert!(!read.is_terminal());

    let updated = storage
        .finish_automation_run("run-1", RunOutcome::Success, Some("/tmp/out.md"))
        .expect("finish");
    assert!(updated);

    let done = storage
        .get_automation_run("run-1")
        .expect("get")
        .expect("present");
    assert_eq!(done.outcome, Some(RunOutcome::Success));
    assert!(done.ended_at.is_some());
    assert_eq!(done.output_ref.as_deref(), Some("/tmp/out.md"));
    assert!(done.is_terminal());
}

#[test]
fn list_runs_is_newest_first_and_scoped_to_one_automation() {
    // FR-018: run history is scoped per automation.
    let storage = storage();
    for (id, automation, trigger) in [
        ("a-1", "on-issue", AutomationTrigger::Webhook),
        ("a-2", "on-issue", AutomationTrigger::Schedule),
        ("b-1", "nightly", AutomationTrigger::Manual),
    ] {
        let run = AutomationRun::start(
            id.to_string(),
            automation.to_string(),
            trigger,
            "local".to_string(),
        );
        storage.insert_automation_run(&run).expect("insert");
    }

    let runs = storage.list_automation_runs("on-issue", 10).expect("list");
    assert_eq!(runs.len(), 2);
    assert!(runs.iter().all(|r| r.automation_id == "on-issue"));

    let nightly = storage.list_automation_runs("nightly", 10).expect("list");
    assert_eq!(nightly.len(), 1);
    assert_eq!(nightly[0].trigger, AutomationTrigger::Manual);
}

#[test]
fn prune_keeps_only_the_newest_records() {
    // FR-018: history is bounded to the configured cap.
    let storage = storage();
    for i in 0..5 {
        let run = AutomationRun::start(
            format!("run-{i}"),
            "on-issue".to_string(),
            AutomationTrigger::Webhook,
            "local".to_string(),
        );
        // Distinct start times so the ordering is deterministic.
        let mut run = run;
        run.started_at += chrono::Duration::seconds(i);
        storage.insert_automation_run(&run).expect("insert");
    }
    let deleted = storage.prune_automation_runs("on-issue", 2).expect("prune");
    assert_eq!(deleted, 3);
    let remaining = storage.list_automation_runs("on-issue", 10).expect("list");
    assert_eq!(remaining.len(), 2);
}

#[test]
fn trigger_and_outcome_labels_default_safely() {
    // A corrupt label maps to a safe default rather than failing a read.
    assert_eq!(
        AutomationTrigger::from_label("nonsense"),
        AutomationTrigger::Manual
    );
    assert_eq!(RunOutcome::from_label("nonsense"), RunOutcome::Error);
    assert_eq!(RunOutcome::from_label("success"), RunOutcome::Success);
}
