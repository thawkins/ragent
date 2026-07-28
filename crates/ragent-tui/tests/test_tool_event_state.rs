//! Integration tests for TUI tool-call event handling.
//!
//! Verifies that the event handler populates `ToolCallState` correctly across
//! the `ToolCallStart`, `ToolCallArgs`, `ToolCallEnd`, and `ToolCallResult`
//! lifecycle, and that the atomic `ToolCallBatch` event acts as a fallback when
//! per-call events are dropped.

use ragent_agent::event::Event;
use ragent_agent::message::{MessagePart, ToolCallStatus};
use ragent_types::event::ToolCallBatchEntry;

mod support;

fn app_with_session() -> ragent_tui::App {
    let mut app = support::make_app();
    app.session_id = Some("sess-1".to_string());
    app.needs_redraw = false;
    app
}

#[test]
fn test_tool_event_sequence_updates_state() {
    let mut app = app_with_session();

    app.handle_event(Event::ToolCallStart {
        session_id: "sess-1".to_string(),
        call_id: "c1".to_string(),
        tool: "read".to_string(),
    });

    assert!(app.needs_redraw, "ToolCallStart must request a redraw");
    let part = app.messages[0].parts[0].clone();
    let MessagePart::ToolCall { state, .. } = part else {
        panic!("expected ToolCall part");
    };
    assert_eq!(state.status, ToolCallStatus::Running);
    assert!(state.input.is_null());

    app.needs_redraw = false;
    app.handle_event(Event::ToolCallArgs {
        session_id: "sess-1".to_string(),
        call_id: "c1".to_string(),
        tool: "read".to_string(),
        args: r#"{"path":"src/main.rs"}"#.to_string(),
    });

    assert!(app.needs_redraw, "ToolCallArgs must request a redraw");
    let part = app.messages[0].parts[0].clone();
    let MessagePart::ToolCall { state, .. } = part else {
        panic!("expected ToolCall part");
    };
    assert_eq!(state.input["path"], "src/main.rs");

    app.needs_redraw = false;
    app.handle_event(Event::ToolCallEnd {
        session_id: "sess-1".to_string(),
        call_id: "c1".to_string(),
        tool: "read".to_string(),
        error: None,
        duration_ms: 1234,
    });

    assert!(app.needs_redraw, "ToolCallEnd must request a redraw");
    let part = app.messages[0].parts[0].clone();
    let MessagePart::ToolCall { state, .. } = part else {
        panic!("expected ToolCall part");
    };
    assert_eq!(state.status, ToolCallStatus::Completed);
    assert_eq!(state.duration_ms, Some(1234));
}

#[test]
fn test_tool_call_batch_updates_status_and_duration() {
    let mut app = app_with_session();

    app.handle_event(Event::ToolCallStart {
        session_id: "sess-1".to_string(),
        call_id: "c2".to_string(),
        tool: "bash".to_string(),
    });
    app.needs_redraw = false;

    app.handle_event(Event::ToolCallBatch {
        session_id: "sess-1".to_string(),
        step: 1,
        calls: vec![ToolCallBatchEntry {
            call_id: "c2".to_string(),
            tool: "bash".to_string(),
            error: None,
            duration_ms: 999,
            content: "ok".to_string(),
            content_line_count: 0,
            metadata: None,
            success: true,
        }],
    });

    assert!(app.needs_redraw, "ToolCallBatch must request a redraw");
    let part = app.messages[0].parts[0].clone();
    let MessagePart::ToolCall { state, .. } = part else {
        panic!("expected ToolCall part");
    };
    assert_eq!(state.status, ToolCallStatus::Completed);
    assert_eq!(state.duration_ms, Some(999));
}

#[test]
fn test_tool_call_batch_creates_missing_part() {
    let mut app = app_with_session();

    app.handle_event(Event::ToolCallBatch {
        session_id: "sess-1".to_string(),
        step: 1,
        calls: vec![ToolCallBatchEntry {
            call_id: "c3".to_string(),
            tool: "write".to_string(),
            error: None,
            duration_ms: 42,
            content: "written".to_string(),
            content_line_count: 0,
            metadata: None,
            success: true,
        }],
    });

    let part = app.messages[0].parts[0].clone();
    let MessagePart::ToolCall { tool, state, .. } = part else {
        panic!("expected ToolCall part");
    };
    assert_eq!(tool, "write");
    assert_eq!(state.status, ToolCallStatus::Completed);
    assert_eq!(state.duration_ms, Some(42));
}

#[test]
fn test_tool_call_batch_marks_error() {
    let mut app = app_with_session();

    app.handle_event(Event::ToolCallStart {
        session_id: "sess-1".to_string(),
        call_id: "c4".to_string(),
        tool: "read".to_string(),
    });

    app.handle_event(Event::ToolCallBatch {
        session_id: "sess-1".to_string(),
        step: 1,
        calls: vec![ToolCallBatchEntry {
            call_id: "c4".to_string(),
            tool: "read".to_string(),
            error: Some("not found".to_string()),
            duration_ms: 7,
            content: "not found".to_string(),
            content_line_count: 0,
            metadata: None,
            success: false,
        }],
    });

    let part = app.messages[0].parts[0].clone();
    let MessagePart::ToolCall { state, .. } = part else {
        panic!("expected ToolCall part");
    };
    assert_eq!(state.status, ToolCallStatus::Error);
    assert_eq!(state.error.as_deref(), Some("not found"));
}
