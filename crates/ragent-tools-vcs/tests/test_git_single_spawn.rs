//! PERF-057: every git tool must spawn the `git` subprocess exactly once.
//!
//! The former `run_git_or_error` implementation spawned `git` twice (once for
//! stdout/stderr and once to read the exit status). For mutating subcommands
//! that meant double side effects. These tests install a recording `git` shim
//! that delegates to the real executable and logs its working directory, then
//! assert one invocation per tool call.
//!
//! Unix-only: the shim is a `sh` script. The PATH swap is process-global, so
//! the shim faithfully `exec`s the real git (resolved before the swap) and the
//! assertions filter the log by the test's own repository path, which keeps
//! the test correct even if another test binary runs concurrently.

#![cfg(unix)]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::MutexGuard;

#[path = "support/mod.rs"]
mod support;

use ragent_tools_vcs::git::*;
use ragent_tools_vcs::{Tool, ToolContext};
use serde_json::json;
use support::setup_git_repo;

/// The `PATH` swap is process-global, so the two tests in this file must not
/// run concurrently: one test's guard dropping would restore `PATH` mid-run of
/// the other. A blocking guard is held across the test's awaits (the shim
/// spawns a subprocess), which is intentional - the test is single-threaded by
/// construction.

/// The process-wide cwd/env lock (see `tests/all.rs`).
fn shim_lock() -> MutexGuard<'static, ()> {
    crate::global_env_lock()
}

/// Restores `PATH` on drop.
struct PathGuard {
    saved_path: Option<String>,
}

impl Drop for PathGuard {
    fn drop(&mut self) {
        // SAFETY: test-process env restoration; the workspace denies
        // `unsafe_code`, so the allowance is scoped to this test-only guard.
        #[allow(unsafe_code)]
        unsafe {
            match self.saved_path.take() {
                Some(v) => std::env::set_var("PATH", v),
                None => std::env::remove_var("PATH"),
            }
        }
    }
}

/// Install the recording shim and return the guard plus the log file path.
fn install_shim(shim_dir: &Path, log_file: &Path) -> PathGuard {
    fs::create_dir_all(shim_dir).unwrap();
    let real_git = real_git_path();

    // Log the child's working directory, then delegate to the real git so the
    // observable output and exit status are unchanged. The log path is baked
    // into the script at generation time so no `$` needs escaping.
    let script = format!(
        "#!/bin/sh\n\
         printf '%s\\n' \"$PWD\" >> '{}'\n\
         exec '{}' \"$@\"\n",
        log_file.display(),
        real_git.display()
    );
    let shim = shim_dir.join("git");
    fs::write(&shim, script).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&shim, fs::Permissions::from_mode(0o755)).unwrap();
    }

    let guard = PathGuard {
        saved_path: std::env::var("PATH").ok(),
    };

    let mut new_path = shim_dir.to_string_lossy().into_owned();
    if let Some(existing) = &guard.saved_path {
        new_path.push(':');
        new_path.push_str(existing);
    }

    // SAFETY: test-process env mutation, restored by `PathGuard`.
    #[allow(unsafe_code)]
    unsafe {
        std::env::set_var("PATH", new_path);
    }

    guard
}

/// Resolve the real `git` executable before `PATH` is modified.
fn real_git_path() -> PathBuf {
    let out = Command::new("sh")
        .args(["-c", "command -v git"])
        .output()
        .expect("run `command -v git`");
    assert!(out.status.success(), "git must be installed for this test");
    PathBuf::from(String::from_utf8(out.stdout).unwrap().trim())
}

/// Count shim invocations whose working directory is exactly `repo`.
fn spawns_for(log_file: &Path, repo: &Path) -> usize {
    let Ok(contents) = fs::read_to_string(log_file) else {
        return 0;
    };
    let want = repo.to_string_lossy();
    contents.lines().filter(|line| *line == want).count()
}

fn make_ctx(dir: &Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: dir.to_path_buf(),
        storage: None,
        config: None,
    }
}

/// PERF-057: `run_git_or_error` must spawn `git` once, not twice.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_run_git_or_error_spawns_once() {
    let _lock = shim_lock();
    let tmp = tempfile::tempdir().unwrap();
    let shim_dir = tmp.path().join("shim");
    let log_file = tmp.path().join("spawns.log");
    let _guard = install_shim(&shim_dir, &log_file);

    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();
    setup_git_repo(&repo);

    let before = spawns_for(&log_file, &repo);
    let out = run_git_or_error(&["status", "--porcelain"], &repo).unwrap();
    let after = spawns_for(&log_file, &repo);

    assert_eq!(
        after - before,
        1,
        "run_git_or_error must spawn git exactly once (got {})",
        after - before
    );
    assert!(out.is_empty() || out.contains("README.md") || out.contains('#'));
}

/// PERF-057: each git tool spawns `git` exactly once per `execute` call.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_each_git_tool_spawns_once() {
    let _lock = shim_lock();
    let tmp = tempfile::tempdir().unwrap();
    let shim_dir = tmp.path().join("shim");
    let log_file = tmp.path().join("spawns.log");
    let _guard = install_shim(&shim_dir, &log_file);

    let repo = tmp.path().join("repo");
    fs::create_dir_all(&repo).unwrap();
    setup_git_repo(&repo);
    let ctx = make_ctx(&repo);

    let calls: Vec<(&str, Box<dyn Tool>, serde_json::Value)> = vec![
        ("git_status", Box::new(GitStatusTool), json!({})),
        ("git_log", Box::new(GitLogTool), json!({"limit": 2})),
        (
            "git_diff",
            Box::new(GitDiffTool),
            json!({"target": "working"}),
        ),
        ("git_branch", Box::new(GitBranchTool), json!({})),
        ("git_show", Box::new(GitShowTool), json!({"ref": "HEAD"})),
        ("git_tag", Box::new(GitTagTool), json!({"action": "list"})),
        (
            "git_remote",
            Box::new(GitRemoteTool),
            json!({"action": "list"}),
        ),
        (
            "git_stash",
            Box::new(GitStashTool),
            json!({"action": "list"}),
        ),
    ];

    for (name, tool, input) in calls {
        let before = spawns_for(&log_file, &repo);
        tool.execute(input, &ctx)
            .await
            .unwrap_or_else(|e| panic!("{name} failed: {e}"));
        let after = spawns_for(&log_file, &repo);
        assert_eq!(
            after - before,
            1,
            "{name} must spawn git exactly once (got {})",
            after - before
        );
    }
}
