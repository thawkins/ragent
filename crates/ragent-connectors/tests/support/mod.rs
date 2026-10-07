//! Shared test-support helpers for `ragent-connectors` integration tests.
//!
//! Provides [`TempTree`], the RAII scratch directory used by every test file
//! under `tests/`. It was previously copy-pasted verbatim (with only the
//! directory-name prefix differing) into 10 test files; it now has a single
//! definition here (code-audit T-602).
//!
//! Every tree is rooted at `target/temp/connectors-test/`, never `/tmp`
//! (AGENTS.md).

// The helper methods below are only used by a subset of the test binaries that
// compile this module, so silence dead-code warnings per binary.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use ragent_connectors::{StoreDirs, StoreLedger, add, store_dirs_at};

/// Per-process sequence, so two trees created in the same test with the same
/// human-readable name never collide.
static SEQ: AtomicUsize = AtomicUsize::new(0);

/// An RAII scratch directory rooted at `target/temp/connectors-test/`.
///
/// The inner path is public so tests that need the raw root can use `tree.0`;
/// the directory is removed recursively when the value is dropped.
pub struct TempTree(pub PathBuf);

impl TempTree {
    /// Create a fresh scratch tree whose name embeds `name`, the process id,
    /// and a per-process sequence number.
    #[must_use]
    pub fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/connectors-test/shared-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp tree creatable");
        std::fs::create_dir_all(path.join(".ragent")).expect("tree .ragent creatable");
        Self(path)
    }

    /// The tree root as a [`Path`].
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }

    /// The project connector store (`<tree>/.ragent/connectors/`).
    #[must_use]
    pub fn store(&self) -> PathBuf {
        self.0.join(".ragent").join("connectors")
    }

    /// Store dirs pinned to this tree's project store (no global leg).
    #[must_use]
    pub fn dirs(&self) -> StoreDirs {
        store_dirs_at(&self.0, Some(&self.store()), None)
    }

    /// The workspace fixture root `assets/connectors/fixtures/`.
    #[must_use]
    pub fn fixtures_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/connectors/fixtures")
    }

    /// Install fixture `name` through the real `add` path and return its id.
    pub fn install(&self, name: &str) -> String {
        let source = Self::fixtures_root().join(name);
        let outcome = add(
            &self.dirs(),
            &self.0,
            source.to_str().expect("utf8"),
            false,
            &[],
        )
        .unwrap_or_else(|e| panic!("install fixture {name}: {e}"));
        outcome.descriptor.id.as_str().to_string()
    }

    /// Install archive `name` (e.g. `echo.zip`) and return the id.
    pub fn install_archive(&self, name: &str, force: bool) -> String {
        let source = Self::fixtures_root().join("archives").join(name);
        let outcome = add(
            &self.dirs(),
            &self.0,
            source.to_str().expect("utf8"),
            force,
            &[],
        )
        .unwrap_or_else(|e| panic!("install archive {name}: {e}"));
        outcome.descriptor.id.as_str().to_string()
    }

    /// The store ledger as written by the installs.
    #[must_use]
    pub fn ledger(&self) -> StoreLedger {
        StoreLedger::load(&self.store())
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
