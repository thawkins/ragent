//! YOLO mode — bypass all command validation and tool restrictions.
//!
//! When enabled, the following safety checks are skipped:
//! - **Bash denied patterns** — destructive commands like `rm -rf /` are allowed
//! - **Dynamic context allowlist** — any executable can run in skill bodies
//! - **MCP config validation** — shell metacharacters and unvalidated paths are permitted
//!
//! # Warning
//!
//! This is inherently dangerous. Use only when you trust the agent and its
//! inputs completely, or for local development/debugging.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

static YOLO_MODE: AtomicBool = AtomicBool::new(false);
static YOLO_LOCK: Mutex<()> = Mutex::new(());

/// Returns `true` if YOLO mode is currently enabled.
pub fn is_enabled() -> bool {
    let _guard = YOLO_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    YOLO_MODE.load(Ordering::Relaxed)
}

/// Enable or disable YOLO mode globally.
pub fn set_enabled(enabled: bool) {
    let _guard = YOLO_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    YOLO_MODE.store(enabled, Ordering::Relaxed);
}

/// Toggle YOLO mode and return the new state.
pub fn toggle() -> bool {
    let _guard = YOLO_LOCK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let was = YOLO_MODE.fetch_xor(true, Ordering::Relaxed);
    !was
}

/// Persist the requested YOLO state to the config file and update the runtime flag.
///
/// The current config is reloaded, the `yolo` field is updated, and the result is
/// written back to the same source file that was loaded (project config preferred
/// over global config). Any error during persistence is returned so callers can
/// decide how to report it.
pub fn persist_yolo(enabled: bool) -> anyhow::Result<()> {
    let mut config = crate::config::Config::load().unwrap_or_default();
    config.yolo = enabled;
    config.save_to_source()?;
    set_enabled(enabled);
    Ok(())
}

/// Load the current config and update the runtime YOLO flag from its value.
///
/// This is intentionally separate from [`Config::load`](crate::config::Config::load)
/// so that config reloads used only to read settings do not race with an in-flight
/// toggle in multi-threaded contexts (e.g. parallel tests).
pub fn sync_from_config() {
    let enabled = crate::config::Config::load()
        .map(|c| c.yolo)
        .unwrap_or_default();
    set_enabled(enabled);
}

/// Toggle YOLO mode, persist the new state, and return it.
///
/// This is the recommended path for UI toggles (`Alt+Y`, `/yolo`) because
/// `Config::load()` no longer syncs the runtime flag automatically. Using
/// plain [`toggle()`] would change the flag only until the next explicit sync
/// or persistence call.
pub fn toggle_persist() -> anyhow::Result<bool> {
    let new_state = !is_enabled();
    persist_yolo(new_state)?;
    Ok(new_state)
}
