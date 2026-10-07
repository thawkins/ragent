//! Shared test-support helpers for `ragent-plugins` integration tests.
//!
//! Provides [`TempTree`], the RAII scratch directory used by every test file
//! under `tests/`. It was previously copy-pasted verbatim (with only the
//! directory-name prefix differing) into 18 test files; it now has a single
//! definition here (code-audit T-602).
//!
//! Every tree is rooted at `target/temp/plugins-test/`, never `/tmp`
//! (AGENTS.md).

// The helper methods below are only used by a subset of the test binaries that
// compile this module, so silence dead-code warnings per binary.
#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use ragent_config::PluginsConfig;
use ragent_plugins::{PluginManager, StoreDirs, StoreLedger, add, store_dirs_at};

/// Per-process sequence, so two trees created in the same test with the same
/// human-readable name never collide.
static SEQ: AtomicUsize = AtomicUsize::new(0);

/// An RAII scratch directory rooted at `target/temp/plugins-test/`.
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
            "../../target/temp/plugins-test/shared-{name}-{}-{unique}",
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

    /// The project plugin store (`<tree>/.ragent/plugins/`).
    #[must_use]
    pub fn store(&self) -> PathBuf {
        self.0.join(".ragent/plugins")
    }

    /// Store dirs pinned to this tree's project store (no global leg).
    #[must_use]
    pub fn dirs(&self) -> StoreDirs {
        store_dirs_at(&self.0, Some(&self.store()), None)
    }

    /// A fresh manager bound to this tree's project store (no global leg).
    #[must_use]
    pub fn manager(&self) -> PluginManager {
        PluginManager::new(self.dirs(), PluginsConfig::default())
    }

    /// The workspace fixture root `assets/plugins/fixtures/`.
    #[must_use]
    pub fn fixtures_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/plugins/fixtures")
    }

    /// Install fixture `name` through the real `add` path and return its id.
    pub fn install(&self, name: &str) -> String {
        let source = Self::fixtures_root().join(name);
        let outcome = add(&self.dirs(), &self.0, source.to_str().expect("utf8"), false)
            .unwrap_or_else(|e| panic!("install fixture {name}: {e}"));
        outcome.parsed.descriptor.id
    }

    /// Install fixture `name` then mark it **disabled** in the ledger.
    ///
    /// Enable-focused acceptance tests use this so the explicit
    /// `PluginSession::enable` under test is the call that loads the plugin: a
    /// plain `add` now records the plugin enabled (FR-007), so a subsequent
    /// `PluginSession::start` would already load it and the explicit enable
    /// would then collide with the tool it had registered.
    pub fn install_disabled(&self, name: &str) -> String {
        let id = self.install(name);
        let mut ledger = StoreLedger::load(&self.store());
        ledger.state_mut(&id).enabled = false;
        ledger.save(&self.store()).expect("ledger saved");
        id
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
