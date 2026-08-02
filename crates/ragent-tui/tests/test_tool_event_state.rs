//! Integration tests for TUI tool-call event handling.
//!
//! Verifies that the event handler populates `ToolCallState` correctly across
//! the `ToolCallStart`, `ToolCallArgs`, `ToolCallEnd`, and `ToolCallResult`
//! lifecycle, and that the atomic `ToolCallBatch` event acts as a fallback when
//! per-call events are dropped.

use ragent_agent::event::Event;
use ragent_agent::message::{MessagePart, Role, ToolCallState, ToolCallStatus};
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

/// Safety-net: when `pending_tool_args` ends up holding args for a part that
/// already exists (i.e. the Start handler was somehow processed without
/// consuming the buffer), the next tool-lifecycle event must drain the buffer
/// into the part so the header summary still shows the parameters.
#[test]
fn test_pending_args_drained_by_next_lifecycle_event() {
    let mut app = app_with_session();

    // Args arrive first; the part doesn't exist yet, so they are buffered.
    app.handle_event(Event::ToolCallArgs {
        session_id: "sess-1".to_string(),
        call_id: "c9".to_string(),
        tool: "bash".to_string(),
        args: r#"{"command":"ls -la"}"#.to_string(),
    });

    // Simulate the Start handler having been processed by a path that created
    // the part without consuming the pending args (mirrors the reported bug:
    // the log shows the args but the message header never displays them).
    app.messages.push(ragent_agent::message::Message::new(
        "sess-1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "bash".to_string(),
            call_id: "c9".to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Running,
                input: serde_json::Value::Null,
                output: None,
                error: None,
                duration_ms: None,
            },
        }],
    ));

    // The next tool-lifecycle event should drain the pending buffer.
    app.handle_event(Event::ToolCallArgs {
        session_id: "sess-1".to_string(),
        call_id: "c10".to_string(),
        tool: "read".to_string(),
        args: r#"{"path":"src/main.rs"}"#.to_string(),
    });

    let part = app
        .messages
        .iter()
        .flat_map(|m| &m.parts)
        .find(|p| matches!(p, MessagePart::ToolCall { call_id, .. } if call_id == "c9"))
        .expect("bash part exists");
    let MessagePart::ToolCall { state, .. } = part else {
        panic!("expected ToolCall part");
    };
    assert_eq!(
        state.input["command"], "ls -la",
        "drain should have applied the buffered args to the pre-created part"
    );
    // And the newly arrived args for the unknown call id should be buffered,
    // not lost.
    assert!(
        app.pending_tool_args.contains_key("c10"),
        "unknown call args should remain buffered until their part appears"
    );

    // When c10's part appears, the buffered args must be applied on the next
    // ToolCallStart (the classic out-of-order path that already existed).
    app.handle_event(Event::ToolCallStart {
        session_id: "sess-1".to_string(),
        call_id: "c10".to_string(),
        tool: "read".to_string(),
    });
    let part = app
        .messages
        .iter()
        .flat_map(|m| &m.parts)
        .find(|p| matches!(p, MessagePart::ToolCall { call_id, .. } if call_id == "c10"))
        .expect("read part exists");
    let MessagePart::ToolCall { state, .. } = part else {
        panic!("expected ToolCall part");
    };
    assert_eq!(state.input["path"], "src/main.rs");
}
