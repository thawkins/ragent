//! Tests for the derived installed-plugin-id set (spec `pluginstores` T-007,
//! T-018; FR-005, A5, NFR-001).
//!
//! `installed_ids` is the single derivation of the installed set: it scans the
//! store directories and collects the id of every plugin that parsed cleanly,
//! so a broken directory is not counted as installed and no second install
//! registry is kept (A5).

use std::path::{Path, PathBuf};

use ragent_plugins::{StoreDirs, installed_ids, store_dirs_at};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/installed-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp tree creatable");
        Self(path)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Write a minimal codex-dialect plugin directory carrying `id`.
fn write_codex_plugin(root: &Path, dir_name: &str, id: &str) {
    let plugin_dir = root.join(dir_name);
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir creatable");
    let manifest =
        format!(r#"{{ "id": "{id}", "name": "{id}", "version": "1.0.0", "entry": "index.js" }}"#);
    std::fs::write(plugin_dir.join("codex-plugin.json"), manifest).expect("manifest writable");
}

/// A directory that cannot be parsed as any plugin.
fn write_broken_dir(root: &Path, dir_name: &str) {
    let plugin_dir = root.join(dir_name);
    std::fs::create_dir_all(&plugin_dir).expect("dir creatable");
    std::fs::write(plugin_dir.join("codex-plugin.json"), "{ not valid json").expect("writable");
}

// ── installed_ids (FR-005, A5) ──────────────────────────────────────────────

#[test]
fn an_absent_store_derives_an_empty_installed_set() {
    let tree = TempTree::new("absent");
    let dirs = store_dirs_at(
        &tree.0.join("nowhere"),
        None,
        Some(&tree.0.join("global-ragent")),
    );
    assert!(installed_ids(dirs).is_empty());
}

#[test]
fn parsed_plugins_contribute_their_ids() {
    let tree = TempTree::new("parsed");
    let project_store = tree.0.join("proj/.ragent/plugins");
    write_codex_plugin(&project_store, "weather", "codex-weather");
    // A second dir whose manifest id differs from its directory name proves the
    // descriptor id, not the folder name, is the installed key.
    write_codex_plugin(&project_store, "folder-name", "codex-time");

    let dirs = store_dirs_at(&tree.0.join("proj"), Some(&project_store), None);
    let installed = installed_ids(dirs);
    assert_eq!(
        installed,
        ["codex-time".to_string(), "codex-weather".to_string()]
            .into_iter()
            .collect()
    );
}

#[test]
fn a_broken_directory_is_not_counted_as_installed() {
    let tree = TempTree::new("broken");
    let project_store = tree.0.join("proj/.ragent/plugins");
    write_codex_plugin(&project_store, "good", "codex-good");
    write_broken_dir(&project_store, "broken");

    let dirs = store_dirs_at(&tree.0.join("proj"), Some(&project_store), None);
    let installed = installed_ids(dirs);
    assert!(installed.contains("codex-good"));
    assert!(
        !installed.contains("broken"),
        "a parse failure must not mark an id installed"
    );
    assert_eq!(installed.len(), 1);
}

#[test]
fn the_installed_set_spans_both_store_legs() {
    let tree = TempTree::new("legs");
    let workdir = tree.0.join("proj");
    let global_root = tree.0.join("global-ragent");
    write_codex_plugin(
        &global_store_for(&global_root),
        "global-plugin",
        "codex-global",
    );
    // The default project leg is `<workdir>/.ragent/plugins`.
    write_codex_plugin(
        &workdir.join(".ragent/plugins"),
        "project-plugin",
        "codex-project",
    );

    let dirs = store_dirs_at(&workdir, None, Some(&global_root));
    let installed = installed_ids(dirs);
    assert!(installed.contains("codex-global"), "global leg scanned");
    assert!(installed.contains("codex-project"), "project leg scanned");
}

/// The global `plugins/` child under a user-global root.
fn global_store_for(global_root: &Path) -> PathBuf {
    global_root.join("plugins")
}

#[test]
fn a_store_dir_override_replaces_the_project_leg() {
    let tree = TempTree::new("override");
    let override_dir = tree.0.join("custom-store");
    write_codex_plugin(&override_dir, "custom", "codex-custom");

    let dirs: StoreDirs = store_dirs_at(&tree.0.join("proj"), Some(&override_dir), None);
    let installed = installed_ids(dirs);
    assert!(installed.contains("codex-custom"));
}
