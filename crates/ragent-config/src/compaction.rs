//! Compaction configuration types for ragent.json.
//!
//! These types define the `compaction` section of ragent's configuration file.
//! They control the OpenCode-derived summarisation-based context-window
//! compaction.
//!
//! See `specs/compact/SPEC.md` for the full requirements.

use serde::{Deserialize, Serialize};

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
///     }
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
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            auto: true,
            threshold: Some(0.7),
            buffer: 0.10,
            keep: KeepConfig::default(),
        }
    }
}

impl CompactionConfig {
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
    /// Fixed at `4_096` to match `OpenCode`'s `SUMMARY_OUTPUT_TOKENS` default.
    #[must_use]
    pub const fn summary_output_tokens(&self) -> usize {
        4_096
    }

    /// Return the tool-output truncation limit in characters.
    ///
    /// Long tool outputs are truncated before being serialised into the
    /// compaction prompt. Fixed at `2_000` characters to match `OpenCode`.
    #[must_use]
    pub const fn tool_output_max_chars(&self) -> usize {
        2_000
    }
}

/// Configuration for the verbatim "tail" kept after compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeepConfig {
    /// Maximum number of tokens from recent turns to preserve verbatim.
    ///
    /// Expressed as a fraction of the model's context window (0.0–1.0). The
    /// runner computes the absolute token budget as
    /// `context_window * keep.tokens`. Default: `0.20` (20 %).
    pub tokens: Option<f64>,
}

impl Default for KeepConfig {
    fn default() -> Self {
        Self { tokens: Some(0.20) }
    }
}
