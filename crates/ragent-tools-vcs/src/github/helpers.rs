//! Shared GitHub client/detection helpers.
//!
//! Every `github_*.rs` module previously carried its own copy of the client
//! construction and owner/repo detection; they live here once (see
//! `ANTIPAT.md` M3.9).

use anyhow::{Context, Result};

use super::client::GitHubClient;
use crate::ToolContext;

/// Canonical "GitHub not authenticated" error string.
///
/// Every GitHub tool surfaces this one message when no token is available
/// (ANTIPAT.md I-4); do not re-word it per module.
pub(crate) const NOT_AUTHENTICATED: &str =
    "GitHub not authenticated. Run /github login to authenticate.";

/// Build an authenticated [`GitHubClient`].
///
/// # Errors
///
/// Returns an error when the client cannot be constructed (no stored token).
pub(crate) fn make_client() -> Result<GitHubClient> {
    GitHubClient::new().context(NOT_AUTHENTICATED)
}

/// Resolve `owner/repo` from the working directory.
///
/// # Errors
///
/// Returns an error when no GitHub remote is detected.
pub(crate) fn detect_repo(ctx: &ToolContext) -> Result<(String, String)> {
    GitHubClient::detect_repo(&ctx.working_dir).ok_or_else(|| {
        anyhow::anyhow!(
            "Could not detect GitHub repository from git remote. \
             Ensure you're in a git repo with a GitHub remote."
        )
    })
}

/// Build a client and resolve `owner/repo` in one step.
///
/// # Errors
///
/// Returns an error when the client cannot be built or the repo is undetected.
pub(crate) fn detect(ctx: &ToolContext) -> Result<(GitHubClient, String, String)> {
    let client = make_client()?;
    let (owner, repo) = detect_repo(ctx)?;
    Ok((client, owner, repo))
}
