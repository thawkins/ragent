//! Edit-log persistence helpers.
//!
//! A process-wide atomic flag is synchronised with the `edit_log` field in
//! `ragent.json` so the toggle survives restarts.

use std::sync::atomic::{AtomicBool, Ordering};

static EDIT_LOG_MODE: AtomicBool = AtomicBool::new(false);

/// Returns `true` if edit logging is currently enabled.
#[must_use]
pub fn is_enabled() -> bool {
    EDIT_LOG_MODE.load(Ordering::Relaxed)
}

/// Enable or disable edit logging globally.
pub fn set_enabled(enabled: bool) {
    EDIT_LOG_MODE.store(enabled, Ordering::Relaxed);
}

/// Persist the requested edit-log state to the config file and update the
/// runtime flag.
///
/// The current config is reloaded, the `edit_log` field is updated, and the
/// result is written back to the same source file that was loaded (project
/// config preferred over global config). Any error during persistence is
/// returned so callers can decide how to report it.
///
/// If the config file cannot be loaded (e.g. it is corrupt), the error is
/// propagated rather than silently overwriting the file with defaults.
pub fn persist_edit_log(enabled: bool) -> anyhow::Result<()> {
    let mut config = crate::config::Config::load()
        .context("failed to load config before persisting edit_log")?;
    config.edit_log = enabled;
    config.save_to_source()?;
    set_enabled(enabled);
    Ok(())
}

/// Load the current config and update the runtime edit-log flag from its value.
pub fn sync_from_config() {
    let enabled = crate::config::Config::load()
        .map(|c| c.edit_log)
        .unwrap_or_default();
    set_enabled(enabled);
}

/// Update the runtime edit-log flag from an already-loaded config value,
/// avoiding a redundant disk read.
pub fn sync_from_config_value(enabled: bool) {
    set_enabled(enabled);
}

/// Toggle edit logging, persist the new state, and return it.
///
/// This is the recommended path for UI toggles (`Alt+E`, `/editlog`) because
/// `Config::load()` no longer syncs the runtime flag automatically. Using
/// plain [`set_enabled(false)` followed by a later `sync_from_config`] would
/// change the flag only until the next explicit sync
/// or persistence call.
pub fn toggle_persist() -> anyhow::Result<bool> {
    let new_state = !is_enabled();
    persist_edit_log(new_state)?;
    Ok(new_state)
}

use anyhow::Context as _;
