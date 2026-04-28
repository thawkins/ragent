//! Thinking and reasoning configuration types for the ragent system.
//!
//! Defines a provider-agnostic thinking level enum, a config struct with
//! support for Anthropic-style budget tokens and display modes, and
//! serialization helpers for backward-compatible config loading.

use serde::{Deserialize, Serialize};

/// Provider-agnostic thinking/reasoning effort level.
///
/// These values abstract across all providers' native parameters:
/// - Anthropic: `thinking.type` + `effort` / `budget_tokens`
/// - OpenAI / Copilot: `reasoning_effort` (`"low"`, `"medium"`, `"high"`, `"none"`)
/// - Gemini: `thinkingConfig.thinkingLevel` (`"minimal"`, `"low"`, `"medium"`, `"high"`, `"auto"`)
/// - Ollama: `think` boolean
///
/// See the provider-specific adapter implementations for the exact mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingLevel {
    /// Let the model/provider decide the appropriate thinking depth.
    Auto,
    /// No reasoning / thinking — standard chat completion.
    Off,
    /// Low reasoning effort (fast, minimal chain-of-thought).
    Low,
    /// Medium reasoning effort (balanced depth and speed).
    Medium,
    /// High reasoning effort (deep chain-of-thought, best for complex tasks).
    High,
}

impl ThinkingLevel {
    /// Returns `true` if this level enables any form of thinking/reasoning.
    pub fn is_enabled(self) -> bool {
        !matches!(self, ThinkingLevel::Off)
    }
}

impl Default for ThinkingLevel {
    fn default() -> Self {
        Self::Auto
    }
}

/// Configuration for model thinking/reasoning behaviour.
///
/// Carries the user's chosen thinking level along with optional
/// Anthropic-specific budget tokens and display mode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThinkingConfig {
    /// Whether thinking is enabled. When `false`, `level` is ignored
    /// and the model should produce a standard (non-reasoning) response.
    #[serde(default = "default_thinking_enabled")]
    pub enabled: bool,
    /// The thinking depth level. Ignored when `enabled` is `false`.
    #[serde(default)]
    pub level: ThinkingLevel,
    /// Maximum tokens the model may use for thinking (Anthropic `budget_tokens`).
    /// `None` = use the provider's default budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub budget_tokens: Option<u32>,
    /// Controls how thinking content is surfaced in the response.
    /// (Anthropic `thinking.type` display mode).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub display: Option<ThinkingDisplay>,
}

const fn default_thinking_enabled() -> bool {
    true
}

impl Default for ThinkingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            level: ThinkingLevel::Auto,
            budget_tokens: None,
            display: None,
        }
    }
}

impl ThinkingConfig {
    /// Create a `ThinkingConfig` with the given level and default settings.
    pub fn new(level: ThinkingLevel) -> Self {
        Self {
            enabled: level.is_enabled(),
            level,
            budget_tokens: None,
            display: None,
        }
    }

    /// Create an "off" config — thinking disabled.
    pub fn off() -> Self {
        Self {
            enabled: false,
            level: ThinkingLevel::Off,
            budget_tokens: None,
            display: None,
        }
    }

    /// Returns `true` if thinking is effectively enabled.
    pub fn is_effective_enabled(&self) -> bool {
        self.enabled && self.level.is_enabled()
    }
}

/// Controls how thinking/reasoning content is surfaced in the model response.
///
/// Maps to Anthropic's `thinking.type` field:
/// - `Full` → `"enabled"` — include full thinking content in the response stream.
/// - `Summarized` → future use (not yet supported by Anthropic).
/// - `Omitted` → `"disabled"` — no thinking content in response.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThinkingDisplay {
    /// Return the full thinking content in the response.
    Full,
    /// Return a summarized version of the thinking content.
    Summarized,
    /// Omit thinking content from the response entirely.
    Omitted,
}
