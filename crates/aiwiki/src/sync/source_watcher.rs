//! Source folder watcher for real-time AIWiki updates.
//!
//! `SourceWatcher` monitors registered source folders for file changes using the
//! `notify` crate and emits structured `WatchEvent`s via an async channel.

use crate::source_folder::SourceFolder;
use anyhow::{Context, Result};
use globset::GlobSet;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use tokio::sync::mpsc;
use tracing::{debug, trace, warn};

/// Directories that should never trigger events.
const IGNORED_DIRS: &[&str] = &[
    ".git",
    "target",
    "node_modules",
    "__pycache__",
    ".ragent",
    ".venv",
    "venv",
    "dist",
    "build",
    ".tox",
    ".mypy_cache",
    ".pytest_cache",
];

/// A structured filesystem event for source folder watching.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    /// A file was created.
    Created {
        /// Path to the created file.
        path: PathBuf,
        /// Source folder name.
        source: String,
    },
    /// A file's content changed.
    Changed {
        /// Path to the changed file.
        path: PathBuf,
        /// Source folder name.
        source: String,
    },
    /// A file was deleted.
    Deleted {
        /// Path to the deleted file.
        path: PathBuf,
        /// Source folder name.
        source: String,
    },
}

/// Watches multiple source folders and sends [`WatchEvent`]s on a channel.
pub struct SourceWatcher {
    _watchers: Vec<RecommendedWatcher>,
    watched_paths: Vec<PathBuf>,
}

impl SourceWatcher {
    /// Start watching all enabled source folders. Events are sent to `tx`.
    ///
    /// The watcher filters out events from ignored directories (`.git/`,
    /// `target/`, etc.) and files not matching source patterns.
    pub fn new(
        root: &Path,
        sources: &[SourceFolder],
        raw_dir: &Path,
        tx: mpsc::Sender<WatchEvent>,
    ) -> Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("cannot canonicalize root: {}", root.display()))?;

        let mut watchers = Vec::new();
        let mut watched_paths = Vec::new();

        // Watch each enabled source folder
        for source in sources {
            if !source.enabled {
                continue;
            }

            let source_path = root.join(&source.path);
            if !source_path.exists() {
                warn!(
                    "Source folder '{}' does not exist, skipping",
                    source_path.display()
                );
                continue;
            }

            // Handle single file sources
            if source.is_file {
                if !source_path.is_file() {
                    warn!(
                        "Source '{}' is configured as file but is not a file, skipping",
                        source_path.display()
                    );
                    continue;
                }

                // For single files, watch the parent directory instead
                let parent_path = source_path.parent().unwrap_or(&source_path).to_path_buf();
                let file_name = source_path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(&source.path)
                    .to_string();

                let _parent_path_clone = parent_path.clone();
                let root_clone = root.clone();
                let source_label = source.path.clone();
                let tx_clone = tx.clone();

                let mut watcher =
                    notify::recommended_watcher(move |res: Result<notify::Event, _>| match res {
                        Ok(event) => {
                            // Filter events to only include the target file
                            let filtered_paths: Vec<PathBuf> = event
                                .paths
                                .into_iter()
                                .filter(|p| {
                                    p.file_name()
                                        .and_then(|n| n.to_str())
                                        .map(|n| n == file_name)
                                        .unwrap_or(false)
                                })
                                .collect();

                            if filtered_paths.is_empty() {
                                return;
                            }

                            let filtered_event = notify::Event {
                                paths: filtered_paths,
                                attrs: event.attrs,
                                kind: event.kind,
                            };

                            let events = map_raw_event(&root_clone, &source_label, filtered_event);
                            for ev in events {
                                if tx_clone.try_send(ev).is_err() {
                                    trace!("watcher channel closed or full");
                                    return;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("watch error for source '{}': {}", source_label, e);
                        }
                    })
                    .context("cannot create file watcher")?;

                watcher
                    .watch(&parent_path, RecursiveMode::NonRecursive)
                    .with_context(|| {
                        format!("cannot watch parent directory: {}", parent_path.display())
                    })?;

                debug!(
                    "watching single file source: {} (via {})",
                    source_path.display(),
                    parent_path.display()
                );
                watched_paths.push(source_path);
                watchers.push(watcher);
                continue;
            }

            // Handle directory sources
            if !source_path.is_dir() {
                warn!(
                    "Source '{}' is not a directory (and not marked as is_file), skipping",
                    source_path.display()
                );
                continue;
            }

            // Build globset for this source's patterns
            let globset = match build_globset(&source.patterns) {
                Ok(gs) => gs,
                Err(e) => {
                    warn!("Invalid glob pattern for source '{}': {}", source.path, e);
                    continue;
                }
            };

            let source_path_clone = source_path.clone();
            let root_clone = root.clone();
            let source_label = source.path.clone();
            let tx_clone = tx.clone();

            let mut watcher =
                notify::recommended_watcher(move |res: Result<notify::Event, _>| match res {
                    Ok(event) => {
                        let events = map_event(
                            &root_clone,
                            &source_path_clone,
                            &source_label,
                            &globset,
                            event,
                        );
                        for ev in events {
                            if tx_clone.try_send(ev).is_err() {
                                trace!("watcher channel closed or full");
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("watch error for source '{}': {}", source_label, e);
                    }
                })
                .context("cannot create file watcher")?;

            watcher
                .watch(&source_path, RecursiveMode::Recursive)
                .with_context(|| format!("cannot watch source: {}", source_path.display()))?;

            debug!("watching source: {}", source_path.display());
            watched_paths.push(source_path);
            watchers.push(watcher);
        }

        // Also watch the raw/ directory if it exists
        if raw_dir.exists() {
            let raw_label = "raw".to_string();
            let root_clone = root.clone();
            let tx_clone = tx.clone();

            let mut watcher =
                notify::recommended_watcher(move |res: Result<notify::Event, _>| match res {
                    Ok(event) => {
                        let events = map_raw_event(&root_clone, &raw_label, event);
                        for ev in events {
                            if tx_clone.try_send(ev).is_err() {
                                trace!("watcher channel closed or full");
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        warn!("watch error for raw directory: {}", e);
                    }
                })
                .context("cannot create raw directory watcher")?;

            watcher
                .watch(raw_dir, RecursiveMode::Recursive)
                .with_context(|| format!("cannot watch raw directory: {}", raw_dir.display()))?;

            debug!("watching raw directory: {}", raw_dir.display());
            watched_paths.push(raw_dir.to_path_buf());
            watchers.push(watcher);
        }

        Ok(Self {
            _watchers: watchers,
            watched_paths,
        })
    }

    /// The paths being watched.
    pub fn watched_paths(&self) -> &[PathBuf] {
        &self.watched_paths
    }
}

/// Build a globset from patterns.
fn build_globset(patterns: &[String]) -> Result<GlobSet, globset::Error> {
    let mut builder = globset::GlobSetBuilder::new();
    for pattern in patterns {
        builder.add(globset::Glob::new(pattern)?);
    }
    builder.build()
}

/// Convert a raw `notify::Event` into zero or more `WatchEvent`s for a source folder.
fn map_event(
    root: &Path,
    source_path: &Path,
    source_label: &str,
    globset: &GlobSet,
    event: notify::Event,
) -> Vec<WatchEvent> {
    let paths: Vec<PathBuf> = event
        .paths
        .into_iter()
        .filter(|p| !should_ignore(root, p))
        .filter(|p| matches_glob(p, source_path, globset))
        .collect();

    if paths.is_empty() {
        return Vec::new();
    }

    let source = source_label.to_string();

    match event.kind {
        EventKind::Create(_) => paths
            .into_iter()
            .map(|p| {
                let rel = make_relative(source_path, p);
                WatchEvent::Created {
                    path: rel,
                    source: source.clone(),
                }
            })
            .collect(),
        EventKind::Modify(_) => paths
            .into_iter()
            .map(|p| {
                let rel = make_relative(source_path, p);
                WatchEvent::Changed {
                    path: rel,
                    source: source.clone(),
                }
            })
            .collect(),
        EventKind::Remove(_) => paths
            .into_iter()
            .map(|p| {
                let rel = make_relative(source_path, p);
                WatchEvent::Deleted {
                    path: rel,
                    source: source.clone(),
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Convert a raw `notify::Event` into zero or more `WatchEvent`s for the raw/ directory.
fn map_raw_event(root: &Path, source_label: &str, event: notify::Event) -> Vec<WatchEvent> {
    let paths: Vec<PathBuf> = event
        .paths
        .into_iter()
        .filter(|p| !should_ignore(root, p))
        .collect();

    if paths.is_empty() {
        return Vec::new();
    }

    let source = source_label.to_string();

    match event.kind {
        EventKind::Create(_) => paths
            .into_iter()
            .map(|p| {
                let rel = make_relative(root, p);
                WatchEvent::Created {
                    path: rel,
                    source: source.clone(),
                }
            })
            .collect(),
        EventKind::Modify(_) => paths
            .into_iter()
            .map(|p| {
                let rel = make_relative(root, p);
                WatchEvent::Changed {
                    path: rel,
                    source: source.clone(),
                }
            })
            .collect(),
        EventKind::Remove(_) => paths
            .into_iter()
            .map(|p| {
                let rel = make_relative(root, p);
                WatchEvent::Deleted {
                    path: rel,
                    source: source.clone(),
                }
            })
            .collect(),
        _ => Vec::new(),
    }
}

/// Check if a path matches the source's glob patterns.
fn matches_glob(path: &Path, source_path: &Path, globset: &GlobSet) -> bool {
    if let Ok(relative) = path.strip_prefix(source_path) {
        globset.is_match(relative)
    } else {
        false
    }
}

/// Check if a path falls inside an ignored directory.
fn should_ignore(root: &Path, path: &Path) -> bool {
    // Make it relative first so we check component names.
    let rel = path.strip_prefix(root).unwrap_or(path);
    for component in rel.components() {
        if let std::path::Component::Normal(name) = component {
            if let Some(name_str) = name.to_str() {
                if IGNORED_DIRS.contains(&name_str) {
                    return true;
                }
            }
        }
    }

    // Ignore directories themselves (we only want files).
    if path.is_dir() {
        return true;
    }

    false
}

/// Make a path relative to a base.
fn make_relative(base: &Path, path: PathBuf) -> PathBuf {
    path.strip_prefix(base).unwrap_or(&path).to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::Duration;

    #[test]
    fn test_should_ignore_git() {
        let root = Path::new("/project");
        assert!(should_ignore(root, Path::new("/project/.git/HEAD")));
        assert!(should_ignore(root, Path::new("/project/.git/objects/abc")));
    }

    #[test]
    fn test_should_ignore_target() {
        let root = Path::new("/project");
        assert!(should_ignore(root, Path::new("/project/target/debug/bin")));
    }

    #[test]
    fn test_should_not_ignore_source() {
        let root = Path::new("/project");
        // For files that don't exist on disk, is_dir() returns false,
        // so this should not be ignored.
        assert!(!should_ignore(root, Path::new("/project/src/main.rs")));
    }

    #[tokio::test]
    async fn test_watcher_receives_create_event() {
        let dir = tempfile::tempdir().unwrap();
        let source = SourceFolder::new("test");
        let sources = vec![source];
        let raw_dir = dir.path().join("raw");

        let (tx, mut rx) = mpsc::channel(100);
        let _watcher = SourceWatcher::new(dir.path(), &sources, &raw_dir, tx).unwrap();

        // Give watcher time to initialize
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Create a test directory
        let test_dir = dir.path().join("test");
        fs::create_dir(&test_dir).unwrap();

        // Create a file
        let test_file = test_dir.join("test.txt");
        fs::write(&test_file, "hello").unwrap();

        // Wait for event with timeout
        let event = tokio::time::timeout(Duration::from_secs(2), rx.recv()).await;
        assert!(event.is_ok(), "Should receive create event");

        if let Ok(Some(WatchEvent::Created { path, source })) = event {
            assert_eq!(path, PathBuf::from("test.txt"));
            assert_eq!(source, "test");
        } else {
            panic!("Expected Created event, got {:?}", event);
        }
    }

    #[tokio::test]
    async fn test_watcher_handles_single_file_source() {
        // Test that single file sources don't cause errors when setting up watchers
        let dir = tempfile::tempdir().unwrap();

        // Create a single file to watch
        let single_file = dir.path().join("README.md");
        fs::write(&single_file, "# Test").unwrap();

        // Create a source folder pointing to this single file
        let source = SourceFolder::from_file_path("README.md");
        let sources = vec![source];
        let raw_dir = dir.path().join("raw");

        // This should not panic or fail - single files should be handled gracefully
        let (tx, _rx) = mpsc::channel(100);
        let watcher = SourceWatcher::new(dir.path(), &sources, &raw_dir, tx);

        // The watcher should be created successfully even with a single file source
        assert!(
            watcher.is_ok(),
            "Should create watcher for single file source"
        );

        let _watcher = watcher.unwrap();

        // Give watcher time to initialize
        tokio::time::sleep(Duration::from_millis(100)).await;

        // Verify the file path is in the watched paths
        assert!(
            _watcher.watched_paths().contains(&single_file),
            "Single file should be in watched paths"
        );
    }
}
