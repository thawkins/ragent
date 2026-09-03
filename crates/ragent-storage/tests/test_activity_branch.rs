#![allow(clippy::assert_is_empty)]
//! Tests for run branching from checkpoint (maka spec T-019, FR-018).
//!
//! FR-018: "When a run is branched (a new run created from a checkpoint of an
//! existing run), the system shall copy the events up to the checkpoint into
//! the new run's log and shall record the branch origin in both runs."

#![forbid(unsafe_code)]

use ragent_storage::activity_log::ActivityLog;
use ragent_types::activity::{EventKind, RunStatus, TerminationReason};
use ragent_types::id::RunId;

#[test]
fn branch_copies_events_up_to_checkpoint() {
    // FR-018: events up to the checkpoint are copied into the new run.
    let log = ActivityLog::open_in_memory().expect("open");
    let source = RunId::from("run-source");
    let new_run = RunId::from("run-new");

    log.record_model_message(&source, "user", "a", None)
        .expect("m0"); // seq 0
    log.record_model_message(&source, "assistant", "b", None)
        .expect("m1"); // seq 1
    log.record_checkpoint(&source, "cp").expect("cp"); // seq 2, payload seq=1
    log.record_model_message(&source, "user", "c", None)
        .expect("m3"); // seq 3

    log.branch_from_checkpoint(&source, "cp", &new_run)
        .expect("branch");

    // The new run has the 2 source events (seq 0, 1) + the BranchOrigin event.
    let new_events = log.read_run(&new_run).expect("read");
    assert_eq!(new_events.len(), 3, "2 copied events + 1 branch origin");
    // The copied events match the source's content.
    match &new_events[0].kind {
        EventKind::ModelMessage { content, .. } => assert_eq!(content, "a"),
        other => panic!("expected ModelMessage, got {other:?}"),
    }
    match &new_events[1].kind {
        EventKind::ModelMessage { content, .. } => assert_eq!(content, "b"),
        other => panic!("expected ModelMessage, got {other:?}"),
    }
}

#[test]
fn branch_records_origin_in_new_run() {
    // FR-018: the new run records a BranchOrigin event.
    let log = ActivityLog::open_in_memory().expect("open");
    let source = RunId::from("run-source");
    let new_run = RunId::from("run-new");

    log.record_model_message(&source, "user", "hi", None)
        .expect("m0");
    log.record_checkpoint(&source, "cp").expect("cp"); // payload seq=0

    let branch_event = log
        .branch_from_checkpoint(&source, "cp", &new_run)
        .expect("branch");

    match &branch_event.kind {
        EventKind::BranchOrigin {
            source_run_id,
            source_seq,
        } => {
            assert_eq!(*source_run_id, source);
            assert_eq!(*source_seq, 0);
        }
        other => panic!("expected BranchOrigin, got {other:?}"),
    }
    // The branch event is the last event in the new run.
    let new_events = log.read_run(&new_run).expect("read");
    assert!(matches!(
        new_events.last().unwrap().kind,
        EventKind::BranchOrigin { .. }
    ));
}

#[test]
fn branch_records_origin_in_source_run() {
    // FR-018: the source run records the branch (in both runs).
    let log = ActivityLog::open_in_memory().expect("open");
    let source = RunId::from("run-source");
    let new_run = RunId::from("run-new");

    log.record_model_message(&source, "user", "hi", None)
        .expect("m0");
    log.record_checkpoint(&source, "cp").expect("cp");

    log.branch_from_checkpoint(&source, "cp", &new_run)
        .expect("branch");

    // The source run has a Lifecycle event noting the branch.
    let source_events = log.read_run(&source).expect("read");
    let last = source_events.last().unwrap();
    match &last.kind {
        EventKind::Lifecycle { event } => {
            assert!(
                event.contains("branched"),
                "lifecycle notes the branch: {event}"
            );
            assert!(
                event.contains("run-new"),
                "lifecycle mentions the new run: {event}"
            );
        }
        other => panic!("expected Lifecycle, got {other:?}"),
    }
}

#[test]
fn branch_unknown_checkpoint_errors() {
    let log = ActivityLog::open_in_memory().expect("open");
    let source = RunId::from("run-source");
    let new_run = RunId::from("run-new");
    log.record_checkpoint(&source, "real").expect("cp");

    let err = log
        .branch_from_checkpoint(&source, "missing", &new_run)
        .unwrap_err();
    assert!(err.to_string().contains("checkpoint 'missing' not found"));
}

#[test]
fn branch_into_non_empty_run_errors() {
    let log = ActivityLog::open_in_memory().expect("open");
    let source = RunId::from("run-source");
    let new_run = RunId::from("run-new");
    log.record_checkpoint(&source, "cp").expect("cp");
    log.record_model_message(&new_run, "user", "exists", None)
        .expect("msg");

    let err = log
        .branch_from_checkpoint(&source, "cp", &new_run)
        .unwrap_err();
    assert!(err.to_string().contains("already has events"));
}

#[test]
fn branched_run_can_accept_new_events() {
    // After branching, the new run can accept new events (it's Active).
    let log = ActivityLog::open_in_memory().expect("open");
    let source = RunId::from("run-source");
    let new_run = RunId::from("run-new");

    log.record_model_message(&source, "user", "hi", None)
        .expect("m0");
    log.record_checkpoint(&source, "cp").expect("cp");

    log.branch_from_checkpoint(&source, "cp", &new_run)
        .expect("branch");

    // The new run is Active (no termination).
    assert_eq!(log.run_status(&new_run).unwrap(), RunStatus::Active);
    // Append a new event.
    let next = log
        .record_model_message(&new_run, "assistant", "branched", None)
        .expect("append");
    assert!(
        next.seq >= 2,
        "appended after the copied events + branch origin"
    );
}

#[test]
fn branched_run_has_independent_sequence_space() {
    // The new run's sequence starts from 0 (copied events) and continues
    // independently.
    let log = ActivityLog::open_in_memory().expect("open");
    let source = RunId::from("run-source");
    let new_run = RunId::from("run-new");

    log.record_model_message(&source, "user", "a", None)
        .expect("m0"); // seq 0
    log.record_model_message(&source, "assistant", "b", None)
        .expect("m1"); // seq 1
    log.record_checkpoint(&source, "cp").expect("cp"); // payload seq=1

    log.branch_from_checkpoint(&source, "cp", &new_run)
        .expect("branch");

    let new_events = log.read_run(&new_run).expect("read");
    assert_eq!(new_events[0].seq, 0, "copied event keeps seq 0");
    assert_eq!(new_events[1].seq, 1, "copied event keeps seq 1");
    assert_eq!(new_events[2].seq, 2, "branch origin at seq 2");

    // The source run's events are unchanged.
    let source_events = log.read_run(&source).expect("read");
    // m0, m1, cp, lifecycle = 4
    assert_eq!(source_events.len(), 4);
}

#[test]
fn branch_preserves_event_payloads() {
    // Copied events retain their kind, schema_version, and timestamp.
    let log = ActivityLog::open_in_memory().expect("open");
    let source = RunId::from("run-source");
    let new_run = RunId::from("run-new");

    log.record_model_message(&source, "user", "hi", Some("m1".into()))
        .expect("m0");
    log.record_checkpoint(&source, "cp").expect("cp");

    log.branch_from_checkpoint(&source, "cp", &new_run)
        .expect("branch");

    let new_events = log.read_run(&new_run).expect("read");
    match &new_events[0].kind {
        EventKind::ModelMessage {
            role,
            content,
            message_id,
        } => {
            assert_eq!(role, "user");
            assert_eq!(content, "hi");
            assert_eq!(message_id.as_deref(), Some("m1"));
        }
        other => panic!("expected ModelMessage, got {other:?}"),
    }
}

#[test]
fn branch_copied_events_have_fresh_ids() {
    // Copied events get fresh EventIds (global uniqueness prevents reuse).
    let log = ActivityLog::open_in_memory().expect("open");
    let source = RunId::from("run-source");
    let new_run = RunId::from("run-new");

    let src_event = log
        .record_model_message(&source, "user", "hi", None)
        .expect("m0");
    log.record_checkpoint(&source, "cp").expect("cp");

    log.branch_from_checkpoint(&source, "cp", &new_run)
        .expect("branch");

    let new_events = log.read_run(&new_run).expect("read");
    assert_ne!(
        new_events[0].id, src_event.id,
        "copied event has a fresh id"
    );
}

#[test]
fn branch_can_complete_independently() {
    // A branched run can complete independently of the source.
    let log = ActivityLog::open_in_memory().expect("open");
    let source = RunId::from("run-source");
    let new_run = RunId::from("run-new");

    log.record_model_message(&source, "user", "hi", None)
        .expect("m0");
    log.record_checkpoint(&source, "cp").expect("cp");

    log.branch_from_checkpoint(&source, "cp", &new_run)
        .expect("branch");

    // The new run continues and completes.
    log.record_model_message(&new_run, "assistant", "done", None)
        .expect("msg");
    log.record_termination(&new_run, TerminationReason::Completed)
        .expect("term");

    assert_eq!(log.run_status(&new_run).unwrap(), RunStatus::Completed);
    assert_eq!(
        log.run_status(&source).unwrap(),
        RunStatus::Active,
        "source run is unaffected by the branch completing"
    );
}

#[test]
fn branched_run_replay_excludes_branch_origin_from_messages() {
    // The BranchOrigin event has no projection effect (it's a meta event).
    let log = ActivityLog::open_in_memory().expect("open");
    let source = RunId::from("run-source");
    let new_run = RunId::from("run-new");

    log.record_model_message(&source, "user", "a", None)
        .expect("m0");
    log.record_checkpoint(&source, "cp").expect("cp");

    log.branch_from_checkpoint(&source, "cp", &new_run)
        .expect("branch");

    let proj = log.replay_run(&new_run).expect("replay");
    assert_eq!(proj.messages.len(), 1, "only the copied ModelMessage");
    assert!(matches!(proj.tool_calls.as_slice(), []));
}
