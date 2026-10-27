//! Consolidated integration-test target: one test binary for the root crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child module
//! via `#[path]`, so it keeps its own file-relative `mod` / `#[path]`
//! resolution and its `test_<file>::<fn>` test name. See `AGENTS.md`
//! ("Test Layout") and `docs/reports/testconsolidation-review.md`.

#![allow(clippy::duplicate_mod)]
// ^ Intentional: `test_crash_capture.rs` re-includes `src/crash_dump.rs` via
// `#[path]`, and other children may load the same helper; under one binary for
// the crate that file becomes a module of several children.

/// The one process-wide lock that serialises tests which mutate or observe
/// process-global state (`set_current_dir`, `XDG_CONFIG_HOME`, `HOME`).
///
/// Before consolidation each test file was its own binary, so a file-local
/// mutex was enough. One binary per crate merges them, so every mutating test
/// must contend on the SAME mutex; `crate::` resolves at this binary root,
/// which is what makes that possible. See AGENTS.md ("Test Layout").
pub fn global_env_mutex() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

/// Acquire [`global_env_mutex`], recovering from a poisoned lock (a panicking
/// test must not wedge every later test).
pub fn global_env_lock() -> std::sync::MutexGuard<'static, ()> {
    global_env_mutex()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[path = "test_connectors_cli_command.rs"]
mod test_connectors_cli_command;

#[path = "test_crash_capture.rs"]
mod test_crash_capture;

#[path = "test_new_cli_command.rs"]
mod test_new_cli_command;

#[path = "test_plugins_cli_command.rs"]
mod test_plugins_cli_command;

#[path = "test_serve_openai_gate.rs"]
mod test_serve_openai_gate;

#[path = "test_version_flag.rs"]
mod test_version_flag;

#[path = "test_wire_matrix_flag.rs"]
mod test_wire_matrix_flag;
