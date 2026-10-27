//! Consolidated integration-test target: one test binary per crate.
//!
//! Each former top-level `tests/test_*.rs` file is included as a child
//! module via `#[path]`, so it keeps its own file-relative `mod` /
//! `#[path]` resolution and its `test_<file>::<fn>` test name. See
//! `AGENTS.md` ("Test Layout") and
//! `docs/reports/testconsolidation-review.md`.

#![allow(clippy::duplicate_mod)]
// ^ Intentional: each `#[path]`-included child file declares its own
// `mod support;` / `mod <fixture>;`, so a shared helper file is a module of
// several children. Under one-binary-per-crate that loads it more than once,
// which clippy::duplicate_mod flags; hoisting it would require editing every
// child file. See AGENTS.md ("Test Layout").

/// The one process-wide lock that serialises tests which mutate or observe
/// process-global state (`set_current_dir`, `XDG_CONFIG_HOME`, `HOME`,
/// `PATH`).
///
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

#[path = "test_add.rs"]
mod test_add;

#[path = "test_bridge.rs"]
mod test_bridge;

#[path = "test_command_adapter.rs"]
mod test_command_adapter;

#[path = "test_commands.rs"]
mod test_commands;

#[path = "test_control.rs"]
mod test_control;

#[path = "test_descriptor.rs"]
mod test_descriptor;

#[path = "test_fixtures.rs"]
mod test_fixtures;

#[path = "test_fr032_plugin_bridge.rs"]
mod test_fr032_plugin_bridge;

#[path = "test_harness.rs"]
mod test_harness;

#[path = "test_help.rs"]
mod test_help;

#[path = "test_host_api.rs"]
mod test_host_api;

#[path = "test_lifecycle.rs"]
mod test_lifecycle;

#[path = "test_manifest.rs"]
mod test_manifest;

#[path = "test_marketplace.rs"]
mod test_marketplace;

#[path = "test_marketplace_add.rs"]
mod test_marketplace_add;

#[path = "test_marketplace_ops.rs"]
mod test_marketplace_ops;

#[path = "test_marketplace_registry_bound.rs"]
mod test_marketplace_registry_bound;

#[path = "test_marketplace_show.rs"]
mod test_marketplace_show;

#[path = "test_merge_contributions.rs"]
mod test_merge_contributions;

#[path = "test_ms04_plugin_paths.rs"]
mod test_ms04_plugin_paths;

#[path = "test_non_js_plugin.rs"]
mod test_non_js_plugin;

#[path = "test_remove.rs"]
mod test_remove;

#[path = "test_report.rs"]
mod test_report;

#[path = "test_runtime.rs"]
mod test_runtime;

#[path = "test_session.rs"]
mod test_session;

#[path = "test_store.rs"]
mod test_store;

#[path = "test_store_endpoint_resolution.rs"]
mod test_store_endpoint_resolution;

#[path = "test_store_enrich.rs"]
mod test_store_enrich;

#[path = "test_store_fetch.rs"]
mod test_store_fetch;

#[path = "test_store_git_source.rs"]
mod test_store_git_source;

#[path = "test_store_index.rs"]
mod test_store_index;

#[path = "test_store_installed_ids.rs"]
mod test_store_installed_ids;

#[path = "test_store_provider.rs"]
mod test_store_provider;

#[path = "test_store_seam.rs"]
mod test_store_seam;

#[path = "test_stores_check.rs"]
mod test_stores_check;

#[path = "test_stores_report.rs"]
mod test_stores_report;

#[path = "test_tool_adapter.rs"]
mod test_tool_adapter;
