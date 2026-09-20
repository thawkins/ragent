//! Plugin system configuration (spec `plugins` T-001).
//!
//! Loaded from the optional `"plugins"` block in `ragent.json`. All fields
//! default sensibly so the plugin subsystem works out-of-the-box without
//! explicit configuration; `enabled: false` makes the entire subsystem inert
//! (no discovery, no loading).
//!
//! ```jsonc
//! {
//!   "plugins": {
//!     "enabled": true,                 // master switch; default true
//!     "max_execution_ms": 5000,        // per-tool wall-clock budget
//!     "max_entry_ms": 10000,           // entry-point wall-clock budget
//!     "max_memory_mb": 64,             // per-context memory ceiling
//!     "store_dir": null,               // optional override of the plugin store path
//!     "permissions": {                 // optional per-plugin permission grants
//!       "codex-weather": ["network.outbound"]
//!     }
//!   }
//! }
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Plugin subsystem configuration.
///
/// All fields use `serde(default)` so a partial `plugins` block overlays the
/// compiled defaults; the whole section is omitted from serialisation when the
/// value is `None` on the parent [`Config`](crate::Config).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginsConfig {
    /// Master switch for the plugin subsystem. When `false`, no discovery or
    /// loading happens and `/plugins` subcommands other than `help` report the
    /// system is disabled. Default: `true`.
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    /// Per-plugin-tool wall-clock execution budget in milliseconds.
    /// Default: 5000 (5 seconds).
    #[serde(default = "default_max_execution_ms")]
    pub max_execution_ms: u64,
    /// Wall-clock budget for executing a plugin's entry point in milliseconds.
    /// Default: 10000 (10 seconds).
    #[serde(default = "default_max_entry_ms")]
    pub max_entry_ms: u64,
    /// Per-JavaScript-context memory ceiling in mebibytes. Default: 64.
    #[serde(default = "default_max_memory_mb")]
    pub max_memory_mb: u64,
    /// Optional override of the plugin store directory. When `None`, the store
    /// is discovered at the project `.ragent/plugins/` directory, falling back
    /// to the user-global `~/.config/ragent/plugins/` directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_dir: Option<PathBuf>,
    /// Per-plugin permission grants keyed by plugin id (for example
    /// `{"codex-weather": ["network.outbound"]}`). Defaults to empty.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub permissions: BTreeMap<String, Vec<String>>,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            max_execution_ms: default_max_execution_ms(),
            max_entry_ms: default_max_entry_ms(),
            max_memory_mb: default_max_memory_mb(),
            store_dir: None,
            permissions: BTreeMap::new(),
        }
    }
}

impl PluginsConfig {
    /// Returns `true` when the plugin subsystem is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

const fn default_enabled() -> bool {
    true
}

const fn default_max_execution_ms() -> u64 {
    5_000
}

const fn default_max_entry_ms() -> u64 {
    10_000
}

const fn default_max_memory_mb() -> u64 {
    64
}
