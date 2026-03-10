//! File-level snapshot and restore for session state.
//!
//! A [`Snapshot`] captures the byte contents of a set of files at a point in
//! time so they can later be restored with [`restore_snapshot`]. This is used
//! for undo / rollback support within an agent session.

use anyhow::Result;
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::PathBuf;

/// A point-in-time capture of file contents for a session message.
///
/// The `files` map stores each file's absolute path to its raw byte content.
#[derive(Debug, Clone)]
pub struct Snapshot {
    pub id: String,
    pub session_id: String,
    pub message_id: String,
    pub files: HashMap<PathBuf, Vec<u8>>,
    pub created_at: DateTime<Utc>,
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
/// use ragent_core::snapshot::take_snapshot;
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
/// use ragent_core::snapshot::{take_snapshot, restore_snapshot};
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
