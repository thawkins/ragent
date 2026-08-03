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
///     "threshold": 0.8,
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
    /// Fraction of the context window at which to trigger compaction
    /// (0.0–1.0). Default: `None`.
    ///
    /// When set (e.g. `0.8` = 80%), compaction fires once the effective request
    /// token count reaches `context_window * threshold`. When `None`, the
    /// buffer-based model is used instead: compaction fires once tokens exceed
    /// `context_window - max(output_tokens, buffer)` (the SPEC FR-003 default).
    /// The legacy `compression.auto_threshold` value (e.g. `0.8`) is migrated
    /// into this field so existing configurations keep their configured trigger
    /// point instead of falling back to the buffer model.
    pub threshold: Option<f64>,
    /// Token buffer reserved for the model's response and safety margin.
    ///
    /// Only used when `threshold` is `None`. Compaction then triggers when
    /// estimated request tokens exceed `context_window - max(output_tokens,
    /// buffer)`. Default: `20_000`.
    pub buffer: usize,
    /// Recent conversation turns to keep verbatim after compaction.
    pub keep: KeepConfig,
}

impl Default for CompactionConfig {
    fn default() -> Self {
        Self {
            auto: true,
            threshold: None,
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
/// to `compaction.auto`, and from `compression.auto_threshold` to
/// `compaction.threshold`. If a loaded config still contains the legacy
/// `compression` key, treat `compression.enabled` as `compaction.auto` and
/// `compression.auto_threshold` as `compaction.threshold`.
///
/// # Arguments
///
/// * `legacy` — the legacy [`LegacyCompressionConfig`] value, if any.
/// * `current` — the current [`CompactionConfig`] value parsed from `compaction`.
///
/// # Returns
///
/// A `CompactionConfig` with `auto` set from the legacy `enabled` field and
/// `threshold` set from the legacy `auto_threshold` field when they were
/// provided and the new `compaction` section did not override them. All other
/// fields come from `current`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct LegacyCompressionConfig {
    /// Deprecated field mapped to `CompactionConfig::auto`.
    pub enabled: Option<bool>,
    /// Deprecated field mapped to `CompactionConfig::threshold` (fraction of the
    /// context window, e.g. `0.8` = 80%). The Headroom pipeline's `auto_threshold`.
    pub auto_threshold: Option<f64>,
}

/// Merge a legacy `compression` section into a new [`CompactionConfig`].
///
/// Maps `compression.enabled` → `auto` and `compression.auto_threshold` →
/// `threshold`. Explicit values in the new `compaction` section take
/// precedence; the legacy values only fill fields that the new section left at
/// their defaults.
#[must_use]
pub const fn apply_legacy_compression_alias(
    current: CompactionConfig,
    legacy: &LegacyCompressionConfig,
) -> CompactionConfig {
    let mut merged = current;
    if let Some(enabled) = legacy.enabled {
        merged.auto = enabled;
    }
    if let Some(threshold) = legacy.auto_threshold
        && merged.threshold.is_none()
    {
        merged.threshold = Some(threshold);
    }
    merged
}
