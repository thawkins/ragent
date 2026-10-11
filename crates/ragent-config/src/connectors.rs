//! Connector system configuration (spec `connectors` T-001; FR-007, FR-021, FR-024).
//!
//! Loaded from the optional `"connectors"` block in `ragent.json`, mirroring the
//! plugin subsystem's `"plugins"` block (spec `plugins` T-001). All fields
//! default sensibly so the connector subsystem works out-of-the-box without
//! explicit configuration; `enabled: false` makes the entire subsystem inert
//! (no discovery, no catalogue fetch, no connection) and every `/connectors`
//! subcommand other than `help` reports that the system is disabled (FR-021).
//!
//! ```jsonc
//! {
//!   "connectors": {
//!     "enabled": true,                 // master switch; default true
//!     "store_dir": null,               // optional override of the connector store path
//!     "stores": {                      // catalogue endpoints
//!       "community": { "url": "https://example.org/connectors/index.json" },
//!       "timeout_ms": 10000,
//!       "max_index_bytes": 2097152,
//!       "cache_ttl_secs": 3600
//!     },
//!     "credentials": {                 // non-secret credential-name mapping
//!       "google-drive": { "token": "GDRIVE_TOKEN" }
//!     }
//!   }
//! }
//! ```

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

/// Connector subsystem configuration (spec `connectors` FR-007, FR-021, FR-024).
///
/// All fields use `serde(default)` so a partial `connectors` block overlays the
/// compiled defaults; the whole section is omitted from serialisation when the
/// value is `None` on the parent [`Config`](crate::Config).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectorsConfig {
    /// Master switch for the connector subsystem. When `false`, no discovery,
    /// catalogue fetch, or connection happens, and `/connectors` subcommands
    /// other than `help` report the system is disabled (FR-021). Default: `true`.
    #[serde(default = "crate::config::default_true")]
    pub enabled: bool,
    /// Optional override of the connector store directory (FR-007). When `None`,
    /// the store is discovered at the project `.ragent/connectors/` directory,
    /// falling back to the user-global `~/.config/ragent/connectors/` directory.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub store_dir: Option<PathBuf>,
    /// Catalogue endpoints and shared fetch budgets (FR-024). When `None` the
    /// browsers use the compiled default endpoints and budgets.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stores: Option<ConnectorStoresConfig>,
    /// Non-secret credential-name mapping keyed by connector id (FR-007). Each
    /// value holds the *name* of a credential in the encrypted credential store,
    /// never the secret value itself.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub credentials: BTreeMap<String, ConnectorCredentialsConfig>,
}

/// Connector catalogue configuration (spec `connectors` FR-024).
///
/// Named catalogue endpoints are collected in [`catalogues`](Self::catalogues);
/// each is optional so a catalogue can keep the compiled default while another
/// is overridden (FR-024). The shared budgets (`timeout_ms`, `max_index_bytes`,
/// `cache_ttl_secs`) apply to every catalogue.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectorStoresConfig {
    /// Named catalogue endpoints keyed by catalogue name (for example `claude`
    /// or `community`). An entry whose `url` is absent or empty falls back to
    /// the compiled default for that catalogue (FR-024, FR-035).
    #[serde(default, flatten)]
    pub catalogues: BTreeMap<String, ConnectorStoreEndpoint>,
    /// Additional named marketplaces for the catalogues (spec `connectors`
    /// FR-043). Each entry extends the catalogue it names, so the browse panel
    /// lists the union of the base endpoint and every added marketplace.
    /// Written by `/connectors marketplace <store> add <source>` and removed by
    /// `/connectors marketplace <store> remove <name>`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub marketplaces: Vec<ConnectorMarketplaceEntry>,
    /// Per-fetch wall-clock budget in milliseconds. Default: 10000.
    #[serde(default = "crate::config::default_store_timeout_ms")]
    pub timeout_ms: u64,
    /// Maximum accepted catalogue-index size in bytes; a larger index aborts the
    /// fetch (FR-031). Default: 2097152 (2 MiB).
    #[serde(default = "crate::config::default_max_index_bytes")]
    pub max_index_bytes: u64,
    /// Time-to-live for a cached catalogue in seconds; `0` disables the index
    /// cache. Default: 3600.
    #[serde(default = "crate::config::default_cache_ttl_secs")]
    pub cache_ttl_secs: u64,
}

/// One named connector catalogue endpoint (spec `connectors` FR-024, FR-035).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConnectorStoreEndpoint {
    /// The catalogue-index `https://` URL. A non-`https` or host-less value is
    /// refused when the catalogue is fetched (FR-028, FR-037).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

/// Non-secret credential references for one connector (spec `connectors` FR-007).
///
/// Each field is the *name* of a credential held in the encrypted credential
/// store (spec `connectors` FR-005); the value itself is never written here.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct ConnectorCredentialsConfig {
    /// Credential-store name holding the connector's token.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<String>,
}

impl Default for ConnectorsConfig {
    fn default() -> Self {
        Self {
            enabled: crate::config::default_true(),
            store_dir: None,
            stores: None,
            credentials: BTreeMap::new(),
        }
    }
}

/// One named connector marketplace attached to a catalogue (spec `connectors`
/// FR-043).
///
/// `store` names which catalogue the marketplace extends (`claude`); `source`
/// is the user-typed source in any of the accepted forms (`owner/repo`,
/// `git@github.com:owner/repo.git`, `https://.../marketplace.json`, or a local
/// directory holding a marketplace manifest). `name` is the resolved display
/// name (used by `remove`) and `url` is the resolved `https` marketplace URL,
/// recorded at add time so the panel does not have to re-resolve the source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConnectorMarketplaceEntry {
    /// Which catalogue the marketplace extends (`claude`).
    pub store: String,
    /// The display name (used by `/connectors marketplace claude remove <name>`).
    pub name: String,
    /// The marketplace index `https://` URL, resolved at add time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The source string exactly as typed by the user (informational).
    pub source: String,
}

impl Default for ConnectorStoresConfig {
    fn default() -> Self {
        Self {
            catalogues: BTreeMap::new(),
            marketplaces: Vec::new(),
            timeout_ms: crate::config::default_store_timeout_ms(),
            max_index_bytes: crate::config::default_max_index_bytes(),
            cache_ttl_secs: crate::config::default_cache_ttl_secs(),
        }
    }
}

impl ConnectorsConfig {
    /// Returns `true` when the connector subsystem is enabled (FR-021).
    #[must_use]
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// The effective catalogue settings, falling back to the compiled defaults
    /// when the `stores` block is absent (FR-024).
    #[must_use]
    pub fn stores_or_default(&self) -> ConnectorStoresConfig {
        self.stores.clone().unwrap_or_default()
    }

    /// Ensure a `stores` block is present (for mutation) and return it mutably,
    /// creating the compiled-default block when the section is absent.
    pub fn stores_mut(&mut self) -> &mut ConnectorStoresConfig {
        self.stores
            .get_or_insert_with(ConnectorStoresConfig::default)
    }

    /// The configured URL for the named catalogue, if any (FR-024). A `None` or
    /// empty value means the compiled default endpoint applies.
    #[must_use]
    pub fn catalogue_url(&self, name: &str) -> Option<&str> {
        self.stores
            .as_ref()
            .and_then(|stores| stores.catalogues.get(name))
            .and_then(|endpoint| endpoint.url.as_deref())
            .filter(|url| !url.trim().is_empty())
    }
}
