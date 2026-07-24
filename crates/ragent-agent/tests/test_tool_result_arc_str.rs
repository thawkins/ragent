//! Integration tests for the `Arc<str>` tool-result content path
//! (`AgentPerf` T-008 / FR-013).
//!
//! Validates that `tool_result_content_for_llm` returns an `Arc<str>` and
//! that the `Arc<str>` round-trips through the on-wire JSON
//! serialisation unchanged.

use ragent_agent::session::processor::tool_result_content_for_llm;
use ragent_llm::llm::{ChatContent, ChatMessage, ContentPart};
use serde_json::json;
use std::sync::Arc;

#[test]
fn small_tool_result_returns_arc_str() {
    let result = tool_result_content_for_llm("read", "hello world", None);
    assert_eq!(result.as_ref(), "hello world");
    // Cheap clone: must be the same pointer.
    let clone = Arc::clone(&result);
    assert!(Arc::ptr_eq(&result, &clone));
}

#[test]
fn large_tool_result_returns_arc_str() {
    let content = "x".repeat(20_000);
    let result = tool_result_content_for_llm("read", &content, Some(&json!({"total_lines": 600})));
    let as_str: &str = result.as_ref();
    assert!(as_str.contains("[tool result truncated for context"));
    assert!(as_str.contains('x'));
}

#[test]
fn arc_str_round_trips_through_json() {
    let original = Arc::<str>::from("hello world");
    let part = ContentPart::ToolResult {
        tool_use_id: "call_1".to_string(),
        content: Arc::clone(&original),
    };
    let message = ChatMessage {
        role: "tool".to_string(),
        content: ChatContent::Parts(vec![part]),
    };
    let serialised = serde_json::to_string(&message).expect("serialise");
    assert!(serialised.contains("\"content\":\"hello world\""));
    let parsed: ChatMessage = serde_json::from_str(&serialised).expect("parse");
    let ChatContent::Parts(parts) = parsed.content else {
        panic!("expected Parts content");
    };
    let ContentPart::ToolResult { content, .. } = &parts[0] else {
        panic!("expected ToolResult");
    };
    assert_eq!(content.as_ref(), "hello world");
}

#[test]
fn arc_str_is_cheaply_cloneable() {
    let original: Arc<str> = Arc::from("some moderately long tool result content");
    let clones: Vec<Arc<str>> = (0..100).map(|_| Arc::clone(&original)).collect();
    for c in &clones {
        assert!(Arc::ptr_eq(&original, c));
    }
}
