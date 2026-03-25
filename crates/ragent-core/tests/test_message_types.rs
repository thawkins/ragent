#![allow(missing_docs, unused_variables, unused_imports, dead_code, unused_mut)]

use ragent_core::message::*;

// ── Multi-part messages ──────────────────────────────────────────

#[test]
fn test_message_multipart_text_and_tool_call() {
    let msg = Message::new(
        "s1",
        Role::Assistant,
        vec![
            MessagePart::Text {
                text: "I'll read the file".to_string(),
            },
            MessagePart::ToolCall {
                tool: "read".to_string(),
                call_id: "call-1".to_string(),
                state: ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: serde_json::json!({"path": "foo.txt"}),
                    output: Some(serde_json::json!({"content": "file data"})),
                    error: None,
                    duration_ms: Some(42),
                },
            },
            MessagePart::Text {
                text: " and here are the results".to_string(),
            },
        ],
    );

    assert_eq!(msg.role, Role::Assistant);
    assert_eq!(msg.parts.len(), 3);
    assert_eq!(
        msg.text_content(),
        "I'll read the file and here are the results"
    );
}

#[test]
fn test_message_with_reasoning_part() {
    let msg = Message::new(
        "s1",
        Role::Assistant,
        vec![
            MessagePart::Reasoning {
                text: "Let me think about this...".to_string(),
            },
            MessagePart::Text {
                text: "The answer is 42.".to_string(),
            },
        ],
    );

    assert_eq!(msg.parts.len(), 2);
    // text_content only concatenates Text parts
    assert_eq!(msg.text_content(), "The answer is 42.");
}

#[test]
fn test_message_text_content_empty_parts() {
    let msg = Message::new("s1", Role::Assistant, vec![]);
    assert_eq!(msg.text_content(), "");
}

#[test]
fn test_message_text_content_only_tool_calls() {
    let msg = Message::new(
        "s1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "bash".to_string(),
            call_id: "c1".to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Pending,
                input: serde_json::json!({"command": "ls"}),
                output: None,
                error: None,
                duration_ms: None,
            },
        }],
    );

    assert_eq!(msg.text_content(), "");
}

// ── Display formatting ───────────────────────────────────────────

#[test]
fn test_message_display_simple_text() {
    let msg = Message::user_text("s1", "Hello, world!");
    let display = format!("{}", msg);
    assert!(display.contains("[user]"));
    assert!(display.contains("Hello, world!"));
}

#[test]
fn test_message_display_with_tool_calls() {
    let msg = Message::new(
        "s1",
        Role::Assistant,
        vec![
            MessagePart::Text {
                text: "Working on it".to_string(),
            },
            MessagePart::ToolCall {
                tool: "read".to_string(),
                call_id: "c1".to_string(),
                state: ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: serde_json::json!({}),
                    output: None,
                    error: None,
                    duration_ms: None,
                },
            },
            MessagePart::ToolCall {
                tool: "write".to_string(),
                call_id: "c2".to_string(),
                state: ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: serde_json::json!({}),
                    output: None,
                    error: None,
                    duration_ms: None,
                },
            },
        ],
    );

    let display = format!("{}", msg);
    assert!(display.contains("[assistant]"));
    assert!(display.contains("2 tool calls"));
}

#[test]
fn test_message_display_single_tool_call() {
    let msg = Message::new(
        "s1",
        Role::Assistant,
        vec![
            MessagePart::Text {
                text: "Done".to_string(),
            },
            MessagePart::ToolCall {
                tool: "bash".to_string(),
                call_id: "c1".to_string(),
                state: ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: serde_json::json!({}),
                    output: None,
                    error: None,
                    duration_ms: None,
                },
            },
        ],
    );

    let display = format!("{}", msg);
    assert!(display.contains("1 tool call"));
    assert!(!display.contains("1 tool calls"));
}

#[test]
fn test_message_display_truncation() {
    let long_text = "x".repeat(200);
    let msg = Message::user_text("s1", &long_text);
    let display = format!("{}", msg);

    // Display should truncate at 80 chars + ellipsis
    assert!(display.contains("…"));
    assert!(display.len() < 200);
}

// ── Serialization with all part types ────────────────────────────

#[test]
fn test_message_serde_all_part_types() {
    let msg = Message::new(
        "session-1",
        Role::Assistant,
        vec![
            MessagePart::Text {
                text: "hello".to_string(),
            },
            MessagePart::Reasoning {
                text: "thinking".to_string(),
            },
            MessagePart::ToolCall {
                tool: "read".to_string(),
                call_id: "c1".to_string(),
                state: ToolCallState {
                    status: ToolCallStatus::Completed,
                    input: serde_json::json!({"path": "test.rs"}),
                    output: Some(serde_json::json!("file content")),
                    error: None,
                    duration_ms: Some(15),
                },
            },
        ],
    );

    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.id, msg.id);
    assert_eq!(deserialized.session_id, "session-1");
    assert_eq!(deserialized.role, Role::Assistant);
    assert_eq!(deserialized.parts.len(), 3);

    match &deserialized.parts[0] {
        MessagePart::Text { text } => assert_eq!(text, "hello"),
        _ => panic!("Expected Text"),
    }
    match &deserialized.parts[1] {
        MessagePart::Reasoning { text } => assert_eq!(text, "thinking"),
        _ => panic!("Expected Reasoning"),
    }
    match &deserialized.parts[2] {
        MessagePart::ToolCall {
            tool,
            call_id,
            state,
        } => {
            assert_eq!(tool, "read");
            assert_eq!(call_id, "c1");
            assert_eq!(state.status, ToolCallStatus::Completed);
            assert_eq!(state.duration_ms, Some(15));
        }
        _ => panic!("Expected ToolCall"),
    }
}

// ── Role ─────────────────────────────────────────────────────────

#[test]
fn test_role_display() {
    assert_eq!(Role::User.to_string(), "user");
    assert_eq!(Role::Assistant.to_string(), "assistant");
}

#[test]
fn test_role_serde() {
    let json = serde_json::to_string(&Role::User).unwrap();
    assert_eq!(json, r#""user""#);

    let json = serde_json::to_string(&Role::Assistant).unwrap();
    assert_eq!(json, r#""assistant""#);

    let parsed: Role = serde_json::from_str(r#""user""#).unwrap();
    assert_eq!(parsed, Role::User);
}

// ── ToolCallStatus ───────────────────────────────────────────────

#[test]
fn test_tool_call_status_display() {
    assert_eq!(ToolCallStatus::Pending.to_string(), "pending");
    assert_eq!(ToolCallStatus::Running.to_string(), "running");
    assert_eq!(ToolCallStatus::Completed.to_string(), "completed");
    assert_eq!(ToolCallStatus::Error.to_string(), "error");
}

#[test]
fn test_tool_call_status_serde() {
    for status in &[
        ToolCallStatus::Pending,
        ToolCallStatus::Running,
        ToolCallStatus::Completed,
        ToolCallStatus::Error,
    ] {
        let json = serde_json::to_string(status).unwrap();
        let deserialized: ToolCallStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(&deserialized, status);
    }
}

// ── ToolCallState with error ─────────────────────────────────────

#[test]
fn test_tool_call_state_with_error() {
    let state = ToolCallState {
        status: ToolCallStatus::Error,
        input: serde_json::json!({"command": "rm -rf /"}),
        output: None,
        error: Some("Permission denied".to_string()),
        duration_ms: Some(1),
    };

    let json = serde_json::to_string(&state).unwrap();
    let deserialized: ToolCallState = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.status, ToolCallStatus::Error);
    assert_eq!(deserialized.error.as_deref(), Some("Permission denied"));
}

// ── Message ID uniqueness ────────────────────────────────────────

#[test]
fn test_message_ids_unique() {
    let msg1 = Message::user_text("s1", "hello");
    let msg2 = Message::user_text("s1", "hello");
    assert_ne!(msg1.id, msg2.id, "Message IDs should be unique");
}
