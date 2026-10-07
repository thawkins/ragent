//! Shared store-directory resolution for the `/plugins` and `/connectors`
//! stores (spec `plugins` FR-001 / spec `connectors` FR-001).
//!
//! Both families keep two store roots searched in ascending-priority order
//! (user-global first, project-local last, so "last match wins" yields
//! closest-wins on id collision) and both follow a symlinked store entry only
//! when the resolved target stays inside the store. The resolution and the
//! symlink guard were byte-identical apart from the store leaf name
//! (`plugins`/`connectors`); they now have one implementation here and each
//! crate delegates.

use std::path::{Path, PathBuf};

/// One store directory set. `project` is the project-local store, `global` the
/// user-global fallback. Scanning reads both; project entries take precedence
/// on id collision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoreDirs {
    /// Project-local store: `<workdir>/.ragent/<leaf>/`. `None` when no working
    /// directory is in effect.
    pub project: Option<PathBuf>,
    /// User-global store: `~/.config/ragent/<leaf>/`. `None` when the platform
    /// config directory cannot be determined.
    pub global: Option<PathBuf>,
}

impl StoreDirs {
    /// The destination store leg for a new install: the project store, or the
    /// global fallback. `None` when neither leg resolves.
    #[must_use]
    pub fn destination(&self) -> Option<&Path> {
        self.project.as_deref().or(self.global.as_deref())
    }
}

/// Resolve the store directories for the given legs.
///
/// Pure over its inputs so tests avoid env mutation: pass the project working
/// directory (or a `store_dir` override) and the user-global root (resolved via
/// [`ragent_config::user_dirs::global_state_dir`] by the caller) explicitly.
/// `leaf` is the store subdirectory name (`plugins` or `connectors`).
///
/// The returned order is ascending priority: global first, project last, so
/// iteration with "last write wins" yields closest-wins on id collision.
/// `global_root` is the `~/.config/ragent` root (not the `<leaf>/` child).
#[must_use]
pub fn store_dirs_at(
    workdir: &Path,
    store_dir_override: Option<&Path>,
    global_root: Option<&Path>,
    leaf: &str,
) -> StoreDirs {
    let global = global_root.map(|d| d.join(leaf));
    let project = store_dir_override
        .map(Path::to_path_buf)
        .or_else(|| Some(workdir.join(".ragent").join(leaf)));
    StoreDirs { project, global }
}

/// Resolve the store directories from the live environment.
///
/// `workdir` is the current working directory (pass [`std::env::current_dir`]
/// from the session layer). A `store_dir` override from the family's config
/// replaces the project leg; the global leg comes from
/// [`ragent_config::user_dirs::global_state_dir`]. `leaf` is the store
/// subdirectory name (`plugins` or `connectors`).
#[must_use]
pub fn store_dirs(workdir: &Path, store_dir_override: Option<&Path>, leaf: &str) -> StoreDirs {
    store_dirs_at(
        workdir,
        store_dir_override,
        ragent_config::user_dirs::global_state_dir().as_deref(),
        leaf,
    )
}

/// A store entry is a candidate directory when it is a real directory, or a
/// symlink whose canonical target is a directory inside the store.
///
/// `label` names the family (`plugin`/`connector`) for the skip warning.
#[must_use]
pub fn store_entry_is_dir(store: &Path, dir: &Path, label: &str) -> bool {
    let Ok(file_type) = dir.symlink_metadata() else {
        return false;
    };
    if file_type.is_dir() {
        return true;
    }
    if !file_type.file_type().is_symlink() {
        return false;
    }
    // Symlink: follow only when the resolved path stays inside the store.
    let (Ok(target), Ok(store_root)) = (dir.canonicalize(), store.canonicalize()) else {
        tracing::warn!(
            link = %dir.display(),
            "{label} store symlink cannot be resolved; skipping"
        );
        return false;
    };
    if target.starts_with(&store_root) {
        target.is_dir()
    } else {
        tracing::warn!(
            link = %dir.display(),
            target = %target.display(),
            "{label} store symlink escapes the store directory; skipping"
        );
        false
    }
}
