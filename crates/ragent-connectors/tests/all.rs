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

#[path = "test_catalogue_enrich.rs"]
mod test_catalogue_enrich;

#[path = "test_connector_auth.rs"]
mod test_connector_auth;

#[path = "test_connector_bridge.rs"]
mod test_connector_bridge;

#[path = "test_connector_browse.rs"]
mod test_connector_browse;

#[path = "test_connector_commands.rs"]
mod test_connector_commands;

#[path = "test_connector_descriptor.rs"]
mod test_connector_descriptor;

#[path = "test_connector_fetch.rs"]
mod test_connector_fetch;

#[path = "test_connector_fixtures.rs"]
mod test_connector_fixtures;

#[path = "test_connector_harness.rs"]
mod test_connector_harness;

#[path = "test_connector_install.rs"]
mod test_connector_install;

#[path = "test_connector_lifecycle.rs"]
mod test_connector_lifecycle;

#[path = "test_connector_management.rs"]
mod test_connector_management;

#[path = "test_connector_manifest.rs"]
mod test_connector_manifest;

#[path = "test_connector_store.rs"]
mod test_connector_store;

#[path = "test_connector_store_index.rs"]
mod test_connector_store_index;

#[path = "test_connector_store_ops.rs"]
mod test_connector_store_ops;

#[path = "test_connector_stores_report.rs"]
mod test_connector_stores_report;

#[path = "test_marketplace_merge.rs"]
mod test_marketplace_merge;

#[path = "test_marketplace_ops.rs"]
mod test_marketplace_ops;

#[path = "test_marketplace_show.rs"]
mod test_marketplace_show;
