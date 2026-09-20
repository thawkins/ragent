//! Tests for the plugin lifecycle layer: enable/disable/load/unload, errored
//! states, and the consecutive-failure auto-unload threshold (spec `plugins`
//! T-009; FR-008, FR-011, FR-012, FR-016, FR-019).

use std::path::PathBuf;

use ragent_plugins::{
    DEFAULT_AUTO_UNLOAD_THRESHOLD, LifecycleState, PluginError, PluginManager, StoreLedger,
    store_dirs_at,
};

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/lifecycle-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp tree creatable");
        Self(path)
    }

    /// The project store (`<tree>/.ragent/plugins/`).
    fn store(&self) -> PathBuf {
        self.0.join(".ragent/plugins")
    }

    /// A fresh manager bound to this tree's project store (no global leg).
    /// The `store_dir` override pins the project leg to the actual store so
    /// discovery does not depend on the store living under a workdir-shaped
    /// layout.
    fn manager(&self) -> PluginManager {
        PluginManager::new(
            store_dirs_at(&self.0, Some(&self.store()), None),
            ragent_config::PluginsConfig::default(),
        )
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Write a Codex-dialect plugin into the tree's project store.
fn write_plugin(tree: &TempTree, dir_name: &str, manifest: &str, entry: &str) {
    let plugin_dir = tree.store().join(dir_name);
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir creatable");
    std::fs::write(plugin_dir.join("codex-plugin.json"), manifest).expect("manifest writable");
    std::fs::write(plugin_dir.join("index.js"), entry).expect("entry writable");
}

const GOOD_MANIFEST: &str =
    r#"{ "id": "codex-weather", "name": "Weather", "version": "1.0.0", "entry": "index.js" }"#;
const GOOD_ENTRY: &str = r#"
ragent.register_tool({ name: "get_weather", description: "Get weather", parameters: { type: "object" } });
ragent.register_command({ name: "weather", description: "Weather command" });
ragent.message.info("weather entry ran");
"#;

// ── FR-008/FR-011: enable loads immediately ────────────────────────────────

#[test]
fn enable_loads_plugin_and_persists_enabled_state() {
    let tree = TempTree::new("enable-loads");
    write_plugin(&tree, "codex-weather", GOOD_MANIFEST, GOOD_ENTRY);

    let mut manager = tree.manager();
    let report = manager.enable("codex-weather").expect("enable succeeds");

    assert_eq!(report.state, LifecycleState::Loaded);
    assert_eq!(report.error, None);
    assert_eq!(report.tools, vec!["plugin_codex-weather_get_weather"]);
    assert_eq!(report.commands, vec!["weather"]);

    // Enabled state persisted to the ledger in the project store.
    let ledger = StoreLedger::load(&tree.store());
    assert!(ledger.state("codex-weather").expect("row").enabled);
    // Load telemetry recorded (loads_ok = 1, no failures).
    let counters = &ledger.state("codex-weather").expect("row").counters;
    assert_eq!(counters.loads_ok, 1);
    assert_eq!(counters.load_failures, 0);

    // The entry ran: its message is in the plugin's captured sink.
    let loaded = manager.get("codex-weather").expect("tracked");
    let messages = loaded.calls.messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].text, "weather entry ran");
    // Live context retained for tool dispatch (T-010).
    assert!(loaded.context().is_some());
}

#[test]
fn enable_unknown_plugin_is_an_error_and_changes_no_state() {
    let tree = TempTree::new("enable-unknown");
    let mut manager = tree.manager();
    let err = manager
        .enable("no-such-plugin")
        .expect_err("unknown id refused");
    assert!(matches!(err, PluginError::UnknownPlugin(id) if id == "no-such-plugin"));
    let ledger = StoreLedger::load(&tree.store());
    assert!(ledger.state("no-such-plugin").is_none());
}

// ── FR-008/FR-016: session start loads enabled only ───────────────────────

#[test]
fn load_all_enabled_loads_only_enabled_plugins() {
    let tree = TempTree::new("load-all");
    write_plugin(&tree, "codex-weather", GOOD_MANIFEST, GOOD_ENTRY);
    write_plugin(
        &tree,
        "codex-inert",
        r#"{ "id": "codex-inert", "name": "Inert", "version": "1", "entry": "index.js" }"#,
        r#"ragent.message.info("inert entry ran");"#,
    );

    // Enable only one via the manager, then simulate a session restart with a
    // fresh manager over the same stores.
    tree.manager().enable("codex-weather").expect("enable");
    let mut restarted = tree.manager();
    let reports = restarted.load_all_enabled();

    assert_eq!(reports.len(), 1, "only the enabled plugin loads");
    assert_eq!(reports[0].id, "codex-weather");
    assert_eq!(reports[0].state, LifecycleState::Loaded);

    // FR-016: the disabled plugin never executed (its sink would hold the
    // message had its entry run; nothing is tracked for it).
    assert!(restarted.get("codex-inert").is_none());
    assert_eq!(restarted.state_of("codex-inert"), LifecycleState::Disabled);
    assert_eq!(restarted.state_of("codex-weather"), LifecycleState::Loaded);
}

// ── FR-015/FR-019: failure states ──────────────────────────────────────────

#[test]
fn entry_exception_marks_errored_and_discards_registrations() {
    let tree = TempTree::new("entry-throws");
    write_plugin(
        &tree,
        "codex-broken",
        r#"{ "id": "codex-broken", "name": "Broken", "version": "1", "entry": "index.js" }"#,
        r#"
ragent.register_tool({ name: "half", description: "registered before the throw" });
throw new Error("boom from entry");
"#,
    );

    let mut manager = tree.manager();
    let report = manager
        .enable("codex-broken")
        .expect("enable returns report");

    assert_eq!(report.state, LifecycleState::Errored);
    let cause = report.error.expect("cause recorded");
    assert!(cause.starts_with("entry:"), "cause class entry: {cause}");
    assert!(
        cause.contains("boom from entry"),
        "exception detail: {cause}"
    );
    assert!(report.tools.is_empty(), "partial registration discarded");
    // No live context is retained for a failed load.
    assert!(
        manager
            .get("codex-broken")
            .expect("tracked")
            .context()
            .is_none()
    );
    // Failure telemetry persisted.
    let ledger = StoreLedger::load(&tree.store());
    assert_eq!(
        ledger
            .state("codex-broken")
            .expect("row")
            .counters
            .load_failures,
        1
    );
}

#[test]
fn api_version_newer_than_host_is_refused() {
    let tree = TempTree::new("api-version");
    write_plugin(
        &tree,
        "codex-future",
        r#"{ "id": "codex-future", "name": "Future", "version": "1", "entry": "index.js", "api_version": 99 }"#,
        r#"ragent.message.info("never runs");"#,
    );

    let mut manager = tree.manager();
    let report = manager.enable("codex-future").expect("report");

    assert_eq!(report.state, LifecycleState::Errored);
    let cause = report.error.expect("cause");
    assert!(cause.starts_with("api-version:"), "cause class: {cause}");
    assert!(cause.contains("99"), "declared version named: {cause}");
    assert!(
        manager
            .get("codex-future")
            .expect("tracked")
            .calls
            .messages()
            .is_empty(),
        "entry never executed"
    );
}

#[test]
fn infinite_loop_entry_times_out_and_is_errored() {
    let tree = TempTree::new("entry-timeout");
    write_plugin(
        &tree,
        "loop-forever",
        r#"{ "id": "loop-forever", "name": "Loop", "version": "1", "entry": "index.js" }"#,
        "while (true) {}",
    );

    let config = ragent_config::PluginsConfig {
        max_entry_ms: 200,
        ..ragent_config::PluginsConfig::default()
    };
    let mut manager = PluginManager::new(store_dirs_at(&tree.0, Some(&tree.store()), None), config);

    let started = std::time::Instant::now();
    let report = manager.enable("loop-forever").expect("report");
    let elapsed = started.elapsed();

    assert_eq!(report.state, LifecycleState::Errored);
    let cause = report.error.expect("cause");
    assert!(cause.contains("timeout"), "timeout named: {cause}");
    assert!(
        elapsed < std::time::Duration::from_secs(3),
        "entry budget enforced, took {elapsed:?}"
    );
}

// ── FR-012: disable unloads and reports deregistration counts ─────────────

#[test]
fn disable_unloads_and_reports_deregistration_counts() {
    let tree = TempTree::new("disable");
    write_plugin(&tree, "codex-weather", GOOD_MANIFEST, GOOD_ENTRY);

    let mut manager = tree.manager();
    manager.enable("codex-weather").expect("enable");
    let report = manager.disable("codex-weather").expect("disable");

    assert_eq!(report.tools_deregistered, 1);
    assert_eq!(report.commands_deregistered, 1);
    assert_eq!(manager.state_of("codex-weather"), LifecycleState::Disabled);
    assert!(
        manager.get("codex-weather").is_none(),
        "runtime record dropped: fully inert"
    );
    assert!(
        !StoreLedger::load(&tree.store())
            .state("codex-weather")
            .expect("row")
            .enabled
    );
}

#[test]
fn disable_never_loaded_plugin_counts_zero() {
    let tree = TempTree::new("disable-cold");
    write_plugin(&tree, "codex-weather", GOOD_MANIFEST, GOOD_ENTRY);

    let mut manager = tree.manager();
    let report = manager.disable("codex-weather").expect("disable");
    assert_eq!(report.tools_deregistered, 0);
    assert_eq!(report.commands_deregistered, 0);
}

#[test]
fn disable_unknown_plugin_is_an_error() {
    let tree = TempTree::new("disable-unknown");
    let mut manager = tree.manager();
    let err = manager.disable("ghost").expect_err("unknown id refused");
    assert!(matches!(err, PluginError::UnknownPlugin(_)));
}

// ── Auto-unload threshold ──────────────────────────────────────────────────

#[test]
fn consecutive_failures_auto_unload_at_threshold() {
    let tree = TempTree::new("auto-unload");
    write_plugin(&tree, "codex-weather", GOOD_MANIFEST, GOOD_ENTRY);

    let mut manager = tree.manager().with_auto_unload_threshold(3);
    manager.enable("codex-weather").expect("enable");

    for (i, expected) in [(1, false), (2, false), (3, true)] {
        let unloaded = manager.record_tool_outcome("codex-weather", false);
        assert_eq!(unloaded, expected, "failure {i}");
    }

    let tracked = manager.get("codex-weather").expect("still tracked");
    assert_eq!(tracked.state, LifecycleState::Errored);
    assert!(
        tracked
            .error
            .as_deref()
            .is_some_and(|e| e.starts_with("auto-unload")),
        "auto-unload cause: {:?}",
        tracked.error
    );
    assert!(tracked.context().is_none(), "sandbox dropped");
    // Ledger counters persisted: 3 failures, consecutive stuck at 3.
    let ledger = StoreLedger::load(&tree.store());
    let counters = &ledger.state("codex-weather").expect("row").counters;
    assert_eq!(counters.tool_failures, 3);
    assert_eq!(counters.consecutive_failures, 3);
    // The plugin is errored, not disabled: still enabled in the ledger, so the
    // next session start retries the load (SPEC error-handling policy).
    assert!(ledger.state("codex-weather").expect("row").enabled);
}

#[test]
fn success_resets_the_consecutive_failure_counter() {
    let tree = TempTree::new("counter-reset");
    write_plugin(&tree, "codex-weather", GOOD_MANIFEST, GOOD_ENTRY);

    let mut manager = tree.manager().with_auto_unload_threshold(2);
    manager.enable("codex-weather").expect("enable");

    assert!(!manager.record_tool_outcome("codex-weather", false));
    assert!(!manager.record_tool_outcome("codex-weather", true)); // resets
    assert!(!manager.record_tool_outcome("codex-weather", false)); // 1 again
    assert_eq!(
        manager.get("codex-weather").expect("tracked").state,
        LifecycleState::Loaded
    );

    let ledger = StoreLedger::load(&tree.store());
    let counters = &ledger.state("codex-weather").expect("row").counters;
    assert_eq!(counters.tool_invocations, 3);
    assert_eq!(counters.tool_failures, 2);
    assert_eq!(counters.consecutive_failures, 1);
}

#[test]
fn outcomes_for_unknown_plugin_are_ignored() {
    let tree = TempTree::new("outcome-unknown");
    let mut manager = tree.manager();
    assert!(!manager.record_tool_outcome("nope", false));
}

#[test]
fn default_threshold_is_three_per_spec() {
    assert_eq!(DEFAULT_AUTO_UNLOAD_THRESHOLD, 3);
}

// ── FR-020: permission gate wiring from config ─────────────────────────────

#[test]
fn config_grants_gate_the_host_api_surface() {
    let tree = TempTree::new("gated-load");
    write_plugin(
        &tree,
        "codex-weather",
        GOOD_MANIFEST,
        r#"
ragent.message.info("granted capability works");
try { ragent.register_tool({ name: "get_weather" }); } catch (e) { /* gated out */ }
try { ragent.log("info", "gated out"); } catch (e) { /* gated out */ }
"#,
    );

    let mut config = ragent_config::PluginsConfig::default();
    config
        .permissions
        .insert("codex-weather".to_string(), vec!["message".to_string()]);
    let mut manager = PluginManager::new(store_dirs_at(&tree.0, Some(&tree.store()), None), config);
    let report = manager.enable("codex-weather").expect("report");

    // Only `message` was granted: register_tool was absent, so no tools were
    // recorded, but the entry's message.info still landed.
    assert_eq!(report.state, LifecycleState::Loaded);
    assert!(report.tools.is_empty(), "tools capability gated out");
    let tracked = manager.get("codex-weather").expect("tracked");
    let messages = tracked.calls.messages();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].text, "granted capability works");
    assert!(tracked.calls.logs().is_empty(), "log capability gated out");
}
