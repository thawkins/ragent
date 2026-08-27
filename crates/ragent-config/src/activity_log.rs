//! Activity-log persistence helpers.
//!
//! A process-wide atomic flag is synchronised with the `activity_log` field in
//! `ragent.json` so the toggle survives restarts. Unlike `edit_log` the default
//! is `true` (activity logging is on unless explicitly disabled).

use std::sync::atomic::{AtomicBool, Ordering};

static ACTIVITY_LOG_MODE: AtomicBool = AtomicBool::new(true);

/// Returns `true` if activity logging is currently enabled.
#[must_use]
pub fn is_enabled() -> bool {
    ACTIVITY_LOG_MODE.load(Ordering::Relaxed)
}

/// Enable or disable activity logging globally.
pub fn set_enabled(enabled: bool) {
    ACTIVITY_LOG_MODE.store(enabled, Ordering::Relaxed);
}

/// Persist the requested activity-log state to the config file and update the
/// runtime flag.
///
/// The current config is reloaded, the `activity_log` field is updated, and
/// the result is written back to the same source file that was loaded
/// (project config preferred over global config). Any error during
/// persistence is returned so callers can decide how to report it.
///
/// If the config file cannot be loaded (e.g. it is corrupt), the error is
/// propagated rather than silently overwriting the file with defaults.
pub fn persist_activity_log(enabled: bool) -> anyhow::Result<()> {
    let mut config = crate::config::Config::load()
        .context("failed to load config before persisting activity_log")?;
    config.activity_log = enabled;
    config.save_to_source()?;
    set_enabled(enabled);
    Ok(())
}

/// Load the current config and update the runtime activity-log flag from its
/// value.
pub fn sync_from_config() {
    let enabled = crate::config::Config::load()
        .map(|c| c.activity_log)
        .unwrap_or(true);
    set_enabled(enabled);
}

/// Update the runtime activity-log flag from an already-loaded config value,
/// avoiding a redundant disk read.
pub fn sync_from_config_value(enabled: bool) {
    set_enabled(enabled);
}

/// Toggle activity logging, persist the new state, and return it.
pub fn toggle_persist() -> anyhow::Result<bool> {
    let new_state = !is_enabled();
    persist_activity_log(new_state)?;
    Ok(new_state)
}

use anyhow::Context as _;
