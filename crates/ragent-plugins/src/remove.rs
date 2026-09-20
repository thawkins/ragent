//! `/plugins remove` store operation (spec `plugins` T-011; FR-010).
//!
//! Uninstalls a plugin by deleting its directory from the store the plugin was
//! discovered in, then clearing its row from that store's `_state.json` ledger
//! so a later re-add starts from a clean slate.
//!
//! Guards (both refuse and change nothing):
//!
//! - an unknown plugin id ([`RemoveError::UnknownPlugin`]);
//! - a plugin currently enabled in the store ledger
//!   ([`RemoveError::Enabled`] — "disable it first"); the caller refuses rather
//!   than silently unloading a running plugin, matching the T-011 plan.
//!
//! Disabled plugins are inert (FR-016), so removing one touches only its files.

use std::path::PathBuf;

use crate::error::PluginError;
use crate::store::{ScannedPlugin, StoreDirs, StoreLedger, scan_dirs};

/// The outcome of a successful [`remove`].
#[derive(Debug)]
pub struct RemoveOutcome {
    /// The plugin id that was removed.
    pub id: String,
    /// The plugin directory that was deleted.
    pub dir: PathBuf,
    /// The store directory the plugin lived in.
    pub store: PathBuf,
}

/// A refusal or failure from [`remove`].
#[derive(Debug, thiserror::Error)]
pub enum RemoveError {
    /// No store contains a plugin with this id.
    #[error("plugin remove: unknown plugin id {0}")]
    UnknownPlugin(String),

    /// The plugin is enabled; it must be disabled before removal.
    #[error("plugin remove: plugin {0} is enabled; run `/plugins disable {0}` first")]
    Enabled(String),

    /// An I/O failure while deleting the plugin directory or the ledger row.
    #[error("plugin remove: {0}")]
    Io(String),
}

/// Uninstall a plugin from its store (FR-010).
///
/// Finds the plugin across the configured stores (project wins on id
/// collision), refuses when it is enabled, then deletes its directory and
/// clears its ledger row.
///
/// # Errors
///
/// Returns [`RemoveError::UnknownPlugin`] when no store holds `plugin_id`,
/// [`RemoveError::Enabled`] when the plugin is enabled, or [`RemoveError::Io`]
/// when the directory cannot be deleted.
pub fn remove(dirs: &StoreDirs, plugin_id: &str) -> Result<RemoveOutcome, RemoveError> {
    let found = scan_dirs(dirs.clone())
        .into_iter()
        .find(|p| plugin_identity(p) == plugin_id)
        .ok_or_else(|| RemoveError::UnknownPlugin(plugin_id.to_string()))?;

    if found.enabled {
        return Err(RemoveError::Enabled(plugin_id.to_string()));
    }

    std::fs::remove_dir_all(&found.dir)
        .map_err(|e| RemoveError::Io(PluginError::io(e).to_string()))?;

    // Drop the ledger row so a re-add starts clean; only rewrite the ledger
    // when the row was actually present so an absent `_state.json` is not
    // created as a side effect of a removal.
    let mut ledger = StoreLedger::load(&found.store);
    if ledger.plugins.remove(plugin_id).is_some()
        && let Err(e) = ledger.save(&found.store)
    {
        tracing::warn!(
            plugin = plugin_id,
            error = %e,
            "plugin ledger row could not be cleared after remove"
        );
    }

    Ok(RemoveOutcome {
        id: plugin_id.to_string(),
        dir: found.dir,
        store: found.store,
    })
}

/// The identity key for a scanned plugin: descriptor id when parsed, else the
/// directory name (mirrors `lifecycle::scan_id`; a directory whose manifest
/// failed to parse has no descriptor id).
fn plugin_identity(plugin: &ScannedPlugin) -> String {
    match &plugin.outcome {
        Ok(parsed) => parsed.descriptor.id.clone(),
        Err(_) => plugin
            .dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default(),
    }
}
