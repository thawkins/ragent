//! Tests for plugin store paths, discovery scan, and the state ledger
//! (spec `plugins` T-005; FR-001, FR-023).

use std::path::{Path, PathBuf};

use ragent_plugins::{
    LifecycleState, PluginError, STATE_FILE, StoreLedger, scan_dirs, store_dirs_at,
};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/store-{name}-{}-{unique}",
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

const CODEX_MANIFEST: &str = r#"{
    "name": "Weather", "version": "1.0.0", "entry": "index.js"
}"#;

fn manifest_for(name: &str, version: &str) -> String {
    format!(r#"{{ "name": "{name}", "version": "{version}", "entry": "index.js" }}"#)
}

fn write_plugin(root: &Path, dir_name: &str, manifest_rel: &str, manifest: &str) {
    let plugin_dir = root.join(dir_name);
    let manifest_path = plugin_dir.join(manifest_rel);
    std::fs::create_dir_all(manifest_path.parent().expect("has parent")).expect("dirs creatable");
    std::fs::write(manifest_path, manifest).expect("manifest writable");
}

// ── store_dirs_at (FR-001) ──────────────────────────────────────────────────

#[test]
fn store_dirs_at_orders_global_then_project() {
    let dirs = store_dirs_at(
        Path::new("/work"),
        None,
        Some(Path::new("/home/u/.config/ragent")),
    );
    assert_eq!(
        dirs.project.as_deref(),
        Some(Path::new("/work/.ragent/plugins"))
    );
    assert_eq!(
        dirs.global.as_deref(),
        Some(Path::new("/home/u/.config/ragent/plugins"))
    );
}

#[test]
fn store_dirs_at_honours_store_dir_override_for_project_leg() {
    let dirs = store_dirs_at(
        Path::new("/work"),
        Some(Path::new("/custom/store")),
        Some(Path::new("/home/u/.config/ragent")),
    );
    assert_eq!(dirs.project.as_deref(), Some(Path::new("/custom/store")));
    // Global leg unchanged by the project override.
    assert_eq!(
        dirs.global.as_deref(),
        Some(Path::new("/home/u/.config/ragent/plugins"))
    );
}

// ── scan (FR-001, FR-023) ───────────────────────────────────────────────────

#[test]
fn scan_discovers_project_and_global_plugins_project_wins_on_collision() {
    let tree = TempTree::new("scan");
    let project_store = tree.0.join("proj/.ragent/plugins");
    let global_root = tree.0.join("global-ragent");
    let global_store = global_root.join("plugins");

    // Same plugin id in both stores: project wins (closest-wins, FR-001).
    write_plugin(
        &project_store,
        "codex-weather",
        "codex-plugin.json",
        &manifest_for("Weather", "1.0.0-project"),
    );
    write_plugin(
        &global_store,
        "codex-weather",
        "codex-plugin.json",
        &manifest_for("Weather", "9.9.9-global"),
    );
    write_plugin(
        &global_store,
        "claude-todo",
        ".claude-plugin/plugin.json",
        r#"{ "name": "Todo", "version": "2.0", "main": "main.js" }"#,
    );
    // A non-plugin directory and the ledger file must be skipped.
    std::fs::create_dir_all(project_store.join("random-folder")).unwrap();
    std::fs::write(
        project_store.join("random-folder/README.md"),
        "not a plugin",
    )
    .unwrap();
    std::fs::write(global_store.join(STATE_FILE), "{}").unwrap();

    let found = scan_dirs(store_dirs_at(
        &tree.0.join("proj"),
        None,
        Some(&global_root),
    ));
    assert_eq!(
        found.len(),
        2,
        "id collision should collapse to one row: got {} rows",
        found.len()
    );

    let weather = found
        .iter()
        .find(|p| {
            p.outcome
                .as_ref()
                .is_ok_and(|m| m.descriptor.id == "weather")
        })
        .expect("weather discovered");
    let parsed = weather.outcome.as_ref().expect("weather parsed");
    assert_eq!(parsed.descriptor.version, "1.0.0-project");
    assert_eq!(weather.store, project_store);
    assert!(!weather.enabled);

    let todo = found
        .iter()
        .find(|p| p.outcome.as_ref().is_ok_and(|m| m.descriptor.id == "todo"))
        .expect("todo discovered");
    assert_eq!(todo.store, global_store);
}

#[test]
fn scan_surfaces_parse_failures_per_plugin_without_failing_the_scan() {
    let tree = TempTree::new("scan-errors");
    let project_store = tree.0.join("proj/.ragent/plugins");
    write_plugin(
        &project_store,
        "bad-manifest",
        "codex-plugin.json",
        r#"{ "name": "broken", "#,
    );
    write_plugin(
        &project_store,
        "good-one",
        "codex-plugin.json",
        CODEX_MANIFEST,
    );

    let found = scan_dirs(store_dirs_at(&tree.0.join("proj"), None, None));
    assert_eq!(found.len(), 2);

    let bad = found
        .iter()
        .find(|p| p.dir.ends_with("bad-manifest"))
        .expect("bad plugin row present (FR-025 reporting needs a row)");
    let failure = bad.outcome.as_ref().expect_err("bad parse recorded");
    assert!(matches!(failure.error, PluginError::ManifestParse { .. }));

    let good = found
        .iter()
        .find(|p| p.dir.ends_with("good-one"))
        .expect("good plugin row present");
    assert!(good.outcome.is_ok());
}

#[test]
fn scan_marks_enabled_from_ledger() {
    let tree = TempTree::new("scan-ledger");
    let project_store = tree.0.join("proj/.ragent/plugins");
    write_plugin(
        &project_store,
        "codex-weather",
        "codex-plugin.json",
        CODEX_MANIFEST,
    );
    std::fs::write(
        project_store.join(STATE_FILE),
        r#"{ "plugins": { "weather": { "enabled": true } } }"#,
    )
    .unwrap();

    let found = scan_dirs(store_dirs_at(&tree.0.join("proj"), None, None));
    assert_eq!(found.len(), 1);
    assert!(found[0].enabled);
}

#[cfg(unix)]
#[test]
fn scan_skips_symlinks_escaping_the_store() {
    let tree = TempTree::new("scan-symlink");
    let project_store = tree.0.join("proj/.ragent/plugins");
    std::fs::create_dir_all(&project_store).unwrap();

    // A real plugin outside the store, linked in — must be skipped.
    write_plugin(
        &tree.0,
        "outside-weather",
        "codex-plugin.json",
        CODEX_MANIFEST,
    );
    std::os::unix::fs::symlink(
        tree.0.join("outside-weather"),
        project_store.join("linked-weather"),
    )
    .expect("symlink creatable");

    // A symlink to a plugin *inside* the store is followed.
    write_plugin(
        &project_store,
        "real-one",
        "codex-plugin.json",
        CODEX_MANIFEST,
    );
    // Fix the id so the link target differs from the escapee above.
    std::fs::write(
        project_store.join("real-one/codex-plugin.json"),
        manifest_for("RealOne", "1.0"),
    )
    .unwrap();
    std::os::unix::fs::symlink(
        project_store.join("real-one"),
        project_store.join("linked-inside"),
    )
    .expect("symlink creatable");

    let found = scan_dirs(store_dirs_at(&tree.0.join("proj"), None, None));
    let ids: Vec<String> = found
        .iter()
        .filter_map(|p| p.outcome.as_ref().ok().map(|m| m.descriptor.id.clone()))
        .collect();
    assert_eq!(
        ids,
        vec!["realone".to_string()],
        "escaping link skipped, in-store link followed exactly once: {ids:?}"
    );
}

#[test]
fn scan_treats_absent_store_as_empty() {
    let tree = TempTree::new("scan-absent");
    let found = scan_dirs(store_dirs_at(
        &tree.0.join("no-such-proj"),
        None,
        Some(&tree.0.join("no-such-global")),
    ));
    assert!(found.is_empty());
}

// ── ledger ──────────────────────────────────────────────────────────────────

#[test]
fn ledger_round_trips_state_and_counters() {
    let tree = TempTree::new("ledger");
    let store = tree.0.join("store");
    let mut ledger = StoreLedger::default();
    {
        let state = ledger.state_mut("weather");
        state.enabled = true;
        state.counters.loads_ok = 3;
        state.counters.tool_invocations = 12;
        state.counters.consecutive_failures = 1;
    }
    ledger.save(&store).expect("ledger saves");

    let loaded = StoreLedger::load(&store);
    let state = loaded.state("weather").expect("state persisted");
    assert!(state.enabled);
    assert_eq!(state.counters.loads_ok, 3);
    assert_eq!(state.counters.tool_invocations, 12);
    assert_eq!(state.counters.consecutive_failures, 1);

    assert!(loaded.state("nope").is_none());
}

#[test]
fn ledger_corrupt_file_is_renamed_aside_and_restarted() {
    let tree = TempTree::new("ledger-corrupt");
    let store = tree.0.join("store");
    std::fs::create_dir_all(&store).unwrap();
    std::fs::write(store.join(STATE_FILE), b"{ not json").unwrap();

    let loaded = StoreLedger::load(&store);
    assert!(loaded.plugins.is_empty());
    assert!(
        store.join(format!("{STATE_FILE}.corrupt")).exists(),
        "corrupt ledger renamed aside"
    );
    assert!(!store.join(STATE_FILE).exists());
}

// ── lifecycle state display (FR-009) ────────────────────────────────────────

#[test]
fn lifecycle_state_display_names_match_spec_rows() {
    assert_eq!(LifecycleState::Disabled.to_string(), "disabled");
    assert_eq!(LifecycleState::Enabled.to_string(), "enabled");
    assert_eq!(LifecycleState::Loaded.to_string(), "loaded");
    assert_eq!(LifecycleState::Errored.to_string(), "errored");
}
