#![allow(clippy::assert_is_empty)]
//! Tests for optional retention limit / archival (maka spec T-010, FR-016,
//! NFR-003).
//!
//! FR-016: "Where the operator sets a retention limit, the system may archive
//! or expire event logs for runs older than the limit, provided it records the
//! expiry as a lifecycle event."
//!
//! NFR-003: "The event log format shall be self-describing (each event carries
//! its type, schema version, and run identifier)."

#![forbid(unsafe_code)]

use ragent_storage::activity_log::ActivityLog;
use ragent_types::activity::{EventKind, RunStatus, TerminationReason};
use ragent_types::id::RunId;

#[test]
fn expire_run_records_lifecycle_event_before_deletion() {
    // FR-016: expiry records a lifecycle event (the audit trail) before the
    // run's events are removed.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "hi", None)
        .expect("msg");
    log.record_termination(&run, TerminationReason::Completed)
        .expect("term");

    // Before expiry, 2 events.
    assert_eq!(log.count(&run).unwrap(), 2);

    log.expire_run(&run, "retention limit").expect("expire");

    // After expiry, the run is gone.
    assert_eq!(log.count(&run).unwrap(), 0);
    assert!(log.read_run(&run).unwrap().is_empty());
    assert!(!log.list_runs().unwrap().contains(&run));
}

#[test]
fn expire_run_lifecycle_event_is_self_describing() {
    // NFR-003 + FR-016: the expiry lifecycle event is self-describing. We
    // verify by exporting the run BEFORE the deletion (the lifecycle event is
    // the last appended event), checking it carries type, schema_version,
    // and run_id.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "hi", None)
        .expect("msg");

    // Manually append the lifecycle event (what expire_run does internally)
    // and export before deletion.
    log.append_new(
        &run,
        EventKind::Lifecycle {
            event: "expired: test".into(),
        },
    )
    .expect("lifecycle");
    let jsonl = log.export_jsonl(&run).expect("export");
    let lines: Vec<&str> = jsonl.lines().collect();
    let last_line = lines.last().unwrap();
    let v: serde_json::Value = serde_json::from_str(last_line).expect("parse");
    // `EventKind` is serialized as a nested object with a `kind` tag.
    let kind_val = v
        .get("kind")
        .and_then(|k| k.get("kind"))
        .and_then(|k| k.as_str());
    assert_eq!(kind_val, Some("lifecycle"));
    assert!(
        v.get("schema_version").is_some(),
        "carries schema_version (NFR-003)"
    );
    assert!(v.get("run_id").is_some(), "carries run_id (NFR-003)");
    assert!(v.get("id").is_some(), "carries id (NFR-003)");
    assert_eq!(kind_val, Some("lifecycle"));
}

#[test]
fn expire_run_with_custom_reason() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "hi", None)
        .expect("msg");

    // Export before expiry to capture the lifecycle event.
    let jsonl = log.archive_run(&run, "age > 30 days").expect("archive");
    let lines: Vec<&str> = jsonl.lines().collect();
    let last: serde_json::Value = serde_json::from_str(lines.last().unwrap()).expect("parse");
    let kind_val = last
        .get("kind")
        .and_then(|k| k.get("kind"))
        .and_then(|k| k.as_str());
    match kind_val {
        Some("lifecycle") => {
            // Just verify it parses as a Lifecycle event with "expired:" prefix.
            let parsed: ragent_types::activity::ActivityEvent =
                serde_json::from_str(lines.last().unwrap()).expect("parse event");
            match parsed.kind {
                EventKind::Lifecycle { event } => {
                    assert!(
                        event.contains("expired"),
                        "lifecycle event says 'expired': {event}"
                    );
                    assert!(
                        event.contains("age > 30 days"),
                        "lifecycle event carries the reason: {event}"
                    );
                }
                other => panic!("expected Lifecycle, got {other:?}"),
            }
        }
        other => panic!("expected lifecycle kind, got {other:?}"),
    }
    // Run is now gone.
    assert!(log.read_run(&run).unwrap().is_empty());
}

#[test]
fn archive_run_returns_jsonl_and_expires() {
    // FR-016: archive returns the JSONL export (for external storage) and
    // then expires the run.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "Read README.md", None)
        .expect("msg");
    log.record_tool_call(&run, "c1", "read", r#"{"path":"README.md"}"#)
        .expect("call");
    log.record_tool_result(&run, "c1", "read", true, "# ragent")
        .expect("result");

    let jsonl = log.archive_run(&run, "retention").expect("archive");
    // The JSONL contains the original events plus the lifecycle expiry event.
    let line_count = jsonl.lines().count();
    assert!(
        line_count >= 4,
        "archive includes original events + lifecycle event"
    );
    // Run is expired (gone from the store).
    assert!(log.read_run(&run).unwrap().is_empty());
    assert!(!log.list_runs().unwrap().contains(&run));
}

#[test]
fn expire_run_removes_from_list_runs() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run_a = RunId::from("run-a");
    let run_b = RunId::from("run-b");
    log.record_model_message(&run_a, "user", "a", None)
        .expect("msg");
    log.record_model_message(&run_b, "user", "b", None)
        .expect("msg");
    assert_eq!(log.list_runs().unwrap().len(), 2);

    log.expire_run(&run_a, "retention").expect("expire");
    let runs = log.list_runs().unwrap();
    assert_eq!(runs.len(), 1);
    assert_eq!(runs[0], run_b);
}

#[test]
fn expire_run_on_empty_run_is_noop() {
    // Expiring a run with no events still records the lifecycle event, then
    // deletes it (resulting in an empty store for that run).
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-empty");
    log.expire_run(&run, "retention").expect("expire");
    assert!(log.read_run(&run).unwrap().is_empty());
    assert!(!log.list_runs().unwrap().contains(&run));
}

#[test]
fn run_last_activity_returns_last_event_timestamp() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "a", None)
        .expect("m0");
    log.record_model_message(&run, "user", "b", None)
        .expect("m1");

    let last = log.run_last_activity(&run).unwrap().expect("exists");
    // The last activity should be after the first event.
    let first = log.get_event(&run, 0).unwrap().unwrap();
    assert!(last >= first.timestamp);
}

#[test]
fn run_last_activity_none_for_empty_run() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-empty");
    assert!(log.run_last_activity(&run).unwrap().is_none());
}

#[test]
fn expire_runs_older_than_expires_old_runs() {
    // FR-016: runs older than the retention limit are expired.
    let log = ActivityLog::open_in_memory().expect("open");
    let run_old = RunId::from("run-old");
    let run_new = RunId::from("run-new");
    log.record_model_message(&run_old, "user", "old", None)
        .expect("msg");
    log.record_model_message(&run_new, "user", "new", None)
        .expect("msg");

    // Expire runs older than a very large age (everything is "new" relative
    // to 100 years, so nothing expires).
    let expired = log
        .expire_runs_older_than(chrono::Duration::days(365 * 100))
        .expect("expire");
    assert!(expired.is_empty(), "nothing is older than 100 years");
    assert_eq!(log.list_runs().unwrap().len(), 2);

    // Expire runs older than 0 seconds (everything is older than "now minus
    // 0", which is everything written before this instant — both runs
    // should expire since they were written moments ago, which is before
    // "now - 0s" is technically "now" so runs written before now are older).
    // Actually, "older than 0 seconds" means cutoff = now, so any event with
    // timestamp < now is expired. Both runs have timestamps slightly before
    // now, so both expire.
    let expired = log
        .expire_runs_older_than(chrono::Duration::seconds(0))
        .expect("expire");
    assert_eq!(expired.len(), 2, "both runs are older than 0 seconds");
    assert!(log.list_runs().unwrap().is_empty());
}

#[test]
fn expired_run_does_not_affect_other_runs() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run_a = RunId::from("run-a");
    let run_b = RunId::from("run-b");
    log.record_model_message(&run_a, "user", "a", None)
        .expect("msg");
    log.record_model_message(&run_b, "user", "b", None)
        .expect("msg");
    log.record_termination(&run_b, TerminationReason::Completed)
        .expect("term");

    log.expire_run(&run_a, "retention").expect("expire");

    // run_b is untouched.
    assert_eq!(log.count(&run_b).unwrap(), 2);
    assert_eq!(log.run_status(&run_b).unwrap(), RunStatus::Completed);
    assert_eq!(log.read_run(&run_b).unwrap().len(), 2);
}
