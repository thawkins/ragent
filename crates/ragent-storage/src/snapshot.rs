//! File-level snapshot and restore for session state.
//!
//! A [`Snapshot`] captures the byte contents of a set of files at a point in
//! time so they can later be restored with [`restore_snapshot`]. This is used
//! for undo / rollback support within an agent session.
//!
//! # Incremental snapshots
//!
//! [`incremental_save`] can be used to derive a memory-efficient delta from an
//! existing base snapshot. Only files that changed relative to the base are
//! stored (as unified diffs). The resulting [`IncrementalSnapshot`] can be
//! expanded back into a full [`Snapshot`] via [`IncrementalSnapshot::to_full`].

use anyhow::Result;
use chrono::{DateTime, Utc};
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::path::PathBuf;

/// A point-in-time capture of file contents for a session message.
///
/// The `files` map stores each file's absolute path to its raw byte content.
#[derive(Debug, Clone)]
pub struct Snapshot {
    /// Unique identifier for this snapshot.
    pub id: String,
    /// Session this snapshot belongs to.
    pub session_id: String,
    /// Message that triggered this snapshot.
    pub message_id: String,
    /// Map of file paths to their captured byte contents.
    pub files: HashMap<PathBuf, Vec<u8>>,
    /// Timestamp when the snapshot was taken.
    pub created_at: DateTime<Utc>,
}

/// A compact delta relative to a base [`Snapshot`].
///
/// Only files that have changed are stored (as unified diffs). Files present
/// in the base but absent from `changed` are implicitly carried forward
/// unchanged.
#[derive(Debug, Clone)]
pub struct IncrementalSnapshot {
    /// Unique identifier for this incremental snapshot.
    pub id: String,
    /// ID of the [`Snapshot`] this delta was computed from.
    pub base_id: String,
    /// Session this snapshot belongs to.
    pub session_id: String,
    /// Message that triggered this snapshot.
    pub message_id: String,
    /// Only the files that changed vs the base: path → unified diff text.
    pub diffs: HashMap<PathBuf, String>,
    /// Files added since the base (new files, stored as full content).
    pub added: HashMap<PathBuf, Vec<u8>>,
    /// Files that were deleted since the base.
    pub deleted: Vec<PathBuf>,
    /// Timestamp when the delta was taken.
    pub created_at: DateTime<Utc>,
}

impl IncrementalSnapshot {
    /// Expand this incremental snapshot into a full [`Snapshot`] by applying
    /// the stored diffs against the provided base snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error if a diff cannot be applied (base file missing or diff
    /// corrupt). If a base file was pure binary the diff text will be empty and
    /// the file will be carried forward unchanged.
    pub fn to_full(&self, base: &Snapshot) -> Result<Snapshot> {
        let mut files = base.files.clone();

        // Remove deleted files
        for path in &self.deleted {
            files.remove(path);
        }

        // Apply text diffs
        for (path, diff_text) in &self.diffs {
            if diff_text.is_empty() {
                // Binary file or empty diff — carry base forward unchanged
                continue;
            }
            let base_bytes = files.get(path).map(|b| b.as_slice()).unwrap_or(b"");
            let base_str = String::from_utf8_lossy(base_bytes);
            let patched = apply_unified_diff(&base_str, diff_text)?;
            files.insert(path.clone(), patched.into_bytes());
        }

        // Add new files
        for (path, content) in &self.added {
            files.insert(path.clone(), content.clone());
        }

        Ok(Snapshot {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: self.session_id.clone(),
            message_id: self.message_id.clone(),
            files,
            created_at: self.created_at,
        })
    }
}

/// Take a snapshot of the specified files by reading them into memory.
///
/// Files that do not exist or are not regular files are silently skipped.
///
/// # Errors
///
/// Returns an error if a file exists but cannot be read.
///
/// # Examples
///
/// ```rust,no_run
/// use std::path::PathBuf;
/// use ragent_storage::snapshot::take_snapshot;
///
/// let files = vec![PathBuf::from("/tmp/example.txt")];
/// let snap = take_snapshot("session-1", "msg-1", &files).unwrap();
/// assert_eq!(snap.session_id, "session-1");
/// ```
pub fn take_snapshot(session_id: &str, message_id: &str, files: &[PathBuf]) -> Result<Snapshot> {
    let mut file_contents = HashMap::new();

    for path in files {
        if path.exists() && path.is_file() {
            let content = std::fs::read(path)?;
            file_contents.insert(path.clone(), content);
        }
    }

    Ok(Snapshot {
        id: uuid::Uuid::new_v4().to_string(),
        session_id: session_id.to_string(),
        message_id: message_id.to_string(),
        files: file_contents,
        created_at: Utc::now(),
    })
}

/// Restore a snapshot by writing all files back to disk.
///
/// Parent directories are created as needed.
///
/// # Errors
///
/// Returns an error if a directory cannot be created or a file cannot be written.
///
/// # Examples
///
/// ```rust,no_run
/// use std::path::PathBuf;
/// use ragent_storage::snapshot::{take_snapshot, restore_snapshot};
///
/// let files = vec![PathBuf::from("/tmp/example.txt")];
/// let snap = take_snapshot("session-1", "msg-1", &files).unwrap();
/// // … later, restore the captured file contents
/// restore_snapshot(&snap).unwrap();
/// ```
pub fn restore_snapshot(snapshot: &Snapshot) -> Result<()> {
    for (path, content) in &snapshot.files {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
    }
    Ok(())
}

/// Compute a memory-efficient incremental snapshot relative to a base.
///
/// Only files that have changed (or are new/deleted) are stored. Text files
/// are stored as unified diffs; binary files are treated as changed-in-full and
/// stored in `added`. Files that are identical to the base are omitted.
///
/// # Errors
///
/// Returns an error if any of the `current_files` cannot be read.
///
/// # Examples
///
/// ```rust,no_run
/// use std::path::PathBuf;
/// use ragent_storage::snapshot::{take_snapshot, incremental_save};
///
/// let files = vec![PathBuf::from("/tmp/example.txt")];
/// let base = take_snapshot("session-1", "msg-1", &files).unwrap();
/// let delta = incremental_save(&base, "msg-2", &files).unwrap();
/// assert_eq!(delta.base_id, base.id);
/// ```
pub fn incremental_save(
    base: &Snapshot,
    message_id: &str,
    current_files: &[PathBuf],
) -> Result<IncrementalSnapshot> {
    let mut diffs: HashMap<PathBuf, String> = HashMap::new();
    let mut added: HashMap<PathBuf, Vec<u8>> = HashMap::new();
    let mut deleted: Vec<PathBuf> = Vec::new();

    let current_set: std::collections::HashSet<&PathBuf> = current_files.iter().collect();

    // Detect deleted files
    for path in base.files.keys() {
        if !current_set.contains(path) && !path.exists() {
            deleted.push(path.clone());
        }
    }

    // Detect added and changed files
    for path in current_files {
        if !path.exists() || !path.is_file() {
            continue;
        }
        let current_bytes = std::fs::read(path)?;

        match base.files.get(path) {
            None => {
                // New file
                added.insert(path.clone(), current_bytes);
            }
            Some(base_bytes) if base_bytes == &current_bytes => {
                // Unchanged — skip
            }
            Some(base_bytes) => {
                // Changed — try to produce a unified diff for text files
                let base_str = std::str::from_utf8(base_bytes);
                let current_str = std::str::from_utf8(&current_bytes);
                match (base_str, current_str) {
                    (Ok(old), Ok(new)) => {
                        let diff = make_unified_diff(old, new);
                        diffs.insert(path.clone(), diff);
                    }
                    _ => {
                        // Binary file — store full content as "added"
                        added.insert(path.clone(), current_bytes);
                    }
                }
            }
        }
    }

    Ok(IncrementalSnapshot {
        id: uuid::Uuid::new_v4().to_string(),
        base_id: base.id.clone(),
        session_id: base.session_id.clone(),
        message_id: message_id.to_string(),
        diffs,
        added,
        deleted,
        created_at: Utc::now(),
    })
}

/// Produce a unified diff string from `old` to `new` using the `similar` crate.
fn make_unified_diff(old: &str, new: &str) -> String {
    let diff = TextDiff::from_lines(old, new);
    let mut out = String::new();
    for group in diff.grouped_ops(3) {
        for op in &group {
            for change in diff.iter_changes(op) {
                let sign = match change.tag() {
                    ChangeTag::Delete => "-",
                    ChangeTag::Insert => "+",
                    ChangeTag::Equal => " ",
                };
                out.push_str(sign);
                out.push_str(change.value());
            }
        }
    }
    out
}

/// Apply a unified diff (as produced by [`make_unified_diff`]) to `base`.
///
/// This is a simplified line-based patch that handles the `+`/`-`/` ` prefix
/// format produced by [`make_unified_diff`]. It does not attempt to handle
/// hunk headers — changes are applied in the order they appear.
fn apply_unified_diff(base: &str, diff: &str) -> Result<String> {
    // Fast path: if diff is empty return base unchanged
    if diff.trim().is_empty() {
        return Ok(base.to_string());
    }

    let mut result = String::new();
    let base_lines: Vec<&str> = base.lines().collect();
    let mut base_idx = 0;

    for diff_line in diff.lines() {
        if diff_line.is_empty() {
            continue;
        }
        let (tag, content) = diff_line.split_at(1);
        match tag {
            " " => {
                // Context line — output base line and advance
                if base_idx < base_lines.len() {
                    result.push_str(base_lines[base_idx]);
                    result.push('\n');
                    base_idx += 1;
                }
            }
            "-" => {
                // Removed line — skip the matching base line
                if base_idx < base_lines.len() {
                    base_idx += 1;
                }
            }
            "+" => {
                // Added line — emit without consuming base
                result.push_str(content);
                result.push('\n');
            }
            _ => {}
        }
    }

    // Append any remaining base lines not covered by context
    while base_idx < base_lines.len() {
        result.push_str(base_lines[base_idx]);
        result.push('\n');
        base_idx += 1;
    }

    Ok(result)
}
