//! Tests for the append-only activity event log store (maka spec T-002).
//!
//! Covers FR-001 (append persists before projection), FR-002 (monotonic seq +
//! immutable id), FR-017 (storage failure fails the operation without
//! advancing state), and NFR-001 (single-append latency).

#![forbid(unsafe_code)]

use ragent_storage::activity_log::{ActivityLog, AppendError};
use ragent_types::activity::{ACTIVITY_EVENT_SCHEMA_VERSION, ActivityEvent, EventKind};
use ragent_types::id::RunId;

fn make_event(run: &str, seq: u64, kind: EventKind) -> ActivityEvent {
    ActivityEvent::new(RunId::from(run), seq, kind)
}

#[test]
fn append_new_assigns_monotonic_sequence_numbers() {
    // FR-002: the store assigns a monotonically increasing per-run seq.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    assert_eq!(log.next_seq(&run).unwrap(), 0);

    let e0 = log
        .append_new(
            &run,
            EventKind::Lifecycle {
                event: "start".into(),
            },
        )
        .unwrap();
    let e1 = log
        .append_new(
            &run,
            EventKind::Lifecycle {
                event: "step".into(),
            },
        )
        .unwrap();
    let e2 = log
        .append_new(
            &run,
            EventKind::Lifecycle {
                event: "end".into(),
            },
        )
        .unwrap();

    assert_eq!(e0.seq, 0);
    assert_eq!(e1.seq, 1);
    assert_eq!(e2.seq, 2);
    assert_eq!(log.last_seq(&run).unwrap(), Some(2));
    assert_eq!(log.count(&run).unwrap(), 3);
}

#[test]
fn append_rejects_duplicate_sequence_number() {
    // FR-002: appending the same seq twice is rejected (append-only, no
    // mutation).
    let log = ActivityLog::open_in_memory().unwrap();
    let run = RunId::from("run-1");
    log.append_new(&run, EventKind::Lifecycle { event: "x".into() })
        .unwrap();

    let dup = make_event(
        "run-1",
        0,
        EventKind::Lifecycle {
            event: "dup".into(),
        },
    );
    let err = log.append(&dup).unwrap_err();
    match err {
        AppendError::DuplicateSeq { run_id, seq } => {
            assert_eq!(run_id, RunId::from("run-1"));
            assert_eq!(seq, 0);
        }
        other => panic!("expected DuplicateSeq, got {other:?}"),
    }
}

#[test]
fn append_rejects_out_of_order_sequence_number() {
    // FR-002: skipping ahead is rejected (no gaps).
    let log = ActivityLog::open_in_memory().unwrap();
    let run = RunId::from("run-1");
    log.append_new(&run, EventKind::Lifecycle { event: "x".into() })
        .unwrap();
    // next expected is 1; try 2
    let gap = make_event(
        "run-1",
        2,
        EventKind::Lifecycle {
            event: "gap".into(),
        },
    );
    let err = log.append(&gap).unwrap_err();
    match err {
        AppendError::OutOfOrder {
            run_id,
            seq,
            expected,
        } => {
            assert_eq!(run_id, RunId::from("run-1"));
            assert_eq!(seq, 2);
            assert_eq!(expected, 1);
        }
        other => panic!("expected OutOfOrder, got {other:?}"),
    }
}

#[test]
fn read_run_returns_events_in_sequence_order() {
    // FR-001: every appended event is durable and replayable in order.
    let log = ActivityLog::open_in_memory().unwrap();
    let run = RunId::from("run-1");
    for i in 0..5 {
        log.append_new(
            &run,
            EventKind::ModelMessage {
                role: "assistant".into(),
                content: format!("msg-{i}"),
                message_id: None,
            },
        )
        .unwrap();
    }
    let events = log.read_run(&run).unwrap();
    assert_eq!(events.len(), 5);
    for (i, e) in events.iter().enumerate() {
        assert_eq!(e.seq, i as u64);
    }
}

#[test]
fn read_run_upto_replays_only_up_to_target() {
    // FR-012 (preview): replay up to a chosen seq returns only those events.
    let log = ActivityLog::open_in_memory().unwrap();
    let run = RunId::from("run-1");
    for i in 0..5 {
        log.append_new(
            &run,
            EventKind::Lifecycle {
                event: format!("e{i}"),
            },
        )
        .unwrap();
    }
    let upto = log.read_run_upto(&run, 2).unwrap();
    assert_eq!(upto.len(), 3);
    assert_eq!(upto[0].seq, 0);
    assert_eq!(upto[2].seq, 2);
}

#[test]
fn events_round_trip_with_full_payload() {
    // Events must survive a write/read cycle with all fields intact.
    let log = ActivityLog::open_in_memory().unwrap();
    let run = RunId::from("run-1");
    let appended = log
        .append_new(
            &run,
            EventKind::ToolCall {
                tool_call_id: "call-1".into(),
                tool: "read".into(),
                args: r#"{"path":"README.md"}"#.into(),
            },
        )
        .unwrap();
    let read = log.read_run(&run).unwrap();
    assert_eq!(read.len(), 1);
    let got = &read[0];
    assert_eq!(got.id, appended.id);
    assert_eq!(got.run_id, appended.run_id);
    assert_eq!(got.seq, appended.seq);
    assert_eq!(got.schema_version, appended.schema_version);
    assert_eq!(got.kind, appended.kind);
}

#[test]
fn schema_version_is_stamped_on_stored_event() {
    // NFR-003: the stored event carries its schema version.
    let log = ActivityLog::open_in_memory().unwrap();
    let run = RunId::from("run-1");
    let e = log
        .append_new(&run, EventKind::Lifecycle { event: "x".into() })
        .unwrap();
    assert_eq!(e.schema_version, ACTIVITY_EVENT_SCHEMA_VERSION);
    let read = log.read_run(&run).unwrap();
    assert_eq!(read[0].schema_version, ACTIVITY_EVENT_SCHEMA_VERSION);
}

#[test]
fn multiple_runs_are_isolated_by_run_id() {
    // Each run has its own sequence space.
    let log = ActivityLog::open_in_memory().unwrap();
    let run_a = RunId::from("run-a");
    let run_b = RunId::from("run-b");
    log.append_new(&run_a, EventKind::Lifecycle { event: "a0".into() })
        .unwrap();
    log.append_new(&run_a, EventKind::Lifecycle { event: "a1".into() })
        .unwrap();
    log.append_new(&run_b, EventKind::Lifecycle { event: "b0".into() })
        .unwrap();

    assert_eq!(log.read_run(&run_a).unwrap().len(), 2);
    assert_eq!(log.read_run(&run_b).unwrap().len(), 1);
    assert_eq!(log.last_seq(&run_a).unwrap(), Some(1));
    assert_eq!(log.last_seq(&run_b).unwrap(), Some(0));
}

#[test]
fn last_seq_none_for_empty_run() {
    let log = ActivityLog::open_in_memory().unwrap();
    let run = RunId::from("run-empty");
    assert_eq!(log.last_seq(&run).unwrap(), None);
    assert_eq!(log.count(&run).unwrap(), 0);
    assert!(log.read_run(&run).unwrap().is_empty());
}

#[test]
fn file_backed_log_persists_across_reopen() {
    // FR-001: events are durable on local storage, not just in-memory.
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = format!("target/temp/activity_log_test_{n}");
    let path = std::path::PathBuf::from(&dir).join("log.db");
    let run = RunId::from("run-1");

    {
        let log = ActivityLog::open(&path).unwrap();
        log.append_new(
            &run,
            EventKind::Lifecycle {
                event: "first".into(),
            },
        )
        .unwrap();
        log.append_new(
            &run,
            EventKind::Lifecycle {
                event: "second".into(),
            },
        )
        .unwrap();
    }
    {
        let log = ActivityLog::open(&path).unwrap();
        assert_eq!(log.count(&run).unwrap(), 2);
        assert_eq!(log.last_seq(&run).unwrap(), Some(1));
        let events = log.read_run(&run).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].seq, 0);
        assert_eq!(events[1].seq, 1);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn append_failure_does_not_advance_state() {
    // FR-017: when an append is rejected, the store does not advance — the
    // next valid append still uses the correct sequence.
    let log = ActivityLog::open_in_memory().unwrap();
    let run = RunId::from("run-1");
    log.append_new(&run, EventKind::Lifecycle { event: "ok".into() })
        .unwrap();
    // attempt an out-of-order append — must fail
    let bad = make_event(
        "run-1",
        5,
        EventKind::Lifecycle {
            event: "bad".into(),
        },
    );
    assert!(log.append(&bad).is_err());
    // the store has not advanced: next seq is still 1
    assert_eq!(log.next_seq(&run).unwrap(), 1);
    let good = log
        .append_new(
            &run,
            EventKind::Lifecycle {
                event: "good".into(),
            },
        )
        .unwrap();
    assert_eq!(good.seq, 1);
    assert_eq!(log.count(&run).unwrap(), 2);
}

#[test]
fn append_latency_single_event_under_budget() {
    // NFR-001 target: p99 < 10ms for a single append on local storage. Here we
    // do a coarse smoke check (in-memory) that a single append completes in
    // well under 10ms on average, so a regression to O(n) per-append would be
    // caught.
    use std::time::Instant;
    let log = ActivityLog::open_in_memory().unwrap();
    let run = RunId::from("run-1");
    // warm up
    for _ in 0..10 {
        log.append_new(
            &run,
            EventKind::Lifecycle {
                event: "warm".into(),
            },
        )
        .unwrap();
    }
    let start = Instant::now();
    for _ in 0..100 {
        log.append_new(
            &run,
            EventKind::Lifecycle {
                event: "bench".into(),
            },
        )
        .unwrap();
    }
    let elapsed = start.elapsed();
    let per_append = elapsed / 100;
    // generous upper bound: 10ms each. In-memory is far faster; this just
    // guards against an accidental O(n) scan per append.
    assert!(
        per_append.as_millis() < 10,
        "per-append took {:?}, expected < 10ms",
        per_append
    );
}
