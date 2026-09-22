//! Tests for `/plugins remove` store operation (spec `plugins` T-011; FR-010).

use std::path::{Path, PathBuf};

use ragent_plugins::{RemoveError, StoreDirs, StoreLedger, add, remove, scan_dirs, store_dirs_at};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/remove-{name}-{}-{unique}",
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

fn dirs(tree: &TempTree) -> StoreDirs {
    StoreDirs {
        project: Some(tree.0.join("proj/.ragent/plugins")),
        global: Some(tree.0.join("global/plugins")),
    }
}

const CODEX: &str =
    r#"{ "id": "codex-weather", "name": "Weather", "version": "1.0.0", "entry": "index.js" }"#;

fn stage_codex_plugin(root: &Path, dir_name: &str) -> PathBuf {
    let plugin = root.join(dir_name);
    std::fs::create_dir_all(&plugin).unwrap();
    std::fs::write(plugin.join("codex-plugin.json"), CODEX).unwrap();
    std::fs::write(plugin.join("index.js"), "// entry").unwrap();
    plugin
}

/// Install a plugin into the project store, returning its installed directory.
///
/// `add` records the plugin enabled (FR-007); these tests exercise removing a
/// *disabled* plugin, so the helper turns the enable flag back off.
fn install(tree: &TempTree, dir_name: &str) -> PathBuf {
    let source = stage_codex_plugin(&tree.0.join("src"), dir_name);
    let outcome = add(&dirs(tree), &tree.0, source.to_str().unwrap(), false).expect("add");
    let store = outcome.installed_dir.parent().expect("store dir");
    let mut ledger = StoreLedger::load(store);
    ledger.state_mut("codex-weather").enabled = false;
    ledger.save(store).expect("ledger saved");
    outcome.installed_dir
}

// ── FR-010: remove a disabled plugin ────────────────────────────────────────

#[test]
fn remove_disabled_plugin_deletes_dir_and_clears_ledger_row() {
    let tree = TempTree::new("disabled");
    let installed = install(&tree, "codex-weather");
    assert!(installed.exists());

    // Give the plugin a ledger row first, so removal has state to clear.
    let mut ledger = StoreLedger::load(installed.parent().unwrap());
    ledger.state_mut("codex-weather").counters.loads_ok = 2;
    ledger
        .save(installed.parent().unwrap())
        .expect("ledger saved");

    let outcome = remove(&dirs(&tree), "codex-weather").expect("remove succeeds");
    assert_eq!(outcome.id, "codex-weather");
    assert_eq!(outcome.dir, installed);
    assert!(!installed.exists(), "plugin directory deleted");

    // Ledger row cleared; scanning now finds nothing.
    assert!(
        StoreLedger::load(installed.parent().unwrap())
            .state("codex-weather")
            .is_none()
    );
    assert!(scan_dirs(dirs(&tree)).is_empty());
}

#[test]
fn remove_unknown_plugin_is_refused_and_leaves_store_untouched() {
    let tree = TempTree::new("unknown");
    install(&tree, "codex-weather");

    let err = remove(&dirs(&tree), "no-such-plugin").expect_err("unknown id refused");
    assert!(matches!(err, RemoveError::UnknownPlugin(id) if id == "no-such-plugin"));
    // The other plugin is untouched.
    assert_eq!(scan_dirs(dirs(&tree)).len(), 1);
}

#[test]
fn remove_enabled_plugin_is_refused_with_disable_hint() {
    let tree = TempTree::new("enabled");
    let installed = install(&tree, "codex-weather");

    // Mark the plugin enabled in its store ledger (as `/plugins enable` would).
    let mut ledger = StoreLedger::load(installed.parent().unwrap());
    ledger.state_mut("codex-weather").enabled = true;
    ledger
        .save(installed.parent().unwrap())
        .expect("ledger saved");

    let err = remove(&dirs(&tree), "codex-weather").expect_err("enabled refused");
    assert!(matches!(err, RemoveError::Enabled(id) if id == "codex-weather"));
    // Directory and ledger row both preserved.
    assert!(installed.exists());
    assert!(
        StoreLedger::load(installed.parent().unwrap())
            .state("codex-weather")
            .is_some_and(|s| s.enabled)
    );
}

#[test]
fn remove_without_prior_ledger_row_succeeds_and_creates_no_ledger_file() {
    let tree = TempTree::new("no-ledger");
    let installed = install(&tree, "codex-weather");
    let store = installed.parent().unwrap().to_path_buf();
    // Remove any ledger from the add path (none is created by add today, but
    // assert the invariant explicitly).
    let _ = std::fs::remove_file(store.join(ragent_plugins::STATE_FILE));

    remove(&dirs(&tree), "codex-weather").expect("remove succeeds");
    assert!(!installed.exists());
    // No ledger file is created as a side effect of removing a plugin with no
    // recorded state.
    assert!(!store.join(ragent_plugins::STATE_FILE).exists());
}

#[test]
fn remove_resolves_plugin_from_project_over_global() {
    let tree = TempTree::new("precedence");
    // Install the same id into both stores.
    let project_source = stage_codex_plugin(&tree.0.join("src-project"), "codex-weather");
    add(
        &dirs(&tree),
        &tree.0,
        project_source.to_str().unwrap(),
        false,
    )
    .expect("project add");

    // Build a global store manually with the same id.
    let global_dir = tree.0.join("global/plugins/codex-weather");
    std::fs::create_dir_all(&global_dir).unwrap();
    std::fs::write(global_dir.join("codex-plugin.json"), CODEX).unwrap();
    std::fs::write(global_dir.join("index.js"), "// entry").unwrap();

    // `add` enabled the project copy; remove refuses an enabled plugin, so turn
    // the project leg's flag off first (as `/plugins disable` would).
    let project_store = tree.0.join("proj/.ragent/plugins");
    let mut ledger = StoreLedger::load(&project_store);
    ledger.state_mut("codex-weather").enabled = false;
    ledger.save(&project_store).expect("ledger saved");

    remove(&dirs(&tree), "codex-weather").expect("remove succeeds");
    // Project copy (higher priority) is removed; global copy remains.
    assert!(!tree.0.join("proj/.ragent/plugins/codex-weather").exists());
    assert!(global_dir.exists());
}

#[test]
fn remove_error_display_messages_are_actionable() {
    let enabled = RemoveError::Enabled("codex-weather".to_string());
    let msg = enabled.to_string();
    assert!(msg.contains("codex-weather"));
    assert!(msg.contains("/plugins disable"));
}

#[test]
fn store_dirs_at_is_reusable_for_remove() {
    // Guard against accidental coupling to `store_dirs` (env-dependent).
    let tree = TempTree::new("dirs");
    let resolved = store_dirs_at(&tree.0, None, None);
    assert_eq!(
        resolved.project.as_deref(),
        Some(tree.0.join(".ragent/plugins").as_path())
    );
    // `remove` over an empty store reports unknown rather than panicking.
    assert!(matches!(
        remove(&resolved, "anything"),
        Err(RemoveError::UnknownPlugin(_))
    ));
}
