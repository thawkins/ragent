//! Tests for session-start integration: discover, load enabled plugins,
//! register tools and commands, record telemetry, and deregister on shutdown
//! (spec `plugins` T-016; FR-008, FR-022).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use ragent_plugins::{
    LifecycleState, PluginCommandAdapter, PluginManager, PluginSession, PluginSurface,
    PluginToolAdapter, StoreLedger, store_dirs_at,
};
use ragent_tools_core::Tool;

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/session-{name}-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("temp tree creatable");
        Self(path)
    }

    /// The project store (`<tree>/.ragent/plugins/`).
    fn store(&self) -> PathBuf {
        self.0.join(".ragent/plugins")
    }

    /// Store dirs pinned to this tree's project store (no global leg).
    fn dirs(&self) -> ragent_plugins::StoreDirs {
        store_dirs_at(&self.0, Some(&self.store()), None)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Write a Codex-dialect plugin with an explicit id so `enable`/load match.
fn write_plugin(tree: &TempTree, id: &str, entry: &str) {
    let plugin_dir = tree.store().join(id);
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir creatable");
    std::fs::write(
        plugin_dir.join("codex-plugin.json"),
        format!(r#"{{ "id": "{id}", "name": "{id}", "version": "1.0.0", "entry": "index.js" }}"#),
    )
    .expect("manifest writable");
    std::fs::write(plugin_dir.join("index.js"), entry).expect("entry writable");
}

/// Mark a plugin enabled in its store ledger so session start loads it.
fn enable_in_ledger(tree: &TempTree, id: &str) {
    let mut ledger = StoreLedger::load(&tree.store());
    ledger.state_mut(id).enabled = true;
    ledger.save(&tree.store()).expect("ledger save");
}

/// A test surface: records registrations/deregistrations and exposes a seeded
/// collision set (built-in names, so a plugin tool can collide on demand).
#[derive(Default)]
struct TestSurface {
    tools: BTreeSet<String>,
    commands: BTreeSet<String>,
    existing_tools: BTreeSet<String>,
    existing_commands: BTreeSet<String>,
    tool_adapters: Vec<Arc<PluginToolAdapter>>,
    command_adapters: Vec<Arc<PluginCommandAdapter>>,
    deregistered_tools: Vec<String>,
    deregistered_commands: Vec<String>,
}

impl PluginSurface for TestSurface {
    fn existing_tool_names(&self) -> BTreeSet<String> {
        let mut set = self.existing_tools.clone();
        set.extend(self.tools.iter().cloned());
        set
    }

    fn existing_command_names(&self) -> BTreeSet<String> {
        let mut set = self.existing_commands.clone();
        set.extend(self.commands.iter().cloned());
        set
    }

    fn register_tool(&mut self, adapter: Arc<PluginToolAdapter>) {
        self.tools.insert(adapter.name().to_string());
        self.tool_adapters.push(adapter);
    }

    fn register_command(&mut self, adapter: Arc<PluginCommandAdapter>) {
        self.commands.insert(adapter.name().to_string());
        self.command_adapters.push(adapter);
    }

    fn deregister_tool(&mut self, registry_name: &str) {
        self.tools.remove(registry_name);
        self.deregistered_tools.push(registry_name.to_string());
    }

    fn deregister_command(&mut self, trigger: &str) {
        self.commands.remove(trigger);
        self.deregistered_commands.push(trigger.to_string());
    }
}

const WEATHER_ENTRY: &str = r#"
ragent.register_tool({ name: "get_weather", description: "Get weather", parameters: { type: "object" }, handler: function(a){ return "sunny"; } });
ragent.register_command({ name: "weather", description: "Weather command" });
ragent.message.info("weather entry ran");
"#;

const TODO_ENTRY: &str = r#"
ragent.register_tool({ name: "add_todo", description: "Add a todo", parameters: { type: "object" }, handler: function(a){ return "ok"; } });
ragent.register_command({ name: "todo-add", description: "Add a todo" });
"#;

/// Session start loads only enabled plugins and registers their contributions.
#[test]
fn session_start_loads_enabled_and_registers_contributions() {
    let tree = TempTree::new("start-basic");
    write_plugin(&tree, "codex-weather", WEATHER_ENTRY);
    write_plugin(&tree, "inert", TODO_ENTRY);
    enable_in_ledger(&tree, "codex-weather");

    let mut surface = TestSurface::default();
    let session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );

    // Only the enabled plugin is loaded; the disabled one stays inert (FR-016).
    assert_eq!(session.reports().len(), 1);
    assert_eq!(session.reports()[0].id, "codex-weather");
    assert_eq!(session.reports()[0].state, LifecycleState::Loaded);
    assert_eq!(
        session.manager().state_of("inert"),
        LifecycleState::Disabled
    );

    // Contributions registered into the surface.
    assert!(surface.tools.contains("plugin_codex-weather_get_weather"));
    assert!(surface.commands.contains("weather"));
    assert!(!surface.tools.contains("plugin_inert_add_todo"));

    // Telemetry: loads_ok recorded in the ledger (FR-022).
    let ledger = StoreLedger::load(&tree.store());
    assert_eq!(
        ledger
            .state("codex-weather")
            .expect("row")
            .counters
            .loads_ok,
        1
    );
}

/// A manifest that fails to parse is reported errored and contributes nothing.
#[test]
fn session_start_reports_unparseable_enabled_plugin_as_errored() {
    let tree = TempTree::new("start-bad-manifest");
    write_plugin(&tree, "good", WEATHER_ENTRY);
    enable_in_ledger(&tree, "good");
    // A second enabled plugin whose manifest is truncated JSON.
    let bad_dir = tree.store().join("bad-manifest");
    std::fs::create_dir_all(&bad_dir).expect("dir");
    std::fs::write(bad_dir.join("codex-plugin.json"), "{ \"id\": \"bad\"").expect("manifest");
    std::fs::write(bad_dir.join("index.js"), "// unreachable").expect("entry");
    enable_in_ledger(&tree, "bad-manifest");

    let mut surface = TestSurface::default();
    let session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );

    let reports = session.reports();
    assert_eq!(reports.len(), 2);
    let bad = reports
        .iter()
        .find(|r| r.id == "bad-manifest")
        .expect("bad manifest reported");
    assert_eq!(bad.state, LifecycleState::Errored);
    assert!(
        bad.error
            .as_deref()
            .unwrap_or_default()
            .contains("manifest-parse"),
        "cause names manifest-parse; got {:?}",
        bad.error
    );
    // The healthy plugin still loaded.
    assert_eq!(surface.tools.len(), 1);
}

/// A tool name colliding with a built-in is refused; the plugin is errored and
/// contributes nothing (FR-024).
#[test]
fn session_start_rejects_tool_name_collision_with_builtin() {
    let tree = TempTree::new("start-tool-collision");
    write_plugin(&tree, "collider", WEATHER_ENTRY);
    enable_in_ledger(&tree, "collider");

    let mut surface = TestSurface::default();
    // Pre-seed the exact registry name the plugin will contribute, simulating a
    // built-in (or earlier plugin) already holding the slot.
    surface
        .existing_tools
        .insert("plugin_collider_get_weather".to_string());

    let session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );

    let outcome = session
        .registrations()
        .iter()
        .find(|r| r.plugin_id == "collider")
        .expect("registration outcome recorded");
    assert!(outcome.error.is_some(), "collision refused");
    assert!(outcome.tools.is_empty() && outcome.commands.is_empty());
    assert!(!surface.tools.contains("plugin_collider_get_weather"));
    assert_eq!(
        session.manager().state_of("collider"),
        LifecycleState::Errored
    );
}

/// A command trigger colliding with a built-in makes the whole plugin inert:
/// tools registered for the same plugin are not left behind.
#[test]
fn session_start_command_collision_rolls_back_tools() {
    let tree = TempTree::new("start-cmd-collision");
    write_plugin(&tree, "todo", TODO_ENTRY);
    enable_in_ledger(&tree, "todo");

    let mut surface = TestSurface::default();
    // `spec` is a real built-in slash command; simulate it as pre-existing.
    surface.existing_commands.insert("todo-add".to_string());

    let session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );

    let outcome = session
        .registrations()
        .iter()
        .find(|r| r.plugin_id == "todo")
        .expect("outcome");
    assert!(outcome.error.is_some());
    assert!(
        !surface.tools.contains("plugin_todo_add_todo"),
        "tool from a plugin whose command collided is not registered"
    );
    assert_eq!(session.manager().state_of("todo"), LifecycleState::Errored);
}

/// Two plugins contributing the same tool/command name: the second collides.
#[test]
fn session_start_second_plugin_collides_on_shared_name() {
    let tree = TempTree::new("start-cross-collision");
    // Same tool name under different ids does not collide (names are prefixed),
    // but the *command* trigger is unprefixed, so it does.
    write_plugin(
        &tree,
        "one",
        r#"ragent.register_command({ name: "shared", description: "one" });"#,
    );
    write_plugin(
        &tree,
        "two",
        r#"ragent.register_command({ name: "shared", description: "two" });"#,
    );
    enable_in_ledger(&tree, "one");
    enable_in_ledger(&tree, "two");

    let mut surface = TestSurface::default();
    let session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );

    let errored = session
        .registrations()
        .iter()
        .filter(|r| r.error.is_some())
        .count();
    assert_eq!(errored, 1, "exactly one of the two collides");
    // Exactly one registration succeeded with the shared trigger.
    assert_eq!(
        surface.commands.iter().filter(|c| *c == "shared").count(),
        1
    );
}

/// Shutdown unloads loaded plugins and deregisters what session start added.
#[test]
fn shutdown_deregisters_contributions_and_unloads() {
    let tree = TempTree::new("shutdown");
    write_plugin(&tree, "codex-weather", WEATHER_ENTRY);
    enable_in_ledger(&tree, "codex-weather");

    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );
    assert!(surface.tools.contains("plugin_codex-weather_get_weather"));

    let unloaded = session.shutdown(&mut surface);
    assert_eq!(unloaded.len(), 1);
    assert_eq!(unloaded[0].plugin_id, "codex-weather");
    assert_eq!(unloaded[0].tools, vec!["plugin_codex-weather_get_weather"]);
    assert_eq!(unloaded[0].commands, vec!["weather"]);

    // Nothing remains registered on the surface.
    assert!(surface.tools.is_empty());
    assert!(surface.commands.is_empty());
    assert!(
        surface
            .deregistered_tools
            .contains(&"plugin_codex-weather_get_weather".to_string())
    );
    assert!(
        surface
            .deregistered_commands
            .contains(&"weather".to_string())
    );

    // The live sandbox is gone: dispatching the tool now fails cleanly.
    let (result, _) = session
        .manager_mut()
        .execute_tool("plugin_codex-weather_get_weather", serde_json::json!({}));
    assert!(result.is_err(), "unloaded plugin cannot dispatch");
}

/// Shutdown is idempotent and leaves the enable flag intact so the plugin loads
/// again next session (FR-008).
#[test]
fn shutdown_is_idempotent_and_preserves_enabled_flag() {
    let tree = TempTree::new("shutdown-idempotent");
    write_plugin(&tree, "codex-weather", WEATHER_ENTRY);
    enable_in_ledger(&tree, "codex-weather");

    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );
    assert_eq!(session.shutdown(&mut surface).len(), 1);
    assert!(
        session.shutdown(&mut surface).is_empty(),
        "second shutdown no-op"
    );

    // Still enabled in the ledger: a fresh session loads it again.
    let ledger = StoreLedger::load(&tree.store());
    assert!(ledger.state("codex-weather").expect("row").enabled);
    let mut surface2 = TestSurface::default();
    let session2 = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface2,
    );
    assert_eq!(
        session2.manager().state_of("codex-weather"),
        LifecycleState::Loaded
    );
}

/// An entry that throws is errored and contributes no tools, but a healthy
/// sibling still loads.
#[test]
fn session_start_entry_failure_does_not_affect_other_plugins() {
    let tree = TempTree::new("start-entry-fail");
    write_plugin(&tree, "boom", r#"throw new Error("entry blew up");"#);
    write_plugin(&tree, "codex-weather", WEATHER_ENTRY);
    enable_in_ledger(&tree, "boom");
    enable_in_ledger(&tree, "codex-weather");

    let mut surface = TestSurface::default();
    let session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );

    let boom = session
        .reports()
        .iter()
        .find(|r| r.id == "boom")
        .expect("boom reported");
    assert_eq!(boom.state, LifecycleState::Errored);
    assert!(boom.error.as_deref().unwrap_or_default().contains("entry"));

    assert_eq!(
        session.manager().state_of("codex-weather"),
        LifecycleState::Loaded
    );
    assert!(surface.tools.contains("plugin_codex-weather_get_weather"));
    // The failed plugin contributes no tool names.
    assert!(!surface.tools.iter().any(|t| t.starts_with("plugin_boom_")));
}

/// No enabled plugins: session start is a quiet no-op.
#[test]
fn session_start_with_no_enabled_plugins_registers_nothing() {
    let tree = TempTree::new("start-empty");
    write_plugin(&tree, "disabled", WEATHER_ENTRY);

    let mut surface = TestSurface::default();
    let session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );
    assert!(session.reports().is_empty());
    assert!(surface.tools.is_empty());
    assert!(surface.commands.is_empty());
}

/// `record_registration_error` marks a plugin errored and drops its context,
/// and is a no-op for an unknown id.
#[test]
fn record_registration_error_marks_errored_and_drops_context() {
    let tree = TempTree::new("record-error");
    write_plugin(&tree, "codex-weather", WEATHER_ENTRY);
    let mut manager = PluginManager::new(tree.dirs(), ragent_config::PluginsConfig::default());
    let report = manager.enable("codex-weather").expect("enable");
    assert_eq!(report.state, LifecycleState::Loaded);
    assert!(
        manager
            .get("codex-weather")
            .expect("tracked")
            .context()
            .is_some()
    );

    manager.record_registration_error("codex-weather", "name-collision: boom".to_string());
    let tracked = manager.get("codex-weather").expect("tracked");
    assert_eq!(tracked.state, LifecycleState::Errored);
    assert!(tracked.context().is_none());
    assert_eq!(tracked.error.as_deref(), Some("name-collision: boom"));
    // Dispatch is refused once the context is gone.
    let (result, _) =
        manager.execute_tool("plugin_codex-weather_get_weather", serde_json::json!({}));
    assert!(result.is_err());

    // Unknown id: no panic, no state change.
    manager.record_registration_error("ghost", "ignored".to_string());
    assert!(manager.get("ghost").is_none());
}
