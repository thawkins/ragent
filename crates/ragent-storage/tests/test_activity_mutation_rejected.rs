//! Tests for rejecting mutation of committed events (maka spec T-016, FR-010).
//!
//! FR-010: "If an attempt is made to delete or mutate an already-committed
//! event, the system shall reject the attempt and shall record the rejected
//! mutation as a separate audit event."

#![forbid(unsafe_code)]

use ragent_storage::activity_log::{ActivityLog, AppendError};
use ragent_types::activity::{ActivityEvent, EventKind};
use ragent_types::id::RunId;

#[test]
fn try_delete_event_rejects_and_records_audit() {
    // FR-010: deleting a committed event is rejected, and a MutationRejected
    // audit event is recorded.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.append_new(&run, EventKind::Lifecycle { event: "e0".into() })
        .expect("append");
    log.append_new(&run, EventKind::Lifecycle { event: "e1".into() })
        .expect("append");

    let err = log.try_delete_event(&run, 0).unwrap_err();
    assert!(
        matches!(err, AppendError::MutationRejected { target_seq: 0, .. }),
        "expected MutationRejected, got {err:?}"
    );

    // The committed event is still there (not deleted).
    assert!(log.get_event(&run, 0).unwrap().is_some());

    // A MutationRejected audit event was appended after the existing events.
    let events = log.read_run(&run).expect("read");
    let audit = events.last().expect("audit event exists");
    match &audit.kind {
        EventKind::MutationRejected {
            target_seq,
            attempted,
        } => {
            assert_eq!(*target_seq, 0);
            assert!(
                attempted.contains("delete"),
                "audit describes a delete: {attempted}"
            );
        }
        other => panic!("expected MutationRejected audit, got {other:?}"),
    }
    assert!(audit.seq > 1, "audit is appended after existing events");
}

#[test]
fn try_update_event_rejects_and_records_audit() {
    // FR-010: mutating (overwriting) a committed event is rejected, and a
    // MutationRejected audit event is recorded.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.append_new(
        &run,
        EventKind::Lifecycle {
            event: "original".into(),
        },
    )
    .expect("append");

    let err = log
        .try_update_event(
            &run,
            0,
            &EventKind::Lifecycle {
                event: "tampered".into(),
            },
        )
        .unwrap_err();
    assert!(matches!(
        err,
        AppendError::MutationRejected { target_seq: 0, .. }
    ));

    // The committed event is unchanged.
    let evt = log.get_event(&run, 0).unwrap().unwrap();
    match &evt.kind {
        EventKind::Lifecycle { event } => assert_eq!(event, "original"),
        other => panic!("event unchanged, got {other:?}"),
    }

    // A MutationRejected audit event was recorded.
    let events = log.read_run(&run).expect("read");
    assert!(matches!(
        events.last().unwrap().kind,
        EventKind::MutationRejected { .. }
    ));
}

#[test]
fn try_delete_nonexistent_event_rejects_without_audit() {
    // FR-010: if the target doesn't exist, the delete is still rejected but
    // no audit event is recorded (nothing committed was mutated).
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let err = log.try_delete_event(&run, 99).unwrap_err();
    assert!(matches!(
        err,
        AppendError::MutationRejected { target_seq: 99, .. }
    ));
    // No events at all — no audit recorded for a non-existent target.
    assert_eq!(log.count(&run).unwrap(), 0);
}

#[test]
fn append_duplicate_seq_rejects_and_records_audit() {
    // FR-010: appending an event with an already-committed seq (an overwrite
    // attempt) is rejected, and a MutationRejected audit event is recorded.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let first = log
        .append_new(
            &run,
            EventKind::Lifecycle {
                event: "first".into(),
            },
        )
        .expect("append");

    // Try to append another event at the same seq (overwrite attempt).
    let dup = ActivityEvent::new(
        run.clone(),
        0,
        EventKind::Lifecycle {
            event: "overwrite".into(),
        },
    );
    let err = log.append(&dup).unwrap_err();
    assert!(
        matches!(err, AppendError::DuplicateSeq { seq: 0, .. }),
        "expected DuplicateSeq, got {err:?}"
    );

    // The original event is unchanged.
    let evt = log.get_event(&run, 0).unwrap().unwrap();
    assert_eq!(evt.id, first.id, "original event id unchanged");

    // A MutationRejected audit event was recorded.
    let events = log.read_run(&run).expect("read");
    assert!(matches!(
        events.last().unwrap().kind,
        EventKind::MutationRejected { .. }
    ));
}

#[test]
fn append_duplicate_event_id_rejects_and_records_audit() {
    // FR-010: appending an event with an already-committed event id (an
    // identity-mutation attempt) is rejected, and a MutationRejected audit
    // event is recorded.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let first = log
        .append_new(
            &run,
            EventKind::Lifecycle {
                event: "first".into(),
            },
        )
        .expect("append");

    // Build a second event with a fresh (valid) seq but the SAME id.
    let second = ActivityEvent {
        id: first.id.clone(),
        run_id: run.clone(),
        seq: first.seq + 1,
        schema_version: first.schema_version,
        timestamp: first.timestamp,
        kind: EventKind::Lifecycle {
            event: "reuse-id".into(),
        },
    };
    let err = log.append(&second).unwrap_err();
    assert!(
        matches!(err, AppendError::DuplicateEventId { .. }),
        "expected DuplicateEventId, got {err:?}"
    );

    // A MutationRejected audit event was recorded.
    let events = log.read_run(&run).expect("read");
    assert!(matches!(
        events.last().unwrap().kind,
        EventKind::MutationRejected { .. }
    ));
}

#[test]
fn mutation_rejected_audit_carries_target_seq() {
    // FR-010: the audit event records the seq of the committed event that was
    // the target of the rejected mutation.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.append_new(&run, EventKind::Lifecycle { event: "a".into() })
        .expect("e0");
    log.append_new(&run, EventKind::Lifecycle { event: "b".into() })
        .expect("e1");
    log.append_new(&run, EventKind::Lifecycle { event: "c".into() })
        .expect("e2");

    log.try_delete_event(&run, 1).unwrap_err();

    let events = log.read_run(&run).expect("read");
    let audit = events.last().unwrap();
    match &audit.kind {
        EventKind::MutationRejected { target_seq, .. } => assert_eq!(*target_seq, 1),
        other => panic!("expected MutationRejected, got {other:?}"),
    }
}

#[test]
fn multiple_mutation_attempts_each_record_audit() {
    // FR-010: each rejected mutation attempt records its own audit event.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.append_new(&run, EventKind::Lifecycle { event: "e0".into() })
        .expect("append");

    log.try_delete_event(&run, 0).unwrap_err();
    log.try_update_event(&run, 0, &EventKind::Lifecycle { event: "x".into() })
        .unwrap_err();

    let events = log.read_run(&run).expect("read");
    // 1 original + 2 audit events.
    assert_eq!(events.len(), 3);
    assert!(matches!(events[1].kind, EventKind::MutationRejected { .. }));
    assert!(matches!(events[2].kind, EventKind::MutationRejected { .. }));
}

#[test]
fn committed_events_remain_intact_after_mutation_attempts() {
    // FR-010: after mutation attempts, all original committed events are
    // intact.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let e0 = log
        .append_new(&run, EventKind::Lifecycle { event: "e0".into() })
        .expect("append");
    let e1 = log
        .append_new(&run, EventKind::Lifecycle { event: "e1".into() })
        .expect("append");

    log.try_delete_event(&run, 0).unwrap_err();
    log.try_update_event(
        &run,
        1,
        &EventKind::Lifecycle {
            event: "tampered".into(),
        },
    )
    .unwrap_err();

    // Both originals unchanged.
    let g0 = log.get_event(&run, 0).unwrap().unwrap();
    let g1 = log.get_event(&run, 1).unwrap().unwrap();
    assert_eq!(g0.id, e0.id);
    assert_eq!(g1.id, e1.id);
    match (&g0.kind, &g1.kind) {
        (EventKind::Lifecycle { event: a }, EventKind::Lifecycle { event: b }) => {
            assert_eq!(a, "e0");
            assert_eq!(b, "e1");
        }
        other => panic!("originals intact, got {other:?}"),
    }
}

#[test]
fn store_has_no_delete_or_update_api() {
    // FR-010 structural: the ActivityLog exposes only try_delete_event /
    // try_update_event, which always reject. There is no method that
    // actually deletes or updates a committed row. This test documents that
    // the audit count grows but the original events persist.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.append_new(
        &run,
        EventKind::Lifecycle {
            event: "original".into(),
        },
    )
    .expect("append");
    let before = log.count(&run).unwrap();
    log.try_delete_event(&run, 0).unwrap_err();
    log.try_update_event(&run, 0, &EventKind::Lifecycle { event: "x".into() })
        .unwrap_err();
    let after = log.count(&run).unwrap();
    // Count grew by 2 (the two audit events); the original event is still
    // there (not deleted).
    assert_eq!(after, before + 2);
    assert_eq!(log.get_event(&run, 0).unwrap().unwrap().kind, {
        let e: EventKind = EventKind::Lifecycle {
            event: "original".into(),
        };
        e
    });
}
