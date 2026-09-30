//! Shared GitLab client/detection helpers.
//!
//! Every `gitlab_*.rs` module previously carried its own copy of the client
//! construction and project detection; they live here once (see `ANTIPAT.md`
//! M3.9).

use anyhow::{Context, Result};

use super::client::GitLabClient;
use crate::ToolContext;

/// Canonical "GitLab not configured" error string.
///
/// Every GitLab tool surfaces this one message when no configuration is
/// available (ANTIPAT.md I-4); do not re-word it per module.
pub(crate) const NOT_CONFIGURED: &str = "GitLab not configured. Run /gitlab setup to configure.";

/// Build a [`GitLabClient`] from the context's storage handle.
///
/// # Errors
///
/// Returns an error when storage is unavailable or GitLab is not configured.
pub(crate) fn make_client(ctx: &ToolContext) -> Result<GitLabClient> {
    let storage = ctx
        .storage
        .as_deref()
        .context("Storage not available for GitLab client")?;
    GitLabClient::new(storage).context(NOT_CONFIGURED)
}

/// Resolve the URL-encoded project path from the working directory.
///
/// # Errors
///
/// Returns an error when no GitLab remote is detected.
pub(crate) fn detect_project(ctx: &ToolContext) -> Result<String> {
    GitLabClient::detect_project(&ctx.working_dir).ok_or_else(|| {
        anyhow::anyhow!(
            "Could not detect GitLab project from git remote. \
             Ensure you're in a git repo with a GitLab remote."
        )
    })
}

/// Build a client and resolve the project path in one step.
///
/// # Errors
///
/// Returns an error when the client cannot be built or the project is
/// undetected.
pub(crate) fn detect(ctx: &ToolContext) -> Result<(GitLabClient, String)> {
    let client = make_client(ctx)?;
    let project = detect_project(ctx)?;
    Ok((client, project))
}
