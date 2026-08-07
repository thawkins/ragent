//! Edit-log persistence helpers.
//!
//! Mirrors the `ragent_config::yolo` module: a process-wide atomic flag is
//! synchronised with the `edit_log` field in `ragent.json` so the toggle
//! survives restarts.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

static EDIT_LOG_MODE: AtomicBool = AtomicBool::new(false);
static EDIT_LOG_LOCK: Mutex<()> = Mutex::new(());

/// Returns `true` if edit logging is currently enabled.
#[must_use]
pub fn is_enabled() -> bool {
    let _guard = EDIT_LOG_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    EDIT_LOG_MODE.load(Ordering::Relaxed)
}

/// Enable or disable edit logging globally.
pub fn set_enabled(enabled: bool) {
    let _guard = EDIT_LOG_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    EDIT_LOG_MODE.store(enabled, Ordering::Relaxed);
}

/// Toggle edit logging and return the new state.
#[must_use]
pub fn toggle() -> bool {
    let _guard = EDIT_LOG_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let was = EDIT_LOG_MODE.fetch_xor(true, Ordering::Relaxed);
    !was
}

/// Persist the requested edit-log state to the config file and update the
/// runtime flag.
///
/// The current config is reloaded, the `edit_log` field is updated, and the
/// result is written back to the same source file that was loaded (project
/// config preferred over global config). Any error during persistence is
/// returned so callers can decide how to report it.
pub fn persist_edit_log(enabled: bool) -> anyhow::Result<()> {
    let mut config = crate::config::Config::load().unwrap_or_default();
    config.edit_log = enabled;
    config.save_to_source()?;
    set_enabled(enabled);
    Ok(())
}

/// Load the current config and update the runtime edit-log flag from its value.
///
/// This is intentionally separate from [`Config::load`](crate::config::Config::load)
/// so that config reloads used only to read settings do not race with an in-flight
/// toggle in multi-threaded contexts (e.g. parallel tests).
pub fn sync_from_config() {
    let enabled = crate::config::Config::load()
        .map(|c| c.edit_log)
        .unwrap_or_default();
    set_enabled(enabled);
}

/// Toggle edit logging, persist the new state, and return it.
///
/// This is the recommended path for UI toggles (`Alt+E`, `/editlog`) because
/// `Config::load()` no longer syncs the runtime flag automatically. Using
/// plain [`toggle()`] would change the flag only until the next explicit sync
/// or persistence call.
pub fn toggle_persist() -> anyhow::Result<bool> {
    let new_state = !is_enabled();
    persist_edit_log(new_state)?;
    Ok(new_state)
}
