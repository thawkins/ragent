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

use anyhow::{Context, Result, bail};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Wall-clock budget for a single local `git` invocation. An interactive
/// credential prompt or a hung network operation would otherwise block a tokio
/// worker forever (FUNC-050).
pub const GIT_TIMEOUT: Duration = Duration::from_secs(60);

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
/// credential prompts from hanging in non-TTY environments, and enforces
/// [`GIT_TIMEOUT`]: a git that overruns is killed and reported as an error, so
/// a hung credential prompt or network fetch cannot block indefinitely
/// (FUNC-050).
pub fn run_git_output(args: &[&str], cwd: &std::path::Path) -> Result<GitOutput> {
    let mut child = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "false")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("failed to execute `git` — is git installed?")?;

    let deadline = std::time::Instant::now() + GIT_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_status)) => break,
            Ok(None) => {
                if std::time::Instant::now() >= deadline {
                    let _ = child.kill();
                    let _ = child.wait();
                    bail!(
                        "`git {}` timed out after {}s and was killed",
                        args.join(" "),
                        GIT_TIMEOUT.as_secs()
                    );
                }
                std::thread::sleep(Duration::from_millis(20));
            }
            Err(e) => return Err(anyhow::Error::from(e).context("git wait failed")),
        }
    }

    let output = child
        .wait_with_output()
        .context("failed to collect git output")?;

    Ok(GitOutput {
        stdout: output.stdout,
        stderr: output.stderr,
        success: output.status.success(),
    })
}

/// Timeout-bounded `git` invocation off the async runtime.
///
/// Runs the blocking, timeout-bounded [`run_git_output`] on the blocking pool
/// so a slow git cannot park a tokio worker (FUNC-050).
///
/// # Errors
///
/// Returns an error if the git process fails to spawn, times out, or the
/// blocking task panics.
pub async fn run_git_output_async(args: Vec<String>, cwd: std::path::PathBuf) -> Result<GitOutput> {
    tokio::task::spawn_blocking(move || {
        let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_git_output(&arg_refs, &cwd)
    })
    .await
    .context("git task panicked")?
}

/// Async wrapper around [`run_git`] for use inside async tool `execute` bodies.
///
/// # Errors
///
/// As [`run_git`].
pub async fn run_git_async(args: Vec<String>, cwd: std::path::PathBuf) -> Result<(String, String)> {
    let output = run_git_output_async(args, cwd).await?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    Ok((stdout, stderr))
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
