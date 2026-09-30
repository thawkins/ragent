//! Inline tests for `thinking.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::llm::{ChatContent, ChatMessage};
use std::sync::Arc;

fn make_request() -> ChatRequest {
    ChatRequest {
        model: "test-model".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
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
    }
}

#[test]
fn test_reasoning_effort_prefers_typed_thinking() {
    let mut request = make_request();
    request.thinking = Some(ThinkingConfig::new(ThinkingLevel::High));
    request
        .options
        .insert("reasoning_effort".to_string(), json!("low"));

    assert_eq!(reasoning_effort_from_request(&request), Some("high"));
}

#[test]
fn test_reasoning_effort_falls_back_to_legacy_options() {
    let mut request = make_request();
    request
        .options
        .insert("thinking".to_string(), json!("disabled"));

    assert_eq!(reasoning_effort_from_request(&request), Some("none"));
}

#[test]
fn test_reasoning_levels_from_supported_efforts_includes_auto() {
    let efforts = vec!["high".to_string(), "medium".to_string(), "none".to_string()];
    assert_eq!(
        reasoning_levels_from_supported_efforts(Some(&efforts)),
        vec![
            ThinkingLevel::Auto,
            ThinkingLevel::Off,
            ThinkingLevel::Medium,
            ThinkingLevel::High,
        ]
    );
}

#[test]
fn test_anthropic_payload_prefers_budget_tokens() {
    let mut request = make_request();
    request.thinking = Some(ThinkingConfig {
        enabled: true,
        level: ThinkingLevel::High,
        budget_tokens: Some(4096),
        display: Some(ThinkingDisplay::Full),
    });

    assert_eq!(
        anthropic_thinking_payload_from_request(&request),
        Some(json!({
            "type": "enabled",
            "budget_tokens": 4096,
        }))
    );
}

#[test]
fn test_gemini_thinking_config_maps_omitted_to_minimal() {
    let mut request = make_request();
    request.thinking = Some(ThinkingConfig {
        enabled: true,
        level: ThinkingLevel::High,
        budget_tokens: None,
        display: Some(ThinkingDisplay::Omitted),
    });

    assert_eq!(
        gemini_thinking_config_from_request(&request),
        Some(json!({
            "thinkingLevel": "minimal",
            "includeThoughts": false,
        }))
    );
}

#[test]
fn test_think_flag_from_request_uses_binary_enablement() {
    let mut request = make_request();
    request.thinking = Some(ThinkingConfig::new(ThinkingLevel::Auto));
    assert_eq!(think_flag_from_request(&request), Some(true));

    request.thinking = Some(ThinkingConfig::off());
    assert_eq!(think_flag_from_request(&request), Some(false));
}

#[test]
fn test_binary_thinking_support_heuristics() {
    // Classic thinking models
    assert!(model_supports_binary_thinking("deepseek-r1:latest"));
    assert!(model_supports_binary_thinking("qwen3:30b"));
    assert!(model_supports_binary_thinking("qwq:32b"));
    assert!(model_supports_binary_thinking("reasoner:latest"));
    // Newly recognized thinking models
    assert!(model_supports_binary_thinking("kimi-k2.7-code"));
    assert!(model_supports_binary_thinking("kimi:latest"));
    assert!(model_supports_binary_thinking("gemma3:27b"));
    assert!(model_supports_binary_thinking("phi4-reasoning:14b"));
    assert!(model_supports_binary_thinking("magistral:latest"));
    assert!(model_supports_binary_thinking("mistral-small3:latest"));
    // Non-thinking models
    assert!(!model_supports_binary_thinking("llama3.2"));
    assert!(!model_supports_binary_thinking("mistral:7b"));
    assert!(!model_supports_binary_thinking("codellama:latest"));
}
