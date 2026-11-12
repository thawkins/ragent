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
