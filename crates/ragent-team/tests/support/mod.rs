//! Shared test-support helpers for `ragent-team` integration tests.
//!
//! Provides [`setup_workspace`] — a temp-directory + path helper used across
//! team test files.  Previously copy-pasted into 5 test files (see
//! `DUPPLAN.md` Milestone I, `cargo dupes` group 37).

use std::path::PathBuf;

use tempfile::TempDir;

/// Create a temp directory and return it along with a `.ragent/teams` path
/// inside it.
///
/// # Returns
///
/// A tuple of `(TempDir, PathBuf)` where the `TempDir` owns the temporary
/// directory (and cleans it up when dropped) and the `PathBuf` points to the
/// `.ragent/teams` subdirectory within it.
#[must_use]
pub fn setup_workspace() -> (TempDir, PathBuf) {
    let dir = TempDir::new().unwrap();
    let teams_dir = dir.path().join(".ragent").join("teams");
    std::fs::create_dir_all(&teams_dir).unwrap();
    (dir, teams_dir)
}
