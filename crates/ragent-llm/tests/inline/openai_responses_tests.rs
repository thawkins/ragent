//! Inline tests for `openai_responses.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::llm::{ChatMessage, ToolDefinition};
use std::sync::Arc;

#[test]
fn test_provider_id_and_name() {
    let provider = ResponsesApiProvider;
    assert_eq!(provider.id(), "openai_responses");
    assert_eq!(provider.name(), "OpenAI Responses API");
}

#[test]
fn test_default_models() {
    let provider = ResponsesApiProvider;
    let models = provider.default_models();

    assert!(!models.is_empty());
    assert!(models.iter().any(|m| m.id == "gpt-5.6"));
    assert!(models.iter().any(|m| m.id == "o1"));

    // All models should have reasoning capability
    for model in &models {
        assert!(model.capabilities.reasoning);
    }
}

#[test]
fn test_build_request_body_basic() {
    let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
    let request = ChatRequest {
        model: "gpt-5.6".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let body = client.build_request_body(&request);

    assert_eq!(body["model"], "gpt-5.6");
    assert_eq!(body["reasoning"]["effort"], "medium");
    assert!(body["stream"].as_bool().unwrap());

    let input = body["input"].as_array().unwrap();
    assert_eq!(input.len(), 1);
    assert_eq!(input[0]["role"], "user");
    assert_eq!(input[0]["content"], "Hello");
}

#[test]
fn test_build_request_body_with_tools() {
    let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
    let tools = vec![ToolDefinition {
        name: "test_tool".to_string(),
        description: "A test tool".to_string(),
        parameters: json!({"type": "object"}),
    }];

    let request = ChatRequest {
        model: "gpt-5.6".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Test".to_string()),
        }]),
        tools: Arc::new(tools),
        temperature: None,
        top_p: None,
        max_tokens: Some(1000),
        system: Some("You are helpful".into()),
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let body = client.build_request_body(&request);

    assert!(body["tools"].is_array());
    assert_eq!(body["tools"].as_array().unwrap().len(), 1);
    assert_eq!(body["max_output_tokens"], 1000);
    assert_eq!(body["instructions"], "You are helpful");
}

#[test]
fn test_build_request_body_with_thinking() {
    use ragent_types::thinking::{ThinkingConfig, ThinkingLevel};

    let client = ResponsesApiClient::new("test-key", "https://api.openai.com");
    let request = ChatRequest {
        model: "gpt-5.6".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Test".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: HashMap::new(),
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: Some(ThinkingConfig {
            enabled: true,
            level: ThinkingLevel::High,
            budget_tokens: None,
            display: None,
        }),
    };

    let body = client.build_request_body(&request);

    assert_eq!(body["reasoning"]["effort"], "high");
}
