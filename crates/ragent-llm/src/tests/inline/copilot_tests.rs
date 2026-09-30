//! Inline tests for `copilot.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::llm::{ChatContent, ChatMessage, ChatRequest};
use std::sync::Arc;

#[test]
fn test_provider_defaults() {
    let provider = CopilotProvider::new();
    assert_eq!(provider.id(), "copilot");
    assert_eq!(provider.name(), "GitHub Copilot");
    assert!(
        provider.default_models().is_empty(),
        "Copilot default_models should be empty; models are discovered at runtime"
    );
}

#[test]
#[allow(clippy::used_underscore_binding)]
fn test_with_custom_url() {
    let provider = CopilotProvider::with_url("https://proxy.example.com/");
    assert_eq!(provider._base_url, "https://proxy.example.com");
}

#[test]
fn test_models_are_free() {
    let provider = CopilotProvider::new();
    for m in provider.default_models() {
        assert!(m.cost.input.abs() < f64::EPSILON);
        assert!(m.cost.output.abs() < f64::EPSILON);
    }
}

#[test]
fn test_format_request_multiplier_trims_trailing_zeroes() {
    assert_eq!(format_request_multiplier(1.0), "1");
    assert_eq!(format_request_multiplier(1.5), "1.5");
    assert_eq!(format_request_multiplier(0.25), "0.25");
}

#[test]
fn test_request_multiplier_prefers_pricing_block() {
    let entry = CopilotModelEntry {
        id: "x".to_string(),
        name: None,
        model_picker_enabled: true,
        vendor: None,
        pricing: Some(CopilotModelPricing {
            request_cost: Some(2.0),
        }),
        request_cost: Some(1.0),
        request_cost_multiplier: None,
        premium_request_multiplier: None,
        multiplier: None,
        supported_endpoints: None,
        api: None,
        capabilities: None,
    };
    assert_eq!(entry.request_multiplier(), Some(2.0));
}

#[test]
fn test_request_multiplier_ignores_invalid_values() {
    let entry = CopilotModelEntry {
        id: "x".to_string(),
        name: None,
        model_picker_enabled: true,
        vendor: None,
        pricing: None,
        request_cost: Some(0.0),
        request_cost_multiplier: Some(f64::NAN),
        premium_request_multiplier: Some(-1.0),
        multiplier: None,
        supported_endpoints: None,
        api: None,
        capabilities: None,
    };
    assert_eq!(entry.request_multiplier(), None);
}

#[test]
fn test_supports_chat_completions_uses_endpoint_metadata() {
    let entry = CopilotModelEntry {
        id: "gpt-5.3-codex".to_string(),
        name: None,
        model_picker_enabled: true,
        vendor: None,
        pricing: None,
        request_cost: None,
        request_cost_multiplier: None,
        premium_request_multiplier: None,
        multiplier: None,
        supported_endpoints: Some(vec!["/chat/completions".to_string()]),
        api: None,
        capabilities: None,
    };
    assert!(entry.supports_chat_completions());
}

#[test]
fn test_supports_chat_completions_filters_codex_without_metadata() {
    let entry = CopilotModelEntry {
        id: "gpt-5.3-codex".to_string(),
        name: None,
        model_picker_enabled: true,
        vendor: None,
        pricing: None,
        request_cost: None,
        request_cost_multiplier: None,
        premium_request_multiplier: None,
        multiplier: None,
        supported_endpoints: None,
        api: None,
        capabilities: None,
    };
    assert!(!entry.supports_chat_completions());
}

#[test]
fn test_reasoning_effort_from_options_accepts_levels() {
    let mk = |v: &str| {
        let mut request = ChatRequest {
            model: "o3-mini".to_string(),
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
        };
        request
            .options
            .insert("reasoning_effort".to_string(), json!(v));
        request
    };
    assert_eq!(reasoning_effort_from_request(&mk("low")), Some("low"));
    assert_eq!(reasoning_effort_from_request(&mk("medium")), Some("medium"));
    assert_eq!(reasoning_effort_from_request(&mk("high")), Some("high"));
}

#[test]
fn test_reasoning_effort_from_options_alias_and_invalid() {
    let mut alias = HashMap::new();
    alias.insert("reasoning_level".to_string(), json!("HIGH"));
    let alias_request = ChatRequest {
        model: "o3-mini".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: alias,
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };
    assert_eq!(reasoning_effort_from_request(&alias_request), Some("high"));

    let mut invalid_options = HashMap::new();
    invalid_options.insert("reasoning_effort".to_string(), json!("turbo"));
    let invalid_request = ChatRequest {
        model: "o3-mini".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options: invalid_options,
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };
    assert_eq!(reasoning_effort_from_request(&invalid_request), None);
}

#[test]
fn test_reasoning_effort_prefers_typed_thinking() {
    let mut request = ChatRequest {
        model: "o3-mini".to_string(),
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
        thinking: Some(ragent_types::ThinkingConfig::new(
            ragent_types::ThinkingLevel::High,
        )),
    };
    request
        .options
        .insert("reasoning_effort".to_string(), json!("low"));

    assert_eq!(reasoning_effort_from_request(&request), Some("high"));
}

#[test]
fn test_build_request_body_applies_reasoning_effort() {
    let client = CopilotClient {
        token: "x".to_string(),
        base_url: "https://api.githubcopilot.com".to_string(),
        http: crate::provider::http_client::create_http_client(),
    };
    let mut options = HashMap::new();
    options.insert("reasoning_effort".to_string(), json!("medium"));
    let req = ChatRequest {
        model: "o3-mini".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options,
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let body = client.build_request_body(&req, &[]);
    assert_eq!(body["reasoning_effort"], json!("medium"));
}

#[test]
fn test_build_request_body_thinking_disabled_fallback() {
    let client = CopilotClient {
        token: "x".to_string(),
        base_url: "https://api.githubcopilot.com".to_string(),
        http: crate::provider::http_client::create_http_client(),
    };
    let mut options = HashMap::new();
    options.insert("thinking".to_string(), json!("disabled"));
    let req = ChatRequest {
        model: "gpt-4o".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options,
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let body = client.build_request_body(&req, &[]);
    assert_eq!(body["reasoning_effort"], json!("none"));
}

#[test]
fn test_build_request_body_reasoning_effort_overrides_thinking_toggle() {
    let client = CopilotClient {
        token: "x".to_string(),
        base_url: "https://api.githubcopilot.com".to_string(),
        http: crate::provider::http_client::create_http_client(),
    };
    let mut options = HashMap::new();
    options.insert("reasoning_effort".to_string(), json!("high"));
    options.insert("thinking".to_string(), json!("disabled"));
    let req = ChatRequest {
        model: "o3-mini".to_string(),
        messages: Arc::new(vec![ChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("hello".to_string()),
        }]),
        tools: Arc::new(vec![]),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: None,
        options,
        session_id: None,
        request_id: None,
        stream_timeout_secs: None,
        thinking: None,
    };

    let body = client.build_request_body(&req, &[]);
    assert_eq!(body["reasoning_effort"], json!("high"));
}

#[test]
fn test_copilot_premium_multiplier_table() {
    // Included models (0x)
    assert_eq!(copilot_premium_multiplier("gpt-4o"), Some(0.0));
    assert_eq!(copilot_premium_multiplier("GPT-4o"), Some(0.0)); // case insensitive
    assert_eq!(copilot_premium_multiplier("gpt-4.1"), Some(0.0));
    assert_eq!(copilot_premium_multiplier("gpt-5-mini"), Some(0.0));

    // Low-cost models (0.25x - 0.33x)
    assert_eq!(copilot_premium_multiplier("claude-haiku-4.5"), Some(0.33));
    assert_eq!(copilot_premium_multiplier("gemini-3-flash"), Some(0.33));
    assert_eq!(copilot_premium_multiplier("grok-code-fast-1"), Some(0.25));

    // Standard models (1x)
    assert_eq!(copilot_premium_multiplier("claude-sonnet-4"), Some(1.0));
    assert_eq!(copilot_premium_multiplier("gemini-2.5-pro"), Some(1.0));

    // High-cost models (3x)
    assert_eq!(copilot_premium_multiplier("claude-opus-4.5"), Some(3.0));
    assert_eq!(copilot_premium_multiplier("claude-opus-4.6"), Some(3.0));

    // Very high-cost models
    assert_eq!(copilot_premium_multiplier("claude-opus-4.7"), Some(7.5));

    // Fallback pattern matching
    assert_eq!(
        copilot_premium_multiplier("claude-3-sonnet-latest"),
        Some(1.0)
    ); // contains "sonnet"
    assert_eq!(
        copilot_premium_multiplier("claude-3-haiku-latest"),
        Some(0.33)
    ); // contains "haiku"

    // Unknown models return None
    assert_eq!(copilot_premium_multiplier("unknown-model"), None);
}
