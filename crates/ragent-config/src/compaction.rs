//! Compaction configuration types for ragent.json.
//!
//! These types define the `compaction` section of ragent's configuration file.
//! They control the OpenCode-derived summarisation-based context-window
//! compaction that replaces the older Headroom `compression` scheme.
//!
//! See `specs/compact/SPEC.md` for the full requirements and migration notes.

use serde::{Deserialize, Serialize};

/// Top-level compaction configuration.
///
/// Corresponds to the `compaction` key in `ragent.json`. When `auto` is
/// `true`, the agent automatically summarises conversation history before
/// sending a request that would exceed the context window minus the configured
/// buffer. When `auto` is `false`, only emergency overflow summarisation runs.
///
/// # Example
///
/// ```json
/// {
///   "compaction": {
///     "auto": true,
///     "buffer": 20000,
///     "keep": {
///       "tokens": 8000
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
    /// Token buffer reserved for the model's response and safety margin.
    ///
    /// Compaction triggers when estimated request tokens exceed
    /// `context_window - max(output_tokens, buffer)`. Default: `20_000`.
    pub buffer: usize,
    /// Recent conversation turns to keep verbatim after compaction.
    pub keep: KeepConfig,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            auto: true,
            buffer: 20_000,
            keep: KeepConfig::default(),
        }
    }
}

impl CompactionConfig {
    /// Return the maximum number of recent-turn tokens to preserve verbatim.
    ///
    /// This is the value used by the `select` algorithm in the compaction
    /// runner to choose which recent messages stay in context after a summary.
    #[must_use]
    pub fn keep_tokens(&self) -> usize {
        self.keep.tokens.unwrap_or(8_000)
    }

    /// Return the maximum number of tokens to request for a compaction summary.
    ///
    /// Fixed at 4_096 to match OpenCode's `SUMMARY_OUTPUT_TOKENS` default.
    #[must_use]
    pub fn summary_output_tokens(&self) -> usize {
        4_096
    }

    /// Return the tool-output truncation limit in characters.
    ///
    /// Long tool outputs are truncated before being serialised into the
    /// compaction prompt. Fixed at 2_000 characters to match OpenCode.
    #[must_use]
    pub fn tool_output_max_chars(&self) -> usize {
        2_000
    }
}

/// Configuration for the verbatim "tail" kept after compaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct KeepConfig {
    /// Maximum number of tokens from recent turns to preserve verbatim.
    ///
    /// The runner selects recent user/assistant/tool turns in reverse
    /// chronological order until this token budget is reached. Default:
    /// `8_000`.
    pub tokens: Option<usize>,
}

impl Default for KeepConfig {
    fn default() -> Self {
        Self {
            tokens: Some(8_000),
        }
    }
}

/// Deprecated alias: old Headroom `compression` section mapped to compaction.
///
/// This helper exists to ease one-release migration from `compression.enabled`
/// to `compaction.auto`. If a loaded config still contains the legacy
/// `compression` key, treat `compression.enabled` as `compaction.auto`.
///
/// # Arguments
///
/// * `legacy` — the legacy [`LegacyCompressionConfig`] value, if any.
/// * `current` — the current [`CompactionConfig`] value parsed from `compaction`.
///
/// # Returns
///
/// A `CompactionConfig` with `auto` set from the legacy `enabled` field when the
/// new `compaction.auto` was not explicitly provided. All other fields come
/// from `current`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct LegacyCompressionConfig {
    /// Deprecated field mapped to `CompactionConfig::auto`.
    pub enabled: Option<bool>,
}

impl Default for LegacyCompressionConfig {
    fn default() -> Self {
        Self { enabled: None }
    }
}

/// Merge a legacy `compression.enabled` flag into a new `CompactionConfig`.
///
/// Only applies when the new config's `auto` equals the default (`true`).
/// This preserves explicit user overrides in the new `compaction` section.
#[must_use]
pub fn apply_legacy_compression_alias(
    current: CompactionConfig,
    legacy: &LegacyCompressionConfig,
) -> CompactionConfig {
    let mut merged = current;
    if let Some(enabled) = legacy.enabled {
        merged.auto = enabled;
    }
    merged
}
