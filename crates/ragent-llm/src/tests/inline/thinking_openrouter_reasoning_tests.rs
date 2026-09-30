//! Inline tests for `thinking.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use serde_json::json;

use super::*;
use crate::llm::{ChatContent, ChatMessage, ChatRequest};
use ragent_types::{ThinkingConfig, ThinkingLevel};
use std::sync::Arc;

fn make_request() -> ChatRequest {
    ChatRequest {
        model: "openrouter/anthropic/claude-sonnet-4".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hi".to_string()),
        }]),
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

#[test]
fn test_openrouter_reasoning_disabled() {
    let mut request = make_request();
    request.thinking = Some(ThinkingConfig::off());
    assert_eq!(
        openrouter_reasoning_payload_from_request(&request),
        Some(json!({ "effort": "none" }))
    );
}

#[test]
fn test_openrouter_reasoning_auto_no_budget_omits_payload() {
    let mut request = make_request();
    request.thinking = Some(ThinkingConfig::new(ThinkingLevel::Auto));
    assert_eq!(openrouter_reasoning_payload_from_request(&request), None);
}

#[test]
fn test_openrouter_reasoning_auto_with_budget_sets_max_tokens() {
    let mut request = make_request();
    request.thinking = Some(ThinkingConfig {
        enabled: true,
        level: ThinkingLevel::Auto,
        budget_tokens: Some(4096),
        display: None,
    });
    assert_eq!(
        openrouter_reasoning_payload_from_request(&request),
        Some(json!({ "max_tokens": 4096 }))
    );
}

#[test]
fn test_openrouter_reasoning_low_sets_effort() {
    let mut request = make_request();
    request.thinking = Some(ThinkingConfig::new(ThinkingLevel::Low));
    assert_eq!(
        openrouter_reasoning_payload_from_request(&request),
        Some(json!({ "effort": "low" }))
    );
}

#[test]
fn test_openrouter_reasoning_medium_sets_effort_and_budget() {
    let mut request = make_request();
    request.thinking = Some(ThinkingConfig {
        enabled: true,
        level: ThinkingLevel::Medium,
        budget_tokens: Some(8192),
        display: None,
    });
    assert_eq!(
        openrouter_reasoning_payload_from_request(&request),
        Some(json!({ "effort": "medium", "max_tokens": 8192 }))
    );
}

#[test]
fn test_openrouter_reasoning_high_sets_effort_and_budget() {
    let mut request = make_request();
    request.thinking = Some(ThinkingConfig {
        enabled: true,
        level: ThinkingLevel::High,
        budget_tokens: Some(16_384),
        display: None,
    });
    assert_eq!(
        openrouter_reasoning_payload_from_request(&request),
        Some(json!({ "effort": "high", "max_tokens": 16_384 }))
    );
}

#[test]
fn test_openrouter_reasoning_prefers_typed_over_legacy_options() {
    let mut request = make_request();
    request.thinking = Some(ThinkingConfig::new(ThinkingLevel::High));
    request
        .options
        .insert("reasoning_effort".to_string(), json!("low"));
    let payload = openrouter_reasoning_payload_from_request(&request).unwrap();
    assert_eq!(payload["effort"], "high");
}

#[test]
fn test_openrouter_reasoning_legacy_effort_fallback() {
    let mut request = make_request();
    request
        .options
        .insert("reasoning_effort".to_string(), json!("medium"));
    assert_eq!(
        openrouter_reasoning_payload_from_request(&request),
        Some(json!({ "effort": "medium" }))
    );
}

#[test]
fn test_openrouter_reasoning_legacy_thinking_string_fallback() {
    let mut request = make_request();
    request
        .options
        .insert("thinking".to_string(), json!("disabled"));
    assert_eq!(
        openrouter_reasoning_payload_from_request(&request),
        Some(json!({ "effort": "none" }))
    );
}
