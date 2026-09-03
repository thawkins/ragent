#![allow(clippy::assert_is_empty)]
//! Integration tests for the `SharedChatRequest` type
//! (`AgentPerf` T-009 / FR-014).

use std::sync::Arc;

use ragent_llm::llm::{ChatContent, ChatMessage, ContentPart, ToolDefinition};
use ragent_llm::shared_request::SharedChatRequest;
use serde_json::json;

fn user_msg(text: &str) -> ChatMessage {
    ChatMessage {
        role: "user".to_string(),
        content: ChatContent::Text(text.to_string()),
    }
}

fn tool_def(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: "desc".to_string(),
        parameters: json!({}),
    }
}

#[test]
fn shared_request_round_trips_through_chat_request() {
    let body = SharedChatRequest::new(vec![user_msg("hello")], vec![tool_def("read")]);
    let request = ragent_llm::llm::ChatRequest {
        model: "mock".to_string(),
        messages: Arc::clone(&body.messages),
        tools: Arc::clone(&body.tools),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: std::collections::HashMap::new(),
        thinking: None,
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
    };
    // The `ChatRequest` and the `SharedChatRequest` share the same `Arc`.
    assert!(Arc::ptr_eq(&request.messages, &body.messages));
    assert!(Arc::ptr_eq(&request.tools, &body.tools));
}

#[test]
fn shared_request_arc_str_tool_result_is_preserved() {
    use std::sync::Arc as StdArc;
    let part = ContentPart::ToolResult {
        tool_use_id: "call_x".to_string(),
        content: StdArc::from("output"),
    };
    let body = SharedChatRequest::new(
        vec![ChatMessage {
            role: "tool".to_string(),
            content: ChatContent::Parts(vec![part]),
        }],
        vec![],
    );
    let ChatContent::Parts(parts) = &body.messages[0].content else {
        panic!("expected Parts");
    };
    let ContentPart::ToolResult { content, .. } = &parts[0] else {
        panic!("expected ToolResult");
    };
    assert_eq!(content.as_ref(), "output");
}
