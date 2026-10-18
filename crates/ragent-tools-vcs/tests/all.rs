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

#[path = "test_api_error_handling.rs"]
mod test_api_error_handling;

#[path = "test_backward_compat.rs"]
mod test_backward_compat;

#[path = "test_build_reverse_prompt.rs"]
mod test_build_reverse_prompt;

#[path = "test_git_single_spawn.rs"]
mod test_git_single_spawn;

#[path = "test_git_tools.rs"]
mod test_git_tools;

#[path = "test_github_actions.rs"]
mod test_github_actions;

#[path = "test_github_actions_extra.rs"]
mod test_github_actions_extra;

#[path = "test_github_auth_coverage.rs"]
mod test_github_auth_coverage;

#[path = "test_github_client_coverage.rs"]
mod test_github_client_coverage;

#[path = "test_github_client_writes.rs"]
mod test_github_client_writes;

#[path = "test_github_merge_method.rs"]
mod test_github_merge_method;

#[path = "test_gitlab_auth_coverage.rs"]
mod test_gitlab_auth_coverage;

#[path = "test_gitlab_client_coverage.rs"]
mod test_gitlab_client_coverage;

#[path = "test_gitlab_fetch_methods.rs"]
mod test_gitlab_fetch_methods;

#[path = "test_gitlab_helpers_coverage.rs"]
mod test_gitlab_helpers_coverage;

#[path = "test_gitlab_legacy_migration.rs"]
mod test_gitlab_legacy_migration;

#[path = "test_gitlab_token_cache.rs"]
mod test_gitlab_token_cache;

#[path = "test_ms0_git_arg_guards.rs"]
mod test_ms0_git_arg_guards;

#[path = "test_ms5_vocab_policy.rs"]
mod test_ms5_vocab_policy;

#[path = "test_parse_reverse_repo_formats.rs"]
mod test_parse_reverse_repo_formats;

#[path = "test_percent_encoding.rs"]
mod test_percent_encoding;

#[path = "test_recursive_tree_fetch.rs"]
mod test_recursive_tree_fetch;

#[path = "test_repo_input_validation.rs"]
mod test_repo_input_validation;

#[path = "test_repo_metadata.rs"]
mod test_repo_metadata;

#[path = "test_repo_parse_url.rs"]
mod test_repo_parse_url;

#[path = "test_repo_readme.rs"]
mod test_repo_readme;

#[path = "test_repo_root_tree.rs"]
mod test_repo_root_tree;

#[path = "test_shared_http_client.rs"]
mod test_shared_http_client;

#[path = "test_vcs_error_classifier.rs"]
mod test_vcs_error_classifier;

#[path = "test_vcs_registry_coverage.rs"]
mod test_vcs_registry_coverage;
