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

// ---------------------------------------------------------------------------
// Legacy thinking-option parsing (covers parse_legacy_thinking_option arms)
// ---------------------------------------------------------------------------

#[test]
fn test_legacy_thinking_option_disabled_aliases() {
    for raw in ["disabled", "off", "none", "  DISABLED  "] {
        let mut request = make_request();
        request.options.insert("thinking".to_string(), json!(raw));
        let config = parse_legacy_thinking_option(&request.options).expect("config");
        assert!(!config.is_effective_enabled(), "{raw} should disable");
    }
}

#[test]
fn test_legacy_thinking_option_auto_with_budget() {
    let mut request = make_request();
    request
        .options
        .insert("thinking".to_string(), json!("adaptive"));
    request
        .options
        .insert("thinking_budget_tokens".to_string(), json!(4096));
    let config = parse_legacy_thinking_option(&request.options).expect("config");
    assert!(config.is_effective_enabled());
    assert_eq!(config.level, ThinkingLevel::Auto);
    assert_eq!(config.budget_tokens, Some(4096));

    // Non-u64 budget is ignored.
    let mut request = make_request();
    request
        .options
        .insert("thinking".to_string(), json!("auto"));
    request
        .options
        .insert("thinking_budget_tokens".to_string(), json!("lots"));
    let config = parse_legacy_thinking_option(&request.options).expect("config");
    assert!(config.budget_tokens.is_none());
}

#[test]
fn test_legacy_thinking_option_level_names() {
    for (raw, level) in [
        ("low", ThinkingLevel::Low),
        ("medium", ThinkingLevel::Medium),
        ("high", ThinkingLevel::High),
    ] {
        let mut request = make_request();
        request.options.insert("thinking".to_string(), json!(raw));
        let config = parse_legacy_thinking_option(&request.options).expect("config");
        assert_eq!(config.level, level);
    }
    // Unknown value -> None.
    let mut request = make_request();
    request
        .options
        .insert("thinking".to_string(), json!("nonsense"));
    assert!(parse_legacy_thinking_option(&request.options).is_none());
}

#[test]
fn test_reasoning_effort_from_legacy_level_options() {
    let mut request = make_request();
    request
        .options
        .insert("thinking".to_string(), json!("medium"));
    assert_eq!(reasoning_effort_from_request(&request), Some("medium"));
}

#[test]
fn test_reasoning_levels_from_supported_efforts_empty_and_subset() {
    assert_eq!(reasoning_levels_from_supported_efforts(None).len(), 0);
    assert_eq!(reasoning_levels_from_supported_efforts(Some(&[])).len(), 0);
    let efforts = vec!["low".to_string(), "unknown".to_string()];
    assert_eq!(
        reasoning_levels_from_supported_efforts(Some(&efforts)),
        vec![ThinkingLevel::Auto, ThinkingLevel::Low],
    );
}

#[test]
fn test_gemini_thinking_config_include_thoughts_only() {
    let mut request = make_request();
    request
        .options
        .insert("include_thoughts".to_string(), json!(true));
    assert_eq!(
        gemini_thinking_config_from_request(&request),
        Some(json!({ "includeThoughts": true }))
    );
}

#[test]
fn test_gemini_thinking_config_disabled_is_minimal() {
    let mut request = make_request();
    request.thinking = Some(ThinkingConfig::off());
    let value = gemini_thinking_config_from_request(&request).expect("config");
    assert_eq!(value["thinkingLevel"], "minimal");
    assert_eq!(value["includeThoughts"], false);
}

#[test]
fn test_should_warn_unsupported_thinking_legacy_enabled() {
    let mut request = make_request();
    request
        .options
        .insert("thinking".to_string(), json!("high"));
    assert!(should_warn_unsupported_thinking(&request));
    let request = make_request();
    assert!(!should_warn_unsupported_thinking(&request));
}
