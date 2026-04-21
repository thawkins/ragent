//! Per-file locking for edit operations.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock};
use tokio::sync::{Mutex, OwnedMutexGuard, RwLock};

static FILE_LOCKS: LazyLock<Arc<RwLock<HashMap<PathBuf, Arc<Mutex<()>>>>>> =
    LazyLock::new(|| Arc::new(RwLock::new(HashMap::new())));

/// Acquire an exclusive lock for editing a file.
pub async fn lock_file(path: &Path) -> OwnedMutexGuard<()> {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());

    let mutex = {
        let mut locks = FILE_LOCKS.write().await;
        locks
            .entry(canonical)
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone()
    };

    mutex.lock_owned().await
}
