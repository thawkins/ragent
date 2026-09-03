#![allow(clippy::assert_is_empty)]
//! Tests for JSON Lines export of a run's complete event log (maka spec
//! T-020, NFR-004).
//!
//! NFR-004: "The system shall provide an export of a run's complete event log
//! in a machine-readable format (JSON Lines) for external audit."

#![forbid(unsafe_code)]

use std::io::BufWriter;

use ragent_storage::activity_log::ActivityLog;
use ragent_types::activity::{ActivityEvent, EventKind, TerminationReason};
use ragent_types::id::RunId;

#[test]
fn export_jsonl_produces_one_line_per_event() {
    // NFR-004: the export is JSON Lines — one JSON object per line.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "hi", None)
        .expect("msg");
    log.record_tool_call(&run, "c1", "read", "{}")
        .expect("call");
    log.record_tool_result(&run, "c1", "read", true, "ok")
        .expect("result");

    let jsonl = log.export_jsonl(&run).expect("export");
    let lines: Vec<&str> = jsonl.lines().collect();
    assert_eq!(lines.len(), 3, "one line per event");
    // Each line is a standalone JSON object (not an array).
    for line in &lines {
        let trimmed = line.trim();
        assert!(
            trimmed.starts_with('{') && trimmed.ends_with('}'),
            "each line is a JSON object: {trimmed}"
        );
        assert!(
            serde_json::from_str::<serde_json::Value>(trimmed).is_ok(),
            "each line parses as JSON"
        );
    }
}

#[test]
fn exported_lines_deserialize_back_to_events() {
    // NFR-004: the export is machine-readable — each line deserializes to an
    // ActivityEvent matching the stored event.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "prompt", None)
        .expect("msg");
    log.record_tool_call(&run, "c1", "read", r#"{"path":"a"}"#)
        .expect("call");
    log.record_tool_result(&run, "c1", "read", true, "content")
        .expect("result");
    log.record_termination(&run, TerminationReason::Completed)
        .expect("term");

    let jsonl = log.export_jsonl(&run).expect("export");
    let stored = log.read_run(&run).expect("read");
    let exported: Vec<ActivityEvent> = jsonl
        .lines()
        .map(|l| serde_json::from_str(l).expect("parse"))
        .collect();
    assert_eq!(
        exported, stored,
        "exported events match stored events in order"
    );
}

#[test]
fn export_preserves_event_order() {
    // NFR-004: events are exported in ascending sequence-number order.
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
    let jsonl = log.export_jsonl(&run).expect("export");
    let events: Vec<ActivityEvent> = jsonl
        .lines()
        .map(|l| serde_json::from_str(l).expect("parse"))
        .collect();
    for (i, e) in events.iter().enumerate() {
        assert_eq!(e.seq, i as u64, "event at line {} has seq {}", i, e.seq);
    }
}

#[test]
fn export_includes_all_event_types_in_a_full_turn() {
    // NFR-004: the export captures every event type in a full turn.
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

    let jsonl = log.export_jsonl(&run).expect("export");
    let events: Vec<ActivityEvent> = jsonl
        .lines()
        .map(|l| serde_json::from_str(l).expect("parse"))
        .collect();
    assert!(matches!(events[0].kind, EventKind::ModelMessage { .. }));
    assert!(matches!(events[1].kind, EventKind::ToolCall { .. }));
    assert!(matches!(events[2].kind, EventKind::ToolResult { .. }));
    assert!(matches!(events[3].kind, EventKind::Termination { .. }));
}

#[test]
fn export_is_self_describing() {
    // NFR-003 + NFR-004: each exported line carries its type, schema version,
    // and run identifier.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "assistant", "hello", Some("m1".into()))
        .expect("msg");

    let jsonl = log.export_jsonl(&run).expect("export");
    let line = jsonl.lines().next().expect("one line");
    let v: serde_json::Value = serde_json::from_str(line).expect("parse");
    assert!(v.get("kind").is_some(), "carries kind discriminator");
    assert!(v.get("schema_version").is_some(), "carries schema_version");
    assert!(v.get("run_id").is_some(), "carries run_id");
    assert!(v.get("id").is_some(), "carries id");
    assert!(v.get("seq").is_some(), "carries seq");
}

#[test]
fn export_empty_run_yields_empty_string() {
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-empty");
    let jsonl = log.export_jsonl(&run).expect("export");
    assert!(jsonl.is_empty(), "empty run exports empty string");
}

#[test]
fn export_jsonl_to_writes_to_writer() {
    // NFR-004: the export can be written to any writer (file, stream).
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "hi", None)
        .expect("msg");
    log.record_tool_call(&run, "c1", "read", "{}")
        .expect("call");

    let mut buf = Vec::new();
    log.export_jsonl_to(&run, &mut buf).expect("export");
    let jsonl = String::from_utf8(buf).expect("utf8");
    assert_eq!(jsonl.lines().count(), 2);
    for line in jsonl.lines() {
        assert!(serde_json::from_str::<serde_json::Value>(line).is_ok());
    }
}

#[test]
fn export_jsonl_to_file_is_valid_jsonl() {
    // NFR-004: the export written to a file is valid JSONL readable back.
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    let dir = format!("target/temp/activity_jsonl_test_{n}");
    let path = std::path::PathBuf::from(&dir).join("export.jsonl");
    let run = RunId::from("run-1");

    {
        let log = ActivityLog::open_in_memory().expect("open");
        log.record_model_message(&run, "user", "Read README.md", None)
            .expect("msg");
        log.record_tool_call(&run, "c1", "read", r#"{"path":"README.md"}"#)
            .expect("call");
        log.record_tool_result(&run, "c1", "read", true, "# ragent")
            .expect("result");
        log.record_termination(&run, TerminationReason::Completed)
            .expect("term");

        std::fs::create_dir_all(&dir).expect("mkdir");
        let file = std::fs::File::create(&path).expect("create");
        let mut writer = BufWriter::new(file);
        log.export_jsonl_to(&run, &mut writer).expect("export");
    }

    let content = std::fs::read_to_string(&path).expect("read");
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 4, "4 events in the export file");
    for line in &lines {
        let evt: ActivityEvent = serde_json::from_str(line).expect("parse");
        assert_eq!(evt.run_id, run);
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn export_includes_checkpoint_events() {
    // NFR-004: checkpoint events are included in the export.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_checkpoint(&run, "cp1").expect("cp");
    let jsonl = log.export_jsonl(&run).expect("export");
    let events: Vec<ActivityEvent> = jsonl
        .lines()
        .map(|l| serde_json::from_str(l).expect("parse"))
        .collect();
    assert_eq!(events.len(), 1);
    assert!(matches!(events[0].kind, EventKind::Checkpoint { .. }));
}

#[test]
fn export_includes_mutation_rejected_audit_events() {
    // NFR-004: MutationRejected audit events (FR-010) are included.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.append_new(&run, EventKind::Lifecycle { event: "e0".into() })
        .expect("append");
    log.try_delete_event(&run, 0).unwrap_err();

    let jsonl = log.export_jsonl(&run).expect("export");
    let events: Vec<ActivityEvent> = jsonl
        .lines()
        .map(|l| serde_json::from_str(l).expect("parse"))
        .collect();
    assert_eq!(events.len(), 2);
    assert!(matches!(events[1].kind, EventKind::MutationRejected { .. }));
}

#[test]
fn export_jsonl_matches_read_run() {
    // NFR-004: the export is a faithful copy of the log.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_model_message(&run, "user", "hi", None)
        .expect("msg");
    log.record_permission_decision(
        &run,
        "bash",
        ragent_types::activity::Principal::Operator,
        ragent_types::activity::BoundaryTarget::Shell,
        true,
    )
    .expect("perm");
    log.record_tool_call(&run, "c1", "bash", r#"{"command":"ls"}"#)
        .expect("call");
    log.record_tool_result(&run, "c1", "bash", true, "file.txt")
        .expect("result");
    log.record_termination(&run, TerminationReason::Completed)
        .expect("term");

    let jsonl = log.export_jsonl(&run).expect("export");
    let from_jsonl: Vec<ActivityEvent> = jsonl
        .lines()
        .map(|l| serde_json::from_str(l).expect("parse"))
        .collect();
    let from_store = log.read_run(&run).expect("read");
    assert_eq!(from_jsonl, from_store);
}
