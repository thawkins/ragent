//! Plugin system configuration (spec `plugins` T-001; spec `pluginstores` T-001).
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
//!     },
//!     "stores": {                      // plugin store browser endpoints
//!       "codex":  { "url": "https://example.org/codex/index.json" },
//!       "claude": { "url": "https://example.org/claude/index.json" },
//!       "timeout_ms": 10000,
//!       "max_index_bytes": 2097152,
//!       "cache_ttl_secs": 3600
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
    #[serde(default = "crate::config::default_true")]
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
    /// Plugin store browser settings (spec `pluginstores` FR-019). When `None`
    /// the browser uses the compiled default endpoints and budgets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stores: Option<PluginStoresConfig>,
}

/// Plugin store browser configuration (spec `pluginstores` T-001; FR-019).
///
/// Endpoints are HTTPS store-index URLs; each is optional so a store can keep
/// the compiled default while the other is overridden. The shared budgets
/// (`timeout_ms`, `max_index_bytes`, `cache_ttl_secs`) apply to both stores.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PluginStoresConfig {
    /// Codex store endpoint override (`https://` index URL). When `None`, the
    /// compiled default endpoint is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub codex: Option<PluginStoreEndpoint>,
    /// Claude store endpoint override (`https://` index URL). When `None`, the
    /// compiled default endpoint is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub claude: Option<PluginStoreEndpoint>,
    /// Additional named marketplaces for the Codex and Claude stores (spec
    /// `pluginstores` FR-048). Each entry extends the store it names, so the
    /// browse panel lists the union of the compiled default and every added
    /// marketplace. Written by `/plugins marketplace <store> add <source>` and
    /// removed by `/plugins marketplace <store> remove <name>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marketplaces: Vec<MarketplaceEntry>,
    /// Per-fetch wall-clock budget in milliseconds. Default: 10000.
    #[serde(default = "crate::config::default_store_timeout_ms")]
    pub timeout_ms: u64,
    /// Maximum accepted store-index size in bytes; a larger index aborts the
    /// fetch. Default: 2097152 (2 MiB).
    #[serde(default = "crate::config::default_max_index_bytes")]
    pub max_index_bytes: u64,
    /// Time-to-live for a cached store index in seconds; `0` disables the
    /// index cache. Default: 3600.
    #[serde(default = "crate::config::default_cache_ttl_secs")]
    pub cache_ttl_secs: u64,
}

/// One named plugin marketplace attached to a store (spec `pluginstores`
/// FR-048).
///
/// `store` names which store the marketplace extends (`codex` or `claude`);
/// `source` is the user-typed source in any of the accepted forms (`owner/repo`,
/// `git@github.com:owner/repo.git`, `https://.../marketplace.json`, or a local
/// directory holding a marketplace manifest). `name` is the resolved display
/// name (used by `remove`) and `url` is the resolved `https` marketplace URL,
/// recorded at add time so the panel does not have to re-resolve the source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplaceEntry {
    /// Which store the marketplace extends (`codex` or `claude`).
    pub store: String,
    /// The display name (used by `/plugins marketplace <store> remove <name>`).
    pub name: String,
    /// The marketplace index `https://` URL, resolved at add time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The source string exactly as typed by the user (informational).
    pub source: String,
}

impl Default for PluginStoresConfig {
    fn default() -> Self {
        Self {
            codex: None,
            claude: None,
            marketplaces: Vec::new(),
            timeout_ms: crate::config::default_store_timeout_ms(),
            max_index_bytes: crate::config::default_max_index_bytes(),
            cache_ttl_secs: crate::config::default_cache_ttl_secs(),
        }
    }
}

/// One plugin store endpoint (spec `pluginstores` T-001; FR-019, FR-024).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct PluginStoreEndpoint {
    /// The store-index `https://` URL. A non-`https` value is refused when the
    /// browser is opened (FR-024).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

impl Default for PluginsConfig {
    fn default() -> Self {
        Self {
            enabled: crate::config::default_true(),
            max_execution_ms: default_max_execution_ms(),
            max_entry_ms: default_max_entry_ms(),
            max_memory_mb: default_max_memory_mb(),
            store_dir: None,
            permissions: BTreeMap::new(),
            stores: None,
        }
    }
}

impl PluginsConfig {
    /// Returns `true` when the plugin subsystem is enabled.
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// The effective store browser settings, falling back to the compiled
    /// defaults when the `stores` block is absent (spec `pluginstores` FR-019).
    #[must_use]
    pub fn stores_or_default(&self) -> PluginStoresConfig {
        self.stores.clone().unwrap_or_default()
    }

    /// Ensure a `stores` block is present (for mutation) and return it mutably,
    /// creating the compiled-default block when the section is absent.
    pub fn stores_mut(&mut self) -> &mut PluginStoresConfig {
        self.stores.get_or_insert_with(PluginStoresConfig::default)
    }
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
