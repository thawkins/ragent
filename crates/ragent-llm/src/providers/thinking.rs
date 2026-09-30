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
pub fn full_reasoning_levels() -> Vec<ThinkingLevel> {
    normalize_levels([
        ThinkingLevel::Auto,
        ThinkingLevel::Off,
        ThinkingLevel::Low,
        ThinkingLevel::Medium,
        ThinkingLevel::High,
    ])
}

/// Returns the canonical thinking-level set exposed for Ollama-family models.
///
/// Ollama is a boolean thinker: the wire parameter is simply `think: true` or
/// `think: false`. We still present the full user-facing effort range
/// (`Auto`/`Off`/`Low`/`Medium`/`High`) because model-name detection of
/// thinking support is unreliable and users want to pick a level. Any non-`Off`
/// level is mapped to `think: true` at request time.
pub fn binary_thinking_levels() -> Vec<ThinkingLevel> {
    full_reasoning_levels()
}

/// Returns the thinking levels supported by Anthropic models known to expose
/// extended thinking.
pub fn anthropic_thinking_levels_for_model(model_id: &str) -> Vec<ThinkingLevel> {
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
pub fn openai_thinking_levels_for_model(model_id: &str) -> Vec<ThinkingLevel> {
    let model_id = model_id.to_ascii_lowercase();
    if model_id.contains("gpt-5") || model_id.contains("o1") || model_id.contains("o3") {
        full_reasoning_levels()
    } else {
        Vec::new()
    }
}

/// Returns the thinking levels supported by Gemini models with configurable
/// thinking.
pub fn gemini_thinking_levels_for_model(model_id: &str) -> Vec<ThinkingLevel> {
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
///
/// Ollama models that support the `think` parameter include those with
/// `{{--think}}` tags in their Modelfile template. Since we cannot inspect
/// the template at discovery time, we match known model name patterns.
pub fn model_supports_binary_thinking(model_id: &str) -> bool {
    let model_id = model_id.to_ascii_lowercase();
    model_id.contains("deepseek-r1")
        || model_id.contains("qwen3")
        || model_id.contains("qwq")
        || model_id.contains("reasoner")
        || model_id.contains("kimi")
        || model_id.contains("gemma3")
        || model_id.contains("phi4-reasoning")
        || model_id.contains("magistral")
        || model_id.contains("mistral-small3")
}

/// Returns the thinking levels supported by an Ollama-family model.
pub fn binary_thinking_levels_for_model(model_id: &str) -> Vec<ThinkingLevel> {
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

/// Builds OpenRouter's `reasoning` payload from a typed request or legacy
/// options.
///
/// OpenRouter's native reasoning control (used by models such as
/// `anthropic/claude-sonnet-4` and `openai/o3-mini` when routed through it) is
/// a `reasoning` object with two optional fields:
///
/// - `effort`: `"low"`, `"medium"`, `"high"`, or `"none"`
/// - `max_tokens`: explicit reasoning-budget ceiling (mirrors Anthropic
///   `budget_tokens` and is converted from `ThinkingConfig::budget_tokens`).
///
/// This table drives FR-018:
///
/// | `thinking.enabled` | `thinking.level` | `reasoning.effort` | `reasoning.max_tokens` |
/// |--------------------|------------------|--------------------|------------------------|
/// | `false` / `Off`    | any              | `"none"`           | `None`                 |
/// | `true`             | `Auto`           | omitted            | `budget_tokens`        |
/// | `true`             | `Off`            | `"none"`           | `None`                 |
/// | `true`             | `Low`            | `"low"`            | `budget_tokens`        |
/// | `true`             | `Medium`         | `"medium"`         | `budget_tokens`        |
/// | `true`             | `High`           | `"high"`           | `budget_tokens`        |
///
/// When `budget_tokens` is set it is always emitted, matching the treatment of
/// Anthropic's `thinking.budget_tokens`.  A missing budget leaves `max_tokens`
/// unset so the upstream model uses its default.
///
/// Legacy fallback keys are `reasoning_effort`, `reasoning_level`, and the
/// generic `thinking` string option.
pub fn openrouter_reasoning_payload_from_request(request: &ChatRequest) -> Option<Value> {
    let thinking = request
        .thinking
        .clone()
        .or_else(|| {
            request
                .options
                .get("reasoning_effort")
                .or_else(|| request.options.get("reasoning_level"))
                .and_then(Value::as_str)
                .and_then(|raw| {
                    normalize_reasoning_effort(raw).and_then(|effort| match effort {
                        "none" | "off" => Some(ThinkingConfig::off()),
                        "low" => Some(ThinkingConfig::new(ThinkingLevel::Low)),
                        "medium" => Some(ThinkingConfig::new(ThinkingLevel::Medium)),
                        "high" => Some(ThinkingConfig::new(ThinkingLevel::High)),
                        _ => None,
                    })
                })
        })
        .or_else(|| parse_legacy_thinking_option(&request.options))?;

    // If the user explicitly disabled thinking, emit a clean {"effort": "none"}
    // so the model skips the reasoning pass instead of using its default.
    if !thinking.is_effective_enabled() {
        return Some(json!({ "effort": "none" }));
    }

    let effort = core_level_effort(thinking.level);
    let mut payload = json!({});
    if let Some(effort) = effort {
        payload["effort"] = json!(effort);
    }
    if let Some(budget_tokens) = thinking.budget_tokens {
        payload["max_tokens"] = json!(budget_tokens);
    }

    if payload.as_object().is_some_and(|o| !o.is_empty()) {
        Some(payload)
    } else {
        // Auto with no budget means "let the model decide"; omit the reasoning
        // object entirely to avoid constraining the upstream provider.
        None
    }
}

#[cfg(test)]
#[path = "../tests/inline/thinking_openrouter_reasoning_tests.rs"]
mod openrouter_reasoning_tests;

/// Maps the `Low`/`Medium`/`High` levels to their shared effort string.
///
/// Every provider payload table (OpenAI `reasoning_effort`, Anthropic
/// `effort`, Gemini `thinkingLevel`) uses the same three strings; keep them
/// single-sourced so adding a level cannot drift between providers.
fn core_level_effort(level: ThinkingLevel) -> Option<&'static str> {
    match level {
        ThinkingLevel::Low => Some("low"),
        ThinkingLevel::Medium => Some("medium"),
        ThinkingLevel::High => Some("high"),
        ThinkingLevel::Auto | ThinkingLevel::Off => None,
    }
}

fn map_openai_reasoning_effort(thinking: &ThinkingConfig) -> Option<&'static str> {
    if !thinking.is_effective_enabled() {
        return Some("none");
    }

    match thinking.level {
        ThinkingLevel::Auto => None,
        ThinkingLevel::Off => Some("none"),
        level => core_level_effort(level),
    }
}

/// Resolves an OpenAI-style `reasoning_effort` value from a typed request or
/// legacy options.
pub fn reasoning_effort_from_request(request: &ChatRequest) -> Option<&'static str> {
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
pub fn reasoning_levels_from_supported_efforts(efforts: Option<&[String]>) -> Vec<ThinkingLevel> {
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
pub fn anthropic_thinking_payload_from_request(request: &ChatRequest) -> Option<Value> {
    let thinking = request
        .thinking
        .clone()
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

    if let Some(effort) = core_level_effort(thinking.level) {
        payload["effort"] = json!(effort);
    }

    Some(payload)
}

/// Returns `true` when the request asks Anthropic for a summarized thinking
/// display mode that the current adapter cannot faithfully encode.
pub fn request_uses_unsupported_anthropic_display(request: &ChatRequest) -> bool {
    request
        .thinking
        .as_ref()
        .is_some_and(|thinking| matches!(thinking.display, Some(ThinkingDisplay::Summarized)))
}

/// Builds Gemini's `thinkingConfig` payload from a typed request or legacy
/// options.
pub fn gemini_thinking_config_from_request(request: &ChatRequest) -> Option<Value> {
    let thinking = request
        .thinking
        .clone()
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
            level => core_level_effort(level).unwrap_or("minimal"),
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
pub fn think_flag_from_request(request: &ChatRequest) -> Option<bool> {
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
pub fn should_warn_unsupported_thinking(request: &ChatRequest) -> bool {
    request.thinking.as_ref().map_or_else(
        || {
            parse_legacy_thinking_option(&request.options)
                .is_some_and(|thinking| thinking.is_effective_enabled())
        },
        ThinkingConfig::is_effective_enabled,
    )
}

#[cfg(test)]
#[path = "../tests/inline/thinking_tests.rs"]
mod tests;
