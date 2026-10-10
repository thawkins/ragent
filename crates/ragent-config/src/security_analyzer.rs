//! LLM security-analyzer configuration (spec `openhands` FR-006, FR-016,
//! FR-017).
//!
//! The security analyzer is an opt-in permission mode: instead of deciding a
//! proposed tool action from the static permission rules alone, ragent asks a
//! model to return an `allow` / `ask` / `deny` verdict plus a rationale. The
//! verdict and rationale are published *before* the action is permitted or
//! refused (FR-017), so the user can see why a call was allowed, denied, or
//! escalated to an interactive prompt.
//!
//! ## Fail-safe design
//!
//! The analyzer is advisory and **tightening-only**:
//!
//! - an analyzer `deny` is honoured as a hard denial;
//! - an analyzer `allow` can satisfy a bare `Ask` (no explicit policy rule) but
//!   never overrides an explicit policy `Deny`;
//! - any analyzer failure (missing model, provider error, unparseable output,
//!   timeout) degrades to `ask` - the normal interactive flow - and never
//!   silently broadens access.
//!
//! `pre_tool_use` hook exit-code semantics are unchanged (FR-016): exit code 2
//! blocks and returns the hook stderr as the tool result, exit code 1 warns and
//! continues.

use serde::{Deserialize, Serialize};

/// The `security_analyzer` section of `ragent.json` (FR-006).
///
/// ```jsonc
/// {
///   "security_analyzer": {
///     "enabled": true,
///     "model": { "provider_id": "ollama", "model_id": "qwen2.5:7b" },
///     "timeout_secs": 20
///   }
/// }
/// ```
///
/// Opt-in: absent from the default config, so the static permission rules decide
/// alone and the analyzer makes no LLM call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct SecurityAnalyzerConfig {
    /// Whether the analyzer evaluates proposed tool actions.
    #[serde(default)]
    pub enabled: bool,
    /// Optional model override for the analyzer call.
    ///
    /// When set, the analyzer routes to this model instead of the session's
    /// primary model. Pointing the analyzer at a fast/cheap model keeps the
    /// per-action latency low. When `None`, the session model is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<AnalyzerModelRef>,
    /// Wall-clock budget in seconds for one analyzer call.
    ///
    /// A verdict that does not arrive within the budget degrades to `ask` (the
    /// safe interactive default). Default: `20`.
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
}

/// Reference to a model in `provider/model` form used for the analyzer call.
///
/// Config-only mirror of the runtime `ModelRef` type (kept separate so
/// `ragent-config` does not depend on the agent crate), matching
/// [`CompactionModelRef`](crate::compaction::CompactionModelRef).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalyzerModelRef {
    /// Provider identifier (e.g. `"anthropic"`, `"ollama"`).
    pub provider_id: String,
    /// Model identifier within the provider.
    pub model_id: String,
}

/// Serde default for [`SecurityAnalyzerConfig::timeout_secs`].
fn default_timeout_secs() -> u64 {
    20
}

impl Default for SecurityAnalyzerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: None,
            timeout_secs: default_timeout_secs(),
        }
    }
}

impl SecurityAnalyzerConfig {
    /// `true` when the section carries no deviation from the disabled default.
    ///
    /// Used by config-consistency checks; the section is stored as an
    /// `Option` and omitted entirely when absent, so a disabled analyzer is
    /// normally represented by `None` rather than this value.
    #[must_use]
    pub fn is_default(&self) -> bool {
        self == &Self::default()
    }
}

/// Persist the `security_analyzer.enabled` switch to the loaded config source
/// (FR-006).
///
/// The section is created when absent so a `/security on` persists as
/// `{ "security_analyzer": { "enabled": true } }`. Other fields (model, timeout)
/// are preserved.
///
/// # Errors
///
/// Propagates config load and write failures rather than silently discarding
/// the change.
pub fn persist(enabled: bool) -> anyhow::Result<()> {
    let mut config = crate::config::Config::load()?;
    config
        .security_analyzer
        .get_or_insert_with(SecurityAnalyzerConfig::default)
        .enabled = enabled;
    config.save_to_source()?;
    Ok(())
}
