#![allow(clippy::assert_is_empty)]
//! Tests for optional context pruning integration with the activity log
//! (maka spec T-018, FR-009 + FR-015).
//!
//! These tests verify that pruning tool results from the model prompt does not
//! delete the underlying events from the append-only log.

#![forbid(unsafe_code)]

use ragent_storage::activity_log::ActivityLog;
use ragent_types::activity::EventKind;
use ragent_types::id::RunId;

#[test]
fn pruning_does_not_delete_events_from_log() {
    // FR-009 + FR-015: pruning omits tool results from the prompt but the
    // events remain in the log.
    let log = ActivityLog::open_in_memory().expect("open");
    let run = RunId::from("run-1");
    log.record_tool_call(&run, "c1", "read", "{}")
        .expect("call1");
    log.record_tool_result(&run, "c1", "read", true, "old")
        .expect("result1");
    log.record_tool_call(&run, "c2", "read", "{}")
        .expect("call2");
    log.record_tool_result(&run, "c2", "read", true, "new")
        .expect("result2");

    let proj = log.replay_run(&run).expect("replay");
    let pruned = proj.pruned_tool_results(1);

    // The pruned prompt has only 1 result.
    assert_eq!(pruned.len(), 1);

    // But the log still has all 4 events.
    assert_eq!(log.count(&run).unwrap(), 4);
    let events = log.read_run(&run).expect("read");
    assert_eq!(events.len(), 4);
    // The old tool result is still in the log.
    assert!(
        events
            .iter()
            .any(|e| matches!(&e.kind, EventKind::ToolResult { content, .. } if content == "old"))
    );
}
