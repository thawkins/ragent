//! Tests-only coverage for the ACP update decoder and structured error
//! (`ragent_agent::acp::{parse_update, AcpUpdate, AcpError}`).
//!
//! Pure JSON decoding - no subprocess is spawned.

use ragent_agent::acp::{AcpError, AcpErrorKind, AcpUpdate, parse_update};
use serde_json::json;

#[test]
fn parse_update_decodes_thought_chunk_from_a_content_array() {
    // An array-form `content` concatenates every text block.
    let frame = json!({
        "params": { "update": {
            "sessionUpdate": "agent_thought_chunk",
            "content": [
                {"type": "text", "text": "think "},
                {"type": "text", "text": "harder"}
            ]
        }}
    });
    assert_eq!(
        parse_update(&frame),
        AcpUpdate::AgentThoughtChunk {
            text: "think harder".to_string()
        }
    );
}

#[test]
fn parse_update_decodes_tool_call_update_with_optional_fields() {
    // Present title/status.
    let with_fields = json!({
        "update": {
            "sessionUpdate": "tool_call_update",
            "toolCallId": "call-9",
            "title": "Now reading",
            "status": "completed"
        }
    });
    assert_eq!(
        parse_update(&with_fields),
        AcpUpdate::ToolCallUpdate {
            tool_call_id: "call-9".to_string(),
            title: Some("Now reading".to_string()),
            status: Some("completed".to_string()),
        }
    );

    // Absent title/status become None.
    let bare = json!({
        "update": { "sessionUpdate": "tool_call_update", "toolCallId": "call-10" }
    });
    assert_eq!(
        parse_update(&bare),
        AcpUpdate::ToolCallUpdate {
            tool_call_id: "call-10".to_string(),
            title: None,
            status: None,
        }
    );
}

#[test]
fn parse_update_decodes_plan_entry_count() {
    let frame = json!({
        "update": {
            "sessionUpdate": "plan",
            "entries": [{"step": 1}, {"step": 2}, {"step": 3}]
        }
    });
    assert_eq!(parse_update(&frame), AcpUpdate::Plan { entries: 3 });
}

#[test]
fn parse_update_on_missing_discriminator_is_other_empty() {
    let frame = json!({ "params": { "update": {} } });
    assert_eq!(
        parse_update(&frame),
        AcpUpdate::Other {
            kind: String::new()
        }
    );
}

#[test]
fn acp_update_kind_matches_the_discriminator() {
    assert_eq!(
        AcpUpdate::AgentMessageChunk {
            text: String::new()
        }
        .kind(),
        "agent_message_chunk"
    );
    assert_eq!(
        AcpUpdate::AgentThoughtChunk {
            text: String::new()
        }
        .kind(),
        "agent_thought_chunk"
    );
    assert_eq!(
        AcpUpdate::ToolCall {
            tool_call_id: String::new(),
            title: String::new(),
            status: String::new(),
        }
        .kind(),
        "tool_call"
    );
    assert_eq!(
        AcpUpdate::ToolCallUpdate {
            tool_call_id: String::new(),
            title: None,
            status: None,
        }
        .kind(),
        "tool_call_update"
    );
    assert_eq!(AcpUpdate::Plan { entries: 0 }.kind(), "plan");
    assert_eq!(
        AcpUpdate::Other {
            kind: "weird".to_string()
        }
        .kind(),
        "weird"
    );
}

#[test]
fn acp_error_accessors_and_display() {
    let err = AcpError::new("claude", AcpErrorKind::Exited, "process died").with_exit_code(Some(3));
    assert_eq!(err.agent(), "claude");
    assert_eq!(err.kind(), AcpErrorKind::Exited);
    assert_eq!(err.exit_code(), Some(3));
    assert_eq!(err.message(), "process died");
    let rendered = err.to_string();
    assert!(rendered.contains("claude"));
    assert!(rendered.contains("process died"));
    assert!(rendered.contains("Exited"));
}

#[test]
fn acp_error_without_exit_code_reports_none() {
    let err = AcpError::new("gemini", AcpErrorKind::Spawn, "not found");
    assert_eq!(err.exit_code(), None);
}
