//! Tests for resume of an interrupted run (maka spec T-014, FR-006, FR-013).
//!
//! FR-006: "While a run is in the 'interrupted' state, the system shall expose
//! the run as resumable and shall not allow new events to be appended to it
//! until a resume operation is initiated."
//!
//! FR-013: "When a resume operation is invoked on an interrupted run, the
//! system shall replay the event log to reconstruct the active context, then
//! continue execution from the event following the last committed sequence
//! number."

#![forbid(unsafe_code)]

use ragent_storage::activity_log::{ActivityLog, AppendError};
use ragent_types::activity::{EventKind, RunStatus, TerminationReason};
use ragent_types::id::RunId;

#[test]
fn interrupted_run_rejects_new_appends() {
    // FR-006: while interrupted, new events cannot be appended.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "hi", None)
        .expect("msg");
    log.record_interruption(&run).expect("interrupt");

    // Append is rejected.
    let err = log
        .record_model_message(&run, "assistant", "hello", None)
        .unwrap_err();
    assert!(
        matches!(err, AppendError::RunInterrupted { ref run_id } if *run_id == run),
        "expected RunInterrupted, got {err:?}"
    );
    // The run is still resumable.
    assert_eq!(log.run_status(&run).unwrap(), RunStatus::Interrupted);
}

#[test]
fn interrupted_run_rejects_raw_append() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_interruption(&run).expect("interrupt");

    let event = ragent_types::activity::ActivityEvent::new(
        run.clone(),
        1,
        EventKind::Lifecycle { event: "x".into() },
    );
    let err = log.append(&event).unwrap_err();
    assert!(matches!(err, AppendError::RunInterrupted { .. }));
}

#[test]
fn active_run_allows_appends() {
    // A run that is NOT interrupted accepts appends normally.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "hi", None)
        .expect("msg");
    // No termination -> Active -> appends allowed.
    log.record_model_message(&run, "assistant", "hello", None)
        .expect("msg2");
    assert_eq!(log.run_status(&run).unwrap(), RunStatus::Active);
}

#[test]
fn completed_run_allows_appends() {
    // A completed run is NOT interrupted, so appends are allowed (the store
    // doesn't block completed runs — only interrupted ones per FR-006).
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_termination(&run, TerminationReason::Completed)
        .expect("term");
    // Append after completion is allowed (not interrupted).
    log.record_model_message(&run, "user", "more", None)
        .expect("msg");
    assert_eq!(log.count(&run).unwrap(), 2);
}

#[test]
fn resume_run_reconstructs_active_context() {
    // FR-013: resume replays the log to reconstruct the active context.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "Read README.md", None)
        .expect("msg");
    log.record_tool_call(&run, "c1", "read", r#"{"path":"README.md"}"#)
        .expect("call");
    log.record_interruption(&run).expect("interrupt");

    let result = log.resume_run(&run).expect("resume");
    // The projection contains the events before the interruption.
    assert_eq!(result.projection.messages.len(), 1);
    assert_eq!(result.projection.tool_calls.len(), 1);
    assert_eq!(
        result.projection.pending_tool_calls().len(),
        1,
        "c1 is pending"
    );
    assert!(
        result.projection.is_resumable(),
        "projection shows interrupted run"
    );
}

#[test]
fn resume_run_returns_resume_from_seq() {
    // FR-013: resume continues from the event following the last committed
    // sequence number (after the resume marker).
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "hi", None)
        .expect("msg"); // seq 0
    log.record_interruption(&run).expect("interrupt"); // seq 1

    let result = log.resume_run(&run).expect("resume");
    // The "resumed" lifecycle event is at seq 2; resume_from_seq is 3.
    assert_eq!(result.resume_from_seq, 3);
}

#[test]
fn resume_run_allows_subsequent_appends() {
    // FR-006: after resume is initiated, new events can be appended.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "hi", None)
        .expect("msg");
    log.record_interruption(&run).expect("interrupt");

    // Before resume: append rejected.
    assert!(
        log.record_model_message(&run, "assistant", "x", None)
            .is_err()
    );

    // Resume.
    let result = log.resume_run(&run).expect("resume");

    // After resume: append allowed at resume_from_seq.
    let next = log
        .record_model_message(&run, "assistant", "resumed", None)
        .expect("append after resume");
    assert_eq!(next.seq, result.resume_from_seq);

    // The run is now Active (events after the interruption termination).
    assert_eq!(log.run_status(&run).unwrap(), RunStatus::Active);
}

#[test]
fn resume_run_appends_resumed_lifecycle_event() {
    // The resume operation appends a "resumed" lifecycle event to the log.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_interruption(&run).expect("interrupt"); // seq 0

    log.resume_run(&run).expect("resume");

    let events = log.read_run(&run).expect("read");
    assert_eq!(events.len(), 2);
    match &events[1].kind {
        EventKind::Lifecycle { event } => assert_eq!(event, "resumed"),
        other => panic!("expected Lifecycle 'resumed', got {other:?}"),
    }
}

#[test]
fn resume_run_on_non_interrupted_errors() {
    // Resume on an Active run is an error (not interrupted).
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "hi", None)
        .expect("msg");
    let err = log.resume_run(&run).unwrap_err();
    assert!(
        err.to_string().contains("not interrupted"),
        "error mentions not interrupted: {err}"
    );
}

#[test]
fn resume_run_on_completed_errors() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_termination(&run, TerminationReason::Completed)
        .expect("term");
    let err = log.resume_run(&run).unwrap_err();
    assert!(err.to_string().contains("not interrupted"));
}

#[test]
fn resume_run_on_empty_run_errors() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-empty");
    let err = log.resume_run(&run).unwrap_err();
    assert!(err.to_string().contains("not interrupted"));
}

#[test]
fn resumed_run_can_complete() {
    // After resume, the run can continue to completion.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "Read README.md", None)
        .expect("msg");
    log.record_tool_call(&run, "c1", "read", "{}")
        .expect("call");
    log.record_interruption(&run).expect("interrupt");

    // Resume and complete the pending tool call.
    let result = log.resume_run(&run).expect("resume");
    log.record_tool_result(&run, "c1", "read", true, "# ragent")
        .expect("result");
    log.record_model_message(&run, "assistant", "Done.", None)
        .expect("msg");
    log.record_termination(&run, TerminationReason::Completed)
        .expect("term");

    // The run is now completed.
    assert_eq!(log.run_status(&run).unwrap(), RunStatus::Completed);
    let events = log.read_run(&run).expect("read");
    // msg, call, interrupt, resumed, result, msg, term = 7
    assert_eq!(events.len(), 7);
    assert!(matches!(
        events.last().unwrap().kind,
        EventKind::Termination { .. }
    ));
}

#[test]
fn resume_run_projection_excludes_resumed_event() {
    // The projection returned by resume reflects the active context BEFORE
    // the resume marker (FR-013: replay the log to reconstruct context).
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "hi", None)
        .expect("msg"); // seq 0
    log.record_interruption(&run).expect("interrupt"); // seq 1

    let result = log.resume_run(&run).expect("resume");
    // The projection has 1 message (the "hi"), not the "resumed" lifecycle.
    assert_eq!(result.projection.messages.len(), 1);
    // last_seq is 1 (the interruption event), not 2 (the resumed event).
    assert_eq!(result.projection.last_seq, 1);
}
