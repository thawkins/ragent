//! External tests for the compaction `convert` module (T-012).
//!
//! These exercise the bidirectional `ChatMessage` ↔ `Message` conversion used
//! by the compaction runner and the agent loop's pre-send / emergency-overflow
//! paths. The module is compiled into the crate's module tree via the
//! `#[path]` attribute in `convert.rs` so it can access the crate-private
//! `chat_messages_to_messages` / `messages_to_chat_messages` helpers. It lives
//! under `tests/inline/` so cargo does not also compile it as a standalone
//! integration test.

use ragent_types::llm::{ChatContent, ChatMessage, ContentPart};
use ragent_types::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};

use crate::compaction::convert::{chat_messages_to_messages, messages_to_chat_messages};

fn user_text(text: &str) -> ChatMessage {
    ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Text(text.to_string()),
    }
}

fn assistant_text(text: &str) -> ChatMessage {
    ChatMessage {
        role: "assistant".to_string(),
        content: ChatContent::Text(text.to_string()),
    }
}

fn assistant_tool_use(id: &str, name: &str, input: serde_json::Value) -> ChatMessage {
    ChatMessage {
        role: "assistant".to_string(),
        content: ChatContent::Parts(vec![ContentPart::ToolUse {
            id: id.to_string(),
            name: name.to_string(),
            input,
        }]),
    }
}

fn tool_result(id: &str, content: &str) -> ChatMessage {
    ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Parts(vec![ContentPart::ToolResult {
            tool_use_id: id.to_string(),
            content: content.into(),
        }]),
    }
}

/// Helper: extract the first text content of a [`Message`], or empty string.
fn first_text(msg: &Message) -> String {
    msg.parts
        .iter()
        .find_map(|p| match p {
            MessagePart::Text { text } => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_default()
}

#[test]
fn test_text_messages_round_trip_preserves_text() {
    let chat = vec![user_text("hello"), assistant_text("hi there")];
    let messages = chat_messages_to_messages(&chat);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, Role::User);
    assert_eq!(first_text(&messages[0]), "hello");
    assert_eq!(messages[1].role, Role::Assistant);
    assert_eq!(first_text(&messages[1]), "hi there");

    // Back to chat messages — text content survives the round trip.
    let back = messages_to_chat_messages(&messages);
    assert_eq!(back.len(), 2);
    assert_eq!(back[0].role, "user");
    assert_eq!(back[1].role, "assistant");
    let text0 = match &back[0].content {
        ChatContent::Text(t) => t.clone(),
        _ => String::new(),
    };
    let text1 = match &back[1].content {
        ChatContent::Text(t) => t.clone(),
        _ => String::new(),
    };
    assert_eq!(text0, "hello");
    assert_eq!(text1, "hi there");
}

#[test]
fn test_tool_use_and_result_are_paired_into_tool_call_part() {
    // assistant tool use, then user tool result sharing the same id.
    let chat = vec![
        assistant_tool_use("call_1", "bash", serde_json::json!("ls -la")),
        tool_result("call_1", "file1.txt\nfile2.txt"),
    ];
    let messages = chat_messages_to_messages(&chat);
    // The tool result is paired back into the preceding assistant message's
    // ToolCall part. The user-role chat message carrying the ToolResult still
    // produces a (now empty-parts) user Message, so we get two messages.
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].role, Role::Assistant);
    let tool_part = messages[0].parts.iter().find_map(|p| match p {
        MessagePart::ToolCall { state, .. } => Some(state.clone()),
        _ => None,
    });
    let state = tool_part.expect("expected a ToolCall part");
    assert_eq!(state.status, ToolCallStatus::Completed);
    assert_eq!(state.input, serde_json::json!("ls -la"));
    let output = state
        .output
        .as_ref()
        .and_then(|v| v.as_str().map(std::string::ToString::to_string))
        .unwrap_or_default();
    assert_eq!(output, "file1.txt\nfile2.txt");
}

#[test]
fn test_unpaired_tool_result_becomes_inline_text() {
    // A tool result with no preceding matching ToolUse id is rendered as an
    // inline text part so it is not silently dropped (FR-014).
    let chat = vec![tool_result("orphan_1", "leftover output")];
    let messages = chat_messages_to_messages(&chat);
    assert_eq!(messages.len(), 1);
    // The orphan result is carried as a user text message.
    assert_eq!(messages[0].role, Role::User);
    assert!(first_text(&messages[0]).contains("orphan_1"));
    assert!(first_text(&messages[0]).contains("leftover output"));
}

#[test]
fn test_messages_to_chat_messages_emits_tool_result_followup() {
    // Build an assistant Message carrying a completed ToolCall; converting it
    // back to chat messages must emit the assistant ToolUse part and a
    // following user ToolResult message so the provider sees the pair.
    let msg = Message {
        id: "m1".to_string(),
        session_id: "s1".to_string(),
        role: Role::Assistant,
        parts: vec![MessagePart::ToolCall {
            tool: "bash".to_string(),
            call_id: "call_9".to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Completed,
                input: serde_json::json!("pwd"),
                output: Some(serde_json::json!("/home/user")),
                error: None,
                duration_ms: None,
            },
        }],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        edit_seq: 0,
    };
    let chat = messages_to_chat_messages(&[msg]);
    // Assistant ToolUse message + user ToolResult follow-up.
    assert_eq!(chat.len(), 2);
    assert_eq!(chat[0].role, "assistant");
    assert_eq!(chat[1].role, "user");
    let has_tool_use = matches!(&chat[0].content, ChatContent::Parts(parts) if parts
        .iter()
        .any(|p| matches!(p, ContentPart::ToolUse { id, .. } if id == "call_9")));
    assert!(has_tool_use, "assistant message must carry the ToolUse");
    let has_tool_result = matches!(&chat[1].content, ChatContent::Parts(parts) if parts
        .iter()
        .any(|p| matches!(p, ContentPart::ToolResult { tool_use_id, content } if tool_use_id == "call_9" && &**content == "/home/user")));
    assert!(
        has_tool_result,
        "follow-up user message must carry the ToolResult"
    );
}

#[test]
fn test_compaction_role_maps_to_assistant_chat_role() {
    // A compaction Message must be emitted as an `assistant` chat message so
    // providers that do not understand a custom role still receive the summary.
    let msg = Message {
        id: "c1".to_string(),
        session_id: "s1".to_string(),
        role: Role::Compaction,
        parts: vec![MessagePart::Text {
            text: "## Objective\n- ship it".to_string(),
        }],
        created_at: chrono::Utc::now(),
        updated_at: chrono::Utc::now(),
        edit_seq: 0,
    };
    let chat = messages_to_chat_messages(&[msg]);
    assert_eq!(chat.len(), 1);
    assert_eq!(chat[0].role, "assistant");
    let text = match &chat[0].content {
        ChatContent::Text(t) => t.clone(),
        _ => String::new(),
    };
    assert_eq!(text, "## Objective\n- ship it");
}

#[test]
fn test_empty_input_yields_empty_output() {
    assert!(chat_messages_to_messages(&[]).is_empty());
    assert!(messages_to_chat_messages(&[]).is_empty());
}
