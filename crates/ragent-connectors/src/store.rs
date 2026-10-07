//! Connector store paths, discovery scan, and state ledger (spec `connectors`
//! T-003; FR-001).
//!
//! The connector store has two roots, searched in ascending-priority order
//! (last match wins on connector-id collision):
//!
//! 1. user-global: `~/.config/ragent/connectors/` (via
//!    [`ragent_config::user_dirs`])
//! 2. project-local: `<working dir>/.ragent/connectors/`
//!
//! Discovery ([`scan`]) reads each connector directory's `connector.json`
//! manifest into a [`ConnectorDescriptor`] but never connects an MCP server, so
//! a `/connectors list` with no flags is a store scan plus a ledger read that
//! performs no network access and no MCP connection (FR-001). Enable/disable
//! state and telemetry counters persist in a per-store `_state.json` ledger
//! ([`StoreLedger`]) so state survives restarts.
//!
//! Symlinks during the walk are followed only when their resolved target stays
//! inside the store directory; links escaping the store are skipped and logged.
//!
//! The module is modelled on `ragent_plugins::store` (spec `plugins` T-005) so
//! the two store ledgers are recognisably the same shape.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::descriptor::ConnectorDescriptor;
use crate::error::ConnectorError;

/// Name of the per-store state ledger file.
pub const STATE_FILE: &str = "_state.json";

/// Name of the connector manifest file inside a connector directory.
///
/// A directory in the store is a connector directory when it holds this file;
/// its contents are the serialised [`ConnectorDescriptor`] (minus any secret,
/// which is referenced by credential name only). The format is defined and
/// written by the [`crate::manifest`] module (T-004); discovery reads it
/// through [`crate::manifest::read_manifest`].
pub const MANIFEST_FILE: &str = "connector.json";

/// One connector store directory set (FR-001). Scanning reads both; project
/// connectors take precedence on connector-id collision.
///
/// Shared with the plugin store (T-505): the resolution shape has one
/// implementation in `ragent_surface::store`.
pub type StoreDirs = ragent_surface::store::StoreDirs;

/// Resolve the connector store directories for the given legs (FR-001).
///
/// Delegates to the shared store resolver (`ragent_surface::store`) with the
/// `connectors` store leaf.
#[must_use]
pub fn store_dirs_at(
    workdir: &Path,
    store_dir_override: Option<&Path>,
    global_root: Option<&Path>,
) -> StoreDirs {
    ragent_surface::store::store_dirs_at(workdir, store_dir_override, global_root, "connectors")
}

/// Resolve the connector store directories from the live environment (FR-001).
///
/// `workdir` is the current working directory (pass [`std::env::current_dir`]
/// from the session layer). A `store_dir` override from the `connectors.store_dir`
/// configuration replaces the project leg; the global leg comes from
/// [`ragent_config::user_dirs::global_state_dir`].
#[must_use]
pub fn store_dirs(workdir: &Path, store_dir_override: Option<&Path>) -> StoreDirs {
    ragent_surface::store::store_dirs(workdir, store_dir_override, "connectors")
}

/// One store's on-disk state ledger (`_state.json`): enable/disable state and
/// telemetry counters keyed by connector id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreLedger {
    /// State per connector id. Ids absent from the map are treated as
    /// [`ConnectorState::default`] (disabled, zero counters).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub connectors: BTreeMap<String, ConnectorState>,
}

/// Persisted per-connector state (enable flag + telemetry counters).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorState {
    /// Whether the connector is enabled. A newly installed connector is recorded
    /// **enabled** by `/connectors add` (FR-011) so it is ready to use, but
    /// nothing is connected until the next session start or an explicit
    /// `/connectors connect`; it can be turned off with
    /// `/connectors disable` (FR-013). The durable, cross-project enable ledger
    /// (`<state dir>/mcp_state.json`) governs whether a server actually starts
    /// (FR-012, FR-018).
    #[serde(default)]
    pub enabled: bool,
    /// Telemetry counters for the connector (connects, tool invocations,
    /// failures).
    #[serde(default)]
    pub counters: ConnectorCounters,
}

/// Telemetry counters recorded against a connector.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectorCounters {
    /// Times the connector's bridged servers connected successfully.
    #[serde(default)]
    pub connects_ok: u64,
    /// Times a bridged server failed to connect.
    #[serde(default)]
    pub connect_failures: u64,
    /// Tool invocations routed through this connector's bridged servers.
    #[serde(default)]
    pub tool_invocations: u64,
    /// Tool-invocation failures (transport error, timeout, server error).
    #[serde(default)]
    pub tool_failures: u64,
    /// Consecutive tool-invocation failures since the last success.
    #[serde(default)]
    pub consecutive_failures: u64,
}

impl StoreLedger {
    /// Load the ledger for `store_dir`, returning an empty ledger when the file
    /// is absent. A corrupt ledger is renamed aside (`.corrupt`) and an empty
    /// ledger returned so a bad ledger never bricks the store.
    #[must_use]
    pub fn load(store_dir: &Path) -> Self {
        let path = store_dir.join(STATE_FILE);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Err(error) => {
                // A present-but-unreadable ledger (e.g. a permission fault) must
                // not be treated as absent: the next save would silently discard
                // the previous enable/disable state. Warn instead.
                tracing::warn!(
                    path = %path.display(),
                    error = %error,
                    "connector store ledger unreadable; starting fresh"
                );
                return Self::default();
            }
        };
        match serde_json::from_slice(&bytes) {
            Ok(ledger) => ledger,
            Err(e) => {
                let aside = store_dir.join(format!("{STATE_FILE}.corrupt"));
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "connector store ledger is corrupt; renaming aside and starting fresh"
                );
                let _ = std::fs::rename(&path, aside); // INTENTIONAL: corrupt-ledger quarantine rename; failure leaves the file untouched
                Self::default()
            }
        }
    }

    /// Persist the ledger for `store_dir`, creating the directory first.
    ///
    /// # Errors
    ///
    /// Returns [`ConnectorError::Io`] when the directory cannot be created or the
    /// file cannot be written, and [`ConnectorError::ManifestParse`] when the
    /// ledger cannot be serialised.
    pub fn save(&self, store_dir: &Path) -> Result<(), ConnectorError> {
        std::fs::create_dir_all(store_dir).map_err(ConnectorError::io)?;
        let bytes = serde_json::to_vec_pretty(self).map_err(ConnectorError::io)?;
        std::fs::write(store_dir.join(STATE_FILE), bytes).map_err(ConnectorError::io)
    }

    /// Mutable access to one connector's state, inserting the default when
    /// absent.
    pub fn state_mut(&mut self, connector_id: &str) -> &mut ConnectorState {
        self.connectors.entry(connector_id.to_string()).or_default()
    }

    /// Read-only access to one connector's state (defaults when absent).
    #[must_use]
    pub fn state(&self, connector_id: &str) -> Option<&ConnectorState> {
        self.connectors.get(connector_id)
    }
}

/// One connector discovered by [`scan`].
#[derive(Debug)]
pub struct ScannedConnector {
    /// The connector's identity key: the descriptor id when it parsed, or the
    /// directory name when the manifest failed to parse.
    pub id: String,
    /// Parse outcome: the parsed descriptor, or the parse error with the
    /// connector directory (the connector has no id when parsing failed; the key
    /// identity for display is the directory name).
    pub outcome: Result<ConnectorDescriptor, ScanFailure>,
    /// Whether the connector is enabled in the store ledger.
    pub enabled: bool,
    /// The store directory this connector came from (project or global root).
    pub store: PathBuf,
    /// The connector directory itself.
    pub dir: PathBuf,
}

/// A discovery-time failure with the connector directory (reporting needs a row
/// per problematic directory even when no descriptor exists).
#[derive(Debug)]
pub struct ScanFailure {
    /// The failure that prevented descriptor construction.
    pub error: ConnectorError,
}

/// Walk both store directories (project wins on connector-id collision) and
/// return one [`ScannedConnector`] per connector directory; unrecognised entries
/// (plain directories, the `_state.json` ledger, dotfiles) are skipped.
///
/// Manifests are read into descriptors; no MCP server is connected (FR-001).
/// Symlinked connector directories are followed only when their canonical target
/// stays inside the store directory they were found in.
///
/// Scanning never fails wholesale: an absent/unreadable store directory is
/// treated as empty, and individual bad manifests are reported through
/// [`ScannedConnector::outcome`] instead of failing the scan.
#[must_use]
pub fn scan(workdir: &Path, store_dir_override: Option<&Path>) -> Vec<ScannedConnector> {
    let dirs = store_dirs(workdir, store_dir_override);
    scan_dirs(dirs)
}

/// Scan a pre-resolved [`StoreDirs`] set (testable core of [`scan`]).
///
/// Iterates global first, then project, with later entries overwriting earlier
/// ones on connector-id collision (project wins, FR-001).
#[must_use]
pub fn scan_dirs(dirs: StoreDirs) -> Vec<ScannedConnector> {
    let mut by_id: BTreeMap<String, ScannedConnector> = BTreeMap::new();
    // Global first, project last: later entries overwrite earlier on id
    // collision ("closest wins", FR-001).
    for store in [dirs.global, dirs.project].into_iter().flatten() {
        let ledger = StoreLedger::load(&store);
        let read_dir = match std::fs::read_dir(&store) {
            Ok(rd) => rd,
            Err(_) => continue, // store absent or unreadable: nothing to scan
        };
        for entry in read_dir.flatten() {
            let dir = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if name == STATE_FILE || name.starts_with('.') {
                continue;
            }
            if !ragent_surface::store::store_entry_is_dir(&store, &dir, "connector") {
                continue;
            }
            if !dir.join(MANIFEST_FILE).is_file() {
                continue; // not a connector directory
            }
            let outcome =
                crate::manifest::read_manifest(&dir).map_err(|error| ScanFailure { error });
            let id = match &outcome {
                Ok(descriptor) => descriptor.id.as_str().to_string(),
                Err(_) => name.clone(),
            };
            let enabled = ledger.state(&id).is_some_and(|s| s.enabled);
            by_id.insert(
                id.clone(),
                ScannedConnector {
                    id,
                    outcome,
                    enabled,
                    store: store.clone(),
                    dir,
                },
            );
        }
    }
    by_id.into_values().collect()
}

/// Find the parsed descriptor for `id` across the store legs, project winning on
/// id collision (FR-001). `None` when no store holds a parseable connector
/// matching the reference.
///
/// The reference is matched against three keys, first match wins:
///
/// 1. the descriptor id, byte-for-byte;
/// 2. the descriptor id, case-insensitively;
/// 3. the display name, case-insensitively.
///
/// So `/connectors enable Microsoft-Learn`, `/connectors enable microsoft-learn`
/// and `/connectors enable "Microsoft Learn"` all resolve the same installed
/// connector, which is what makes a connector addressable without its UUID.
///
/// Shared by the TUI and CLI session environments so their descriptor lookup
/// cannot drift.
#[must_use]
pub fn descriptor_by_id(dirs: &StoreDirs, reference: &str) -> Option<ConnectorDescriptor> {
    let reference = reference.trim();
    let descriptors = scan_dirs(dirs.clone())
        .into_iter()
        .filter_map(|connector| connector.outcome.ok());
    // Match precedence: exact id, then case-insensitive id, then
    // case-insensitive display name. Collecting into a `Vec` is unnecessary -
    // the first matching precedence wins, so a single pass that keeps the
    // highest-priority match is equivalent and allocates only the winner.
    let mut exact_id: Option<ConnectorDescriptor> = None;
    let mut ci_id: Option<ConnectorDescriptor> = None;
    let mut ci_name: Option<ConnectorDescriptor> = None;
    for d in descriptors {
        if exact_id.is_none() && d.id.as_str() == reference {
            exact_id = Some(d);
        } else if ci_id.is_none() && d.id.as_str().eq_ignore_ascii_case(reference) {
            ci_id = Some(d);
        } else if ci_name.is_none() && d.name.eq_ignore_ascii_case(reference) {
            ci_name = Some(d);
        }
        if exact_id.is_some() {
            break;
        }
    }
    exact_id.or(ci_id).or(ci_name)
}
