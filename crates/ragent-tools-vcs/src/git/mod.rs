//! Local git workspace tools for ragent.
//!
//! These tools execute the `git` command-line executable in the agent's working
//! directory and let the LLM inspect and manipulate any local git repository.

pub mod git_add;
pub mod git_branch;
pub mod git_checkout;
pub mod git_cherry_pick;
pub mod git_clone;
pub mod git_commit;
pub mod git_diff;
pub mod git_fetch;
pub mod git_log;
pub mod git_merge;
pub mod git_pull;
pub mod git_push;
pub mod git_remote;
pub mod git_reset;
pub mod git_show;
pub mod git_stash;
pub mod git_status;
pub mod git_tag;

pub use git_add::GitAddTool;
pub use git_branch::GitBranchTool;
pub use git_checkout::GitCheckoutTool;
pub use git_cherry_pick::GitCherryPickTool;
pub use git_clone::GitCloneTool;
pub use git_commit::GitCommitTool;
pub use git_diff::GitDiffTool;
pub use git_fetch::GitFetchTool;
pub use git_log::GitLogTool;
pub use git_merge::GitMergeTool;
pub use git_pull::GitPullTool;
pub use git_push::GitPushTool;
pub use git_remote::GitRemoteTool;
pub use git_reset::GitResetTool;
pub use git_show::GitShowTool;
pub use git_stash::GitStashTool;
pub use git_status::GitStatusTool;
pub use git_tag::GitTagTool;

use anyhow::{Context, Result};
use std::process::Command;

/// Captured result of a single `git` subprocess invocation.
pub struct GitOutput {
    /// Raw stdout bytes captured from the child process.
    pub stdout: Vec<u8>,
    /// Raw stderr bytes captured from the child process.
    pub stderr: Vec<u8>,
    /// Whether the child exited with a zero status.
    pub success: bool,
}

/// Spawn `git` once and capture its full output (stdout, stderr, exit status).
///
/// This is the single spawn point for every local git tool: deriving stdout,
/// stderr, and status from one [`std::process::Output`] prevents mutating
/// subcommands (commit, push, merge, ...) from running twice per call
/// (PERF-057).
///
/// Sets `GIT_TERMINAL_PROMPT=0` and `GIT_ASKPASS=false` to prevent interactive
/// credential prompts from hanging in non-TTY environments.
pub fn run_git_output(args: &[&str], cwd: &std::path::Path) -> Result<GitOutput> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "false")
        .output()
        .context("failed to execute `git` — is git installed?")?;

    Ok(GitOutput {
        stdout: output.stdout,
        stderr: output.stderr,
        success: output.status.success(),
    })
}

/// Run a git command in the given working directory and return stdout and stderr.
pub fn run_git(args: &[&str], cwd: &std::path::Path) -> Result<(String, String)> {
    let output = run_git_output(args, cwd)?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Ok((stdout, stderr))
}

/// Run a git command and return stdout only, treating non-zero exit as an error.
pub fn run_git_or_error(args: &[&str], cwd: &std::path::Path) -> Result<String> {
    let output = run_git_output(args, cwd)?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    if output.success {
        Ok(stdout)
    } else {
        let msg = if stderr.is_empty() { stdout } else { stderr };
        Err(anyhow::anyhow!(
            "git {} failed: {}",
            args.join(" "),
            msg.trim()
        ))
    }
}
