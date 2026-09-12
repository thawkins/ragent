//! GCF (Graph Compact Format) tool-result encoding configuration.
//!
//! These types define the `gcf` section of ragent's configuration file
//! (spec `gcf`). GCF is a token-efficient, lossless JSON encoding applied to
//! JSON-dense tool results in the LLM view when the feature is enabled.

use crate::runtime_flag::RuntimeFlag;
use serde::{Deserialize, Serialize};

/// Top-level GCF configuration.
///
/// Corresponds to the `gcf` key in `ragent.json`. The feature defaults to
/// **off**: a config file with no `gcf` section means GCF encoding is
/// disabled and no tool results are encoded (FR-001).
///
/// ```json
/// { "gcf": { "enabled": true } }
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GcfConfig {
    /// Whether GCF encoding of eligible JSON tool results is enabled.
    ///
    /// Default: `false` — tool results reach the LLM as raw JSON.
    #[serde(default)]
    pub enabled: bool,
}

impl GcfConfig {
    /// `true` when the section is at its default state (`enabled == false`).
    ///
    /// Used by the `Config` serialisation to omit the `gcf` key while the
    /// feature is disabled, so a never-toggled config file stays unchanged
    /// in intent (FR-001).
    #[must_use]
    pub fn is_default(&self) -> bool {
        !self.enabled
    }
}

static GCF_MODE: RuntimeFlag = RuntimeFlag::new("gcf", false);

/// Returns `true` if GCF encoding of tool results is currently enabled.
#[must_use]
pub fn is_enabled() -> bool {
    GCF_MODE.is_enabled()
}

/// Enable or disable GCF encoding globally (runtime flag only, not persisted).
pub fn set_enabled(enabled: bool) {
    GCF_MODE.set_enabled(enabled);
}

/// Persist the requested GCF state to the config file and update the runtime
/// flag.
///
/// Any error during persistence is returned so callers can decide how to
/// report it.
pub fn persist_gcf(enabled: bool) -> anyhow::Result<()> {
    GCF_MODE.persist(enabled)
}

/// Load the current config and update the runtime GCF flag from its value,
/// falling back to `false` (default-off, FR-001) on load failure.
pub fn sync_from_config() {
    GCF_MODE.sync_from_config(false);
}

/// Update the runtime GCF flag from an already-loaded config value, avoiding
/// a redundant disk read.
pub fn sync_from_config_value(enabled: bool) {
    GCF_MODE.set_enabled(enabled);
}

/// Toggle GCF encoding, persist the new state, and return it.
pub fn toggle_persist() -> anyhow::Result<bool> {
    GCF_MODE.toggle_persist()
}
