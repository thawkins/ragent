//! Tests for the Context side panel (spec `contextpanel`).
//!
//! - **T-006**: Compute conversation history token size and message count.

mod support;

use ragent_agent::message::{Message, MessagePart, Role};

#[test]
fn test_conversation_history_token_count_zero_with_empty_session() {
    // FR-008: an empty session contributes nothing to the history partition.
    let app = support::make_app();
    assert_eq!(app.conversation_history_token_count(), 0);
    assert_eq!(app.conversation_message_count(), 0);
}

#[test]
fn test_conversation_history_counts_messages_and_grows_with_content() {
    // FR-008: the partition must reflect both the message count and the
    // byte size of the text content held in the active session.
    let mut app = support::make_app();
    let base = app.conversation_history_token_count();
    assert_eq!(base, 0);

    app.messages.push(Message::new(
        "session-1",
        Role::User,
        vec![MessagePart::Text {
            text: "hello world".into(),
        }],
    ));
    let one = app.conversation_history_token_count();
    assert_eq!(app.conversation_message_count(), 1);
    assert!(one > 0, "single small message should still be positive");

    app.messages.push(Message::new(
        "session-1",
        Role::Assistant,
        vec![MessagePart::Text {
            text: "x".repeat(2_000),
        }],
    ));
    let two = app.conversation_history_token_count();
    assert_eq!(app.conversation_message_count(), 2);
    assert!(
        two > one * 10,
        "a large second message should dominate the estimate: one={one}, two={two}"
    );
}

#[test]
fn test_conversation_history_counts_tool_call_payloads() {
    // FR-008: tool invocations and their JSON arguments/results are sent to
    // the model, so they must contribute to the history estimate.
    let mut app = support::make_app();
    let json_input: serde_json::Value = serde_json::json!({ "command": "ls -la /tmp" });
    let json_output: serde_json::Value =
        serde_json::json!({ "stdout": "a\nb\nc\nd\ne\nf\ng\nh\ni\nj" });
    app.messages.push(Message::new(
        "session-1",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "bash".into(),
            call_id: "call-123".into(),
            state: ragent_agent::message::ToolCallState {
                status: ragent_agent::message::ToolCallStatus::Completed,
                input: json_input,
                output: Some(json_output),
                error: None,
                duration_ms: None,
            },
        }],
    ));
    let count = app.conversation_history_token_count();
    // The raw byte accounting is (role label + call_id + wire JSON of the
    // tool input/output + 40), ~200 bytes here; after the ~4-bytes-per-token
    // conversion the estimate still exceeds 25 tokens.
    assert!(
        count > 25,
        "tool-call input/output should contribute to history size; got {count}"
    );
}
