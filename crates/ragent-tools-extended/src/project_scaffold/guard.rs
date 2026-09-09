//! Target-directory guard I/O for the `/new` scaffold command (FR-002).
//!
//! The pure emptiness decision lives in [`super::flags`]
//! ([`super::flags::validate_target_directory`]) so it stays testable without
//! a filesystem. This module is the thin I/O shell around it: it gathers the
//! [`FilePathEntry`] list from the real target directory, maps unreadable or
//! non-directory targets to [`ScaffoldError::TargetUnreadable`], and runs the
//! decision so a failing guard aborts before any filesystem mutation
//! (FR-002: report offending entries, exit without modifying anything).
//!
//! Hidden entries are included on purpose: a stray `.gitkeep`, `.env`, or
//! other dotfile is a real occupancy signal, not a ragent artifact. Only the
//! ragent-owned names (`.ragent`, `log`, `target`) are allowed.

use std::fs;
use std::path::Path;

use super::flags::{FilePathEntry, ScaffoldError, validate_target_directory};

/// Gather [`FilePathEntry`] values for `target`, refusing non-directories.
///
/// # Errors
///
/// [`ScaffoldError::TargetUnreadable`] when `target` does not exist as a
/// directory or `read_dir` fails.
pub fn read_target_entries(target: &Path) -> Result<Vec<FilePathEntry>, ScaffoldError> {
    if !target.is_dir() {
        return Err(ScaffoldError::TargetUnreadable(format!(
            "{}: not a directory",
            target.display()
        )));
    }
    let read = fs::read_dir(target)
        .map_err(|err| ScaffoldError::TargetUnreadable(format!("{}: {err}", target.display())))?;
    let mut entries = Vec::new();
    for entry in read {
        let entry = entry.map_err(|err| {
            ScaffoldError::TargetUnreadable(format!("{}: {err}", target.display()))
        })?;
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.path().is_dir();
        entries.push(FilePathEntry { name, is_dir });
    }
    // Deterministic offending-entry reports regardless of `read_dir` order.
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

/// Enforce the FR-002 empty-directory guard on the real target directory.
///
/// Runs the full event-driven sequence: gather entries, decide emptiness,
/// and refuse with the offending entry names when the directory holds
/// anything beyond the ragent artifact allowlist. The caller must not have
/// mutated the filesystem before calling this.
///
/// # Errors
///
/// [`ScaffoldError::DirectoryNotEmpty`] with the offending entry names
/// (directories suffixed with `/`), or [`ScaffoldError::TargetUnreadable`]
/// when the target cannot be inspected.
pub fn enforce_empty_directory_guard(target: &Path) -> Result<(), ScaffoldError> {
    let entries = read_target_entries(target)?;
    validate_target_directory(&entries)
}
