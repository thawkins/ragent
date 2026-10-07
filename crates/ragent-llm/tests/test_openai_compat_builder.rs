//! Request-shape tests for the shared OpenAI-compatible request-body builder
//! (audit T-401).
//!
//! `openai`, `openrouter`, `generic_openai`, `copilot`, `ollama`, and
//! `ollama_cloud` all delegate message packing and base-body construction to
//! [`build_openai_messages`] / [`OpenAiCompat`], selecting their wire dialect
//! through [`OpenAiCompatSpec`]. These tests pin the shared shape and each
//! dialect divergence so the consolidation cannot silently drift.

use std::sync::Arc;

use ragent_llm::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, ToolDefinition};
use ragent_llm::provider::openai_compat::{OpenAiCompat, OpenAiCompatSpec, build_openai_messages};
use serde_json::{Value, json};

fn tool(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: format!("{name} tool"),
        parameters: json!({"type": "object"}),
    }
}

fn base_request(messages: Vec<ChatMessage>) -> ChatRequest {
    ChatRequest {
        model: "test-model".to_string(),
        messages: Arc::new(messages),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: std::collections::HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    }
}

fn text_message(role: &str, text: &str) -> ChatMessage {
    ChatMessage {
        role: role.to_string(),
        content: ChatContent::Text(text.to_string()),
    }
}

fn parts_message(role: &str, parts: Vec<ContentPart>) -> ChatMessage {
    ChatMessage {
        role: role.to_string(),
        content: ChatContent::Parts(parts),
    }
}

fn tool_use(id: &str, name: &str, input: Value) -> ContentPart {
    ContentPart::ToolUse {
        id: id.to_string(),
        name: name.to_string(),
        input,
    }
}

fn tool_result(id: &str, content: &str) -> ContentPart {
    ContentPart::ToolResult {
        tool_use_id: id.to_string(),
        content: Arc::from(content),
    }
}

// ---------------------------------------------------------------------------
// Shared message packing
// ---------------------------------------------------------------------------

#[test]
fn test_shared_messages_prepend_system_and_pack_text() {
    let mut request = base_request(vec![text_message("user", "hello")]);
    request.system = Some(Arc::from("be brief"));

    let messages = build_openai_messages(&request, &OpenAiCompatSpec::openai());

    assert_eq!(messages.len(), 2, "system + user");
    assert_eq!(
        messages[0],
        json!({"role": "system", "content": "be brief"})
    );
    assert_eq!(messages[1], json!({"role": "user", "content": "hello"}));
}

#[test]
fn test_shared_messages_emit_tool_calls_and_results() {
    let request = base_request(vec![
        parts_message(
            "assistant",
            vec![tool_use("call_1", "read", json!({"path": "x.txt"}))],
        ),
        parts_message("tool", vec![tool_result("call_1", "file body")]),
    ]);

    let messages = build_openai_messages(&request, &OpenAiCompatSpec::openai());

    assert_eq!(messages.len(), 2, "assistant tool_calls + tool result");
    assert_eq!(messages[0]["role"], "assistant");
    assert_eq!(messages[0]["tool_calls"][0]["id"], "call_1");
    assert_eq!(messages[0]["tool_calls"][0]["function"]["name"], "read");
    assert_eq!(
        messages[0]["tool_calls"][0]["function"]["arguments"],
        json!({"path": "x.txt"}).to_string(),
        "tool input must be serialised to a JSON string, not embedded"
    );
    assert_eq!(messages[1]["role"], "tool");
    assert_eq!(messages[1]["tool_call_id"], "call_1");
    assert_eq!(messages[1]["content"], "file body");
    assert!(
        messages[1].get("tool_name").is_none(),
        "the canonical OpenAI dialect does not emit tool_name"
    );
}

#[test]
fn test_ollama_spec_adds_tool_name_to_results() {
    let request = base_request(vec![
        parts_message(
            "assistant",
            vec![tool_use("call_9", "get_weather", json!({}))],
        ),
        parts_message("tool", vec![tool_result("call_9", "sunny")]),
    ]);

    let messages = build_openai_messages(&request, &OpenAiCompatSpec::ollama());

    assert_eq!(
        messages[1]["tool_name"], "get_weather",
        "the Ollama dialect mirrors tool_call_id with the native tool_name"
    );
}

// ---------------------------------------------------------------------------
// Base body / sampling / tools
// ---------------------------------------------------------------------------

#[test]
fn test_openai_base_body_applies_sampling_tools_and_stream_options() {
    let mut request = base_request(vec![text_message("user", "hi")]);
    request.temperature = Some(0.5);
    request.top_p = Some(0.9);
    request.max_tokens = Some(256);
    request.tools = Arc::new(vec![tool("read")]);

    let body =
        OpenAiCompat::base_body(&request, OpenAiCompatSpec::openai(), &request.tools).finish();

    assert_eq!(body["model"], "test-model");
    assert_eq!(body["stream"], true);
    assert_eq!(body["stream_options"], json!({"include_usage": true}));
    assert_eq!(body["temperature"], json!(0.5_f32));
    assert_eq!(body["top_p"], json!(0.9_f32));
    assert_eq!(body["max_tokens"], json!(256));
    assert!(
        body["tools"].as_array().is_some_and(|a| !a.is_empty()),
        "tools must be serialised into the body"
    );
}

#[test]
fn test_openai_spec_collapses_single_text_part() {
    let request = base_request(vec![parts_message(
        "user",
        vec![ContentPart::Text {
            text: "solo".to_string(),
        }],
    )]);

    let messages = build_openai_messages(&request, &OpenAiCompatSpec::openai());

    assert_eq!(
        messages[0]["content"], "solo",
        "a single text part collapses to a bare string"
    );
}

#[test]
fn test_copilot_spec_omits_stream_options_and_does_not_collapse_non_text() {
    let request = base_request(vec![parts_message(
        "user",
        vec![ContentPart::ImageUrl {
            url: "data:image/png;base64,AAAA".to_string(),
        }],
    )]);

    let body =
        OpenAiCompat::base_body(&request, OpenAiCompatSpec::copilot(), &request.tools).finish();

    assert!(
        body.get("stream_options").is_none(),
        "Copilot never sends stream_options"
    );
    assert!(
        body["messages"][0]["content"].is_array(),
        "a single non-text part stays a parts array under the strict collapse rule"
    );
}

#[test]
fn test_ollama_cloud_native_envelope_omits_openai_only_fields() {
    let request = base_request(vec![text_message("user", "hi")]);

    let messages = vec![json!({"role": "user", "content": "hi"})];
    let body = OpenAiCompat::with_messages(
        &request,
        OpenAiCompatSpec::ollama_cloud(),
        &request.tools,
        messages,
    )
    .finish();

    assert_eq!(body["model"], "test-model");
    assert_eq!(body["stream"], true);
    assert!(
        body.get("stream_options").is_none(),
        "the native /api/chat envelope carries no OpenAI-only fields"
    );
}
