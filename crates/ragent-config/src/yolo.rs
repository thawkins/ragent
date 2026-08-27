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

use crate::runtime_flag::RuntimeFlag;

static YOLO_MODE: RuntimeFlag = RuntimeFlag::new("yolo", false);

/// Returns `true` if YOLO mode is currently enabled.
pub fn is_enabled() -> bool {
    YOLO_MODE.is_enabled()
}

/// Enable or disable YOLO mode globally.
pub fn set_enabled(enabled: bool) {
    YOLO_MODE.set_enabled(enabled);
}

/// Persist the requested YOLO state to the config file and update the runtime
/// flag.
///
/// Any error during persistence is returned so callers can decide how to
/// report it.
pub fn persist_yolo(enabled: bool) -> anyhow::Result<()> {
    YOLO_MODE.persist(enabled)
}

/// Load the current config and update the runtime YOLO flag from its value.
pub fn sync_from_config() {
    YOLO_MODE.sync_from_config(false);
}

/// Update the runtime YOLO flag from an already-loaded config value, avoiding
/// a redundant disk read.
pub fn sync_from_config_value(enabled: bool) {
    YOLO_MODE.set_enabled(enabled);
}

/// Toggle YOLO mode, persist the new state, and return it.
///
/// This is the recommended path for UI toggles (`Alt+Y`, `/yolo`).
pub fn toggle_persist() -> anyhow::Result<bool> {
    YOLO_MODE.toggle_persist()
}
