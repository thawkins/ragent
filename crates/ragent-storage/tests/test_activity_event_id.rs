//! Tests for the immutable event-id and sequence-number guarantees of the
//! activity log (maka spec T-003, FR-002).
//!
//! These complement the T-002 store tests by focusing on the *event
//! identifier* half of FR-002:
//!
//! - every appended event carries a non-empty, immutable [`EventId`],
//! - the id is globally unique (a re-append of an existing id is rejected),
//! - a committed event's id can be read back unchanged (immutability),
//! - the per-run sequence number is monotonic and never reused.

#![forbid(unsafe_code)]

use ragent_storage::activity_log::{ActivityLog, AppendError};
use ragent_types::activity::{ActivityEvent, EventKind};
use ragent_types::id::{EventId, RunId};

fn lifecycle(event: &str) -> EventKind {
    EventKind::Lifecycle {
        event: event.into(),
    }
}

#[test]
fn get_event_reads_back_committed_event_with_unchanged_id() {
    // FR-002: the immutable event id survives a write/read cycle.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let appended = log.append_new(&run, lifecycle("first")).expect("append");

    let got = log
        .get_event(&run, appended.seq)
        .expect("read")
        .expect("event exists");
    assert_eq!(got.id, appended.id, "id is unchanged after round-trip");
    assert_eq!(got.seq, appended.seq);
    assert_eq!(got.kind, appended.kind);
}

#[test]
fn find_by_id_reads_back_committed_event() {
    // FR-002: an event can be retrieved by its immutable id alone.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let appended = log.append_new(&run, lifecycle("by-id")).expect("append");

    let got = log
        .find_by_id(&appended.id)
        .expect("read")
        .expect("event exists");
    assert_eq!(got.id, appended.id);
    assert_eq!(got.run_id, run);
    assert_eq!(got.seq, appended.seq);
}

#[test]
fn find_by_id_returns_none_for_unknown_id() {
    let log = ActivityLog::open_in_memory().expect("open");
    let unknown = EventId::from("never-committed");
    assert!(log.find_by_id(&unknown).expect("read").is_none());
}

#[test]
fn get_event_returns_none_for_missing_seq() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    assert!(log.get_event(&run, 0).expect("read").is_none());
}

#[test]
fn append_rejects_empty_event_id() {
    // FR-002: an event without an id is rejected.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let mut event = ActivityEvent::new(run.clone(), 0, lifecycle("no-id"));
    event.id = EventId::from("");
    let err = log.append(&event).unwrap_err();
    assert!(
        matches!(err, AppendError::EmptyEventId { ref run_id, seq: 0 } if *run_id == run),
        "expected EmptyEventId, got {err:?}"
    );
}

#[test]
fn append_rejects_duplicate_event_id() {
    // FR-002: a second event reusing an already-committed id is rejected —
    // ids are immutable and globally unique.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let first = log.append_new(&run, lifecycle("first")).expect("append");

    // Build a second event with a fresh (valid) seq but the SAME id as `first`.
    let second = ActivityEvent {
        id: first.id.clone(),
        run_id: run.clone(),
        seq: first.seq + 1,
        schema_version: first.schema_version,
        timestamp: first.timestamp,
        kind: lifecycle("second"),
    };
    let err = log.append(&second).unwrap_err();
    assert!(
        matches!(err, AppendError::DuplicateEventId { ref id } if *id == first.id),
        "expected DuplicateEventId, got {err:?}"
    );
}

#[test]
fn duplicate_id_rejection_advances_state_with_audit() {
    // FR-010: the rejected mutation records a MutationRejected audit event.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let first = log.append_new(&run, lifecycle("first")).expect("append");
    let dup = ActivityEvent {
        id: first.id.clone(),
        run_id: run.clone(),
        seq: first.seq + 1,
        schema_version: first.schema_version,
        timestamp: first.timestamp,
        kind: lifecycle("dup"),
    };
    assert!(log.append(&dup).is_err());
    // FR-010: the rejected mutation records a MutationRejected audit event,
    // which occupies the next seq (1). The next valid append uses seq 2.
    let next = log.append_new(&run, lifecycle("next")).expect("append");
    assert_eq!(next.seq, 2);
    // The original event is unchanged.
    assert_eq!(log.get_event(&run, 0).unwrap().unwrap().id, first.id);
}

#[test]
fn event_ids_are_globally_unique_across_runs() {
    // FR-002: two events (even across different runs) get distinct ids, and
    // the id column is globally unique.
    let log = ActivityLog::open_in_memory().expect("open");
    let run_a = RunId::from("run-a");
    let run_b = RunId::from("run-b");
    let a = log.append_new(&run_a, lifecycle("a")).expect("append");
    let b = log.append_new(&run_b, lifecycle("b")).expect("append");
    assert_ne!(a.id, b.id, "ids are globally unique");

    // find_by_id resolves each unambiguously.
    assert_eq!(log.find_by_id(&a.id).unwrap().unwrap().run_id, run_a);
    assert_eq!(log.find_by_id(&b.id).unwrap().unwrap().run_id, run_b);
}

#[test]
fn sequence_numbers_are_per_run_and_monotonic() {
    // FR-002: each run has its own monotonic sequence space starting at 0.
    let log = ActivityLog::open_in_memory().expect("open");
    let run_a = RunId::from("run-a");
    let run_b = RunId::from("run-b");

    let a0 = log.append_new(&run_a, lifecycle("a0")).expect("append");
    let b0 = log.append_new(&run_b, lifecycle("b0")).expect("append");
    let a1 = log.append_new(&run_a, lifecycle("a1")).expect("append");

    assert_eq!(a0.seq, 0);
    assert_eq!(a1.seq, 1);
    assert_eq!(b0.seq, 0, "run-b has its own sequence space");
    assert_eq!(log.last_seq(&run_a).unwrap(), Some(1));
    assert_eq!(log.last_seq(&run_b).unwrap(), Some(0));
}

#[test]
fn read_back_event_id_matches_appended_id_for_many_events() {
    // FR-002: immutability holds across a run of many events.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    let mut appended_ids = Vec::new();
    for i in 0..20 {
        let e = log
            .append_new(
                &run,
                EventKind::Lifecycle {
                    event: format!("e{i}"),
                },
            )
            .expect("append");
        appended_ids.push((e.seq, e.id));
    }
    for (seq, id) in &appended_ids {
        let got = log.get_event(&run, *seq).expect("read").expect("exists");
        assert_eq!(&got.id, id, "id at seq {seq} is unchanged");
    }
}
