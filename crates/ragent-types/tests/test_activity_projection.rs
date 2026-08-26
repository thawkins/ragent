//! Tests for optional context pruning (maka spec T-018, FR-009).
//!
//! FR-009: "Where the operator enables context pruning, the system may omit
//! old tool results from the next model prompt without deleting the
//! corresponding events from the log."

#![forbid(unsafe_code)]

use ragent_types::activity::{ActivityEvent, EventKind, Projection};
use ragent_types::id::RunId;

fn ev(seq: u64, kind: EventKind) -> ActivityEvent {
    ActivityEvent::new(RunId::from("run-1"), seq, kind)
}

fn tool_result(call_id: &str, content: &str) -> EventKind {
    EventKind::ToolResult {
        tool_call_id: call_id.into(),
        tool: "read".into(),
        success: true,
        content: content.into(),
    }
}

#[test]
fn pruned_tool_results_keeps_all_when_keep_last_exceeds_count() {
    // FR-009: if keep_last >= len, all results are kept (no pruning).
    let events = [
        ev(
            0,
            EventKind::ToolCall {
                tool_call_id: "c1".into(),
                tool: "read".into(),
                args: "{}".into(),
            },
        ),
        ev(1, tool_result("c1", "r1")),
        ev(
            2,
            EventKind::ToolCall {
                tool_call_id: "c2".into(),
                tool: "read".into(),
                args: "{}".into(),
            },
        ),
        ev(3, tool_result("c2", "r2")),
    ];
    let proj = Projection::replay(&events);
    let pruned = proj.pruned_tool_results(10);
    assert_eq!(pruned.len(), 2);
}

#[test]
fn pruned_tool_results_keeps_last_n() {
    // FR-009: keep_last=1 returns only the most recent tool result.
    let events = [
        ev(
            0,
            EventKind::ToolCall {
                tool_call_id: "c1".into(),
                tool: "read".into(),
                args: "{}".into(),
            },
        ),
        ev(1, tool_result("c1", "old")),
        ev(
            2,
            EventKind::ToolCall {
                tool_call_id: "c2".into(),
                tool: "read".into(),
                args: "{}".into(),
            },
        ),
        ev(3, tool_result("c2", "new")),
    ];
    let proj = Projection::replay(&events);
    let pruned = proj.pruned_tool_results(1);
    assert_eq!(pruned.len(), 1);
    assert_eq!(pruned[0].content, "new", "keeps the most recent result");
}

#[test]
fn pruned_tool_results_zero_omits_all() {
    // FR-009: keep_last=0 omits all tool results.
    let events = [
        ev(
            0,
            EventKind::ToolCall {
                tool_call_id: "c1".into(),
                tool: "read".into(),
                args: "{}".into(),
            },
        ),
        ev(1, tool_result("c1", "r1")),
    ];
    let proj = Projection::replay(&events);
    let pruned = proj.pruned_tool_results(0);
    assert!(pruned.is_empty());
}

#[test]
fn pruned_tool_results_preserves_full_projection() {
    // FR-009: pruning does NOT delete events from the log or modify the
    // projection — it only affects the returned slice.
    let events = [
        ev(
            0,
            EventKind::ToolCall {
                tool_call_id: "c1".into(),
                tool: "read".into(),
                args: "{}".into(),
            },
        ),
        ev(1, tool_result("c1", "r1")),
        ev(
            2,
            EventKind::ToolCall {
                tool_call_id: "c2".into(),
                tool: "read".into(),
                args: "{}".into(),
            },
        ),
        ev(3, tool_result("c2", "r2")),
    ];
    let proj = Projection::replay(&events);
    let _ = proj.pruned_tool_results(1);
    // The full projection still has all tool results.
    assert_eq!(
        proj.tool_results.len(),
        2,
        "projection is unchanged after pruning"
    );
}

#[test]
fn pruned_tool_results_empty_run() {
    let proj = Projection::empty();
    assert!(proj.pruned_tool_results(5).is_empty());
}

#[test]
fn pruned_tool_results_keeps_last_three_of_five() {
    let mut events = Vec::new();
    for i in 0..5 {
        let call_id = format!("c{i}");
        events.push(ev(
            i as u64 * 2,
            EventKind::ToolCall {
                tool_call_id: call_id.clone(),
                tool: "read".into(),
                args: "{}".into(),
            },
        ));
        events.push(ev(
            i as u64 * 2 + 1,
            tool_result(&call_id, &format!("r{i}")),
        ));
    }
    let proj = Projection::replay(&events);
    let pruned = proj.pruned_tool_results(3);
    assert_eq!(pruned.len(), 3);
    assert_eq!(pruned[0].content, "r2");
    assert_eq!(pruned[1].content, "r3");
    assert_eq!(pruned[2].content, "r4");
}

#[test]
fn pruning_keeps_messages_intact() {
    // FR-009: pruning affects only tool results, not messages.
    let events = [
        ev(
            0,
            EventKind::ModelMessage {
                role: "user".into(),
                content: "hi".into(),
                message_id: None,
            },
        ),
        ev(
            1,
            EventKind::ToolCall {
                tool_call_id: "c1".into(),
                tool: "read".into(),
                args: "{}".into(),
            },
        ),
        ev(2, tool_result("c1", "r1")),
    ];
    let proj = Projection::replay(&events);
    let _ = proj.pruned_tool_results(0);
    assert_eq!(proj.messages.len(), 1, "messages are not pruned");
}

#[test]
fn pruning_with_usize_max_keeps_all() {
    let events = [
        ev(
            0,
            EventKind::ToolCall {
                tool_call_id: "c1".into(),
                tool: "read".into(),
                args: "{}".into(),
            },
        ),
        ev(1, tool_result("c1", "r1")),
        ev(
            2,
            EventKind::ToolCall {
                tool_call_id: "c2".into(),
                tool: "read".into(),
                args: "{}".into(),
            },
        ),
        ev(3, tool_result("c2", "r2")),
    ];
    let proj = Projection::replay(&events);
    let pruned = proj.pruned_tool_results(usize::MAX);
    assert_eq!(pruned.len(), 2);
}
