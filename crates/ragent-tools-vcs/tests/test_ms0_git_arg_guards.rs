//! ANTIPAT M0.3 regression tests: git argument-injection guards.
//!
//! `git_checkout`, `git_cherry_pick`, `git_add`, and `git_remote` previously
//! pushed LLM-supplied operands straight into `git` argv with no
//! `reject_option_like` guard, so a value such as `-b` or `--force` was parsed
//! by git as an option rather than a branch/path/remote (ANTIPAT A-1).

use std::path::Path;

use ragent_tools_vcs::git::*;
use ragent_tools_vcs::{Tool, ToolContext};
use serde_json::json;

/// Run a shell command in `cwd`, panicking on a non-zero exit.
fn run_shell(cmd: &str, cwd: &Path) {
    let output = std::process::Command::new("sh")
        .args(["-c", cmd])
        .current_dir(cwd)
        .output()
        .unwrap_or_else(|e| panic!("failed to run '{cmd}': {e}"));
    assert!(
        output.status.success(),
        "command '{cmd}' failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Create a temporary git repository with one commit.
fn setup_git_repo(dir: &Path) {
    run_shell("git init", dir);
    run_shell("git config user.email 'test@example.com'", dir);
    run_shell("git config user.name 'Test User'", dir);
    run_shell("git checkout -b main", dir);
    std::fs::write(dir.join("README.md"), "# Hello").unwrap();
    run_shell("git add README.md", dir);
    run_shell("git commit -m 'Initial commit'", dir);
}

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
