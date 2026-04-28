//! Shared thinking-level mapping helpers for provider adapters.
//!
//! This module centralizes the provider-agnostic `ThinkingConfig` mappings used
//! by the individual provider clients so typed request thinking takes
//! precedence over legacy `options["thinking"]` shims while keeping the
//! fallback path available.

use std::collections::HashMap;

use serde_json::{Value, json};

use crate::llm::ChatRequest;
use ragent_types::{ThinkingConfig, ThinkingDisplay, ThinkingLevel};

fn thinking_level_sort_key(level: ThinkingLevel) -> usize {
    match level {
        ThinkingLevel::Auto => 0,
        ThinkingLevel::Off => 1,
        ThinkingLevel::Low => 2,
        ThinkingLevel::Medium => 3,
        ThinkingLevel::High => 4,
    }
}

fn normalize_levels(levels: impl IntoIterator<Item = ThinkingLevel>) -> Vec<ThinkingLevel> {
    let mut levels: Vec<_> = levels.into_iter().collect();
    levels.sort_by_key(|level| thinking_level_sort_key(*level));
    levels.dedup();
    levels
}

/// Returns the canonical full reasoning-level set for providers that support
/// explicit effort selection.
pub(crate) fn full_reasoning_levels() -> Vec<ThinkingLevel> {
    normalize_levels([
        ThinkingLevel::Auto,
        ThinkingLevel::Off,
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
    ])
}

/// Returns the canonical two-state thinking-level set for boolean-thinking
/// providers such as Ollama.
pub(crate) fn binary_thinking_levels() -> Vec<ThinkingLevel> {
    normalize_levels([ThinkingLevel::Auto, ThinkingLevel::Off])
}

/// Returns the thinking levels supported by Anthropic models known to expose
/// extended thinking.
pub(crate) fn anthropic_thinking_levels_for_model(model_id: &str) -> Vec<ThinkingLevel> {
    let model_id = model_id.to_ascii_lowercase();
    if model_id.contains("claude-sonnet-4")
        || model_id.contains("claude-opus-4")
        || model_id.contains("mythos")
    {
        full_reasoning_levels()
    } else {
        Vec::new()
    }
}

/// Returns the thinking levels supported by OpenAI-compatible reasoning models.
pub(crate) fn openai_thinking_levels_for_model(model_id: &str) -> Vec<ThinkingLevel> {
    let model_id = model_id.to_ascii_lowercase();
    if model_id.contains("gpt-5") || model_id.contains("o1") || model_id.contains("o3") {
        full_reasoning_levels()
    } else {
        Vec::new()
    }
}

/// Returns the thinking levels supported by Gemini models with configurable
/// thinking.
pub(crate) fn gemini_thinking_levels_for_model(model_id: &str) -> Vec<ThinkingLevel> {
    let model_id = model_id.to_ascii_lowercase();
    if model_id.contains("gemini-2.5")
        || model_id.contains("gemini-3")
        || model_id.contains("gemini-1.5-pro")
    {
        full_reasoning_levels()
    } else {
        Vec::new()
    }
}

/// Returns `true` when an Ollama-family model name strongly suggests binary
/// thinking support.
pub(crate) fn model_supports_binary_thinking(model_id: &str) -> bool {
    let model_id = model_id.to_ascii_lowercase();
    model_id.contains("deepseek-r1")
        || model_id.contains("qwen3")
        || model_id.contains("qwq")
        || model_id.contains("reasoner")
}

/// Returns the thinking levels supported by an Ollama-family model.
pub(crate) fn binary_thinking_levels_for_model(model_id: &str) -> Vec<ThinkingLevel> {
    if model_supports_binary_thinking(model_id) {
        binary_thinking_levels()
    } else {
        Vec::new()
    }
}

fn parse_legacy_thinking_option(options: &HashMap<String, Value>) -> Option<ThinkingConfig> {
    let thinking = options
        .get("thinking")?
        .as_str()?
        .trim()
        .to_ascii_lowercase();

    match thinking.as_str() {
        "disabled" | "off" | "none" => Some(ThinkingConfig::off()),
        "enabled" | "adaptive" | "auto" => Some(ThinkingConfig {
            enabled: true,
            level: ThinkingLevel::Auto,
            budget_tokens: options
                .get("thinking_budget_tokens")
                .and_then(Value::as_u64)
                .and_then(|value| u32::try_from(value).ok()),
            display: None,
        }),
        "low" => Some(ThinkingConfig::new(ThinkingLevel::Low)),
        "medium" => Some(ThinkingConfig::new(ThinkingLevel::Medium)),
        "high" => Some(ThinkingConfig::new(ThinkingLevel::High)),
        _ => None,
    }
}

fn normalize_reasoning_effort(raw: &str) -> Option<&'static str> {
    match raw.trim().to_ascii_lowercase().as_str() {
        "low" => Some("low"),
        "medium" => Some("medium"),
        "high" => Some("high"),
        "none" | "off" => Some("none"),
        _ => None,
    }
}

fn map_openai_reasoning_effort(thinking: &ThinkingConfig) -> Option<&'static str> {
    if !thinking.is_effective_enabled() {
        return Some("none");
    }

    match thinking.level {
        ThinkingLevel::Auto => None,
        ThinkingLevel::Off => Some("none"),
        ThinkingLevel::Low => Some("low"),
        ThinkingLevel::Medium => Some("medium"),
        ThinkingLevel::High => Some("high"),
    }
}

/// Resolves an OpenAI-style `reasoning_effort` value from a typed request or
/// legacy options.
pub(crate) fn reasoning_effort_from_request(request: &ChatRequest) -> Option<&'static str> {
    request
        .thinking
        .as_ref()
        .and_then(map_openai_reasoning_effort)
        .or_else(|| {
            request
                .options
                .get("reasoning_effort")
                .or_else(|| request.options.get("reasoning_level"))
                .and_then(Value::as_str)
                .and_then(normalize_reasoning_effort)
        })
        .or_else(|| {
            parse_legacy_thinking_option(&request.options)
                .as_ref()
                .and_then(map_openai_reasoning_effort)
        })
}

/// Resolves Copilot-discovered reasoning effort values into user-facing
/// thinking levels.
pub(crate) fn reasoning_levels_from_supported_efforts(
    efforts: Option<&[String]>,
) -> Vec<ThinkingLevel> {
    let Some(efforts) = efforts else {
        return Vec::new();
    };

    if efforts.is_empty() {
        return Vec::new();
    }

    let mut levels = vec![ThinkingLevel::Auto];
    for effort in efforts {
        match normalize_reasoning_effort(effort) {
            Some("none") => levels.push(ThinkingLevel::Off),
            Some("low") => levels.push(ThinkingLevel::Low),
            Some("medium") => levels.push(ThinkingLevel::Medium),
            Some("high") => levels.push(ThinkingLevel::High),
            _ => {}
        }
    }
    normalize_levels(levels)
}

/// Builds Anthropic's `thinking` payload from a typed request or legacy
/// options.
pub(crate) fn anthropic_thinking_payload_from_request(request: &ChatRequest) -> Option<Value> {
    let thinking = request
        .thinking
        .as_ref()
        .cloned()
        .or_else(|| parse_legacy_thinking_option(&request.options))?;

    if matches!(thinking.display, Some(ThinkingDisplay::Omitted))
        || !thinking.is_effective_enabled()
    {
        return Some(json!({ "type": "disabled" }));
    }

    if let Some(budget_tokens) = thinking.budget_tokens {
        return Some(json!({
            "type": "enabled",
            "budget_tokens": budget_tokens,
        }));
    }

    let mut payload = json!({
        "type": "adaptive",
    });

    if let Some(effort) = match thinking.level {
        ThinkingLevel::Low => Some("low"),
        ThinkingLevel::Medium => Some("medium"),
        ThinkingLevel::High => Some("high"),
        ThinkingLevel::Auto | ThinkingLevel::Off => None,
    } {
        payload["effort"] = json!(effort);
    }

    Some(payload)
}

/// Returns `true` when the request asks Anthropic for a summarized thinking
/// display mode that the current adapter cannot faithfully encode.
pub(crate) fn request_uses_unsupported_anthropic_display(request: &ChatRequest) -> bool {
    request
        .thinking
        .as_ref()
        .is_some_and(|thinking| matches!(thinking.display, Some(ThinkingDisplay::Summarized)))
}

/// Builds Gemini's `thinkingConfig` payload from a typed request or legacy
/// options.
pub(crate) fn gemini_thinking_config_from_request(request: &ChatRequest) -> Option<Value> {
    let thinking = request
        .thinking
        .as_ref()
        .cloned()
        .or_else(|| parse_legacy_thinking_option(&request.options));

    let include_thoughts = request
        .options
        .get("include_thoughts")
        .and_then(Value::as_bool);

    let Some(thinking) = thinking else {
        return include_thoughts.map(|include_thoughts| {
            json!({
                "includeThoughts": include_thoughts,
            })
        });
    };

    let thinking_level = if matches!(thinking.display, Some(ThinkingDisplay::Omitted))
        || !thinking.is_effective_enabled()
    {
        "minimal"
    } else {
        match thinking.level {
            ThinkingLevel::Auto => "auto",
            ThinkingLevel::Off => "minimal",
            ThinkingLevel::Low => "low",
            ThinkingLevel::Medium => "medium",
            ThinkingLevel::High => "high",
        }
    };

    let include_thoughts = include_thoughts.unwrap_or(
        !matches!(thinking.display, Some(ThinkingDisplay::Omitted))
            && thinking.is_effective_enabled(),
    );

    Some(json!({
        "thinkingLevel": thinking_level,
        "includeThoughts": include_thoughts,
    }))
}

/// Resolves Ollama-style `think` state from a typed request or legacy options.
pub(crate) fn think_flag_from_request(request: &ChatRequest) -> Option<bool> {
    request
        .thinking
        .as_ref()
        .map(ThinkingConfig::is_effective_enabled)
        .or_else(|| {
            parse_legacy_thinking_option(&request.options)
                .as_ref()
                .map(ThinkingConfig::is_effective_enabled)
        })
}

/// Returns `true` when HuggingFace should warn that a thinking request will be
/// ignored because the provider has no standard parameter.
pub(crate) fn should_warn_unsupported_thinking(request: &ChatRequest) -> bool {
    request.thinking.as_ref().map_or_else(
        || {
            parse_legacy_thinking_option(&request.options)
                .is_some_and(|thinking| thinking.is_effective_enabled())
        },
        ThinkingConfig::is_effective_enabled,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::llm::{ChatContent, ChatMessage};

    fn make_request() -> ChatRequest {
        ChatRequest {
            model: "test-model".to_string(),
            messages: vec![ChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("hello".to_string()),
            }],
            tools: vec![],
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
        assert!(model_supports_binary_thinking("deepseek-r1:latest"));
        assert!(model_supports_binary_thinking("qwen3:30b"));
        assert!(!model_supports_binary_thinking("llama3.2"));
    }
}
