#![allow(clippy::assert_is_empty)]
//! Tests for recording checkpoint events (maka spec T-008, FR-008).
//!
//! FR-008: "Where the operator configures automatic checkpointing, the system
//! may create a checkpoint after each completed turn, recording the checkpoint
//! name, sequence number, and timestamp as an event."

#![forbid(unsafe_code)]

use ragent_storage::activity_log::ActivityLog;
use ragent_types::activity::{EventKind, TerminationReason};
use ragent_types::id::RunId;

#[test]
fn record_checkpoint_persists_name_and_seq() {
    // FR-008: a checkpoint records its name and the sequence number it was
    // taken at.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    // Append one event so last_seq is 0.
    log.append_new(
        &run,
        EventKind::Lifecycle {
            event: "pre".into(),
        },
    )
    .expect("pre");

    let cp = log
        .record_checkpoint(&run, "after-turn-1")
        .expect("checkpoint");
    match &cp.kind {
        EventKind::Checkpoint { name, seq } => {
            assert_eq!(name, "after-turn-1");
            // FR-008: seq == last committed seq before the checkpoint event
            // (0 here).
            assert_eq!(*seq, 0);
        }
        other => panic!("expected Checkpoint, got {other:?}"),
    }
    // The checkpoint event itself is appended at the next seq (1).
    assert_eq!(cp.seq, 1);
}

#[test]
fn checkpoint_payload_seq_reflects_last_committed_seq() {
    // FR-008: the checkpoint's seq is the last committed event's seq at the
    // time the checkpoint was taken.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    for i in 0..4 {
        log.append_new(
            &run,
            EventKind::Lifecycle {
                event: format!("e{i}"),
            },
        )
        .expect("append");
    }
    // last committed seq is now 3.
    let cp = log.record_checkpoint(&run, "cp").expect("checkpoint");
    match &cp.kind {
        EventKind::Checkpoint { seq, .. } => assert_eq!(*seq, 3),
        other => panic!("expected Checkpoint, got {other:?}"),
    }
    assert_eq!(cp.seq, 4);
}

#[test]
fn find_checkpoint_returns_named_checkpoint() {
    // FR-008: a checkpoint can be looked up by name (for rollback/resume).
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_checkpoint(&run, "turn-1").expect("checkpoint");

    let found = log.find_checkpoint(&run, "turn-1").expect("find");
    assert!(found.is_some());
    match &found.unwrap().kind {
        EventKind::Checkpoint { name, .. } => assert_eq!(name, "turn-1"),
        other => panic!("expected Checkpoint, got {other:?}"),
    }
}

#[test]
fn find_checkpoint_returns_none_for_unknown_name() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_checkpoint(&run, "real").expect("checkpoint");
    assert!(
        log.find_checkpoint(&run, "missing")
            .expect("find")
            .is_none()
    );
}

#[test]
fn find_checkpoint_returns_most_recent_for_duplicate_name() {
    // If multiple checkpoints share a name, the most recent (highest seq) is
    // returned — the one a rollback would target.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_checkpoint(&run, "dup").expect("cp1");
    log.append_new(&run, EventKind::Lifecycle { event: "x".into() })
        .expect("append");
    log.record_checkpoint(&run, "dup").expect("cp2");

    let found = log
        .find_checkpoint(&run, "dup")
        .expect("find")
        .expect("exists");
    assert_eq!(found.seq, 2);
}

#[test]
fn multiple_checkpoints_get_monotonic_sequence_numbers() {
    // FR-002 + FR-008: each checkpoint is a separate ordered event.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let c0 = log.record_checkpoint(&run, "cp0").expect("cp0");
    let c1 = log.record_checkpoint(&run, "cp1").expect("cp1");
    let c2 = log.record_checkpoint(&run, "cp2").expect("cp2");
    assert_eq!(c0.seq, 0);
    assert_eq!(c1.seq, 1);
    assert_eq!(c2.seq, 2);
}

#[test]
fn checkpoint_after_completed_turn() {
    // FR-008: a checkpoint may be created after each completed turn.
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
    // Checkpoint after the turn: payload seq is the last committed seq (3,
    // the termination event), event seq is 4.
    let cp = log.record_checkpoint(&run, "after-turn-1").expect("cp");
    match &cp.kind {
        EventKind::Checkpoint { name, seq } => {
            assert_eq!(name, "after-turn-1");
            assert_eq!(*seq, 3);
        }
        other => panic!("expected Checkpoint, got {other:?}"),
    }
    assert_eq!(cp.seq, 4);
    let events = log.read_run(&run).expect("read");
    assert_eq!(events.len(), 5);
    assert!(matches!(events[4].kind, EventKind::Checkpoint { .. }));
}

#[test]
fn recorded_checkpoint_is_durable_and_replayable() {
    // FR-001 + FR-008: the checkpoint is persisted and survives a read-back.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let cp = log.record_checkpoint(&run, "cp").expect("checkpoint");
    let read = log.get_event(&run, cp.seq).expect("read").expect("exists");
    assert_eq!(read, cp);
}

#[test]
fn recorded_checkpoint_has_nonempty_event_id() {
    // FR-002: the checkpoint event carries a fresh immutable id.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let cp = log.record_checkpoint(&run, "cp").expect("checkpoint");
    assert!(!cp.id.as_str().is_empty());
}

#[test]
fn checkpoint_on_empty_run_has_payload_seq_zero() {
    // A checkpoint on a run with no prior events marks seq 0.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let cp = log.record_checkpoint(&run, "start").expect("cp");
    match &cp.kind {
        EventKind::Checkpoint { seq, .. } => assert_eq!(*seq, 0),
        other => panic!("expected Checkpoint, got {other:?}"),
    }
    assert_eq!(cp.seq, 0);
}
