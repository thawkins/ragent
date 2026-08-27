//! Activity-log persistence helpers.
//!
//! A process-wide atomic flag is synchronised with the `activity_log` field in
//! `ragent.json` so the toggle survives restarts. Unlike `edit_log` the default
//! is `true` (activity logging is on unless explicitly disabled).

use crate::runtime_flag::RuntimeFlag;

static ACTIVITY_LOG_MODE: RuntimeFlag = RuntimeFlag::new("activity_log", true);

/// Returns `true` if activity logging is currently enabled.
#[must_use]
pub fn is_enabled() -> bool {
    ACTIVITY_LOG_MODE.is_enabled()
}

/// Enable or disable activity logging globally.
pub fn set_enabled(enabled: bool) {
    ACTIVITY_LOG_MODE.set_enabled(enabled);
}

/// Persist the requested activity-log state to the config file and update the
/// runtime flag.
///
/// Any error during persistence is returned so callers can decide how to
/// report it.
pub fn persist_activity_log(enabled: bool) -> anyhow::Result<()> {
    ACTIVITY_LOG_MODE.persist(enabled)
}

/// Load the current config and update the runtime activity-log flag from its
/// value.
pub fn sync_from_config() {
    ACTIVITY_LOG_MODE.sync_from_config(true);
}

/// Update the runtime activity-log flag from an already-loaded config value,
/// avoiding a redundant disk read.
pub fn sync_from_config_value(enabled: bool) {
    ACTIVITY_LOG_MODE.set_enabled(enabled);
}

/// Toggle activity logging, persist the new state, and return it.
pub fn toggle_persist() -> anyhow::Result<bool> {
    ACTIVITY_LOG_MODE.toggle_persist()
}
