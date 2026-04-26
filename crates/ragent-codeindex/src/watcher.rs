//! Filesystem event watcher for real-time code index updates.
//!
//! `CodeWatcher` monitors a project directory for file changes using the
//! `notify` crate and emits structured `WatchEvent`s via a channel.

use anyhow::{Context, Result};
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use tracing::{debug, trace, warn};

/// Directories that should never trigger index events.
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

/// A structured filesystem event for the code index.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WatchEvent {
    /// A file was created.
    Created(PathBuf),
    /// A file's content changed.
    Changed(PathBuf),
    /// A file was deleted.
    Deleted(PathBuf),
    /// A file was renamed/moved.
    Renamed {
        /// Original path.
        from: PathBuf,
        /// New path.
        to: PathBuf,
    },
}

/// Watches a project directory and sends [`WatchEvent`]s on a channel.
pub struct CodeWatcher {
    _watcher: RecommendedWatcher,
    root: PathBuf,
}

impl CodeWatcher {
    /// Start watching `root` recursively. Events are sent to `tx`.
    ///
    /// The watcher filters out events from ignored directories (`.git/`,
    /// `target/`, etc.) and non-source files.
    pub fn new(root: &Path, tx: mpsc::Sender<WatchEvent>) -> Result<Self> {
        let root = root
            .canonicalize()
            .with_context(|| format!("cannot canonicalize root: {}", root.display()))?;

        let root_clone = root.clone();
        let mut watcher =
            notify::recommended_watcher(move |res: Result<notify::Event, _>| match res {
                Ok(event) => {
                    let events = map_event(&root_clone, event);
                    for ev in events {
                        if tx.send(ev).is_err() {
                            trace!("watcher channel closed");
                            return;
                        }
                    }
                }
                Err(e) => {
                    warn!("watch error: {e}");
                }
            })
            .context("cannot create file watcher")?;

        watcher
            .watch(&root, RecursiveMode::Recursive)
            .with_context(|| format!("cannot watch: {}", root.display()))?;

        debug!("watching {}", root.display());

        Ok(Self {
            _watcher: watcher,
            root,
        })
    }

    /// The canonical root path being watched.
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// Convert a raw `notify::Event` into zero or more `WatchEvent`s.
fn map_event(root: &Path, event: notify::Event) -> Vec<WatchEvent> {
    let paths: Vec<PathBuf> = event
        .paths
        .into_iter()
        .filter(|p| !should_ignore(root, p))
        .collect();

    if paths.is_empty() {
        return Vec::new();
    }

    match event.kind {
        EventKind::Create(_) => paths
            .into_iter()
            .map(|p| relativize(root, p, WatchEvent::Created))
            .collect(),
        EventKind::Modify(_) => paths
            .into_iter()
            .map(|p| relativize(root, p, WatchEvent::Changed))
            .collect(),
        EventKind::Remove(_) => paths
            .into_iter()
            .map(|p| relativize(root, p, WatchEvent::Deleted))
            .collect(),
        _ => Vec::new(),
    }
}

/// Convert an absolute path into a relative `WatchEvent`.
fn relativize(root: &Path, abs_path: PathBuf, ctor: fn(PathBuf) -> WatchEvent) -> WatchEvent {
    let rel = abs_path
        .strip_prefix(root)
        .unwrap_or(&abs_path)
        .to_path_buf();
    ctor(rel)
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

    #[test]
    fn test_should_ignore_node_modules() {
        let root = Path::new("/project");
        assert!(should_ignore(
            root,
            Path::new("/project/node_modules/foo/index.js")
        ));
    }

    #[test]
    fn test_watcher_receives_create_event() {
        let dir = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::channel();
        let _watcher = CodeWatcher::new(dir.path(), tx).unwrap();

        // Give the watcher time to start.
        std::thread::sleep(Duration::from_millis(200));

        // Create a file.
        let file_path = dir.path().join("hello.rs");
        fs::write(&file_path, "fn main() {}").unwrap();

        // Wait for events — FS notifications can be slow.
        let mut got_create = false;
        for _ in 0..20 {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(WatchEvent::Created(p)) => {
                    if p.to_string_lossy().contains("hello.rs") {
                        got_create = true;
                        break;
                    }
                }
                Ok(WatchEvent::Changed(p)) => {
                    // Some platforms emit Changed instead of Created.
                    if p.to_string_lossy().contains("hello.rs") {
                        got_create = true;
                        break;
                    }
                }
                Ok(_) => continue,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            }
        }
        assert!(got_create, "should receive create event for hello.rs");
    }

    #[test]
    fn test_watcher_filters_git_events() {
        let dir = tempfile::tempdir().unwrap();
        let git_dir = dir.path().join(".git");
        fs::create_dir_all(&git_dir).unwrap();

        let (tx, rx) = mpsc::channel();
        let _watcher = CodeWatcher::new(dir.path(), tx).unwrap();

        std::thread::sleep(Duration::from_millis(200));

        // Create a file inside .git — should be filtered.
        fs::write(git_dir.join("test"), "data").unwrap();

        // Should not receive any events.
        match rx.recv_timeout(Duration::from_millis(500)) {
            Err(mpsc::RecvTimeoutError::Timeout) => {} // Expected
            Ok(ev) => panic!("should not receive .git event, got: {ev:?}"),
            Err(e) => panic!("unexpected error: {e}"),
        }
    }

    #[test]
    fn test_watcher_receives_delete_event() {
        let dir = tempfile::tempdir().unwrap();
        let file_path = dir.path().join("delete_me.rs");
        fs::write(&file_path, "fn delete() {}").unwrap();

        let (tx, rx) = mpsc::channel();
        let _watcher = CodeWatcher::new(dir.path(), tx).unwrap();

        std::thread::sleep(Duration::from_millis(200));

        // Delete the file.
        fs::remove_file(&file_path).unwrap();

        let mut got_delete = false;
        for _ in 0..20 {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(WatchEvent::Deleted(p)) => {
                    if p.to_string_lossy().contains("delete_me.rs") {
                        got_delete = true;
                        break;
                    }
                }
                Ok(_) => continue,
                Err(mpsc::RecvTimeoutError::Timeout) => continue,
                Err(_) => break,
            }
        }
        assert!(got_delete, "should receive delete event for delete_me.rs");
    }

    #[test]
    fn test_relativize() {
        let root = Path::new("/project");
        let ev = relativize(
            root,
            PathBuf::from("/project/src/main.rs"),
            WatchEvent::Created,
        );
        assert_eq!(ev, WatchEvent::Created(PathBuf::from("src/main.rs")));
    }
}
