//! Repair of interrupted npm `_npx` cache entries.
//!
//! `npx -y <pkg>` installs the package into `<npm cache>/_npx/<hash>/`. To
//! replace an already-installed package, npm's `reify` does it in two steps: it
//! *retires* the current directory - a `rename` onto the deterministic sibling
//! `node_modules/.<basename>-<8 alnum>` - and only then places the new copy at
//! the original path.
//!
//! If that run is interrupted (a killed launcher, a Ctrl-C, a teardown that
//! `killpg`s the process tree) the retired directory is left behind. Its name
//! is a pure function of the package path (`@npmcli/arborist`'s
//! `retire-path.js` hashes the path with SHA-1), so the *next* `reify` for that
//! package renames onto a directory that already exists and fails before the
//! server is ever started:
//!
//! ```text
//! npm error code ENOTEMPTY
//! npm error syscall rename
//! npm error path /home/u/.npm/_npx/15c61037b1978c83/node_modules/chrome-devtools-mcp
//! npm error dest /home/u/.npm/_npx/15c61037b1978c83/node_modules/.chrome-devtools-mcp-FKjxnyhX
//! npm error errno -39
//! ```
//!
//! npm exits non-zero without writing a single byte of MCP framing, so the
//! transport sees a closed stdout and every MCP client can only report the
//! unhelpful `connection closed: initialize response`. The real cause is one
//! stale directory two layers below.
//!
//! # Which state is repairable
//!
//! `reify` renames the live package *away* and then renames the new copy *in*,
//! so during a live replacement the live path is absent and only the retired
//! directory exists. That gives a discriminator that needs no clock:
//!
//! | retired dir | live package | meaning |
//! |-------------|--------------|---------|
//! | present     | present      | the retire rename already failed - every further `reify` of that package fails identically, so nothing can be in flight and the retired dir is pure poison |
//! | present     | absent       | a replacement is mid-flight (or died between its two renames, which npm heals on its own by placing into an empty path) - leave it alone |
//!
//! [`repair_npx_cache`] therefore removes a retired directory exactly when the
//! live package it was replacing is still present. A healthy cache has no
//! retired directories at all, and a mid-flight `reify` has no live package, so
//! neither is touched.

use std::fs;
use std::path::{Path, PathBuf};

use crate::team::lockfile::{LockFile, LockMode};

/// How long to wait for another ragent process to finish its own sweep.
const REPAIR_LOCK_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// Number of alphanumeric characters npm appends to a retired path.
const RETIRE_SUFFIX_LEN: usize = 8;

/// What a sweep removed.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RepairReport {
    /// Poisonous `node_modules/.<name>-<8 alnum>` directories removed.
    pub retired_dirs_removed: usize,
    /// Retired directories left alone because their package was mid-replacement.
    pub skipped_in_flight: usize,
}

impl RepairReport {
    /// Whether the sweep changed anything on disk.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.retired_dirs_removed == 0
    }
}

/// The npm `_npx` cache root for this process, or `None` when it does not
/// exist yet (npm has never run here, so there is nothing to repair).
///
/// Honours `npm_config_cache` first, because that is the variable npm itself
/// reads and an operator who redirects the cache expects ragent to follow.
#[must_use]
pub fn npx_cache_root() -> Option<PathBuf> {
    let cache = std::env::var_os("npm_config_cache")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .or_else(default_npm_cache);
    let root = cache?.join("_npx");
    root.is_dir().then_some(root)
}

/// npm's default cache directory when `npm_config_cache` is unset.
///
/// npm documents `<home>/.npm` on Posix and `%LocalAppData%\npm-cache` on
/// Windows (`@npmcli/config`'s `cache` definition).
fn default_npm_cache() -> Option<PathBuf> {
    #[cfg(windows)]
    {
        dirs::cache_dir().map(|dir| dir.join("npm-cache"))
    }
    #[cfg(not(windows))]
    {
        dirs::home_dir().map(|dir| dir.join(".npm"))
    }
}

/// Run [`repair_default_npx_cache`] at most once per process.
///
/// Called from the stdio connect path so every entry point (TUI, `serve`,
/// `run`, `--dry-run`, a connector probe) is covered without each one
/// remembering to sweep. [`std::sync::OnceLock`] makes concurrent first
/// connects wait for the single sweep instead of racing it, and guarantees it
/// happens before any caller spawns its own `npx`.
pub fn ensure_npx_cache_repaired() {
    static REPAIRED: std::sync::OnceLock<()> = std::sync::OnceLock::new();
    REPAIRED.get_or_init(|| {
        // `None` means npm has never populated a cache on this host, so there
        // is nothing to repair; the sweep logs its own outcome otherwise.
        let _ = repair_default_npx_cache(); // INTENTIONAL: no npm cache on this host
    });
}

/// Resolve the real cache root, take the cross-process repair lock, and sweep.
///
/// Returns `None` when there is no npm `_npx` cache to inspect.
pub fn repair_default_npx_cache() -> Option<RepairReport> {
    let root = npx_cache_root()?;

    // The lock only serialises concurrent *ragent* processes, so two of them
    // starting together do not both walk the same tree. A concurrently running
    // `npx` needs no lock: the live-package rule below leaves its retired
    // directory alone. A contended lock is not a reason to skip the work - the
    // other holder is doing exactly the same thing - so a timeout degrades to a
    // best-effort sweep rather than a failure.
    let lock_target = root.join(".ragent-repair");
    let _lock =
        match LockFile::acquire_timeout(&lock_target, LockMode::Exclusive, REPAIR_LOCK_TIMEOUT) {
            Ok(lock) => Some(lock),
            Err(error) => {
                tracing::debug!(
                    error = %error,
                    "npx cache repair lock contended; sweeping anyway"
                );
                None
            }
        };

    let report = repair_npx_cache(&root);
    if report.is_empty() {
        tracing::debug!(root = %root.display(), "npm npx cache is intact");
    } else {
        tracing::warn!(
            root = %root.display(),
            removed = report.retired_dirs_removed,
            skipped_in_flight = report.skipped_in_flight,
            "Removed an npm npx retired directory that would have failed the next \
             install with ENOTEMPTY"
        );
    }
    Some(report)
}

/// Sweep every install directory under `npx_root` and remove poisonous retired
/// directories, returning what was removed.
///
/// Never fails: an unreadable cache root, or a directory that cannot be
/// removed, is logged and skipped so one bad install cannot block startup.
#[must_use]
pub fn repair_npx_cache(npx_root: &Path) -> RepairReport {
    let mut report = RepairReport::default();
    let Ok(installs) = fs::read_dir(npx_root) else {
        return report;
    };
    for install in installs.flatten() {
        let install_path = install.path();
        if !install_path.is_dir() {
            continue;
        }
        repair_node_modules(&install_path.join("node_modules"), &mut report);
    }
    report
}

/// Sweep one install directory's `node_modules`.
fn repair_node_modules(node_modules: &Path, report: &mut RepairReport) {
    let Ok(entries) = fs::read_dir(node_modules) else {
        return;
    };

    // Collect before removing: mutating a directory while iterating it makes
    // the walk depend on readdir order, and a removal can shift later entries.
    let mut retired: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        let Some(stem) = retired_package_stem(name) else {
            continue;
        };
        let path = entry.path();
        // A regular file that merely matches the name pattern is not something
        // npm created, so it is not evidence of an interrupted install.
        if !path.is_dir() {
            continue;
        }
        retired.push((stem.to_string(), path));
    }

    for (stem, path) in retired {
        if live_package_paths(node_modules, &stem)
            .iter()
            .all(|live| !live.is_dir())
        {
            // No live package: a replacement is mid-flight, and removing the
            // directory it is about to restore would break its rollback.
            report.skipped_in_flight += 1;
            continue;
        }
        match fs::remove_dir_all(&path) {
            Ok(()) => report.retired_dirs_removed += 1,
            // Another ragent process swept it first; the poison is gone, which
            // is the whole point.
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                report.retired_dirs_removed += 1;
            }
            Err(error) => tracing::warn!(
                path = %path.display(),
                error = %error,
                "Could not remove a poisonous npx retired directory"
            ),
        }
    }
}

/// The package name carried by a retired directory name, or `None` when `name`
/// is not one.
///
/// npm retires `node_modules/<basename>` as `node_modules/.<basename>-<hash>`
/// where `<hash>` is eight alphanumeric characters. `.bin` (no dash) and
/// `.package-lock.json` (the suffix carries a dot) therefore never match, and
/// neither does any other dotted sibling npm or a plugin may create.
///
/// `#[doc(hidden)] pub` so the naming rule can be pinned directly by the
/// integration tests, without standing up a cache tree for every case.
#[doc(hidden)]
#[must_use]
pub fn retired_package_stem(name: &str) -> Option<&str> {
    let rest = name.strip_prefix('.')?;
    let (stem, suffix) = rest.rsplit_once('-')?;
    if stem.is_empty() || suffix.len() != RETIRE_SUFFIX_LEN {
        return None;
    }
    if !suffix.bytes().all(|byte| byte.is_ascii_alphanumeric()) {
        return None;
    }
    Some(stem)
}

/// Every path at which `stem` could be installed: plainly, plus under each
/// scope directory (`@scope/stem`). npm retires a scoped package under its
/// basename, so `@scope/pkg` is retired as `.pkg-<hash>`.
fn live_package_paths(node_modules: &Path, stem: &str) -> Vec<PathBuf> {
    let mut paths = vec![node_modules.join(stem)];
    let Ok(entries) = fs::read_dir(node_modules) else {
        return paths;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if name.starts_with('@') {
            paths.push(entry.path().join(stem));
        }
    }
    paths
}
