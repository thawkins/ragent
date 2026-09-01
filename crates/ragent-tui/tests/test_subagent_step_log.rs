//! TUI tests for sub-agent / teammate step logging (agents & teams panels).
//!
//! The Agents and Teams panels' `steps` column counts one step per
//! `ToolCallStart` for the session (via `EventBus::increment_tool_calls`,
//! incremented by the session processor for every session). These tests verify
//! that each of those tool calls is mirrored into the shared log — tagged with
//! the owning agent — so the visible log step count matches the panel column.

use ragent_agent::event::Event;
use ragent_team::team::{TeamConfig, TeamMember};
use ragent_tui::app::LogLevel;
use ragent_types::event::ToolCallBatchEntry;

#[path = "support/mod.rs"]
mod support;

fn tool_call_lines<'a>(entries: &'a [ragent_tui::app::LogEntry], needle: &str) -> Vec<&'a str> {
    entries
        .iter()
        .filter(|e| e.level == LogLevel::Tool)
        .map(|e| e.message.as_str())
        .filter(|m| m.contains(needle))
        .collect()
}

#[test]
fn test_tool_call_start_tracked_subagent_is_logged_with_step_tag() {
    let mut app = support::make_app();
    app.session_id = Some("lead-sess".to_string());

    // Register a sub-agent task so its child session becomes tracked.
    app.handle_event(Event::SubagentStart {
        session_id: "lead-sess".to_string(),
        task_id: "explore-a1b2c3d4".to_string(),
        child_session_id: "child-sess-1".to_string(),
        agent: "explore".to_string(),
        task: "find callers".to_string(),
        background: false,
    });

    // The processor set the child session's loop step to 2 and is executing
    // its first tool call of that step.
    app.event_bus.set_step("child-sess-1", 2);
    app.handle_event(Event::ToolCallStart {
        session_id: "child-sess-1".to_string(),
        call_id: "call-1".to_string(),
        tool: "grep".to_string(),
    });

    let lines = tool_call_lines(&app.log_entries, "tool call: grep");
    assert_eq!(lines.len(), 1, "exactly one tool-call log line: {lines:?}");
    assert!(
        lines[0].contains("[explore-a1b2c3d4] "),
        "line should carry the agent task tag: {}",
        lines[0]
    );
    assert!(
        lines[0].contains("[2.1] tool call: grep"),
        "line should carry the loop step/substep tag: {}",
        lines[0]
    );

    // The step must be visible without polluting the primary transcript.
    assert!(
        !lines[0].starts_with("[2.1]"),
        "agent tag must precede the step tag: {}",
        lines[0]
    );
}

#[test]
fn test_tool_call_count_for_tracked_subagent_matches_panel_counter() {
    let mut app = support::make_app();
    app.session_id = Some("lead-sess".to_string());

    app.handle_event(Event::SubagentStart {
        session_id: "lead-sess".to_string(),
        task_id: "build-99887766".to_string(),
        child_session_id: "child-sess-2".to_string(),
        agent: "build".to_string(),
        task: "build the crate".to_string(),
        background: true,
    });

    // Simulate 3 tool calls across 2 loop steps (second step runs 2 calls).
    // The processor increments the bus counter once per ToolCallStart; the
    // Agents/Teams panels read this counter for the `steps` column.
    for _ in 0..3 {
        app.event_bus.increment_tool_calls("child-sess-2");
    }
    app.event_bus.set_step("child-sess-2", 1);
    for (i, tool) in ["read", "grep"].iter().enumerate() {
        app.handle_event(Event::ToolCallStart {
            session_id: "child-sess-2".to_string(),
            call_id: format!("call-a{i}"),
            tool: (*tool).to_string(),
        });
    }
    app.event_bus.set_step("child-sess-2", 2);
    app.handle_event(Event::ToolCallStart {
        session_id: "child-sess-2".to_string(),
        call_id: "call-a2".to_string(),
        tool: "bash".to_string(),
    });

    let expected = app.event_bus.current_tool_calls("child-sess-2");

    let lines = tool_call_lines(&app.log_entries, "[build-99887766] ");
    assert_eq!(
        lines.len(),
        expected as usize,
        "one log line per tool call so the panel step count is auditable: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("[1.2] tool call: grep")),
        "parallel calls in one step get sequential substeps: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("[2.1] tool call: bash")),
        "a new loop step resets the substep counter: {lines:?}"
    );
}

#[test]
fn test_tool_call_end_tracked_subagent_is_logged_with_step_tag() {
    let mut app = support::make_app();
    app.session_id = Some("lead-sess".to_string());

    app.handle_event(Event::SubagentStart {
        session_id: "lead-sess".to_string(),
        task_id: "plan-11223344".to_string(),
        child_session_id: "child-sess-3".to_string(),
        agent: "plan".to_string(),
        task: "make a plan".to_string(),
        background: false,
    });

    app.event_bus.set_step("child-sess-3", 1);
    app.handle_event(Event::ToolCallStart {
        session_id: "child-sess-3".to_string(),
        call_id: "call-b1".to_string(),
        tool: "read".to_string(),
    });
    app.handle_event(Event::ToolCallEnd {
        session_id: "child-sess-3".to_string(),
        call_id: "call-b1".to_string(),
        tool: "read".to_string(),
        error: None,
        duration_ms: 42,
    });

    let lines = tool_call_lines(&app.log_entries, "tool read completed");
    assert_eq!(lines.len(), 1, "exactly one completion line: {lines:?}");
    assert!(
        lines[0].contains("[plan-11223344] [1.1] tool read completed (42ms)"),
        "completion line should carry agent tag and step tag: {}",
        lines[0]
    );

    // Failed calls are logged at error level with the same attribution.
    app.handle_event(Event::ToolCallStart {
        session_id: "child-sess-3".to_string(),
        call_id: "call-b2".to_string(),
        tool: "write".to_string(),
    });
    app.handle_event(Event::ToolCallEnd {
        session_id: "child-sess-3".to_string(),
        call_id: "call-b2".to_string(),
        tool: "write".to_string(),
        error: Some("denied".to_string()),
        duration_ms: 5,
    });
    let failed: Vec<_> = app
        .log_entries
        .iter()
        .filter(|e| e.level == LogLevel::Error && e.message.contains("tool write failed"))
        .map(|e| e.message.as_str())
        .collect();
    assert_eq!(failed.len(), 1, "exactly one failure line: {failed:?}");
    assert!(
        failed[0].contains("[plan-11223344] [1.2] tool write failed: denied (5ms)"),
        "failure line should carry agent tag and step tag: {}",
        failed[0]
    );
}

#[test]
fn test_tool_call_batch_tracked_subagent_logs_untagged_calls() {
    let mut app = support::make_app();
    app.session_id = Some("lead-sess".to_string());

    app.handle_event(Event::SubagentStart {
        session_id: "lead-sess".to_string(),
        task_id: "debug-55667788".to_string(),
        child_session_id: "child-sess-4".to_string(),
        agent: "debug".to_string(),
        task: "debug it".to_string(),
        background: false,
    });

    app.event_bus.set_step("child-sess-4", 1);
    // Both calls arrive only via the atomic batch (their per-call Start
    // events were dropped, e.g. by broadcast lag).
    app.handle_event(Event::ToolCallBatch {
        session_id: "child-sess-4".to_string(),
        step: 1,
        calls: vec![
            ToolCallBatchEntry {
                call_id: "call-c1".to_string(),
                tool: "read".to_string(),
                args: "{}".to_string(),
                error: None,
                duration_ms: 10,
                content: "ok".to_string(),
                content_line_count: 1,
                metadata: None,
                success: true,
            },
            ToolCallBatchEntry {
                call_id: "call-c2".to_string(),
                tool: "grep".to_string(),
                args: "{}".to_string(),
                error: None,
                duration_ms: 11,
                content: "ok".to_string(),
                content_line_count: 1,
                metadata: None,
                success: true,
            },
        ],
    });

    let lines = tool_call_lines(&app.log_entries, "[debug-55667788] ");
    assert_eq!(
        lines.len(),
        2,
        "batch fallback logs one tool-call line per untagged call: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("[1.1] tool call: read")),
        "first call gets substep 1: {lines:?}"
    );
    assert!(
        lines.iter().any(|l| l.contains("[1.2] tool call: grep")),
        "second call gets substep 2: {lines:?}"
    );
}

#[test]
fn test_team_member_tool_calls_logged_with_teammate_name() {
    let mut app = support::make_app();
    app.session_id = Some("lead-sess".to_string());
    app.active_team = Some(TeamConfig::new("alpha", "lead-sess"));

    let mut member = TeamMember::new("reviewer", "tm-001", "general");
    member.session_id = Some("tm-session-9".to_string());
    app.team_members.push(member);

    app.event_bus.set_step("tm-session-9", 3);
    app.handle_event(Event::ToolCallStart {
        session_id: "tm-session-9".to_string(),
        call_id: "call-d1".to_string(),
        tool: "read".to_string(),
    });

    let lines = tool_call_lines(&app.log_entries, "tool call: read");
    assert_eq!(lines.len(), 1, "exactly one tool-call log line: {lines:?}");
    assert!(
        lines[0].contains("[reviewer] [3.1] tool call: read"),
        "line should carry the teammate name tag and step tag: {}",
        lines[0]
    );
}

#[test]
fn test_untracked_session_tool_calls_are_not_logged() {
    let mut app = support::make_app();
    app.session_id = Some("lead-sess".to_string());

    // Neither the current session nor a tracked agent: must be ignored.
    app.handle_event(Event::ToolCallStart {
        session_id: "stranger-sess".to_string(),
        call_id: "call-e1".to_string(),
        tool: "read".to_string(),
    });

    assert!(
        !app.log_entries
            .iter()
            .any(|e| e.message.contains("tool call: read")),
        "untracked sessions must not produce tool-call log lines"
    );
}
