//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

#![allow(clippy::duplicate_mod)]
// ^ Intentional: `src/jsonl_log.rs` is re-included by the crate-root shim here
// (for `edit_log.rs`, compiled through `#[path]`) and again by
// `test_edit_log.rs`, its documented `#[path]` re-import. See AGENTS.md
// ("Test Layout").

// Crate-root shims for child tests that include private source modules via
// `#[path = "../src/..."]`. A `#[path]`-included module resolves `crate::`
// against THIS consolidated binary root, not the ragent-tools-core crate, so
// the names those modules import must exist here.
//
// `test_bash_wrapper_quoting` compiles `src/bash.rs` against local stubs (it
// must not touch a real event bus or askpass broker), and
// `test_edit_log`/`test_edit` compile `src/edit_log.rs` (`crate::jsonl_log`)
// and `src/edit*.rs`. See AGENTS.md ("Test Layout").
pub use crate::test_bash_wrapper_quoting::shim::CanonicalPathCache;

/// Stub event module (the real one would require a live subscriber).
pub mod event {
    pub use crate::test_bash_wrapper_quoting::shim::event::*;
}

/// Stub process-resource gate.
pub mod resource {
    pub use crate::test_bash_wrapper_quoting::shim::resource::*;
}

/// Stub secret redaction (identity, so quoting output is unchanged).
pub mod sanitize {
    pub use crate::test_bash_wrapper_quoting::shim::sanitize::*;
}

/// Stub askpass broker.
pub mod askpass {
    pub use crate::test_bash_wrapper_quoting::shim::askpass::*;
}

/// Shared JSONL append/pick helpers re-included so `edit_log.rs`, compiled
/// through `#[path]`, resolves `crate::jsonl_log`.
///
/// Only the module path is needed (nothing at the binary root calls these
/// helpers directly), so the items are allowed to be unused here.
#[allow(dead_code)]
/// Before consolidation each test file was its own binary, so a file-local
/// mutex was enough. One binary per crate merges them, so every mutating test
/// in every child file must contend on the SAME mutex; `crate::` resolves at
/// this binary root, which is what makes that possible. See AGENTS.md
/// ("Test Layout").
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

#[path = "../src/jsonl_log.rs"]
pub mod jsonl_log;

#[path = "test_agent_complete_tool.rs"]
mod test_agent_complete_tool;

#[path = "test_allowed_roots.rs"]
mod test_allowed_roots;

#[path = "test_apply_patch.rs"]
mod test_apply_patch;

#[path = "test_bash_reset_tool.rs"]
mod test_bash_reset_tool;

#[path = "test_bash_wrapper_quoting.rs"]
mod test_bash_wrapper_quoting;

#[path = "test_bg.rs"]
mod test_bg;

#[path = "test_calculator.rs"]
mod test_calculator;

#[path = "test_datetime.rs"]
mod test_datetime;

#[path = "test_edit.rs"]
mod test_edit;

#[path = "test_edit_integration.rs"]
mod test_edit_integration;

#[path = "test_edit_log.rs"]
mod test_edit_log;

#[path = "test_edit_smoke.rs"]
mod test_edit_smoke;

#[path = "test_edit_utf8_fixes.rs"]
mod test_edit_utf8_fixes;

#[path = "test_file_ops_func061.rs"]
mod test_file_ops_func061;

#[path = "test_file_ops_happy_paths.rs"]
mod test_file_ops_happy_paths;

#[path = "test_get_env_tool.rs"]
mod test_get_env_tool;

#[path = "test_glob_symlink.rs"]
mod test_glob_symlink;

#[path = "test_glob_tool.rs"]
mod test_glob_tool;

#[path = "test_grep_tool.rs"]
mod test_grep_tool;

#[path = "test_lib_registry.rs"]
mod test_lib_registry;

#[path = "test_list_tool.rs"]
mod test_list_tool;

#[path = "test_multiedit.rs"]
mod test_multiedit;

#[path = "test_multiedit_helpers.rs"]
mod test_multiedit_helpers;

#[path = "test_open.rs"]
mod test_open;

#[path = "test_patch_tool.rs"]
mod test_patch_tool;

#[path = "test_path_traversal.rs"]
mod test_path_traversal;

#[path = "test_read_output_cap.rs"]
mod test_read_output_cap;

#[path = "test_read_sections.rs"]
mod test_read_sections;

#[path = "test_read_tool.rs"]
mod test_read_tool;

#[path = "test_schema_validation.rs"]
mod test_schema_validation;

#[path = "test_think.rs"]
mod test_think;

#[path = "test_truncate.rs"]
mod test_truncate;

#[path = "test_xlsx.rs"]
mod test_xlsx;
