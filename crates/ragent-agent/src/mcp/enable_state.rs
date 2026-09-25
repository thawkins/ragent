//! Durable, global enable/disable state for MCP servers.
//!
//! The `ragent.json` `mcp` section describes *how* to reach a server; whether
//! that server is actually started is a separate, user-owned switch. It is
//! persisted in a global ledger (`<global state dir>/mcp_state.json`) rather
//! than in the project config so a server disabled once stays disabled in every
//! project, and so a plugin-contributed server — which has no entry in
//! `ragent.json` at all — can still be turned off.
//!
//! Semantics:
//! - A server id **absent** from the ledger is enabled. This is what makes "a
//!   newly added MCP server starts out enabled" true without any write: adding
//!   a server to `ragent.json` or installing a plugin that bridges one takes
//!   effect immediately, and the ledger only ever records explicit choices.
//! - `McpServerConfig::disabled = true` in `ragent.json` always disables the
//!   server, whatever the ledger says; the config is the harder switch because
//!   it is the one the user edits by hand.
//!
//! The ledger is shared by the connect path (which filters on it), the
//! permission gate (which refuses to call a disabled server's tools), and the
//! `/mcp` and `/plugins` surfaces (which report it).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use ragent_config::McpServerConfig;

/// Name of the global MCP enable-state ledger.
pub const STATE_FILE: &str = "mcp_state.json";

/// The global MCP enable-state ledger: an explicit enabled/disabled choice per
/// server id. Ids absent from the map are enabled (see the module docs).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpEnableLedger {
    /// Explicit enable switch per server id.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub servers: BTreeMap<String, bool>,
}

impl McpEnableLedger {
    /// Load the global ledger, returning an empty ledger when it is absent or
    /// unreadable. A corrupt ledger is renamed aside (`.corrupt`) and an empty
    /// ledger returned, so a bad file can never make every MCP server vanish.
    #[must_use]
    pub fn load() -> Self {
        Self::load_from(&global_state_path())
    }

    /// Load a ledger from an explicit path (test seam for [`Self::load`]).
    #[must_use]
    pub fn load_from(path: &Path) -> Self {
        let Ok(bytes) = std::fs::read(path) else {
            return Self::default();
        };
        match serde_json::from_slice(&bytes) {
            Ok(ledger) => ledger,
            Err(error) => {
                tracing::warn!(
                    path = %path.display(),
                    error = %error,
                    "MCP enable-state ledger is corrupt; renaming aside and starting fresh"
                );
                let _ = std::fs::rename(path, path.with_extension("json.corrupt"));
                Self::default()
            }
        }
    }

    /// Persist the ledger to the global state path, creating the directory if
    /// needed.
    ///
    /// # Errors
    ///
    /// Returns an error when the state directory cannot be created or the file
    /// cannot be written.
    pub fn save(&self) -> anyhow::Result<()> {
        self.save_to(&global_state_path())
    }

    /// Persist the ledger to an explicit path (test seam for [`Self::save`]).
    ///
    /// # Errors
    ///
    /// Returns an error when the parent directory cannot be created or the file
    /// cannot be written.
    pub fn save_to(&self, path: &Path) -> anyhow::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec_pretty(self)?;
        std::fs::write(path, bytes)?;
        Ok(())
    }

    /// Whether `server_id` is enabled under this ledger (absent means enabled).
    #[must_use]
    pub fn is_enabled(&self, server_id: &str) -> bool {
        self.servers.get(server_id).copied().unwrap_or(true)
    }

    /// Record an explicit enabled/disabled choice for `server_id`.
    pub fn set_enabled(&mut self, server_id: &str, enabled: bool) {
        self.servers.insert(server_id.to_string(), enabled);
    }

    /// Drop any explicit choice for `server_id`, restoring the default
    /// (enabled) so a newly re-added server starts enabled.
    pub fn clear(&mut self, server_id: &str) {
        self.servers.remove(server_id);
    }

    /// Snapshot the explicit choices as an owned `id -> enabled` map.
    ///
    /// Surfaces that cache the ledger (the TUI's `mcp_enabled_map`) want a plain
    /// map rather than the ledger wrapper, so this avoids each caller repeating
    /// the `BTreeMap -> HashMap` conversion.
    #[must_use]
    pub fn to_map(&self) -> std::collections::HashMap<String, bool> {
        self.servers
            .iter()
            .map(|(id, enabled)| (id.clone(), *enabled))
            .collect()
    }
}

/// The global ledger path (`<global state dir>/mcp_state.json`).
///
/// Falls back to `.ragent/mcp_state.json` under the process working directory
/// when the platform state directory cannot be determined, so the ledger
/// degrades to a project-local file instead of being silently lost.
#[must_use]
pub fn global_state_path() -> PathBuf {
    match ragent_config::user_dirs::global_state_dir() {
        Some(dir) => dir.join(STATE_FILE),
        None => PathBuf::from(".ragent").join(STATE_FILE),
    }
}

/// Whether a server is enabled, combining the `ragent.json` `disabled` flag
/// with the global ledger.
///
/// The config flag wins: an explicitly disabled entry in `ragent.json` is never
/// started, no matter what the ledger records. A config that leaves `disabled`
/// at its default (`false`) defers to the ledger, so the ledger can switch off
/// both a configured server and a plugin-contributed one.
#[must_use]
pub fn is_server_enabled(
    config: &McpServerConfig,
    ledger: &McpEnableLedger,
    server_id: &str,
) -> bool {
    !config.disabled && ledger.is_enabled(server_id)
}
