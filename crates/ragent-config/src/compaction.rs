//! Compaction configuration types for ragent.json.
//!
//! These types define the `compaction` section of ragent's configuration file.
//! They control the OpenCode-derived summarisation-based context-window
//! compaction.
//!
//! See `specs/compact/SPEC.md` for the full requirements.

use serde::{Deserialize, Serialize};

/// Reference to a model in `provider/model` format used for the compaction
/// summarisation call.
///
/// Config-only mirror of the runtime `ModelRef` type (kept separate so
/// `ragent-config` does not depend on the agent crate).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompactionModelRef {
    /// Provider identifier (e.g. `"anthropic"`, `"ollama"`).
    pub provider_id: String,
    /// Model identifier within the provider (e.g. `"claude-haiku-4-5"`).
    pub model_id: String,
}

/// Top-level compaction configuration.
///
/// Corresponds to the `compaction` key in `ragent.json`. When `auto` is
/// `true`, the agent automatically summarises conversation history before
/// sending a request that would exceed the configured threshold or buffer.
/// When `auto` is `false`, only emergency overflow summarisation runs.
///
/// # Example
///
/// ```json
/// {
///   "compaction": {
///     "auto": true,
///     "threshold": 0.8,
///     "buffer": 0.10,
///     "keep": {
///       "tokens": 0.20
///     },
///     "model": { "provider_id": "ollama", "model_id": "qwen2.5:1.5b" },
///     "summary_tokens": 1500,
///     "tool_output_max_chars": 1200
///   }
/// }
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct CompactionConfig {
    /// Whether automatic pre-send compaction is enabled.
    ///
    /// When `true`, the runner checks token usage before every LLM request and
    /// summarises history when needed. When `false`, only provider context-
    /// overflow errors trigger emergency compaction. Default: `true`.
    pub auto: bool,
    /// Fraction of the context window at which to trigger compaction
    /// (0.0–1.0). Default: `0.7` (70 %).
    ///
    /// When set (e.g. `0.8` = 80%), compaction fires once the effective request
    /// token count reaches `context_window * threshold`. When `None`, the
    /// runner falls back to the buffer-based trigger described on [`buffer`].
    ///
    /// The trigger threshold is raised to at least
    /// [`MIN_COMPACTION_THRESHOLD_FRACTION`] of the context window (70 %), so
    /// automatic pre-send compaction never runs on routine prompts that fill
    /// less than 70 % of the available context.
    pub threshold: Option<f64>,
    /// Token buffer reserved for the model's response and safety margin.
    ///
    /// Expressed as a fraction of the context window (0.0–1.0). When
    /// `threshold` is `None`, compaction triggers when estimated request
    /// tokens exceed `context_window - max(output_tokens,
    /// context_window * buffer)`. Default: `0.10` (10 %).
    pub buffer: f64,
    /// Recent conversation turns to keep verbatim after compaction.
    pub keep: KeepConfig,
    /// Optional model override for the compaction summarisation call.
    ///
    /// When set, compaction routes to this model instead of the session's
    /// primary model. Pointing compaction at a fast/cheap model (e.g. a small
    /// local Ollama model or a low-cost cloud tier) is the single biggest
    /// lever for reducing compaction wall-clock time. When `None`, the
    /// session's primary model is used. Default: `None`.
    pub model: Option<CompactionModelRef>,
    /// Maximum tokens to request for the compaction summary output.
    ///
    /// Default: `1500`. Generation time is the dominant compaction cost —
    /// halving this budget roughly halves worst-case latency. The structured
    /// summary template (Objective / Details / Work State / Next Move /
    /// Relevant Files) fits comfortably in ~1500 tokens.
    pub summary_tokens: Option<usize>,
    /// Truncation limit in characters for individual tool outputs that are
    /// serialised into the compaction prompt.
    ///
    /// Default: `2000` (matches OpenCode's `TOOL_OUTPUT_MAX_CHARS`).
    pub tool_output_max_chars: Option<usize>,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            auto: true,
            threshold: Some(0.7),
            buffer: 0.10,
            keep: KeepConfig::default(),
            model: None,
            summary_tokens: None,
            tool_output_max_chars: None,
        }
    }
}

impl CompactionConfig {
    /// Default maximum tokens to request for a compaction summary when the
    /// user has not configured `summary_tokens`.
    pub const DEFAULT_SUMMARY_OUTPUT_TOKENS: usize = 1_500;
    /// Default tool-output truncation limit in characters when the user has
    /// not configured `tool_output_max_chars`. Matches OpenCode's
    /// `TOOL_OUTPUT_MAX_CHARS`.
    pub const DEFAULT_TOOL_OUTPUT_MAX_CHARS: usize = 2_000;

    /// Return the maximum fraction of the context window to preserve verbatim.
    ///
    /// The compaction runner multiplies this by the model's context window to
    /// obtain the absolute token budget for the recent-turn tail.
    #[must_use]
    pub fn keep_fraction(&self) -> f64 {
        self.keep.tokens.unwrap_or(0.20)
    }

    /// Return the maximum number of tokens to request for a compaction summary.
    ///
    /// Defaults to [`Self::DEFAULT_SUMMARY_OUTPUT_TOKENS`] when
    /// `summary_tokens` is not configured.
    #[must_use]
    pub fn summary_output_tokens(&self) -> usize {
        self.summary_tokens
            .unwrap_or(Self::DEFAULT_SUMMARY_OUTPUT_TOKENS)
    }

    /// Return the tool-output truncation limit in characters.
    ///
    /// Defaults to [`Self::DEFAULT_TOOL_OUTPUT_MAX_CHARS`] when
    /// `tool_output_max_chars` is not configured.
    #[must_use]
    pub fn tool_output_max_chars(&self) -> usize {
        self.tool_output_max_chars
            .unwrap_or(Self::DEFAULT_TOOL_OUTPUT_MAX_CHARS)
    }
}

/// Recent-turn retention configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeepConfig {
    /// Fraction of the context window reserved verbatim for recent turns
    /// (0.0–1.0). Default: `0.20` (20 %).
    pub tokens: Option<f64>,
}

impl Default for KeepConfig {
    fn default() -> Self {
        Self { tokens: Some(0.20) }
    }
}
