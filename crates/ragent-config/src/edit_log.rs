//! Edit-log persistence helpers.
//!
//! A process-wide atomic flag is synchronised with the `edit_log` field in
//! `ragent.json` so the toggle survives restarts.

use crate::runtime_flag::RuntimeFlag;

static EDIT_LOG_MODE: RuntimeFlag = RuntimeFlag::new("edit_log", false);

/// Returns `true` if edit logging is currently enabled.
#[must_use]
pub fn is_enabled() -> bool {
    EDIT_LOG_MODE.is_enabled()
}

/// Enable or disable edit logging globally.
pub fn set_enabled(enabled: bool) {
    EDIT_LOG_MODE.set_enabled(enabled);
}

/// Persist the requested edit-log state to the config file and update the
/// runtime flag.
///
/// Any error during persistence is returned so callers can decide how to
/// report it.
pub fn persist_edit_log(enabled: bool) -> anyhow::Result<()> {
    EDIT_LOG_MODE.persist(enabled)
}

/// Load the current config and update the runtime edit-log flag from its value.
pub fn sync_from_config() {
    EDIT_LOG_MODE.sync_from_config(false);
}

/// Update the runtime edit-log flag from an already-loaded config value,
/// avoiding a redundant disk read.
pub fn sync_from_config_value(enabled: bool) {
    EDIT_LOG_MODE.set_enabled(enabled);
}

/// Toggle edit logging, persist the new state, and return it.
///
/// This is the recommended path for UI toggles (`Alt+E`, `/editlog`).
pub fn toggle_persist() -> anyhow::Result<bool> {
    EDIT_LOG_MODE.toggle_persist()
}
