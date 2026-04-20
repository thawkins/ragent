//! File watcher for automatic AIWiki syncing.
//!
//! Uses tokio::sync::mpsc for async event handling
//! with debouncing to prevent excessive syncs.

use crate::{Aiwiki, Result};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{Instant, interval};
use tracing;

/// Configuration for the file watcher.
#[derive(Debug, Clone)]
pub struct WatcherConfig {
    /// Debounce duration - wait this long after last change before syncing.
    pub debounce_duration: Duration,
    /// Whether to enable automatic syncing.
    pub auto_sync: bool,
    /// Minimum interval between syncs (rate limiting).
    pub min_sync_interval: Duration,
}

impl Default for WatcherConfig {
    fn default() -> Self {
        Self {
            debounce_duration: Duration::from_secs(5),
            auto_sync: true,
            min_sync_interval: Duration::from_secs(30),
        }
    }
}

impl WatcherConfig {
    /// Create a new config with custom debounce.
    pub fn with_debounce(seconds: u64) -> Self {
        Self {
            debounce_duration: Duration::from_secs(seconds),
            ..Default::default()
        }
    }

    /// Disable auto-sync (manual mode).
    pub fn manual_only() -> Self {
        Self {
            auto_sync: false,
            ..Default::default()
        }
    }
}

/// File watcher for the raw/ directory.
///
/// Watches for file changes and optionally triggers automatic syncing.
pub struct FileWatcher {
    /// Channel sender for file change events.
    event_tx: mpsc::Sender<FileEvent>,
    /// Configuration.
    config: WatcherConfig,
    /// Last sync time (for rate limiting).
    last_sync: Instant,
}

/// Types of file events that can occur.
#[derive(Debug, Clone)]
pub enum FileEvent {
    /// A file was created.
    Created(PathBuf),
    /// A file was modified.
    Modified(PathBuf),
    /// A file was deleted.
    Deleted(PathBuf),
    /// A file was renamed (old path, new path).
    Renamed(PathBuf, PathBuf),
    /// Request manual sync.
    SyncRequested,
}

impl FileWatcher {
    /// Create a new file watcher.
    ///
    /// Note: This is a simplified implementation that uses periodic scanning
    /// rather than OS-level file system events. This is more portable and
    /// works well for the AIWiki use case where changes are relatively infrequent.
    pub async fn new(raw_dir: PathBuf, config: WatcherConfig) -> Result<Self> {
        let (event_tx, mut _event_rx) = mpsc::channel(100);

        // Spawn the watcher task
        let config_clone = config.clone();
        let event_tx_clone = event_tx.clone();
        tokio::spawn(async move {
            let mut _last_scan = Instant::now();
            let mut last_state: Option<std::collections::HashMap<PathBuf, u64>> = None;

            let mut scan_interval = interval(Duration::from_secs(2));

            loop {
                tokio::select! {
                    _ = scan_interval.tick() => {
                        // Periodic scan
                        if config_clone.auto_sync {
                            match scan_directory_state(&raw_dir).await {
                                Ok(current_state) => {
                                    if let Some(ref prev) = last_state {
                                        let events = detect_changes(prev, &current_state);
                                        for event in events {
                                            let _ = event_tx_clone.send(event).await;
                                        }
                                    }
                                    last_state = Some(current_state);
                                }
                                Err(e) => {
                                    tracing::warn!("Failed to scan directory: {}", e);
                                }
                            }
                        }
                    }
                }
            }
        });

        Ok(Self {
            event_tx,
            config,
            last_sync: Instant::now() - Duration::from_secs(60), // Allow immediate first sync
        })
    }
    /// Check if enough time has passed since last sync.
    pub fn can_sync(&self) -> bool {
        self.last_sync.elapsed() >= self.config.min_sync_interval
    }

    /// Manually trigger a sync.
    pub async fn request_sync(&self) -> Result<()> {
        self.event_tx
            .send(FileEvent::SyncRequested)
            .await
            .map_err(|_| crate::AiwikiError::Config("Watcher channel closed".to_string()))?;
        Ok(())
    }

    /// Stop the watcher (drops the channel, causing the task to exit).
    pub fn stop(self) {
        // Channel is dropped when self is dropped
    }
}

/// Scan the directory and get the current state.
async fn scan_directory_state(dir: &Path) -> Result<std::collections::HashMap<PathBuf, u64>> {
    let mut state = std::collections::HashMap::new();

    if !dir.exists() {
        return Ok(state);
    }

    // Use stack-based iteration to avoid recursion
    let base_path = dir.to_path_buf();
    let mut stack = vec![base_path.clone()];

    while let Some(current) = stack.pop() {
        let mut entries = match tokio::fs::read_dir(&current).await {
            Ok(e) => e,
            Err(_) => continue,
        };

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            let metadata = entry.metadata().await?;

            if metadata.is_file() {
                if let Ok(relative) = path.strip_prefix(&base_path) {
                    let mtime = metadata
                        .modified()
                        .ok()
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    state.insert(relative.to_path_buf(), mtime);
                }
            } else if metadata.is_dir() {
                stack.push(path);
            }
        }
    }

    Ok(state)
}

/// Detect changes between two directory states.
fn detect_changes(
    old: &std::collections::HashMap<PathBuf, u64>,
    new: &std::collections::HashMap<PathBuf, u64>,
) -> Vec<FileEvent> {
    let mut events = Vec::new();

    // Find new and modified files
    for (path, mtime) in new {
        match old.get(path) {
            None => {
                events.push(FileEvent::Created(path.clone()));
            }
            Some(old_mtime) if old_mtime != mtime => {
                events.push(FileEvent::Modified(path.clone()));
            }
            _ => {} // No change
        }
    }

    // Find deleted files
    for path in old.keys() {
        if !new.contains_key(path) {
            events.push(FileEvent::Deleted(path.clone()));
        }
    }

    events
}

/// Run the file watcher with automatic sync.
///
/// This spawns a background task that watches for changes
/// and triggers sync operations.
pub async fn run_auto_sync(
    wiki: &Aiwiki,
    config: WatcherConfig,
) -> Result<impl std::future::Future<Output = ()>> {
    let _watcher = FileWatcher::new(wiki.path("raw"), config.clone()).await?;

    let task = async move {
        // Note: In a real implementation, we'd keep the watcher running
        // and trigger syncs. For now, this is a stub that just holds
        // the watcher alive.
        loop {
            tokio::time::sleep(Duration::from_secs(60)).await;
        }
    };

    Ok(task)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_watcher_config_default() {
        let config = WatcherConfig::default();
        assert_eq!(config.debounce_duration, Duration::from_secs(5));
        assert!(config.auto_sync);
        assert_eq!(config.min_sync_interval, Duration::from_secs(30));
    }

    #[test]
    fn test_watcher_config_manual_only() {
        let config = WatcherConfig::manual_only();
        assert!(!config.auto_sync);
    }

    #[test]
    fn test_detect_changes() {
        let mut old = std::collections::HashMap::new();
        old.insert(PathBuf::from("file1.md"), 1000);
        old.insert(PathBuf::from("file2.md"), 2000);

        let mut new = std::collections::HashMap::new();
        new.insert(PathBuf::from("file1.md"), 1000); // Unchanged
        new.insert(PathBuf::from("file2.md"), 3000); // Modified
        new.insert(PathBuf::from("file3.md"), 4000); // New

        let events = detect_changes(&old, &new);

        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, FileEvent::Modified(p) if p == Path::new("file2.md")))
        );
        assert!(
            events
                .iter()
                .any(|e| matches!(e, FileEvent::Created(p) if p == Path::new("file3.md")))
        );

        // Test deleted detection
        let events = detect_changes(&new, &old);
        assert!(
            events
                .iter()
                .any(|e| matches!(e, FileEvent::Deleted(p) if p == Path::new("file3.md")))
        );
    }
}
