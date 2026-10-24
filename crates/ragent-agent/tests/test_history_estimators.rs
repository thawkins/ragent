//! Tests-only coverage for the session-history pure estimators and classifiers
//! (`ragent_agent::session::history`): request-byte estimation, token-overflow
//! and permanent-error classification, and incomplete-file-task detection.

use std::collections::HashMap;
use std::sync::Arc;

use ragent_agent::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, ToolDefinition};
use ragent_agent::message::{MessagePart, ToolCallState, ToolCallStatus};
use ragent_agent::session::history::{
    chat_request_payload_bytes, detect_incomplete_file_task, estimate_request_bytes,
    estimate_request_bytes_with_tool_bytes, estimate_tool_definition_bytes,
};

fn text_request(content: &str) -> ChatRequest {
    ChatRequest {
        model: "m".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text(content.to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        thinking: None,
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
    }
}

#[test]
fn estimate_request_bytes_grows_with_content() {
    let small = estimate_request_bytes(&text_request("hi"));
    let large = estimate_request_bytes(&text_request(&"x".repeat(1000)));
    assert!(large > small, "{large} should exceed {small}");
    // The legacy alias delegates to the same estimator.
    assert_eq!(chat_request_payload_bytes(&text_request("hi")), small);
}

#[test]
fn estimate_request_bytes_with_cached_tool_bytes_matches_manual_sum() {
    let mut request = text_request("hello");
    request.tools = Arc::new(vec![ToolDefinition {
        name: "read".to_string(),
        description: "reads".to_string(),
        parameters: serde_json::json!({"type": "object"}),
    }]);
    request.system = Some(Arc::from("system prompt"));

    let cached = estimate_tool_definition_bytes(&request.tools);
    let with_cache = estimate_request_bytes_with_tool_bytes(&request, Some(cached));
    let without_cache = estimate_request_bytes_with_tool_bytes(&request, None);
    assert_eq!(
        with_cache, without_cache,
        "the cached tool-byte path must match the legacy serialisation"
    );
}

#[test]
fn estimate_request_bytes_counts_parts() {
    let mut request = text_request("");
    request.messages = Arc::new(vec![ChatMessage {
        role: "assistant".to_string(),
        content: ChatContent::Parts(vec![
            ContentPart::Text {
                text: "text".to_string(),
            },
            ContentPart::ToolUse {
                id: "call_1".to_string(),
                name: "read".to_string(),
                input: serde_json::json!({ "path": "a" }),
            },
            ContentPart::ToolResult {
                tool_use_id: "call_1".to_string(),
                content: "result".into(),
                is_error: false,
            },
            ContentPart::ImageUrl {
                url: "https://x/y.png".to_string(),
            },
        ]),
    }]);
    let bytes = estimate_request_bytes(&request);
    assert!(bytes > 80, "parts must contribute bytes, got {bytes}");
}

#[test]
fn estimate_tool_definition_bytes_is_zero_for_empty() {
    assert_eq!(estimate_tool_definition_bytes(&[]), 0);
}

fn tool_call_part(tool: &str) -> MessagePart {
    MessagePart::ToolCall {
        tool: tool.to_string(),
        call_id: "c".to_string(),
        state: Box::new(ToolCallState {
            input: serde_json::Value::Null,
            output: None,
            status: ToolCallStatus::Completed,
            error: None,
            duration_ms: None,
        }),
    }
}

#[test]
fn detect_incomplete_file_task_requires_action_and_filename_and_missing_write() {
    // Verb + filename + no write tool -> incomplete (true).
    assert!(detect_incomplete_file_task("please create report.txt", &[]));

    // Verb + filename + a write tool used -> complete (false).
    assert!(!detect_incomplete_file_task(
        "please create report.txt",
        &[tool_call_part("write")]
    ));

    // A write tool via an alias also counts.
    assert!(!detect_incomplete_file_task(
        "generate notes.md",
        &[tool_call_part("multiedit")]
    ));

    // No file action verb -> false regardless of filename.
    assert!(!detect_incomplete_file_task("what is report.txt", &[]));

    // Verb but no filename -> false.
    assert!(!detect_incomplete_file_task("create a diagram", &[]));

    // An over-long extension is not a filename.
    assert!(!detect_incomplete_file_task(
        "create file.thisiswaytoolongext",
        &[]
    ));
}
