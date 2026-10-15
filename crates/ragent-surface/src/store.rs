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

/// Maximum size of a store ledger read into memory (1 MiB).
///
/// SEC-073: a store ledger is small JSON, but a corrupt/hostile file could be
/// arbitrarily large; the size is checked from `metadata()` before the read so a
/// huge file cannot exhaust memory.
pub const MAX_LEDGER_BYTES: u64 = 1024 * 1024;

/// A store entry is a candidate directory when it is a real directory, or a
/// symlink whose canonical target is a directory inside the store.
///
/// `label` names the family (`plugin`/`connector`) for the skip warning.
///
/// SEC-025: this returns only a `bool` and discards the canonical target, so the
/// caller re-opens the raw (possibly symlinked) path - a check-then-use race.
/// Prefer [`store_entry_dir`], which returns the verified canonical path, and
/// open that instead.
#[must_use]
pub fn store_entry_is_dir(store: &Path, dir: &Path, label: &str) -> bool {
    store_entry_dir(store, dir, label).is_some()
}

/// Resolve a store entry to the directory a caller should open.
///
/// Returns `dir` unchanged for a real (non-symlink) directory, or the verified
/// canonical target for a symlink whose target stays inside the store. Returns
/// `None` when the entry is missing, not a directory, or a symlink that escapes
/// the store.
///
/// SEC-025: callers must open the returned path (not the original `dir`) so a
/// symlink cannot be repointed between the check and the use.
#[must_use]
pub fn store_entry_dir(store: &Path, dir: &Path, label: &str) -> Option<PathBuf> {
    let Ok(file_type) = dir.symlink_metadata() else {
        return None;
    };
    if file_type.is_dir() {
        return Some(dir.to_path_buf());
    }
    if !file_type.file_type().is_symlink() {
        return None;
    }
    // Symlink: follow only when the resolved path stays inside the store.
    let (Ok(target), Ok(store_root)) = (dir.canonicalize(), store.canonicalize()) else {
        tracing::warn!(
            link = %dir.display(),
            "{label} store symlink cannot be resolved; skipping"
        );
        return None;
    };
    if target.starts_with(&store_root) && target.is_dir() {
        Some(target)
    } else {
        tracing::warn!(
            link = %dir.display(),
            target = %target.display(),
            "{label} store symlink escapes the store directory; skipping"
        );
        None
    }
}

/// Load a JSON store ledger at `path`, returning `None` when the file is
/// absent or unreadable.
///
/// A **present-but-corrupt** ledger is renamed aside to `<STATE_FILE>.corrupt`
/// (best-effort) and `Ok(None)` is returned, so a bad ledger never bricks the
/// store. The quarantine rename is logged; a failure to rename leaves the file
/// untouched. `label` names the family (`connector`/`plugin`) in the log line.
///
/// # Errors
///
/// A corrupt ledger (present but unparseable) is handled by the caller, which
/// calls [`quarantine_corrupt_ledger`].
pub fn read_ledger_bytes(path: &Path) -> Option<Vec<u8>> {
    // SEC-073: reject an oversized ledger *before* reading it, so a huge (or
    // sparse) file cannot exhaust memory on a corrupt/hostile store.
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_LEDGER_BYTES => {
            tracing::warn!(
                path = %path.display(),
                len = meta.len(),
                "store ledger too large; starting fresh"
            );
            return None;
        }
        Ok(_) | Err(_) => {}
    }
    match std::fs::read(path) {
        Ok(bytes) => Some(bytes),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            tracing::warn!(
                path = %path.display(),
                error = %error,
                "store ledger unreadable; starting fresh"
            );
            None
        }
    }
}

/// Quarantine a corrupt ledger by renaming it aside to `<path>.corrupt`.
///
/// Best-effort: a rename failure leaves the original file in place and is
/// logged, so a corrupt ledger is never silently deleted. `label` names the
/// family in the log line.
pub fn quarantine_corrupt_ledger(path: &Path, label: &str, error: &str) {
    let aside = path.with_extension("json.corrupt");
    tracing::warn!(
        path = %path.display(),
        error = %error,
        "{label} ledger is corrupt; renaming aside and starting fresh"
    );
    if let Err(e) = std::fs::rename(path, &aside) {
        tracing::warn!(
            path = %path.display(),
            error = %e,
            "failed to rename corrupt store ledger aside; leaving it in place"
        );
    }
}
