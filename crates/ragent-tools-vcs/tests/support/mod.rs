//! Shared test-support helpers for `ragent-tools-vcs` integration tests.
//!
//! `run_shell` and `setup_git_repo` were copy-pasted (with only `-q` flag
//! differences) across the git integration-test files; the single definition
//! lives here. Test binaries pull it in with
//! `#[path = "support/mod.rs"] mod support;`.
//!
//! Not every test binary uses both helpers, so silence dead-code warnings per
//! binary.

#![allow(dead_code)]

use std::path::Path;
use std::process::Command;

/// Run a shell command in `cwd`, panicking on a non-zero exit.
///
/// Uses `cmd /C` on Windows and `sh -c` elsewhere.
pub fn run_shell(cmd: &str, cwd: &Path) {
    let output = if cfg!(target_os = "windows") {
        Command::new("cmd")
            .args(["/C", cmd])
            .current_dir(cwd)
            .output()
    } else {
        Command::new("sh")
            .args(["-c", cmd])
            .current_dir(cwd)
            .output()
    }
    .unwrap_or_else(|e| panic!("failed to run '{cmd}': {e}"));
    assert!(
        output.status.success(),
        "command '{cmd}' failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Create a temporary git repository with an initial commit on branch `main`.
///
/// Removes any existing `.git` first so a reused scratch directory starts
/// clean.
pub fn setup_git_repo(dir: &Path) {
    let _ = std::fs::remove_dir_all(dir.join(".git"));
    run_shell("git init -q", dir);
    run_shell("git config user.email 'test@example.com'", dir);
    run_shell("git config user.name 'Test User'", dir);
    run_shell("git checkout -q -b main", dir);
    std::fs::write(dir.join("README.md"), "# Hello").expect("write README");
    run_shell("git add README.md", dir);
    run_shell("git commit -q -m 'Initial commit'", dir);
}
