//! ANTIPAT M0.3 regression tests: git argument-injection guards.
//!
//! `git_checkout`, `git_cherry_pick`, `git_add`, and `git_remote` previously
//! pushed LLM-supplied operands straight into `git` argv with no
//! `reject_option_like` guard, so a value such as `-b` or `--force` was parsed
//! by git as an option rather than a branch/path/remote (ANTIPAT A-1).

// Holds the crate-wide env lock across the body on purpose: it
// serialises against the PATH-swapping git-shim test, so serialisation
// (not parallelism) is the goal. See AGENTS.md ("Test Layout").
#![allow(clippy::await_holding_lock)]

use std::path::Path;

#[path = "support/mod.rs"]
mod support;

use ragent_tools_vcs::git::*;
use ragent_tools_vcs::{Tool, ToolContext};
use serde_json::json;
use support::setup_git_repo;

fn make_ctx(dir: &Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.to_path_buf(),
        storage: None,
        config: None,
    }
}

// ---------------------------------------------------------------------------
// git_checkout
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_checkout_rejects_option_like_branch() {
    // Holds the crate-wide env lock: `test_git_single_spawn` swaps the
    // process-global `PATH` to a recording shim, which would poison this
    // test's own `git` invocations in a shared test binary.
    let _env_lock = crate::global_env_lock();
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let err = GitCheckoutTool
        .execute(json!({"branch": "--orphan"}), &make_ctx(tmp.path()))
        .await
        .expect_err("an option-like branch must be refused");

    assert!(
        err.to_string().contains("branch"),
        "error must name the rejected operand, got: {err}"
    );
}

#[tokio::test]
async fn test_git_checkout_rejects_option_like_source() {
    // Holds the crate-wide env lock: `test_git_single_spawn` swaps the
    // process-global `PATH` to a recording shim, which would poison this
    // test's own `git` invocations in a shared test binary.
    let _env_lock = crate::global_env_lock();
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let err = GitCheckoutTool
        .execute(
            json!({"paths": ["README.md"], "source": "-f"}),
            &make_ctx(tmp.path()),
        )
        .await
        .expect_err("an option-like restore source must be refused");

    assert!(
        err.to_string().contains("source"),
        "error must name the rejected operand, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// git_cherry_pick
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_cherry_pick_rejects_option_like_commit() {
    // Holds the crate-wide env lock: `test_git_single_spawn` swaps the
    // process-global `PATH` to a recording shim, which would poison this
    // test's own `git` invocations in a shared test binary.
    let _env_lock = crate::global_env_lock();
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let err = GitCherryPickTool
        .execute(json!({"commits": ["--abort"]}), &make_ctx(tmp.path()))
        .await
        .expect_err("an option-like commit operand must be refused");

    assert!(
        err.to_string().contains("commits"),
        "error must name the rejected operand, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// git_add
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_add_rejects_option_like_path() {
    // Holds the crate-wide env lock: `test_git_single_spawn` swaps the
    // process-global `PATH` to a recording shim, which would poison this
    // test's own `git` invocations in a shared test binary.
    let _env_lock = crate::global_env_lock();
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let err = GitAddTool
        .execute(json!({"paths": ["--force"]}), &make_ctx(tmp.path()))
        .await
        .expect_err("an option-like pathspec must be refused");

    assert!(
        err.to_string().contains("paths"),
        "error must name the rejected operand, got: {err}"
    );
}

// ---------------------------------------------------------------------------
// git_remote
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_git_remote_rejects_option_like_name_and_url() {
    // Holds the crate-wide env lock: `test_git_single_spawn` swaps the
    // process-global `PATH` to a recording shim, which would poison this
    // test's own `git` invocations in a shared test binary.
    let _env_lock = crate::global_env_lock();
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let name_err = GitRemoteTool
        .execute(
            json!({"action": "add", "name": "-v", "url": "https://example.com/r.git"}),
            &make_ctx(tmp.path()),
        )
        .await
        .expect_err("an option-like remote name must be refused");
    assert!(
        name_err.to_string().contains("name"),
        "error must name the rejected operand, got: {name_err}"
    );

    let url_err = GitRemoteTool
        .execute(
            json!({"action": "add", "name": "origin", "url": "--upload-pack=evil"}),
            &make_ctx(tmp.path()),
        )
        .await
        .expect_err("an option-like remote url must be refused");
    assert!(
        url_err.to_string().contains("url"),
        "error must name the rejected operand, got: {url_err}"
    );
}

#[tokio::test]
async fn test_git_remote_remove_rejects_option_like_name() {
    // Holds the crate-wide env lock: `test_git_single_spawn` swaps the
    // process-global `PATH` to a recording shim, which would poison this
    // test's own `git` invocations in a shared test binary.
    let _env_lock = crate::global_env_lock();
    let tmp = tempfile::tempdir().unwrap();
    setup_git_repo(tmp.path());

    let err = GitRemoteTool
        .execute(
            json!({"action": "remove", "name": "--all"}),
            &make_ctx(tmp.path()),
        )
        .await
        .expect_err("an option-like remote name must be refused");

    assert!(
        err.to_string().contains("name"),
        "error must name the rejected operand, got: {err}"
    );
}
