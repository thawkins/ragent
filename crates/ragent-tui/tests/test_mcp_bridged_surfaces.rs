//! Bridged MCP servers on the `/mcp` surfaces (spec `plugins` FR-030; spec
//! `connectors` FR-003, FR-020, FR-026, FR-033).
//!
//! A server contributed by an enabled plugin's `mcpServers` section or resolved
//! from an enabled connector is *bridged* under `<owner-id>.<server>`. It is part
//! of the session's MCP server set, so every `/mcp` surface must see it:
//!
//! - `/mcp` lists it, names the file that declares it, and names its owner;
//! - `/mcp discover` offers it as a row so it can be enabled and disabled;
//! - `/mcp remove` refuses it and names `/plugins remove` / `/connectors remove`.
//!
//! Persistence tests redirect cwd + `XDG_CONFIG_HOME` into a tempdir so nothing
//! touches the developer's real config. Env mutation is `unsafe` in edition 2024
//! and the workspace denies `unsafe_code`, so this target opts back in
//! explicitly (env mutation is contained to the binary).
#![allow(unsafe_code)]
// The test serialiser is intentionally held across the async `#[tokio::test]`
// bodies so cwd/env mutations cannot interleave.
#![allow(clippy::await_holding_lock)]
#![cfg(test)]

use std::sync::MutexGuard;

use ragent_tui::App;

#[path = "support/mod.rs"]
mod support;

/// The process-wide cwd/env lock (see `tests/all.rs`).
fn test_lock() -> MutexGuard<'static, ()> {
    crate::global_env_lock()
}

/// Restores cwd and config env vars on drop.
struct EnvGuard {
    keys: Vec<(&'static str, Option<String>)>,
    cwd: std::path::PathBuf,
}

impl EnvGuard {
    fn new() -> Self {
        Self {
            keys: vec![
                ("XDG_CONFIG_HOME", std::env::var("XDG_CONFIG_HOME").ok()),
                ("RAGENT_CONFIG", std::env::var("RAGENT_CONFIG").ok()),
                (
                    "RAGENT_CONFIG_CONTENT",
                    std::env::var("RAGENT_CONFIG_CONTENT").ok(),
                ),
            ],
            cwd: std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")),
        }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        for (k, v) in &self.keys {
            match v {
                Some(val) => unsafe { std::env::set_var(k, val) },
                None => unsafe { std::env::remove_var(k) },
            }
        }
        std::env::set_current_dir(&self.cwd).ok();
    }
}

/// Enter an isolated tempdir project.
fn enter_project() -> (
    std::sync::MutexGuard<'static, ()>,
    EnvGuard,
    tempfile::TempDir,
) {
    let guard = test_lock();
    let env_guard = EnvGuard::new();
    let temp = tempfile::tempdir().expect("tempdir");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", temp.path().join(".config")) };
    unsafe { std::env::remove_var("RAGENT_CONFIG") };
    unsafe { std::env::remove_var("RAGENT_CONFIG_CONTENT") };
    std::env::set_current_dir(temp.path()).expect("set cwd");

    let ragent_dir = temp.path().join(".ragent");
    std::fs::create_dir_all(&ragent_dir).expect("create .ragent");
    std::fs::write(ragent_dir.join("ragent.json"), "{}").expect("write project config");
    ragent_config::Config::invalidate_load_cache();
    (guard, env_guard, temp)
}

fn app() -> App {
    let mut app = support::make_app();
    app.session_id = Some("mcp-bridge".to_string());
    app
}

fn last_message(app: &App) -> String {
    app.messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default()
}

/// Install an enabled plugin that declares `servers` inline under `mcpServers`.
fn install_plugin(temp: &tempfile::TempDir, plugin_id: &str, servers_json: &str) {
    let store = temp.path().join(".config/ragent/plugins");
    let plugin = store.join(plugin_id);
    std::fs::create_dir_all(plugin.join(".claude-plugin")).expect("manifest dir");
    std::fs::write(
        plugin.join(".claude-plugin/plugin.json"),
        format!(r#"{{"name":"{plugin_id}","version":"1.0.0","mcpServers":{servers_json}}}"#),
    )
    .expect("write manifest");
    std::fs::write(
        store.join("_state.json"),
        format!(r#"{{"plugins":{{"{plugin_id}":{{"enabled":true}}}}}}"#),
    )
    .expect("write state ledger");
    ragent_config::Config::invalidate_load_cache();
}

/// Install an enabled connector declaring one `http` server `main`.
fn install_connector(temp: &tempfile::TempDir, connector_id: &str) {
    let store = temp.path().join(".ragent/connectors");
    let dir = store.join(connector_id);
    std::fs::create_dir_all(&dir).expect("connector dir");
    std::fs::write(
        dir.join("connector.json"),
        format!(
            r#"{{"id":"{connector_id}","name":"Demo","servers":[
                {{"id":"main","transport":"http","url":"https://example.invalid/mcp"}}
            ]}}"#
        ),
    )
    .expect("write connector manifest");
    let mut ledger = ragent_connectors::StoreLedger::load(&store);
    ledger.state_mut(connector_id).enabled = true;
    ledger.save(&store).expect("ledger save");
    ragent_config::Config::invalidate_load_cache();
}

// ---------------------------------------------------------------------------
// `/mcp` lists bridged servers and names their owner
// ---------------------------------------------------------------------------

/// A connector-bridged server appears in `/mcp` with the file that declares it
/// and the connector that owns it, so the user can tell a bridged row from a
/// configured one.
#[tokio::test]
async fn mcp_list_includes_a_connector_bridged_server_and_its_owner() {
    let (_guard, _env, temp) = enter_project();
    install_connector(&temp, "demo");
    let mut app = app();

    app.execute_slash_command("/mcp").await;

    let report = last_message(&app);
    assert!(
        report.contains("demo.main"),
        "the bridged connector server must be listed: {report}"
    );
    let manifest = temp.path().join(".ragent/connectors/demo/connector.json");
    assert!(
        report.contains(&manifest.display().to_string()),
        "the connector manifest must be the reported source: {report}"
    );
    assert!(
        report.contains("Managed by"),
        "the Managed by block must be present: {report}"
    );
    assert!(
        report.contains("demo"),
        "the owning connector must be named: {report}"
    );
}

/// A plugin-bridged server appears in `/mcp` beside the connector one, each
/// attributing its own owner.
#[tokio::test]
async fn mcp_list_includes_both_a_plugin_and_a_connector_bridged_server() {
    let (_guard, _env, temp) = enter_project();
    install_plugin(&temp, "weather", r#"{"forecast":{"command":"npx"}}"#);
    install_connector(&temp, "demo");
    let mut app = app();

    app.execute_slash_command("/mcp").await;

    let report = last_message(&app);
    assert!(report.contains("weather.forecast"), "report: {report}");
    assert!(report.contains("demo.main"), "report: {report}");
    assert!(
        report.contains("plugin") && report.contains("connector"),
        "both source labels must be visible: {report}"
    );
}

/// A configured `ragent.json` server keeps precedence: a bridged id that would
/// collide with it is never listed twice.
#[tokio::test]
async fn mcp_list_keeps_configured_servers_ahead_of_a_colliding_bridge() {
    let (_guard, _env, temp) = enter_project();
    // A configured server wins the id, so the connector bridge refuses its own.
    // `Config::merge_project` strips a project-local `mcp` section (SEC-001), so
    // the configured server lives in the user-global file.
    let global = temp.path().join(".config/ragent/ragent.json");
    std::fs::create_dir_all(global.parent().expect("parent")).expect("create global dir");
    std::fs::write(
        &global,
        r#"{"mcp":{"demo.main":{"type":"stdio","command":"echo"}}}"#,
    )
    .expect("write global config");
    install_connector(&temp, "demo");
    ragent_config::Config::invalidate_load_cache();
    let mut app = app();

    app.execute_slash_command("/mcp").await;

    let report = last_message(&app);
    // The id appears once in the main table and once in the `Defined in` block;
    // a second main-table row would mean the bridge had not been refused.
    let main_rows = report
        .lines()
        .filter(|line| line.starts_with("| demo.main |") && line.matches('|').count() == 7)
        .count();
    assert_eq!(
        main_rows, 1,
        "a colliding bridged id must be refused, not listed twice: {report}"
    );
}

// ---------------------------------------------------------------------------
// `/mcp discover` offers bridged servers as toggleable rows
// ---------------------------------------------------------------------------

/// The discover rows include the plugin- and connector-bridged servers, labelled
/// with their origin and pointing at the file that declares them.
#[tokio::test]
async fn discover_rows_include_bridged_servers_with_their_origin() {
    let (_guard, _env, temp) = enter_project();
    install_plugin(&temp, "weather", r#"{"forecast":{"command":"npx"}}"#);
    install_connector(&temp, "demo");

    let rows = ragent_tui::app::bridged_discover_rows_for_tests(
        temp.path(),
        &std::collections::HashMap::new(),
    );

    let forecast = rows
        .iter()
        .find(|(id, _, _)| id == "weather.forecast")
        .expect("the plugin-bridged row must be present");
    assert_eq!(forecast.1, "plugin", "origin label: {forecast:?}");
    assert!(
        forecast
            .2
            .as_ref()
            .is_some_and(|path| path.ends_with("plugin.json")),
        "the plugin manifest must be the defining file: {forecast:?}"
    );

    let main = rows
        .iter()
        .find(|(id, _, _)| id == "demo.main")
        .expect("the connector-bridged row must be present");
    assert_eq!(main.1, "connector", "origin label: {main:?}");
    assert!(
        main.2
            .as_ref()
            .is_some_and(|path| path.ends_with("connector.json")),
        "the connector manifest must be the defining file: {main:?}"
    );
}

/// Toggling a bridged row changes only the enable ledger: no `ragent.json` entry
/// is written, so the owner's declaration is not shadowed by a duplicate.
///
/// The press goes through the real input path with the `/mcp discover` dialog
/// open on the bridged row, which is exactly what the user does.
#[tokio::test]
async fn toggling_a_bridged_row_writes_only_the_ledger() {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    let (_guard, _env, temp) = enter_project();
    install_plugin(&temp, "weather", r#"{"forecast":{"command":"npx"}}"#);
    let mut app = app();

    let rows = ragent_tui::app::bridged_discover_rows_for_tests(
        temp.path(),
        &std::collections::HashMap::new(),
    );
    let (id, _, definition_file) = rows
        .into_iter()
        .find(|(id, _, _)| id == "weather.forecast")
        .expect("the plugin-bridged row must be present");
    let server = ragent_agent::mcp::discovery::DiscoveredMcpServer {
        id,
        name: "weather".to_string(),
        executable: std::path::PathBuf::from("npx"),
        args: Vec::new(),
        env: std::collections::HashMap::new(),
        source: ragent_agent::mcp::discovery::McpDiscoverySource::Plugin {
            plugin_id: "weather".to_string(),
        },
        definition_file,
    };
    app.mcp_discover = Some(ragent_tui::app::McpDiscoverState {
        // The bridged server is enabled by default (absent from the ledger).
        enabled: vec![true],
        servers: vec![server],
        selected: 0,
        scroll_offset: 0,
        feedback: None,
    });

    // First press: disable. A bridged server is enabled by default (absent from
    // the ledger), so this records the explicit disable.
    app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    let state = app.mcp_discover.as_ref().expect("dialog open");
    assert!(!state.enabled[0], "the bridged server must read disabled");
    assert!(
        state
            .feedback
            .as_deref()
            .is_some_and(|m| m.starts_with("[ok]")),
        "feedback: {:?}",
        state.feedback
    );

    // The global ledger holds the choice; no `mcp` section was invented.
    let global = temp.path().join(".config/ragent/ragent.json");
    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&global).unwrap_or_else(|_| "{}".to_string()),
    )
    .expect("parse config");
    assert!(
        config.get("mcp").is_none(),
        "a bridged toggle must not write a ragent.json mcp entry: {config}"
    );

    // Second press: enable again, dropping the ledger entry.
    app.handle_key_event(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE))
        .await;
    let state = app.mcp_discover.as_ref().expect("dialog open");
    assert!(state.enabled[0], "the bridged server must read enabled");
    assert!(
        ragent_agent::mcp::McpEnableLedger::load().is_enabled("weather.forecast"),
        "enabling a bridged server clears its ledger entry"
    );
}

// ---------------------------------------------------------------------------
// `/mcp remove` refuses a bridged server
// ---------------------------------------------------------------------------

/// A connector-bridged server cannot be removed through `/mcp remove`: the
/// refusal names `/connectors remove <id>` and the connector's manifest is left
/// untouched.
#[tokio::test]
async fn mcp_remove_refuses_a_connector_bridged_server() {
    let (_guard, _env, temp) = enter_project();
    install_connector(&temp, "demo");
    let manifest = temp.path().join(".ragent/connectors/demo/connector.json");
    let before = std::fs::read_to_string(&manifest).expect("read manifest");
    let mut app = app();

    app.execute_slash_command("/mcp remove demo.main").await;

    let report = last_message(&app);
    assert!(
        report.contains("[err]"),
        "the removal must be refused: {report}"
    );
    assert!(
        report.contains("/connectors remove demo"),
        "the refusal must name the connector's own removal command: {report}"
    );
    assert_eq!(
        std::fs::read_to_string(&manifest).expect("read manifest"),
        before,
        "the connector manifest must be untouched"
    );
}

/// A plugin-bridged server is likewise refused, and the refusal points at
/// `/plugins remove <id>` while noting that enable/disable still works.
#[tokio::test]
async fn mcp_remove_refuses_a_plugin_bridged_server_with_disable_guidance() {
    let (_guard, _env, temp) = enter_project();
    install_plugin(&temp, "weather", r#"{"forecast":{"command":"npx"}}"#);
    let mut app = app();

    app.execute_slash_command("/mcp remove weather.forecast")
        .await;

    let report = last_message(&app);
    assert!(
        report.contains("[err]"),
        "the removal must be refused: {report}"
    );
    assert!(
        report.contains("/plugins remove weather"),
        "the refusal must name the plugin's own removal command: {report}"
    );
    assert!(
        report.contains("/mcp disconnect weather.forecast"),
        "the refusal must say enable/disable still works: {report}"
    );
}

/// The refusal wording for a plugin and for a connector names the right command
/// and never the other subsystem's.
#[test]
fn bridged_removal_refusal_names_the_owning_subsystem() {
    let plugin = ragent_tui::app::bridged_removal_refusal_for_tests("p.s", "p", true);
    assert!(plugin.contains("/plugins remove p"), "{plugin}");
    assert!(!plugin.contains("/connectors remove"), "{plugin}");

    let connector = ragent_tui::app::bridged_removal_refusal_for_tests("c.s", "c", false);
    assert!(connector.contains("/connectors remove c"), "{connector}");
    assert!(!connector.contains("/plugins remove"), "{connector}");
}

/// `/mcp remove` still removes a genuinely configured server: the refusal is
/// scoped to bridged ids only.
#[tokio::test]
async fn mcp_remove_still_removes_a_configured_server_when_a_bridge_exists() {
    let (_guard, _env, temp) = enter_project();
    // `Config::merge_project` strips a project-local `mcp` section (SEC-001), so
    // the configured server lives in the user-global file, exactly as the
    // connect loop reads it.
    let global = temp.path().join(".config/ragent/ragent.json");
    std::fs::create_dir_all(global.parent().expect("parent")).expect("create global dir");
    std::fs::write(
        &global,
        r#"{"mcp":{"configured":{"type":"stdio","command":"echo"}}}"#,
    )
    .expect("write global config");
    install_connector(&temp, "demo");
    ragent_config::Config::invalidate_load_cache();
    let mut app = app();

    app.execute_slash_command("/mcp remove configured").await;

    let report = last_message(&app);
    assert!(report.contains("[ok]"), "report: {report}");
    let config: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(&global).expect("read config"))
            .expect("parse config");
    assert!(
        config.get("mcp").is_none(),
        "the configured entry must be gone: {config}"
    );
}

// ---------------------------------------------------------------------------
// `/mcp connect|disconnect` resolve a bridged id
// ---------------------------------------------------------------------------

/// `/mcp connect <id>` resolves a connector-bridged id to the config the owner
/// declares, so the enable is accepted (and persisted) instead of being rejected
/// as an unknown server.
#[tokio::test]
async fn mcp_connect_resolves_a_connector_bridged_config() {
    let (_guard, _env, temp) = enter_project();
    install_connector(&temp, "demo");
    let mut app = app();

    app.execute_slash_command("/mcp connect demo.main").await;

    let report = last_message(&app);
    assert!(
        !report.contains("[err]"),
        "a bridged id must resolve, not be rejected as unknown: {report}"
    );
    assert!(report.contains("demo.main"), "report: {report}");
}

/// `/mcp disconnect <id>` on a plugin-bridged id records the durable disable in
/// the global ledger and never writes a `ragent.json` entry.
#[tokio::test]
async fn mcp_disconnect_records_a_plugin_bridged_server_in_the_ledger() {
    let (_guard, _env, temp) = enter_project();
    install_plugin(&temp, "weather", r#"{"forecast":{"command":"npx"}}"#);
    let mut app = app();

    app.execute_slash_command("/mcp disconnect weather.forecast")
        .await;

    let report = last_message(&app);
    assert!(
        !report.contains("[err]"),
        "a bridged id must resolve, not be rejected as unknown: {report}"
    );
    assert!(
        !ragent_agent::mcp::McpEnableLedger::load().is_enabled("weather.forecast"),
        "the disable must reach the global ledger"
    );
    let global = temp.path().join(".config/ragent/ragent.json");
    let config: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(&global).unwrap_or_else(|_| "{}".to_string()),
    )
    .expect("parse config");
    assert!(
        config.get("mcp").is_none(),
        "a bridged disable must not write a ragent.json entry: {config}"
    );
}
