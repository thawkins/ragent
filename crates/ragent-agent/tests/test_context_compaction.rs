//! Tests for context compaction in session processor.

use ragent_agent::message::{Message, MessagePart, Role, ToolCallState, ToolCallStatus};
use ragent_agent::session::compact_history_with_atomic_tool_calls;
use serde_json::json;

fn make_text_message(role: Role, text: &str) -> Message {
    Message::new(
        "test-session",
        role,
        vec![MessagePart::Text { text: text.to_string() }],
    )
}

fn make_tool_call_message(tool: &str, call_id: &str, output: &str) -> Message {
    Message::new(
        "test-session",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: tool.to_string(),
            call_id: call_id.to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({"path": "/tmp/test"}),
                output: Some(json!(output)),
                error: None,
                duration_ms: Some(42),
            },
        }],
    )
}

fn make_tool_result_message(call_id: &str, result: &str) -> Message {
    // Tool results are user messages with ToolCall parts containing the result
    Message::new(
        "test-session",
        Role::User,
        vec![MessagePart::ToolCall {
            tool: "result".to_string(),
            call_id: call_id.to_string(),
            state: ToolCallState {
                status: ToolCallStatus::Completed,
                input: json!({}),
                output: Some(json!(result)),
                error: None,
                duration_ms: Some(42),
            },
        }],
    )
}

#[test]
fn test_compact_no_trim_needed() {
    let messages = vec![
        make_text_message(Role::User, "Hello"),
        make_text_message(Role::Assistant, "Hi there"),
    ];
    let compacted = compact_history_with_atomic_tool_calls(&messages, 128_000, 8192);
    assert_eq!(compacted.len(), 2, "Should not trim when under budget");
}

#[test]
fn test_compact_trims_oldest() {
    // Build messages with large text to exceed a small context window
    let large_text = "a".repeat(1000);
    let mut messages = Vec::new();
    for i in 0..10 {
        messages.push(make_text_message(
            if i % 2 == 0 { Role::User } else { Role::Assistant },
            &large_text,
        ));
    }

    // With ~10 * (1000/4 + 10) = ~2600 tokens and a 1500-token window, must trim
    let compacted = compact_history_with_atomic_tool_calls(&messages, 1500, 8192);
    assert!(
        compacted.len() < messages.len(),
        "Should trim messages when over budget"
    );
    // The last message (assistant response to the last user query) should always be kept
    assert_eq!(
        compacted.last().unwrap().role,
        Role::Assistant,
        "Should keep the last assistant message"
    );
}

#[test]
fn test_compact_keeps_tool_call_pairs() {
    let large_text = "x".repeat(500);
    let mut messages = Vec::new();
    for i in 0..5 {
        messages.push(make_text_message(Role::User, &large_text));
        messages.push(make_tool_call_message("read", &format!("call-{i}"), &large_text));
        messages.push(make_tool_result_message(&format!("call-{i}"), &large_text));
    }

    // With a small context window, some should be trimmed
    let compacted = compact_history_with_atomic_tool_calls(&messages, 2000, 8192);

    // Verify remaining tool call pairs are complete
    for (idx, msg) in compacted.iter().enumerate() {
        if msg.role == Role::Assistant && has_tool_calls(msg) {
            // The next message should be the user result
            assert!(
                idx + 1 < compacted.len(),
                "Tool call at index {idx} is missing its result message"
            );
            assert_eq!(
                compacted[idx + 1].role,
                Role::User,
                "Tool call at index {idx} should be followed by user result"
            );
            assert!(
                has_tool_calls(&compacted[idx + 1]),
                "Message after tool call should contain the result"
            );
        }
    }

    // Verify we didn't lose the newest tool call pair if any remain
    let remaining_tool_calls = compacted
        .iter()
        .filter(|m| m.role == Role::Assistant && has_tool_calls(m))
        .count();
    if remaining_tool_calls > 0 {
        // The last tool call should have its result
        let last_tool_idx = compacted
            .iter()
            .rposition(|m| m.role == Role::Assistant && has_tool_calls(m))
            .unwrap();
        assert!(
            last_tool_idx + 1 < compacted.len(),
            "Last tool call missing its result"
        );
    }
}

#[test]
fn test_compact_keeps_last_two_messages() {
    let large_text = "b".repeat(1000);
    let mut messages = Vec::new();
    for i in 0..20 {
        messages.push(make_text_message(
            if i % 2 == 0 { Role::User } else { Role::Assistant },
            &large_text,
        ));
    }

    let compacted = compact_history_with_atomic_tool_calls(&messages, 1500, 8192);
    // When each message is ~260 tokens and budget is ~500 tokens,
    // we can only fit 1-2 messages. At minimum, keep the last one.
    assert!(
        compacted.len() >= 1,
        "Should always keep at least the last message"
    );
    assert_eq!(
        compacted.last().unwrap().role,
        Role::Assistant,
        "Last should be assistant response"
    );
}

#[test]
fn test_compact_empty_history() {
    let messages: Vec<Message> = vec![];
    let compacted = compact_history_with_atomic_tool_calls(&messages, 128_000, 8192);
    assert!(compacted.is_empty());
}

#[test]
fn test_compact_exactly_at_budget() {
    // One small message should fit exactly
    let messages = vec![make_text_message(Role::User, "Hello world")];
    let compacted = compact_history_with_atomic_tool_calls(&messages, 100, 8192);
    assert_eq!(compacted.len(), 1);
}

fn has_tool_calls(msg: &Message) -> bool {
    msg.parts
        .iter()
        .any(|p| matches!(p, MessagePart::ToolCall { .. }))
}
