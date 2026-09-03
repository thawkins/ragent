#![allow(clippy::assert_is_empty)]
//! Tests for recording termination events on interruption (maka spec T-007,
//! FR-003).
//!
//! FR-003: "When an agent run is interrupted by a crash, process exit, or
//! explicit abort, the system shall record a termination event marking the
//! run as interrupted at the last committed sequence number."

#![forbid(unsafe_code)]

use ragent_storage::activity_log::ActivityLog;
use ragent_types::activity::{EventKind, TerminationReason};
use ragent_types::id::RunId;

#[test]
fn record_interruption_marks_run_at_last_committed_seq() {
    // FR-003: a termination event marks the run as interrupted at the last
    // committed sequence number.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    // Append two events so the "last committed seq" is 1.
    log.append_new(&run, EventKind::Lifecycle { event: "a".into() })
        .expect("e0");
    log.append_new(&run, EventKind::Lifecycle { event: "b".into() })
        .expect("e1");

    let term = log.record_interruption(&run).expect("termination");
    match &term.kind {
        EventKind::Termination { reason, seq } => {
            assert_eq!(*reason, TerminationReason::Interrupted);
            // FR-003: payload seq == last committed seq BEFORE the termination
            // event, i.e. 1.
            assert_eq!(*seq, 1, "marks run at last committed seq");
        }
        other => panic!("expected Termination, got {other:?}"),
    }
    // The termination event itself is appended at the next seq (2).
    assert_eq!(term.seq, 2);
}

#[test]
fn record_termination_interrupted_reason() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let term = log
        .record_termination(&run, TerminationReason::Interrupted)
        .expect("term");
    match &term.kind {
        EventKind::Termination { reason, seq } => {
            assert_eq!(*reason, TerminationReason::Interrupted);
            assert_eq!(*seq, 0, "empty run stops at seq 0");
        }
        other => panic!("expected Termination, got {other:?}"),
    }
}

#[test]
fn record_termination_aborted_reason() {
    // FR-003 covers explicit abort as well as crash/process exit.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.append_new(&run, EventKind::Lifecycle { event: "x".into() })
        .expect("e0");
    let term = log
        .record_termination(&run, TerminationReason::Aborted)
        .expect("term");
    match &term.kind {
        EventKind::Termination { reason, seq } => {
            assert_eq!(*reason, TerminationReason::Aborted);
            assert_eq!(*seq, 0, "last committed seq before termination was 0");
        }
        other => panic!("expected Termination, got {other:?}"),
    }
    assert_eq!(term.seq, 1);
}

#[test]
fn record_termination_completed_reason() {
    // Normal turn completion also records a termination event.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let term = log
        .record_termination(&run, TerminationReason::Completed)
        .expect("term");
    match &term.kind {
        EventKind::Termination { reason, .. } => {
            assert_eq!(*reason, TerminationReason::Completed);
        }
        other => panic!("expected Termination, got {other:?}"),
    }
}

#[test]
fn termination_event_is_the_last_event_in_the_run() {
    // FR-003: the termination event is appended after all prior events.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    for i in 0..5 {
        log.append_new(
            &run,
            EventKind::Lifecycle {
                event: format!("e{i}"),
            },
        )
        .expect("append");
    }
    let term = log.record_interruption(&run).expect("term");
    let events = log.read_run(&run).expect("read");
    assert_eq!(events.len(), 6);
    assert_eq!(events.last().map(|e| e.seq), Some(term.seq));
    assert!(matches!(
        events.last().unwrap().kind,
        EventKind::Termination { .. }
    ));
}

#[test]
fn termination_payload_seq_reflects_state_at_interruption() {
    // FR-003: if the run is interrupted after 3 events, the payload seq is 2
    // (the last committed event), regardless of what came after in a resumed
    // continuation (which doesn't exist here).
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    for i in 0..3 {
        log.append_new(
            &run,
            EventKind::Lifecycle {
                event: format!("e{i}"),
            },
        )
        .expect("append");
    }
    let term = log.record_interruption(&run).expect("term");
    match &term.kind {
        EventKind::Termination { seq, .. } => assert_eq!(*seq, 2),
        other => panic!("expected Termination, got {other:?}"),
    }
}

#[test]
fn recorded_termination_is_durable_and_replayable() {
    // FR-001 + FR-003: the termination event is persisted and readable.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let term = log.record_interruption(&run).expect("term");
    let read = log
        .get_event(&run, term.seq)
        .expect("read")
        .expect("exists");
    assert_eq!(read, term);
}

#[test]
fn recorded_termination_has_nonempty_event_id() {
    // FR-002: the termination event carries a fresh immutable id.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let term = log.record_interruption(&run).expect("term");
    assert!(!term.id.as_str().is_empty());
}

#[test]
fn full_turn_terminates_with_termination_event() {
    // FR-003: a full turn (model msg -> tool call -> tool result) ends with a
    // termination event.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "Read README.md", None)
        .expect("msg");
    log.record_tool_call(&run, "c1", "read", r#"{"path":"README.md"}"#)
        .expect("call");
    log.record_tool_result(&run, "c1", "read", true, "# ragent")
        .expect("result");
    let term = log
        .record_termination(&run, TerminationReason::Completed)
        .expect("term");
    let events = log.read_run(&run).expect("read");
    assert_eq!(events.len(), 4);
    assert!(matches!(events[0].kind, EventKind::ModelMessage { .. }));
    assert!(matches!(events[1].kind, EventKind::ToolCall { .. }));
    assert!(matches!(events[2].kind, EventKind::ToolResult { .. }));
    assert!(matches!(events[3].kind, EventKind::Termination { .. }));
    assert_eq!(events[3].seq, term.seq);
}
