//! Tests for the `/plugins list`, `/plugins enable`, and `/plugins disable`
//! command family (spec `plugins` T-013; FR-009, FR-011, FR-012, FR-016,
//! FR-022, FR-025).

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use ragent_plugins::{
    ControlArgError, ControlCommand, LifecycleState, PluginCommandAdapter, PluginSession,
    PluginSurface, PluginToolAdapter, StoreDirs, parse_control_command, render_list,
    run_control_command,
};
use ragent_tools_core::Tool;

static SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// RAII sandboxed temp tree rooted at `target/temp/` (AGENTS.md: no `/tmp`).
struct TempTree(PathBuf);

impl TempTree {
    fn new(name: &str) -> Self {
        let unique = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
            "../../target/temp/plugins-test/control-{name}-{}-{unique}",
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
    fn dirs(&self) -> StoreDirs {
        ragent_plugins::store_dirs_at(&self.0, Some(&self.store()), None)
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// Write a Codex-dialect plugin declaring a tool and a command in its manifest
/// (so `/plugins list` counts them even while the plugin is disabled).
fn write_plugin(tree: &TempTree, id: &str, manifest_extra: &str, entry: &str) {
    let plugin_dir = tree.store().join(id);
    std::fs::create_dir_all(&plugin_dir).expect("plugin dir creatable");
    let manifest = format!(
        r#"{{ "id": "{id}", "name": "{id}", "version": "1.2.3", "entry": "index.js",
             "tools": [{{ "name": "get_weather", "description": "Get weather", "parameters": {{ "type": "object" }} }}],
             "commands": [{{ "name": "{id}-go", "description": "Go" }}]{manifest_extra} }}"#
    );
    std::fs::write(plugin_dir.join("codex-plugin.json"), manifest).expect("manifest writable");
    std::fs::write(plugin_dir.join("index.js"), entry).expect("entry writable");
}

/// A test surface that records registrations and deregistrations.
#[derive(Default)]
struct TestSurface {
    tools: BTreeSet<String>,
    commands: BTreeSet<String>,
    deregistered_tools: Vec<String>,
    deregistered_commands: Vec<String>,
    tool_adapters: Vec<Arc<PluginToolAdapter>>,
    command_adapters: Vec<Arc<PluginCommandAdapter>>,
}

impl PluginSurface for TestSurface {
    fn existing_tool_names(&self) -> BTreeSet<String> {
        self.tools.clone()
    }

    fn existing_command_names(&self) -> BTreeSet<String> {
        self.commands.clone()
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

const ENTRY: &str = r#"
ragent.message.info("entry ran");
"#;

// ── parse_control_command ────────────────────────────────────────────────────

#[test]
fn parse_list_detects_verbose() {
    assert_eq!(
        parse_control_command("list", ""),
        Some(Ok(ControlCommand::List { verbose: false }))
    );
    assert_eq!(
        parse_control_command("list", "--verbose"),
        Some(Ok(ControlCommand::List { verbose: true }))
    );
    assert_eq!(
        parse_control_command("list", "-v"),
        Some(Ok(ControlCommand::List { verbose: true }))
    );
}

#[test]
fn parse_enable_and_disable_take_first_token() {
    assert_eq!(
        parse_control_command("enable", "codex-weather extra"),
        Some(Ok(ControlCommand::Enable {
            plugin_id: "codex-weather".to_string(),
        }))
    );
    assert_eq!(
        parse_control_command("disable", "codex-weather"),
        Some(Ok(ControlCommand::Disable {
            plugin_id: "codex-weather".to_string(),
        }))
    );
}

#[test]
fn parse_enable_without_id_is_missing_id() {
    assert_eq!(
        parse_control_command("enable", "   "),
        Some(Err(ControlArgError::MissingEnableId))
    );
    assert_eq!(
        parse_control_command("disable", ""),
        Some(Err(ControlArgError::MissingDisableId))
    );
}

#[test]
fn parse_other_subcommand_returns_none() {
    assert_eq!(parse_control_command("add", "/tmp/x"), None);
    assert_eq!(parse_control_command("help", ""), None);
}

#[test]
fn arg_error_reports_are_actionable() {
    let report = ControlArgError::MissingEnableId.report("enable");
    assert!(report.contains("From: /plugins enable"));
    assert!(report.contains("[err]"));
    assert!(report.contains("Usage: `/plugins enable <pluginid>`"));
    let report = ControlArgError::MissingDisableId.report("disable");
    assert!(report.contains("Usage: `/plugins disable <pluginid>`"));
}

// ── `/plugins list` (FR-009, FR-022, FR-025) ─────────────────────────────────

#[test]
fn list_empty_store_reports_none_discovered() {
    let tree = TempTree::new("list-empty");
    let manager =
        ragent_plugins::PluginManager::new(tree.dirs(), ragent_config::PluginsConfig::default());
    let out = render_list(&manager, false);
    assert!(out.contains("From: /plugins list"));
    assert!(out.contains("No plugins discovered."));
}

#[test]
fn list_shows_row_state_counts_summary_and_contributions() {
    let tree = TempTree::new("list-rows");
    write_plugin(&tree, "codex-weather", "", ENTRY);
    let manager =
        ragent_plugins::PluginManager::new(tree.dirs(), ragent_config::PluginsConfig::default());

    let out = render_list(&manager, false);
    // Row fields (FR-009).
    assert!(out.contains("codex-weather"));
    assert!(out.contains("1.2.3"));
    assert!(out.contains("codex"));
    assert!(out.contains("disabled"));
    // Contributed names, from the manifest even while disabled.
    assert!(out.contains("plugin_codex-weather_get_weather"));
    assert!(out.contains("codex-weather-go"));
    // Summary totals line: loaded plugins fold into `enabled`.
    assert!(out.contains("Total: 1 plugin(s) - 0 enabled, 1 disabled, 0 errored."));
}

#[test]
fn list_shows_skill_agent_and_hook_counts_and_details() {
    let tree = TempTree::new("list-contributions");
    // A Claude-dialect plugin contributing a skill, an agent profile, and hooks.
    let plugin_dir = tree.store().join("claude-tools");
    std::fs::create_dir_all(plugin_dir.join(".claude-plugin")).unwrap();
    std::fs::create_dir_all(plugin_dir.join("skills/db-setup")).unwrap();
    std::fs::create_dir_all(plugin_dir.join("agents")).unwrap();
    std::fs::write(
        plugin_dir.join(".claude-plugin/plugin.json"),
        r#"{ "name": "claude-tools", "version": "2.0.0", "skills": "./skills/",
             "hooks": { "PreToolUse": "./check.sh", "SessionStart": "./start.sh" } }"#,
    )
    .unwrap();
    std::fs::write(
        plugin_dir.join("skills/db-setup/SKILL.md"),
        "---\nname: db-setup\ndescription: Set up the database\n---\n\nBody.\n",
    )
    .unwrap();
    std::fs::write(
        plugin_dir.join("agents/security-reviewer.md"),
        "---\nname: security-reviewer\nmode: subagent\n---\n\nReview.\n",
    )
    .unwrap();
    let manager =
        ragent_plugins::PluginManager::new(tree.dirs(), ragent_config::PluginsConfig::default());

    let out = render_list(&manager, false);
    // A header row per contribution kind.
    assert!(out.contains("Skills"), "skills column: {out}");
    assert!(out.contains("Agents"), "agents column: {out}");
    assert!(out.contains("Hooks"), "hooks column: {out}");
    // Counts and detailed names/sections.
    assert!(out.contains("skills [db-setup]"), "skill name: {out}");
    assert!(
        out.contains("agents [agents/security-reviewer.md]"),
        "agent name: {out}"
    );
    assert!(
        out.contains("hooks [PreToolUse, SessionStart]"),
        "hooks: {out}"
    );
}

#[test]
fn list_verbose_includes_telemetry_counters() {
    let tree = TempTree::new("list-verbose");
    write_plugin(&tree, "codex-weather", "", ENTRY);
    let manager =
        ragent_plugins::PluginManager::new(tree.dirs(), ragent_config::PluginsConfig::default());

    let plain = render_list(&manager, false);
    assert!(!plain.contains("loads_ok="), "plain list omits telemetry");
    let verbose = render_list(&manager, true);
    assert!(verbose.contains("Telemetry:"));
    assert!(verbose.contains("loads_ok=0"));
    assert!(verbose.contains("tool_invocations=0"));
}

#[test]
fn list_reports_unsupported_capabilities() {
    let tree = TempTree::new("list-unsupported");
    // An unbridgeable MCP shape (a JSON array) keeps the FR-025 label.
    write_plugin(
        &tree,
        "codex-weather",
        r#", "mcp_servers": [ "not", "an", "object" ]"#,
        ENTRY,
    );
    let manager =
        ragent_plugins::PluginManager::new(tree.dirs(), ragent_config::PluginsConfig::default());

    let out = render_list(&manager, false);
    assert!(out.contains("Unsupported capabilities:"));
    assert!(out.contains("mcp server transports"), "got:\n{out}");
}

#[test]
fn list_reports_parse_failure_row() {
    let tree = TempTree::new("list-badmanifest");
    let bad_dir = tree.store().join("bad");
    std::fs::create_dir_all(&bad_dir).unwrap();
    std::fs::write(bad_dir.join("codex-plugin.json"), "{ \"id\": \"bad\"").unwrap();
    std::fs::write(bad_dir.join("index.js"), "// x").unwrap();
    let manager =
        ragent_plugins::PluginManager::new(tree.dirs(), ragent_config::PluginsConfig::default());

    let out = render_list(&manager, false);
    assert!(out.contains("bad"), "directory-name row shown:\n{out}");
    assert!(out.contains("Errors:"));
    assert!(out.contains("manifest-parse"));
}

// ── `/plugins enable` / `/plugins disable` (FR-011, FR-012, FR-016) ──────────

#[test]
fn run_enable_loads_registers_and_reports_state() {
    let tree = TempTree::new("enable");
    write_plugin(&tree, "codex-weather", "", ENTRY);
    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );

    let out = run_control_command(&mut session, &mut surface, "enable", "codex-weather")
        .expect("handled");
    assert!(out.contains("From: /plugins enable codex-weather"));
    assert!(out.contains("[ok]"));
    assert!(out.contains("Declared permissions: none"));
    assert!(out.contains("registered 1 tool(s) and 1 command(s)"));

    assert_eq!(
        session.manager().state_of("codex-weather"),
        LifecycleState::Loaded
    );
    assert!(surface.tools.contains("plugin_codex-weather_get_weather"));
    assert!(surface.commands.contains("codex-weather-go"));

    // Now listed as loaded.
    let list = render_list(session.manager(), false);
    assert!(list.contains("loaded"));
}

#[test]
fn run_enable_states_declared_permissions() {
    let tree = TempTree::new("enable-perms");
    write_plugin(
        &tree,
        "codex-weather",
        r#", "permissions": { "network": ["outbound"] }"#,
        ENTRY,
    );
    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );

    let out = run_control_command(&mut session, &mut surface, "enable", "codex-weather")
        .expect("handled");
    assert!(
        out.contains("Declared permissions: network.outbound"),
        "got:\n{out}"
    );
}

#[test]
fn run_enable_is_idempotent_when_the_plugin_is_already_loaded() {
    // A plugin installed enabled (FR-007) is loaded by `PluginSession::start`;
    // an explicit `enable` must re-report it, not collide with its own
    // contributions.
    let tree = TempTree::new("enable-idempotent");
    write_plugin(&tree, "codex-weather", "", ENTRY);
    let mut ledger = ragent_plugins::StoreLedger::load(&tree.store());
    ledger.state_mut("codex-weather").enabled = true;
    ledger.save(&tree.store()).expect("ledger save");

    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );
    assert!(
        surface.tools.contains("plugin_codex-weather_get_weather"),
        "session start loaded the enabled plugin"
    );

    let out = run_control_command(&mut session, &mut surface, "enable", "codex-weather")
        .expect("handled");
    assert!(out.contains("[ok]"), "re-enable succeeds: {out}");
    assert!(
        out.contains("registered 1 tool(s) and 1 command(s)"),
        "the existing contributions are re-reported: {out}"
    );
    assert_eq!(surface.tool_adapters.len(), 1, "no duplicate registration");
}

#[test]
fn run_enable_unknown_plugin_is_reported_not_returned() {
    let tree = TempTree::new("enable-unknown");
    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );

    let out = run_control_command(&mut session, &mut surface, "enable", "ghost").expect("handled");
    assert!(out.contains("[err]"));
    assert!(out.contains("unknown plugin"));
}

#[test]
fn run_enable_without_id_reports_usage() {
    let tree = TempTree::new("enable-noid");
    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );

    let out = run_control_command(&mut session, &mut surface, "enable", "").expect("handled");
    assert!(out.contains("[err] Missing <pluginid>"));
    assert!(out.contains("Usage: `/plugins enable <pluginid>`"));
}

#[test]
fn run_disable_unloads_and_deregisters() {
    let tree = TempTree::new("disable");
    write_plugin(&tree, "codex-weather", "", ENTRY);
    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );

    run_control_command(&mut session, &mut surface, "enable", "codex-weather").unwrap();
    assert!(surface.tools.contains("plugin_codex-weather_get_weather"));

    let out = run_control_command(&mut session, &mut surface, "disable", "codex-weather")
        .expect("handled");
    assert!(out.contains("From: /plugins disable codex-weather"));
    assert!(out.contains("[ok]"));
    assert!(out.contains("deregistered 1 tool(s) and 1 command(s)"));

    // Disabled plugin is fully inert: nothing left registered, listed disabled
    // (FR-016).
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
            .contains(&"codex-weather-go".to_string())
    );
    assert_eq!(
        session.manager().state_of("codex-weather"),
        LifecycleState::Disabled
    );
    let list = render_list(session.manager(), false);
    assert!(list.contains("disabled"));
}

#[test]
fn run_disable_unknown_plugin_is_reported() {
    let tree = TempTree::new("disable-unknown");
    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );

    let out = run_control_command(&mut session, &mut surface, "disable", "ghost").expect("handled");
    assert!(out.contains("[err]"));
    assert!(out.contains("unknown plugin"));
}

#[test]
fn run_control_command_returns_none_for_other_subcommands() {
    let tree = TempTree::new("none");
    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );
    assert!(run_control_command(&mut session, &mut surface, "add", "/tmp/x").is_none());
    assert!(run_control_command(&mut session, &mut surface, "remove", "x").is_none());
}

#[test]
fn run_control_command_reports_disabled_subsystem_master_switch() {
    // Acceptance criterion 8: `plugins.enabled: false` makes every control
    // subcommand report the disabled subsystem and perform no discovery.
    let tree = TempTree::new("disabled");
    write_plugin(&tree, "codex-weather", "", ENTRY);
    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig {
            enabled: false,
            ..ragent_config::PluginsConfig::default()
        },
        &mut surface,
    );

    for (sub, args) in [
        ("list", ""),
        ("enable", "codex-weather"),
        ("disable", "codex-weather"),
    ] {
        let out = run_control_command(&mut session, &mut surface, sub, args)
            .unwrap_or_else(|| panic!("{sub} must be handled"));
        assert!(out.contains("[err]"), "{sub}: {out}");
        assert!(out.contains("plugins.enabled = false"), "{sub}: {out}");
    }
    // Nothing was loaded or registered.
    assert!(surface.tools.is_empty());
    assert!(session.reports().is_empty());
}

#[test]
fn run_lifecycle_uses_ledger_store_and_reloads_next_session() {
    let tree = TempTree::new("reload");
    write_plugin(&tree, "codex-weather", "", ENTRY);
    let mut surface = TestSurface::default();
    let mut session = PluginSession::start(
        tree.dirs(),
        ragent_config::PluginsConfig::default(),
        &mut surface,
    );
    run_control_command(&mut session, &mut surface, "enable", "codex-weather").unwrap();
    session.shutdown(&mut surface);

    // The enable flag persisted: a fresh session loads it at start.
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
    assert!(surface2.tools.contains("plugin_codex-weather_get_weather"));
}
