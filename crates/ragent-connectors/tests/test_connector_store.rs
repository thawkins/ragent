//! Tests for connector store paths, discovery scan, and the `_state.json`
//! ledger (spec `connectors` T-003; FR-001).

use std::path::{Path, PathBuf};

use ragent_connectors::{
    ConnectorError, MANIFEST_FILE, STATE_FILE, StoreLedger, read_manifest, scan_dirs, store_dirs_at,
};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/connectors-test/store-{name}-{}-{unique}",
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

/// A minimal, valid connector manifest with one stdio server.
fn manifest_for(id: &str, name: &str) -> String {
    format!(
        r#"{{
            "id": "{id}",
            "name": "{name}",
            "servers": [ {{ "id": "main", "transport": "stdio", "command": "/bin/true" }} ]
        }}"#
    )
}

fn write_connector(store: &Path, dir_name: &str, manifest: &str) {
    let dir = store.join(dir_name);
    std::fs::create_dir_all(&dir).expect("connector dir creatable");
    std::fs::write(dir.join(MANIFEST_FILE), manifest).expect("manifest writable");
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
        Some(Path::new("/work/.ragent/connectors"))
    );
    assert_eq!(
        dirs.global.as_deref(),
        Some(Path::new("/home/u/.config/ragent/connectors"))
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
        Some(Path::new("/home/u/.config/ragent/connectors"))
    );
}

// ── scan (FR-001) ───────────────────────────────────────────────────────────

#[test]
fn scan_discovers_project_and_global_connectors_project_wins_on_collision() {
    let tree = TempTree::new("scan");
    let project_store = tree.0.join("proj/.ragent/connectors");
    let global_root = tree.0.join("global-ragent");
    let global_store = global_root.join("connectors");

    // Same connector id in both stores: project wins (closest-wins, FR-001).
    write_connector(
        &project_store,
        "echo",
        &manifest_for("echo", "Echo project"),
    );
    write_connector(&global_store, "echo", &manifest_for("echo", "Echo global"));
    // A connector present only in the global store is still discovered.
    write_connector(&global_store, "solo", &manifest_for("solo", "Solo global"));

    let found = scan_dirs(store_dirs_at(
        &tree.0.join("proj"),
        None,
        Some(&global_root),
    ));

    let by_id: std::collections::BTreeMap<String, &ragent_connectors::ScannedConnector> = found
        .iter()
        .map(|c| {
            let id = match &c.outcome {
                Ok(d) => d.id.as_str().to_string(),
                Err(_) => c.dir.file_name().unwrap().to_string_lossy().into_owned(),
            };
            (id, c)
        })
        .collect();

    assert_eq!(by_id.len(), 2, "one row per distinct id: {by_id:?}");
    let echo = by_id.get("echo").expect("echo discovered");
    assert_eq!(
        echo.store, project_store,
        "project copy shadows the global one"
    );
    assert_eq!(echo.outcome.as_ref().unwrap().name, "Echo project");
    let solo = by_id.get("solo").expect("global-only connector discovered");
    assert_eq!(solo.store, global_store);
}

#[test]
fn scan_surfaces_parse_failures_without_failing_the_scan() {
    let tree = TempTree::new("scan-bad");
    let project_store = tree.0.join("proj/.ragent/connectors");
    write_connector(&project_store, "good", &manifest_for("good", "Good"));
    write_connector(&project_store, "broken", "{ not json");

    let found = scan_dirs(store_dirs_at(&tree.0.join("proj"), None, None));
    assert_eq!(found.len(), 2);

    let broken = found
        .iter()
        .find(|c| c.dir.ends_with("broken"))
        .expect("broken row present");
    assert!(matches!(
        broken.outcome.as_ref().unwrap_err().error,
        ConnectorError::ManifestParse { .. }
    ));
    let good = found
        .iter()
        .find(|c| c.dir.ends_with("good"))
        .expect("good row present");
    assert!(good.outcome.is_ok());
}

#[test]
fn scan_marks_enabled_from_ledger() {
    let tree = TempTree::new("scan-enabled");
    let project_store = tree.0.join("proj/.ragent/connectors");
    write_connector(&project_store, "echo", &manifest_for("echo", "Echo"));

    let mut ledger = StoreLedger::default();
    ledger.state_mut("echo").enabled = true;
    ledger.save(&project_store).expect("ledger saves");

    let found = scan_dirs(store_dirs_at(&tree.0.join("proj"), None, None));
    assert_eq!(found.len(), 1);
    assert!(found[0].enabled);
}

#[test]
fn scan_skips_entries_without_a_manifest() {
    let tree = TempTree::new("scan-nomanifest");
    let project_store = tree.0.join("proj/.ragent/connectors");
    // A plain directory with no connector.json is not a connector.
    std::fs::create_dir_all(project_store.join("not-a-connector")).unwrap();
    // A stray file in the store is ignored.
    std::fs::write(project_store.join("stray.txt"), b"x").unwrap();

    let found = scan_dirs(store_dirs_at(&tree.0.join("proj"), None, None));
    assert!(found.is_empty());
}

#[cfg(unix)]
#[test]
fn scan_skips_symlinks_escaping_the_store() {
    let tree = TempTree::new("scan-symlink");
    let project_store = tree.0.join("proj/.ragent/connectors");
    std::fs::create_dir_all(&project_store).unwrap();

    // A real connector outside the store, linked in - must be skipped.
    write_connector(&tree.0, "outside", &manifest_for("outside", "Outside"));
    std::os::unix::fs::symlink(tree.0.join("outside"), project_store.join("linked-outside"))
        .expect("symlink creatable");

    // A symlink to a connector *inside* the store is followed.
    write_connector(&project_store, "inside", &manifest_for("inside", "Inside"));
    std::os::unix::fs::symlink(
        project_store.join("inside"),
        project_store.join("linked-inside"),
    )
    .expect("symlink creatable");

    let found = scan_dirs(store_dirs_at(&tree.0.join("proj"), None, None));
    let ids: Vec<String> = found
        .iter()
        .filter_map(|c| c.outcome.as_ref().ok().map(|d| d.id.as_str().to_string()))
        .collect();
    assert_eq!(
        ids,
        vec!["inside".to_string()],
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

#[test]
fn read_manifest_reports_io_error_for_absent_file() {
    let tree = TempTree::new("read-manifest");
    let dir = tree.0.join("empty");
    std::fs::create_dir_all(&dir).unwrap();
    assert!(matches!(
        read_manifest(&dir).unwrap_err(),
        ConnectorError::Io(_)
    ));
}

// ── ledger ──────────────────────────────────────────────────────────────────

#[test]
fn ledger_round_trips_state_and_counters() {
    let tree = TempTree::new("ledger");
    let store = tree.0.join("store");
    let mut ledger = StoreLedger::default();
    {
        let state = ledger.state_mut("echo");
        state.enabled = true;
        state.counters.connects_ok = 2;
        state.counters.tool_invocations = 7;
        state.counters.consecutive_failures = 1;
    }
    ledger.save(&store).expect("ledger saves");

    let loaded = StoreLedger::load(&store);
    let state = loaded.state("echo").expect("state persisted");
    assert!(state.enabled);
    assert_eq!(state.counters.connects_ok, 2);
    assert_eq!(state.counters.tool_invocations, 7);
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
    assert!(loaded.connectors.is_empty());
    assert!(
        store.join(format!("{STATE_FILE}.corrupt")).exists(),
        "corrupt ledger renamed aside"
    );
    assert!(!store.join(STATE_FILE).exists());
}

// ── descriptor_by_id: id / slug / display-name resolution (FR-001) ──────────

/// Build the `StoreDirs` for one project-only temp tree.
fn dirs_at(tree: &TempTree) -> ragent_connectors::StoreDirs {
    ragent_connectors::StoreDirs {
        project: Some(tree.0.join("proj/.ragent/connectors")),
        global: None,
    }
}

#[test]
fn descriptor_by_id_resolves_id_slug_and_display_name() {
    let tree = TempTree::new("resolve");
    let store = tree.0.join("proj/.ragent/connectors");
    write_connector(
        &store,
        "microsoft-learn",
        &manifest_for("microsoft-learn", "Microsoft Learn"),
    );
    let dirs = dirs_at(&tree);

    // 1. exact id.
    let by_id = ragent_connectors::descriptor_by_id(&dirs, "microsoft-learn").expect("id");
    assert_eq!(by_id.id.as_str(), "microsoft-learn");
    // 2. case-insensitive id.
    let by_id_ci = ragent_connectors::descriptor_by_id(&dirs, "Microsoft-Learn").expect("id ci");
    assert_eq!(by_id_ci.id.as_str(), "microsoft-learn");
    // 3. case-insensitive display name (so a connector is addressable without
    //    knowing its on-disk id).
    let by_name =
        ragent_connectors::descriptor_by_id(&dirs, "microsoft learn").expect("display name");
    assert_eq!(by_name.id.as_str(), "microsoft-learn");
    // An unknown reference resolves nothing.
    assert!(ragent_connectors::descriptor_by_id(&dirs, "nosuch").is_none());
}

#[test]
fn descriptor_by_id_exact_id_wins_over_a_display_name_collision() {
    let tree = TempTree::new("resolve-order");
    let store = tree.0.join("proj/.ragent/connectors");
    // One connector's id equals the other's display name.
    write_connector(&store, "foo", &manifest_for("foo", "Foo Display"));
    write_connector(&store, "bar", &manifest_for("bar", "foo"));
    let dirs = dirs_at(&tree);

    // The exact id match wins, not the display-name match.
    let found = ragent_connectors::descriptor_by_id(&dirs, "foo").expect("resolved");
    assert_eq!(found.id.as_str(), "foo");
}
