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

use std::sync::atomic::{AtomicBool, Ordering};

static YOLO_MODE: AtomicBool = AtomicBool::new(false);

/// Returns `true` if YOLO mode is currently enabled.
pub fn is_enabled() -> bool {
    YOLO_MODE.load(Ordering::Relaxed)
}

/// Enable or disable YOLO mode globally.
pub fn set_enabled(enabled: bool) {
    YOLO_MODE.store(enabled, Ordering::Relaxed);
}

/// Persist the requested YOLO state to the config file and update the runtime flag.
///
/// The current config is reloaded, the `yolo` field is updated, and the result is
/// written back to the same source file that was loaded (project config preferred
/// over global config). Any error during persistence is returned so callers can
/// decide how to report it.
///
/// If the config file cannot be loaded (e.g. it is corrupt), the error is
/// propagated rather than silently overwriting the file with defaults.
pub fn persist_yolo(enabled: bool) -> anyhow::Result<()> {
    let mut config =
        crate::config::Config::load().context("failed to load config before persisting yolo")?;
    config.yolo = enabled;
    config.save_to_source()?;
    set_enabled(enabled);
    Ok(())
}

/// Load the current config and update the runtime YOLO flag from its value.
pub fn sync_from_config() {
    let enabled = crate::config::Config::load()
        .map(|c| c.yolo)
        .unwrap_or_default();
    set_enabled(enabled);
}

/// Update the runtime YOLO flag from an already-loaded config value, avoiding
/// a redundant disk read.
pub fn sync_from_config_value(enabled: bool) {
    set_enabled(enabled);
}

/// Toggle YOLO mode, persist the new state, and return it.
///
/// This is the recommended path for UI toggles (`Alt+Y`, `/yolo`) because
/// `Config::load()` no longer syncs the runtime flag automatically. Using
/// plain [`set_enabled(false)` followed by a later `sync_from_config`] would
/// change the flag only until the next explicit sync
/// or persistence call.
pub fn toggle_persist() -> anyhow::Result<bool> {
    let new_state = !is_enabled();
    persist_yolo(new_state)?;
    Ok(new_state)
}

use anyhow::Context as _;
