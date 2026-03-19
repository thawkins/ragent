//! Per-file locking for edit operations.
//!
//! Provides a global registry of per-file mutexes to serialize concurrent edits
//! to the same file, preventing race conditions when multiple `edit` or `multiedit`
//! tool calls target the same file.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};

/// Global registry of per-file locks.
///
/// Each file path maps to a `Mutex<()>` that must be acquired before reading
/// and writing that file in an edit operation.
/// Global registry of per-file locks.
///
/// Each file path maps to a `Mutex<()>` that must be acquired before reading
/// and writing that file in an edit operation.
static FILE_LOCKS: LazyLock<Arc<RwLock<HashMap<PathBuf, Arc<Mutex<()>>>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));

/// Acquire an exclusive lock for editing a file.
///
/// Returns a guard that must be held for the duration of the read-modify-write
/// sequence. Multiple edits to different files can proceed in parallel; edits
/// to the same file are serialized.
pub async fn lock_file(path: &Path) -> OwnedMutexGuard<()> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    // Get or create the mutex for this file
    let mutex = {
        let mut locks: tokio::sync::RwLockWriteGuard<'_, HashMap<PathBuf, Arc<Mutex<()>>>> =
            FILE_LOCKS.write().await;
        locks
            .entry(canonical.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };

    // Acquire the lock (will wait if another edit is in progress)
    mutex.lock_owned().await
}

/// Periodically clean up locks for files that are no longer being edited.
///
/// This is optional; locks are lightweight and don't leak resources, but this
/// prevents the HashMap from growing unbounded over a long-running session.
#[allow(dead_code)]
pub async fn cleanup_unused_locks() {
    let mut locks: tokio::sync::RwLockWriteGuard<'_, HashMap<PathBuf, Arc<Mutex<()>>>> =
        FILE_LOCKS.write().await;
    locks.retain(|_, mutex| Arc::strong_count(mutex) > 1);
}
