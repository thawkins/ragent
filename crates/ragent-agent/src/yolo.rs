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

/// Toggle YOLO mode and return the new state.
pub fn toggle() -> bool {
    let was = YOLO_MODE.fetch_xor(true, Ordering::Relaxed);
    !was
}
