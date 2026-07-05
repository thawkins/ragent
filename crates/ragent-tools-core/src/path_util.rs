//! Shared path-resolution helper for file-based tools.
//!
//! Provides [`resolve_path`] — a trivial leaf helper that resolves a path
//! string against a working directory.  Previously this function was
//! copy-pasted into every file tool (see `DUPPLAN.md` Milestone B); it now
//! lives here as the single source of truth.

use std::path::{Path, PathBuf};

/// Resolve a path string against a working directory.
///
/// If `path_str` is absolute, returns it as-is. Otherwise, joins it to
/// `working_dir`.
///
/// # Arguments
///
/// * `working_dir` - The base directory to resolve relative paths against.
/// * `path_str` - The path string to resolve (may be absolute or relative).
///
/// # Returns
///
/// The resolved absolute [`PathBuf`].
#[must_use]
pub fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}
