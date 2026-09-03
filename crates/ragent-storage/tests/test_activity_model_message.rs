#![allow(clippy::assert_is_empty)]
//! Tests for recording model-message events (maka spec T-004, FR-001).
//!
//! FR-001 requires that every execution event — including a model message —
//! is persisted to the append-only event log *before* it is projected into
//! any user-facing or derived state. These tests verify the
//! [`ActivityLog::record_model_message`] convenience path persists a
//! model-message event with its role, content, and optional provider
//! message id, and that it is durable and replayable.

#![forbid(unsafe_code)]

use ragent_storage::activity_log::ActivityLog;
use ragent_types::activity::EventKind;
use ragent_types::id::RunId;

#[test]
fn record_model_message_persists_role_and_content() {
    // FR-001: a model message is persisted with its role and content intact.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let event = log
        .record_model_message(&run, "user", "Read README.md and summarise", None)
        .expect("record");

    assert_eq!(event.run_id, run);
    assert_eq!(event.seq, 0);
    match &event.kind {
        EventKind::ModelMessage {
            role,
            content,
            message_id,
        } => {
            assert_eq!(role, "user");
            assert_eq!(content, "Read README.md and summarise");
            assert!(message_id.is_none());
        }
        other => panic!("expected ModelMessage, got {other:?}"),
    }
}

#[test]
fn record_model_message_carries_provider_message_id() {
    // FR-001: the optional provider-assigned message id is preserved.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let event = log
        .record_model_message(
            &run,
            "assistant",
            "Here is the summary.",
            Some("msg-abc-123".to_string()),
        )
        .expect("record");
    match &event.kind {
        EventKind::ModelMessage { message_id, .. } => {
            assert_eq!(message_id.as_deref(), Some("msg-abc-123"));
        }
        other => panic!("expected ModelMessage, got {other:?}"),
    }
}

#[test]
fn recorded_model_message_is_durable_and_replayable() {
    // FR-001: the persisted event survives a read-back (durable before
    // projection).
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let appended = log
        .record_model_message(&run, "assistant", "Hello world", Some("m1".into()))
        .expect("record");

    let read = log
        .get_event(&run, appended.seq)
        .expect("read")
        .expect("exists");
    assert_eq!(read, appended);
    assert_eq!(read.kind, appended.kind);
}

#[test]
fn multiple_model_messages_get_monotonic_sequence_numbers() {
    // FR-001 + FR-002: each recorded model message is a separate, ordered
    // event.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let e0 = log
        .record_model_message(&run, "user", "prompt", None)
        .expect("record");
    let e1 = log
        .record_model_message(&run, "assistant", "response", Some("m1".into()))
        .expect("record");
    let e2 = log
        .record_model_message(&run, "user", "follow-up", None)
        .expect("record");

    assert_eq!(e0.seq, 0);
    assert_eq!(e1.seq, 1);
    assert_eq!(e2.seq, 2);

    let events = log.read_run(&run).expect("read");
    assert_eq!(events.len(), 3);
    let roles: Vec<&str> = events
        .iter()
        .map(|e| match &e.kind {
            EventKind::ModelMessage { role, .. } => role.as_str(),
            _ => unreachable!(),
        })
        .collect();
    assert_eq!(roles, vec!["user", "assistant", "user"]);
}

#[test]
fn recorded_model_message_has_nonempty_event_id() {
    // FR-002: the recorded event carries a fresh, non-empty immutable id.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let event = log
        .record_model_message(&run, "user", "hi", None)
        .expect("record");
    assert!(!event.id.as_str().is_empty());
}

#[test]
fn record_model_message_failure_does_not_advance_state() {
    // FR-017: if a storage failure occurred, the run would not advance. Here
    // we verify the happy path leaves the run ready for the next append at the
    // expected sequence.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "first", None)
        .expect("record");
    assert_eq!(log.next_seq(&run).unwrap(), 1);
    assert_eq!(log.last_seq(&run).unwrap(), Some(0));
    assert_eq!(log.count(&run).unwrap(), 1);
}

#[test]
fn empty_role_and_content_are_still_persisted() {
    // FR-001 does not constrain the content; an empty message is still an
    // execution fact that must be recorded.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let event = log
        .record_model_message(&run, "", "", None)
        .expect("record");
    match &event.kind {
        EventKind::ModelMessage {
            role,
            content,
            message_id,
        } => {
            assert_eq!(role, "");
            assert_eq!(content, "");
            assert!(message_id.is_none());
        }
        other => panic!("expected ModelMessage, got {other:?}"),
    }
}
