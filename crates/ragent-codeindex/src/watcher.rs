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
    #[must_use]
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
pub fn should_ignore(root: &Path, path: &Path) -> bool {
    // Make it relative first so we check component names.
    let rel = path.strip_prefix(root).unwrap_or(path);
    for component in rel.components() {
        if let std::path::Component::Normal(name) = component
            && let Some(name_str) = name.to_str()
            && IGNORED_DIRS.contains(&name_str)
        {
            return true;
        }
    }

    // Ignore directories themselves (we only want files).
    if path.is_dir() {
        return true;
    }

    false
}
