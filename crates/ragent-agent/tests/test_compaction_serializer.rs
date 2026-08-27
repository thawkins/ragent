//! External tests for `tests` from `crates/ragent-agent/src/compaction/serializer.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_agent::compaction::serializer::*;
use ragent_types::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};

fn user_message(text: &str) -> Message {
    Message {
        id: "u1".to_string(),
        session_id: "s1".to_string(),
        role: Role::User,
        parts: vec![MessagePart::Text {
            text: text.to_string(),
        }],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        edit_seq: 0,
    }
}

fn assistant_text_message(text: &str) -> Message {
    Message {
        id: "a1".to_string(),
        session_id: "s1".to_string(),
        role: Role::Assistant,
        parts: vec![MessagePart::Text {
            text: text.to_string(),
        }],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        edit_seq: 0,
    }
}

#[test]
fn test_serialize_user() {
    let message = user_message("Hello, world!");
    let result = serialize_message(&message, TOOL_OUTPUT_MAX_CHARS);
    assert_eq!(result, "[User]: Hello, world!");
}

#[test]
fn test_serialize_assistant_text() {
    let message = assistant_text_message("Done.");
    let result = serialize_message(&message, TOOL_OUTPUT_MAX_CHARS);
    assert_eq!(result, "[Assistant]: Done.");
}

#[test]
fn test_serialize_system() {
    let message = Message {
        id: "sys1".to_string(),
        session_id: "s1".to_string(),
        role: Role::Assistant,
        parts: vec![MessagePart::Text {
            text: "Be concise.".to_string(),
        }],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        edit_seq: 0,
    };
    let result = serialize_system(&message, TOOL_OUTPUT_MAX_CHARS);
    assert_eq!(result, "[System update]: Be concise.");
}

#[test]
fn test_serialize_assistant_tool_call_completed() {
    let message = Message {
        id: "a2".to_string(),
        session_id: "s1".to_string(),
        role: Role::Assistant,
        parts: vec![MessagePart::ToolCall {
            tool: "bash".to_string(),
            call_id: "call_1".to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Completed,
                input: serde_json::json!("ls -la"),
                output: Some(serde_json::json!("file1.txt\nfile2.txt")),
                error: None,
                duration_ms: Some(123),
            },
        }],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        edit_seq: 0,
    };
    let result = serialize_message(&message, TOOL_OUTPUT_MAX_CHARS);
    assert!(result.contains("[Assistant tool call]: bash(ls -la)"));
    assert!(result.contains("[Tool result]: file1.txt\nfile2.txt"));
}

#[test]
fn test_serialize_assistant_tool_call_error() {
    let message = Message {
        id: "a3".to_string(),
        session_id: "s1".to_string(),
        role: Role::Assistant,
        parts: vec![MessagePart::ToolCall {
            tool: "bash".to_string(),
            call_id: "call_2".to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Error,
                input: serde_json::json!("bad"),
                output: None,
                error: Some("command not found".to_string()),
                duration_ms: None,
            },
        }],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        edit_seq: 0,
    };
    let result = serialize_message(&message, TOOL_OUTPUT_MAX_CHARS);
    assert!(result.contains("[Assistant tool call]: bash(bad)"));
    assert!(result.contains("[Tool error]: command not found"));
}

#[test]
fn test_serialize_messages_joins_with_blank_line() {
    let messages = vec![user_message("Hi"), assistant_text_message("Hello")];
    let result = serialize_messages(&messages, TOOL_OUTPUT_MAX_CHARS).unwrap();
    assert_eq!(result, "[User]: Hi\n\n[Assistant]: Hello");
}

#[test]
fn test_serialize_messages_empty_returns_none() {
    let messages: Vec<Message> = vec![];
    assert!(serialize_messages(&messages, TOOL_OUTPUT_MAX_CHARS).is_none());
}

#[test]
fn test_truncate() {
    assert_eq!(truncate("short", 100), "short");
    let long = "a".repeat(10_000);
    let truncated = truncate(&long, 10);
    assert!(truncated.starts_with("aaaaaaaaaa"));
    assert!(truncated.ends_with("\n[truncated]"));
    assert_eq!(truncated.len(), 10 + "\n[truncated]".len());
}

#[test]
fn test_truncate_is_unicode_safe() {
    // Regression for the coredump at serializer.rs:163: a byte-index cut
    // inside a multi-byte UTF-8 character used to panic. The string below
    // places a 3-byte em dash (U+2014) exactly where a 10-character cut
    // would slice a byte in the middle.
    let text = "123456789−after";
    let truncated = truncate(text, 10);
    assert_eq!(truncated, "123456789−\n[truncated]");

    // A string that is entirely multi-byte should also truncate safely.
    let unicode = "−−−−−−−−−−−−−−−−−−−−";
    assert_eq!(truncate(unicode, 5), "−−−−−\n[truncated]");
}
