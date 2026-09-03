#![allow(clippy::assert_is_empty)]
//! Tests for the ActivityLog replay convenience methods (maka spec T-011,
//! FR-012, FR-013).

#![forbid(unsafe_code)]

use ragent_storage::activity_log::ActivityLog;
use ragent_types::activity::{BoundaryTarget, EventKind, Principal, RunStatus, TerminationReason};
use ragent_types::id::RunId;

#[test]
fn replay_run_reconstructs_projection_from_log() {
    // FR-013: replay the event log to reconstruct the active context.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "Read README.md", None)
        .expect("msg");
    log.record_tool_call(&run, "c1", "read", r#"{"path":"README.md"}"#)
        .expect("call");
    log.record_tool_result(&run, "c1", "read", true, "# ragent")
        .expect("result");
    log.record_termination(&run, TerminationReason::Completed)
        .expect("term");

    let proj = log.replay_run(&run).expect("replay");
    assert_eq!(proj.messages.len(), 1);
    assert_eq!(proj.tool_calls.len(), 1);
    assert_eq!(proj.tool_results.len(), 1);
    assert_eq!(proj.status, RunStatus::Completed);
    assert_eq!(proj.last_seq, 3);
}

#[test]
fn replay_run_upto_ignores_events_after_target() {
    // FR-012: replay up to a target ignores subsequent events.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "a", None)
        .expect("m0");
    log.record_model_message(&run, "assistant", "b", None)
        .expect("m1");
    log.record_model_message(&run, "user", "c", None)
        .expect("m2");

    let proj = log.replay_run_upto(&run, 1).expect("replay");
    assert_eq!(proj.messages.len(), 2);
    assert_eq!(proj.last_seq, 1);
}

#[test]
fn replay_run_upto_includes_target() {
    // FR-012: the target event is included.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_checkpoint(&run, "cp").expect("cp");

    let proj = log.replay_run_upto(&run, 0).expect("replay");
    assert_eq!(proj.checkpoints.len(), 1);
}

#[test]
fn replay_run_empty_yields_empty_projection() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-empty");
    let proj = log.replay_run(&run).expect("replay");
    assert!(proj.messages.is_empty());
    assert_eq!(proj.status, RunStatus::Active);
}

#[test]
fn replay_run_upto_zero_on_empty_run() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-empty");
    let proj = log.replay_run_upto(&run, 0).expect("replay");
    assert!(proj.messages.is_empty());
}

#[test]
fn replay_run_with_permissions_and_checkpoints() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_permission_decision(
        &run,
        "bash",
        Principal::Operator,
        BoundaryTarget::Shell,
        true,
    )
    .expect("perm");
    log.record_tool_call(&run, "c1", "bash", r#"{"command":"ls"}"#)
        .expect("call");
    log.record_tool_result(&run, "c1", "bash", true, "file.txt")
        .expect("result");
    log.record_checkpoint(&run, "after-tool").expect("cp");

    let proj = log.replay_run(&run).expect("replay");
    assert_eq!(proj.permissions.len(), 1);
    assert_eq!(proj.tool_calls.len(), 1);
    assert_eq!(proj.tool_results.len(), 1);
    assert_eq!(proj.checkpoints.len(), 1);
    assert_eq!(proj.checkpoints[0].name, "after-tool");
}

#[test]
fn replay_run_large_log_performance() {
    // NFR-002: rebuild a projection for a large run in under 5 seconds.
    // 10,000 events through the SQLite store (read + replay) to exercise the
    // full path.
    use std::time::Instant;
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    for i in 0..10_000 {
        log.append_new(
            &run,
            EventKind::ModelMessage {
                role: if i % 2 == 0 { "user" } else { "assistant" }.into(),
                content: format!("msg-{i}"),
                message_id: None,
            },
        )
        .expect("append");
    }
    let start = Instant::now();
    let proj = log.replay_run(&run).expect("replay");
    let elapsed = start.elapsed();
    assert_eq!(proj.messages.len(), 10_000);
    assert!(
        elapsed.as_secs() < 5,
        "replay of 10k events from store took {:?}, expected < 5s (NFR-002)",
        elapsed
    );
}
