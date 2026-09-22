//! Plugin store paths, discovery scan, and state ledger (spec `plugins`
//! T-005; FR-001, FR-023).
//!
//! The plugin store has two roots, searched in ascending-priority order
//! (last match wins on plugin-id collision):
//!
//! 1. user-global: `~/.config/ragent/plugins/` (via
//!    [`ragent_config::user_dirs`])
//! 2. project-local: `<working dir>/.ragent/plugins/`
//!
//! Discovery ([`scan`]) recognises dialects and parses manifests but never
//! executes JavaScript (FR-023). Enable/disable state and telemetry counters
//! persist in a per-store `_state.json` ledger ([`StoreLedger`]) so state
//! survives restarts.
//!
//! Symlinks during the walk are followed only when their resolved target
//! stays inside the store directory; links escaping the store are skipped and
//! logged.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::descriptor::detect_dialect;
use crate::error::PluginError;
use crate::manifest::parse_plugin_dir;

/// Name of the per-store state ledger file.
pub const STATE_FILE: &str = "_state.json";

/// One plugin store directory set. `project` is the project-local store,
/// `global` the user-global fallback (FR-001). Scanning reads both; project
/// plugins take precedence on plugin-id collision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreDirs {
    /// Project-local store: `<workdir>/.ragent/plugins/`. `None` when no
    /// working directory is in effect.
    pub project: Option<PathBuf>,
    /// User-global store: `~/.config/ragent/plugins/`. `None` when the
    /// platform config directory cannot be determined.
    pub global: Option<PathBuf>,
}

/// Resolve the plugin store directories for the given legs (FR-001).
///
/// Pure over its inputs so tests avoid env mutation: pass the project working
/// directory (or `store_dir` override) and the user-global root (resolved via
/// [`ragent_config::user_dirs::global_state_dir`] by the caller) explicitly.
///
/// The returned order is ascending priority: global first, project last, so
/// iteration with "last write wins" yields closest-wins on id collision.
/// `global_root` is the `~/.config/ragent` root (not the `plugins/` child).
#[must_use]
pub fn store_dirs_at(
    workdir: &Path,
    store_dir_override: Option<&Path>,
    global_root: Option<&Path>,
) -> StoreDirs {
    let global = global_root.map(|d| d.join("plugins"));
    let project = store_dir_override
        .map(Path::to_path_buf)
        .or_else(|| Some(workdir.join(".ragent").join("plugins")));
    StoreDirs { project, global }
}

/// Resolve the plugin store directories from the live environment (FR-001).
///
/// `workdir` is the current working directory (pass [`std::env::current_dir`]
/// from the session layer). A `store_dir` override from `plugins.store_dir`
/// configuration replaces the project leg; the global leg comes from
/// [`ragent_config::user_dirs::global_state_dir`].
#[must_use]
pub fn store_dirs(workdir: &Path, store_dir_override: Option<&Path>) -> StoreDirs {
    store_dirs_at(
        workdir,
        store_dir_override,
        ragent_config::user_dirs::global_state_dir().as_deref(),
    )
}

/// One store's on-disk state ledger (`_state.json`): enable/disable state and
/// telemetry counters keyed by plugin id.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StoreLedger {
    /// State per plugin id. Ids absent from the map are treated as
    /// `PluginState::default()` (disabled, zero counters).
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub plugins: BTreeMap<String, PluginState>,
}

/// Persisted per-plugin state (enable flag + telemetry counters, FR-022).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PluginState {
    /// Whether the plugin is enabled. A newly installed plugin is recorded
    /// enabled by `/plugins add` (FR-007); it can be turned off with
    /// `/plugins disable`.
    #[serde(default)]
    pub enabled: bool,
    /// Telemetry counters for the plugin (loads, tool invocations, failures).
    #[serde(default)]
    pub counters: TelemetryCounters,
}

/// Telemetry counters recorded against a plugin (FR-022).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TelemetryCounters {
    /// Times the plugin's entry point executed successfully.
    #[serde(default)]
    pub loads_ok: u64,
    /// Times the plugin failed to load (manifest parse, entry error, version
    /// refusal).
    #[serde(default)]
    pub load_failures: u64,
    /// Tool invocations routed through this plugin.
    #[serde(default)]
    pub tool_invocations: u64,
    /// Tool-invocation failures (exceptions, timeouts, memory ceiling).
    #[serde(default)]
    pub tool_failures: u64,
    /// Consecutive tool-invocation failures since the last success; reaching
    /// the configured threshold auto-unloads the plugin (T-009).
    #[serde(default)]
    pub consecutive_failures: u64,
}

impl StoreLedger {
    /// Load the ledger for `store_dir`, returning an empty ledger when the
    /// file is absent. A corrupt ledger is renamed aside (`.corrupt`) and an
    /// empty ledger returned so a bad ledger never bricks the store.
    #[must_use]
    pub fn load(store_dir: &Path) -> Self {
        let path = store_dir.join(STATE_FILE);
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(_) => return Self::default(),
        };
        match serde_json::from_slice(&bytes) {
            Ok(ledger) => ledger,
            Err(e) => {
                let aside = store_dir.join(format!("{STATE_FILE}.corrupt"));
                tracing::warn!(
                    path = %path.display(),
                    error = %e,
                    "plugin store ledger is corrupt; renaming aside and starting fresh"
                );
                let _ = std::fs::rename(&path, aside);
                Self::default()
            }
        }
    }

    /// Persist the ledger for `store_dir`, creating the directory first.
    ///
    /// # Errors
    ///
    /// Returns [`PluginError::Io`] when the file cannot be written.
    pub fn save(&self, store_dir: &Path) -> Result<(), PluginError> {
        std::fs::create_dir_all(store_dir).map_err(PluginError::io)?;
        let bytes =
            serde_json::to_vec_pretty(self).expect("BUG: ledger serialisation is infallible");
        std::fs::write(store_dir.join(STATE_FILE), bytes).map_err(PluginError::io)
    }

    /// Mutable access to one plugin's state, inserting the default when absent.
    pub fn state_mut(&mut self, plugin_id: &str) -> &mut PluginState {
        self.plugins.entry(plugin_id.to_string()).or_default()
    }

    /// Read-only access to one plugin's state (defaults when absent).
    #[must_use]
    pub fn state(&self, plugin_id: &str) -> Option<&PluginState> {
        self.plugins.get(plugin_id)
    }
}

/// The lifecycle state of a discovered plugin (FR-009).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleState {
    /// Present in the store, not enabled.
    Disabled,
    /// Enabled but not yet loaded in this session.
    Enabled,
    /// Loaded into the current session.
    Loaded,
    /// A failure was recorded; the cause is carried separately.
    Errored,
}

impl std::fmt::Display for LifecycleState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Disabled => f.write_str("disabled"),
            Self::Enabled => f.write_str("enabled"),
            Self::Loaded => f.write_str("loaded"),
            Self::Errored => f.write_str("errored"),
        }
    }
}

/// One plugin discovered by [`scan`].
#[derive(Debug)]
pub struct ScannedPlugin {
    /// Parse outcome: the parsed manifest, or the recognition/parse error
    /// with the plugin root (the plugin has no id on failure; the key identity
    /// for display is the directory name).
    pub outcome: Result<crate::manifest::ParsedManifest, ScanFailure>,
    /// Whether the plugin is enabled in the store ledger.
    pub enabled: bool,
    /// The store directory this plugin came from (project or global root).
    pub store: PathBuf,
    /// The plugin directory itself.
    pub dir: PathBuf,
}

/// A discovery-time failure with the plugin directory (FR-025 reporting needs
/// a row per problematic directory even when no descriptor exists).
#[derive(Debug)]
pub struct ScanFailure {
    /// The failure that prevented descriptor construction.
    pub error: PluginError,
}

/// Walk both store directories (project wins on id collision) and return one
/// [`ScannedPlugin`] per recognised plugin directory; unrecognised entries
/// (plain directories, the `_state.json` ledger, dotfiles) are skipped.
///
/// Manifests are parsed; no JavaScript executes (FR-023). Symlinked plugin
/// directories are followed only when their canonical target stays inside the
/// store directory they were found in.
///
/// Scanning never fails wholesale: an absent/unreadable store directory is
/// treated as empty, and individual bad manifests are reported through
/// [`ScannedPlugin::outcome`] instead of failing the scan.
#[must_use]
pub fn scan(workdir: &Path, store_dir_override: Option<&Path>) -> Vec<ScannedPlugin> {
    let dirs = store_dirs(workdir, store_dir_override);
    scan_dirs(dirs)
}

/// The plugin ids a store scan reports as installed (spec `pluginstores` A5,
/// FR-005).
///
/// Derived from [`scan_dirs`]: each plugin that parsed cleanly contributes its
/// descriptor id. A directory that failed to recognise or parse is keyed by its
/// directory name for display but is *not* treated as installed, so a broken
/// entry never marks a store row as present.
///
/// Pure over its input, so the installed set can be derived without a network
/// request and without keeping a second install registry (A5). The result is a
/// set because membership is the only question the browser asks of it.
#[must_use]
pub fn installed_ids(dirs: StoreDirs) -> BTreeSet<String> {
    scan_dirs(dirs)
        .into_iter()
        .filter_map(|scanned| scanned.outcome.ok().map(|parsed| parsed.descriptor.id))
        .collect()
}

/// Scan a pre-resolved [`StoreDirs`] set (testable core of [`scan`]).
///
/// Iterates global first, then project, with later entries overwriting earlier
/// ones on plugin-id collision (project wins, FR-001).
#[must_use]
pub fn scan_dirs(dirs: StoreDirs) -> Vec<ScannedPlugin> {
    let mut by_id: BTreeMap<String, ScannedPlugin> = BTreeMap::new();
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
            if !store_entry_is_dir(&store, &dir) {
                continue;
            }
            let outcome = match detect_dialect(&dir) {
                Ok(Some(_)) => match parse_plugin_dir(&dir) {
                    Ok(Some(parsed)) => Ok(parsed),
                    Ok(None) => continue, // recognised listing raced away
                    Err(error) => Err(ScanFailure { error }),
                },
                Ok(None) => continue, // not a plugin directory
                Err(error) => Err(ScanFailure { error }),
            };
            let id = match &outcome {
                Ok(parsed) => parsed.descriptor.id.clone(),
                Err(_) => name.clone(),
            };
            let enabled = ledger.state(&id).is_some_and(|s| s.enabled);
            by_id.insert(
                id,
                ScannedPlugin {
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

/// A store entry is a candidate plugin directory when it is a real directory,
/// or a symlink whose canonical target is a directory inside the store.
fn store_entry_is_dir(store: &Path, dir: &Path) -> bool {
    let Ok(file_type) = dir.symlink_metadata() else {
        return false;
    };
    if file_type.is_dir() {
        return true;
    }
    if !file_type.file_type().is_symlink() {
        return false;
    }
    // Symlink: follow only when the resolved path stays inside the store.
    let (Ok(target), Ok(store_root)) = (dir.canonicalize(), store.canonicalize()) else {
        tracing::warn!(
            link = %dir.display(),
            "plugin store symlink cannot be resolved; skipping"
        );
        return false;
    };
    if target.starts_with(&store_root) {
        target.is_dir()
    } else {
        tracing::warn!(
            link = %dir.display(),
            target = %target.display(),
            "plugin store symlink escapes the store directory; skipping"
        );
        false
    }
}
