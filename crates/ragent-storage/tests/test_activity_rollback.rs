//! Tests for rollback to checkpoint / sequence (maka spec T-013, FR-007,
//! FR-012).
//!
//! FR-007: "While a run is in the 'rolled-back' state, the system shall
//! preserve all events after the checkpoint in the log for audit purposes and
//! shall not delete them."
//!
//! FR-012: "When a rollback operation is invoked with a checkpoint or sequence
//! number, the system shall rebuild the derived projection by replaying events
//! from the start of the run up to (and including) the target, and shall ignore
//! all subsequent events for that projection."

#![forbid(unsafe_code)]

use ragent_storage::activity_log::ActivityLog;
use ragent_types::activity::{RunStatus, TerminationReason};
use ragent_types::id::RunId;

#[test]
fn rollback_to_seq_rebuilds_projection_up_to_target() {
    // FR-012: rollback to a sequence number rebuilds the projection up to (and
    // including) the target.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "a", None)
        .expect("m0"); // seq 0
    log.record_model_message(&run, "assistant", "b", None)
        .expect("m1"); // seq 1
    log.record_model_message(&run, "user", "c", None)
        .expect("m2"); // seq 2
    log.record_model_message(&run, "assistant", "d", None)
        .expect("m3"); // seq 3

    let result = log.rollback_to_seq(&run, 1).expect("rollback");
    assert_eq!(result.target_seq, 1);
    assert_eq!(
        result.projection.messages.len(),
        2,
        "only events up to seq 1"
    );
    assert_eq!(result.projection.messages[1].content, "b");
    assert_eq!(result.projection.last_seq, 1);
}

#[test]
fn rollback_to_seq_preserves_events_after_target() {
    // FR-007: events after the rollback target are preserved in the log (not
    // deleted).
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "a", None)
        .expect("m0");
    log.record_model_message(&run, "assistant", "b", None)
        .expect("m1");
    log.record_model_message(&run, "user", "c", None)
        .expect("m2");

    let result = log.rollback_to_seq(&run, 0).expect("rollback");
    assert_eq!(
        result.ignored_count, 2,
        "2 events after target are preserved"
    );

    // FR-007: the full log is still intact.
    assert_eq!(
        log.count(&run).unwrap(),
        3,
        "all events preserved in the log"
    );
    let events = log.read_run(&run).expect("read");
    assert_eq!(events.len(), 3);
}

#[test]
fn rollback_to_seq_includes_target_event() {
    // FR-012: the target event is included in the rebuilt projection.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "a", None)
        .expect("m0");
    log.record_tool_call(&run, "c1", "read", "{}")
        .expect("call"); // seq 1
    log.record_tool_result(&run, "c1", "read", true, "ok")
        .expect("result"); // seq 2

    let result = log.rollback_to_seq(&run, 1).expect("rollback");
    assert_eq!(
        result.projection.tool_calls.len(),
        1,
        "target event (call) included"
    );
    assert_eq!(
        result.projection.tool_results.len(),
        0,
        "event after target excluded"
    );
}

#[test]
fn rollback_to_checkpoint_finds_and_rolls_back() {
    // FR-012: rollback to a named checkpoint rebuilds the projection up to the
    // checkpoint.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "a", None)
        .expect("m0"); // seq 0
    log.record_checkpoint(&run, "cp1").expect("cp"); // seq 1, payload seq=0
    log.record_model_message(&run, "assistant", "b", None)
        .expect("m2"); // seq 2
    log.record_model_message(&run, "user", "c", None)
        .expect("m3"); // seq 3

    let result = log.rollback_to_checkpoint(&run, "cp1").expect("rollback");
    // The checkpoint's payload seq is 0, so we replay up to seq 0.
    assert_eq!(result.target_seq, 0);
    assert_eq!(
        result.projection.messages.len(),
        1,
        "only the message before the checkpoint"
    );
    assert_eq!(result.projection.messages[0].content, "a");
    assert_eq!(
        result.ignored_count, 3,
        "3 events after the checkpoint preserved"
    );
}

#[test]
fn rollback_to_checkpoint_unknown_name_errors() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_checkpoint(&run, "real").expect("cp");
    let err = log.rollback_to_checkpoint(&run, "missing").unwrap_err();
    assert!(
        err.to_string().contains("checkpoint 'missing' not found"),
        "error mentions missing checkpoint: {err}"
    );
}

#[test]
fn rollback_preserves_full_log_for_audit() {
    // FR-007: after a rollback, the complete log is still readable and
    // exportable for audit.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "a", None)
        .expect("m0");
    log.record_checkpoint(&run, "cp").expect("cp");
    log.record_model_message(&run, "assistant", "b", None)
        .expect("m2");

    log.rollback_to_checkpoint(&run, "cp").expect("rollback");

    // The full log is intact.
    let events = log.read_run(&run).expect("read");
    assert_eq!(events.len(), 3, "full log preserved after rollback");
    // JSONL export still includes all events.
    let jsonl = log.export_jsonl(&run).expect("export");
    assert_eq!(jsonl.lines().count(), 3);
}

#[test]
fn rollback_to_last_event_yields_full_projection() {
    // Rolling back to the last seq replays everything (no events ignored).
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "a", None)
        .expect("m0");
    log.record_model_message(&run, "assistant", "b", None)
        .expect("m1");

    let result = log.rollback_to_seq(&run, 1).expect("rollback");
    assert_eq!(result.projection.messages.len(), 2);
    assert_eq!(result.ignored_count, 0);
}

#[test]
fn rollback_to_zero_on_empty_run() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-empty");
    let result = log.rollback_to_seq(&run, 0).expect("rollback");
    assert!(result.projection.messages.is_empty());
    assert_eq!(result.ignored_count, 0);
}

#[test]
fn rollback_after_terminated_run_still_preserves_events() {
    // FR-007: rollback on a completed run preserves all events including the
    // termination.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "a", None)
        .expect("m0");
    log.record_termination(&run, TerminationReason::Completed)
        .expect("term");

    let result = log.rollback_to_seq(&run, 0).expect("rollback");
    assert_eq!(result.projection.messages.len(), 1);
    assert_eq!(result.ignored_count, 1, "termination event preserved");
    assert_eq!(
        result.projection.status,
        RunStatus::Active,
        "no termination in replayed range"
    );

    // The termination event is still in the log.
    assert_eq!(log.run_status(&run).unwrap(), RunStatus::Completed);
}

#[test]
fn rollback_ignores_events_after_target_in_projection() {
    // FR-012: events after the target are ignored for the projection but
    // present in the log.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "keep", None)
        .expect("m0");
    log.record_tool_call(&run, "c1", "read", "{}")
        .expect("call"); // seq 1
    log.record_tool_result(&run, "c1", "read", true, "ok")
        .expect("result"); // seq 2
    log.record_model_message(&run, "assistant", "ignore", None)
        .expect("m3"); // seq 3

    let result = log.rollback_to_seq(&run, 2).expect("rollback");
    // Projection includes up to seq 2.
    assert_eq!(result.projection.messages.len(), 1, "only msg at seq 0");
    assert_eq!(result.projection.tool_calls.len(), 1);
    assert_eq!(result.projection.tool_results.len(), 1);
    // Event at seq 3 is ignored.
    assert_eq!(result.ignored_count, 1);
    // But still in the log.
    let events = log.read_run(&run).expect("read");
    assert_eq!(events.len(), 4);
}
