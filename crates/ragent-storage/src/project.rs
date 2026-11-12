//! Project identity for sessions (spec MULTISESSION, milestone M4).
//!
//! A session's project is the git work tree that contains its directory: the
//! nearest ancestor (the directory itself included) holding a `.git` entry,
//! which is a directory in a normal clone and a file in a linked worktree or
//! submodule. A directory outside any work tree is its own project.
//!
//! The result is stored in `sessions.project_id` and is what the session
//! sidebar groups by. It is computed from the filesystem once, when the
//! session is created (or backfilled on migration), never per render.

use std::path::Path;

/// Returns the project root for a session directory, as a string.
///
/// # Arguments
///
/// * `directory` - the session's working directory as stored in
///   `sessions.directory`.
///
/// # Returns
///
/// The canonical path of the nearest ancestor of `directory` (inclusive)
/// that contains a `.git` entry; otherwise the canonical `directory`. When
/// `directory` does not exist (deleted or moved), it is returned unchanged
/// so the session still groups under its last known location.
pub fn project_root(directory: &str) -> String {
    let path = Path::new(directory);
    let Ok(canonical) = path.canonicalize() else {
        return directory.to_string();
    };
    canonical
        .ancestors()
        .find(|dir| dir.join(".git").exists())
        .unwrap_or(&canonical)
        .display()
        .to_string()
}

/// A stored project: a local worktree with an optional base model and an
/// optional description of the remote it was checked out from (spec
/// `projects`).
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProjectRow {
    /// Unique project identifier (UUID v4).
    pub id: String,
    /// Display name; defaults to the worktree's directory name.
    pub name: String,
    /// Canonical root of the local checkout (see [`project_root`]).
    pub worktree: String,
    /// Provider of the base model new sessions start with, if set.
    pub provider_id: Option<String>,
    /// Base model new sessions start with, if set.
    pub model_id: Option<String>,
    /// The remote the worktree was checked out from, if recognised.
    pub remote: Option<ProjectRemoteRow>,
    /// Whether remote detection has run for this project.
    pub remote_checked: bool,
    /// ISO-8601 creation timestamp.
    pub created_at: String,
    /// ISO-8601 last-updated timestamp.
    pub updated_at: String,
    /// ISO-8601 archive timestamp, if the project was removed from the list.
    pub archived_at: Option<String>,
}

/// The stored description of a project's remote.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ProjectRemoteRow {
    /// Hosting system: `github` or `gitlab`.
    pub kind: String,
    /// Host name, e.g. `github.com` or a self-hosted GitLab host.
    pub host: String,
    /// Repository path on the host: `owner/repo` or `group/sub/project`.
    pub path: String,
    /// Remote URL with any embedded credentials removed.
    pub url: String,
    /// Git remote name, normally `origin`.
    pub remote_name: String,
    /// The remote's default branch, when known.
    pub default_branch: Option<String>,
}

/// The git checkout a session owns, plus its last known request count.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct SessionCheckout {
    /// Branch the session's worktree is checked out at.
    pub branch: Option<String>,
    /// Session-owned git worktree; `None` when the session runs in the
    /// project worktree.
    pub worktree: Option<String>,
    /// Last known count of pull/merge requests for `branch`; `None` when
    /// unknown.
    pub pr_count: Option<i64>,
}

/// The default display name for a project rooted at `worktree`: its last
/// path component, or the whole path when there is none (e.g. `/`).
#[must_use]
pub fn default_project_name(worktree: &str) -> String {
    Path::new(worktree).file_name().map_or_else(
        || worktree.to_string(),
        |n| n.to_string_lossy().into_owned(),
    )
}
