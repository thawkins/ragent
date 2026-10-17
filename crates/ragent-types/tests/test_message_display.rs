//! Coverage for [`Message`] and its supporting enums' concrete behaviour.
//!
//! Exercises the hand-written `Display` impls (`Role`, `ToolCallStatus`), the
//! `Message::Display` preview/truncation and tool-call-count branches, the
//! convenience constructors, `text_content` filtering, and `touch`.

use ragent_types::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use serde_json::json;

#[test]
fn role_display_is_lowercase() {
    assert_eq!(Role::User.to_string(), "user");
    assert_eq!(Role::Assistant.to_string(), "assistant");
    assert_eq!(Role::Compaction.to_string(), "compaction");
}

#[test]
fn tool_call_status_display_is_lowercase() {
    assert_eq!(ToolCallStatus::Pending.to_string(), "pending");
    assert_eq!(ToolCallStatus::Running.to_string(), "running");
    assert_eq!(ToolCallStatus::Completed.to_string(), "completed");
    assert_eq!(ToolCallStatus::Error.to_string(), "error");
}

#[test]
fn message_display_plain_text() {
    let msg = Message::user_text("s1", "Fix the bug");
    assert_eq!(msg.to_string(), "[user] Fix the bug");
}

#[test]
fn message_display_truncates_long_text_with_ellipsis() {
    let long = "a".repeat(200);
    let msg = Message::user_text("s1", long);
    let rendered = msg.to_string();
    assert_eq!(rendered, format!("[user] {}...", "a".repeat(80)));
    // 80 a's + 3 dots inside the bracket prefix.
    assert!(rendered.ends_with("..."));
}

#[test]
fn message_display_exactly_80_chars_is_not_truncated() {
    let text = "b".repeat(80);
    let msg = Message::assistant_text("s1", text.clone());
    assert_eq!(msg.to_string(), format!("[assistant] {text}"));
}

#[test]
fn message_display_truncation_respects_char_boundary() {
    // 79 ASCII chars then a 4-byte emoji crosses the 80-byte boundary; the
    // truncation must back up to a valid char boundary and not panic.
    let text = format!("{}😀", "c".repeat(79));
    let msg = Message::user_text("s1", text);
    let rendered = msg.to_string();
    assert!(rendered.starts_with("[user] "));
    assert!(rendered.ends_with("..."));
    // Must have dropped the emoji rather than split it.
    assert!(!rendered.contains('😀'));
}

#[test]
fn message_display_singular_tool_call() {
    let msg = Message::new(
        "s1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "read".into(),
            call_id: "c1".into(),
            state: Box::new(ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"path": "x"}),
                output: None,
                error: None,
                duration_ms: None,
            }),
        }],
    );
    assert_eq!(msg.to_string(), "[assistant]  (1 tool call)");
}

#[test]
fn message_display_plural_tool_calls() {
    let call = || MessagePart::ToolCall {
        tool: "read".into(),
        call_id: "c1".into(),
        state: Box::new(ToolCallState {
            status: ToolCallStatus::Pending,
            input: json!({}),
            output: None,
            error: None,
            duration_ms: None,
        }),
    };
    let msg = Message::new(
        "s1",
        Role::Assistant,
        vec![call(), call(), MessagePart::Text { text: "hi".into() }],
    );
    assert_eq!(msg.to_string(), "[assistant] hi (2 tool calls)");
}

#[test]
fn text_content_concatenates_only_text_parts() {
    let msg = Message::new(
        "s1",
        Role::User,
        vec![
            MessagePart::Text {
                text: "Hello ".into(),
            },
            MessagePart::Reasoning {
                text: "thinking".into(),
            },
            MessagePart::Text {
                text: "world".into(),
            },
        ],
    );
    assert_eq!(msg.text_content(), "Hello world");
}

#[test]
fn touch_bumps_edit_seq_and_updates_timestamp() {
    let mut msg = Message::user_text("s1", "hi");
    assert_eq!(msg.edit_seq, 0);
    let before = msg.updated_at;
    msg.touch();
    msg.touch();
    assert_eq!(msg.edit_seq, 2);
    assert!(msg.updated_at >= before);
}
