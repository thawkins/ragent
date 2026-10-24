//! `/mcp remove <id>` tests.
//!
//! The command disconnects a server and deletes its definition from the file
//! that declares it: the `mcp` section of the `ragent.json` for a configured
//! server, or the `mcpServers` section of a plugin manifest / external
//! `mcp.json` for a plugin-contributed one. `/mcp` reports the defining file in
//! its `Defined in` column so the user can see what would be deleted.
//!
//! Persistence tests redirect cwd + `XDG_CONFIG_HOME` into a tempdir so a write
//! never touches the developer's real config. Env mutation is `unsafe` in
//! edition 2024; the workspace denies `unsafe_code`, so this target opts back in
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

/// Enter an isolated tempdir project, optionally seeding the user-global
/// `ragent.json` with `global_config`.
fn enter_project(
    global_config: Option<&str>,
) -> (
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

    if let Some(config) = global_config {
        let global_dir = temp.path().join(".config").join("ragent");
        std::fs::create_dir_all(&global_dir).expect("create global config dir");
        std::fs::write(global_dir.join("ragent.json"), config).expect("write global config");
    }
    ragent_config::Config::invalidate_load_cache();
    (guard, env_guard, temp)
}

fn app() -> App {
    let mut app = support::make_app();
    app.session_id = Some("mcp-remove".to_string());
    app
}

fn last_message(app: &App) -> String {
    app.messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default()
}

fn read_json(path: &std::path::Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("read file")).expect("parse json")
}

#[tokio::test]
async fn mcp_remove_deletes_the_configured_entry_and_keeps_everything_else() {
    let (_guard, _env, temp) = enter_project(Some(
        r#"{
            "username": "alice",
            "mcp": {
                "echo": { "type": "stdio", "command": "echo" },
                "other": { "type": "stdio", "command": "cat" }
            }
        }"#,
    ));
    let global = temp.path().join(".config/ragent/ragent.json");
    let mut app = app();

    app.execute_slash_command("/mcp remove echo").await;

    let report = last_message(&app);
    assert!(report.contains("[ok]"), "report: {report}");
    assert!(report.contains("echo"), "report: {report}");

    let config = read_json(&global);
    assert!(
        config["mcp"].get("echo").is_none(),
        "the removed entry must be gone: {config}"
    );
    assert_eq!(
        config["mcp"]["other"]["command"], "cat",
        "the sibling entry must survive: {config}"
    );
    assert_eq!(config["username"], "alice", "other keys survive: {config}");
    assert!(!app.mcp_servers.iter().any(|s| s.id == "echo"));
    assert_eq!(app.status, "mcp: remove");
}

#[tokio::test]
async fn mcp_remove_drops_an_emptied_mcp_section() {
    let (_guard, _env, temp) = enter_project(Some(
        r#"{ "mcp": { "echo": { "type": "stdio", "command": "echo" } } }"#,
    ));
    let global = temp.path().join(".config/ragent/ragent.json");
    let mut app = app();

    app.execute_slash_command("/mcp remove echo").await;

    let config = read_json(&global);
    assert!(
        config.get("mcp").is_none(),
        "an emptied `mcp` section is removed entirely: {config}"
    );
}

#[tokio::test]
async fn mcp_remove_clears_the_enable_ledger_entry() {
    let (_guard, _env, temp) = enter_project(Some(
        r#"{ "mcp": { "echo": { "type": "stdio", "command": "echo" } } }"#,
    ));
    let mut ledger = ragent_agent::mcp::McpEnableLedger::load();
    ledger.set_enabled("echo", false);
    ledger.save().expect("save ledger");
    let mut app = app();

    app.execute_slash_command("/mcp remove echo").await;

    assert!(
        ragent_agent::mcp::McpEnableLedger::load().is_enabled("echo"),
        "a re-added server must start enabled again"
    );
    assert!(
        !temp.path().join(".config/ragent/ragent.json").exists()
            || read_json(&temp.path().join(".config/ragent/ragent.json"))
                .get("mcp")
                .is_none(),
        "the config no longer declares the server"
    );
}

#[tokio::test]
async fn mcp_remove_reports_an_unknown_server() {
    let (_guard, _env, _temp) = enter_project(Some(r#"{ "mcp": {} }"#));
    let mut app = app();

    app.execute_slash_command("/mcp remove ghost").await;

    let report = last_message(&app);
    assert!(report.contains("[err]"), "report: {report}");
    assert!(report.contains("ghost"), "report: {report}");
}

#[tokio::test]
async fn mcp_remove_without_an_id_shows_usage() {
    let (_guard, _env, _temp) = enter_project(None);
    let mut app = app();

    app.execute_slash_command("/mcp remove").await;

    assert_eq!(app.status, "Usage: /mcp remove <id>");
}

#[tokio::test]
async fn mcp_remove_help_describes_the_command() {
    let (_guard, _env, _temp) = enter_project(None);
    let mut app = app();

    app.execute_slash_command("/mcp remove help").await;

    let report = last_message(&app);
    assert!(
        report.contains("From: /mcp remove help"),
        "report: {report}"
    );
    // The help must state that a configured entry is removable and that a
    // bridged one is refused with a pointer at the owner's own command.
    assert!(
        report.contains("Bridged servers are refused"),
        "report: {report}"
    );
    assert!(report.contains("/plugins remove"), "report: {report}");
    assert!(report.contains("/connectors remove"), "report: {report}");
}

#[tokio::test]
async fn mcp_list_reports_the_defining_file_for_each_server() {
    let (_guard, _env, temp) = enter_project(Some(
        r#"{ "mcp": { "echo": { "type": "stdio", "command": "echo" } } }"#,
    ));
    let mut app = app();

    app.execute_slash_command("/mcp").await;

    let report = last_message(&app);
    assert!(report.contains("Defined in"), "report: {report}");
    assert!(
        report.contains(
            &temp
                .path()
                .join(".config/ragent/ragent.json")
                .display()
                .to_string()
        ),
        "the global config path must be listed: {report}"
    );
}

/// The `Defined in` block keeps the main table's six columns intact, so the
/// rendered `/mcp` table does not wrap its header cells.
#[tokio::test]
async fn mcp_list_keeps_the_six_column_table_and_a_separate_defined_in_block() {
    let (_guard, _env, temp) = enter_project(Some(
        r#"{ "mcp": { "echo": { "type": "stdio", "command": "echo", "args": ["--flag"] } } }"#,
    ));
    let mut app = app();

    app.execute_slash_command("/mcp").await;

    let report = last_message(&app);
    // The rendered header row must still be the six-column one; a seventh
    // column pushed it past the message width and wrapped every header cell.
    let header = report
        .lines()
        .find(|line| line.contains("Server") && line.contains("Transport"))
        .unwrap_or("<no header row>");
    assert_eq!(
        header.matches('|').count(),
        7,
        "the main header is six columns: {header}"
    );
    assert!(
        !header.contains("Defined in"),
        "`Defined in` must not become a seventh column: {header}"
    );
    let global = temp
        .path()
        .join(".config/ragent/ragent.json")
        .display()
        .to_string();
    assert!(
        report.contains("Defined in:"),
        "the separate block must be present: {report}"
    );
    assert!(report.contains(&global), "report: {report}");
}

/// A plugin-bridged `<plugin-id>.<server>` is **refused**: the definition
/// belongs to the plugin, so `/mcp remove` names `/plugins remove <id>` instead
/// of pruning the plugin's own `mcpServers` object.
#[tokio::test]
async fn mcp_remove_refuses_a_plugin_bridged_server() {
    let (_guard, _env, temp) = enter_project(None);
    let store = temp.path().join(".config/ragent/plugins");
    let plugin = store.join("mongodb");
    std::fs::create_dir_all(plugin.join(".claude-plugin")).expect("manifest dir");
    std::fs::write(
        plugin.join(".claude-plugin/plugin.json"),
        br#"{"name":"mongodb","version":"1.0.0","mcpServers":"./mcp.json"}"#,
    )
    .expect("write manifest");
    let external = plugin.join("mcp.json");
    std::fs::write(
        &external,
        br#"{"mcpServers":{"db":{"command":"npx"},"keep":{"command":"keep-mcp"}}}"#,
    )
    .expect("write mcp.json");
    std::fs::write(
        store.join("_state.json"),
        br#"{"plugins":{"mongodb":{"enabled":true}}}"#,
    )
    .expect("write state ledger");
    let mut app = app();

    app.execute_slash_command("/mcp remove mongodb.db").await;

    let report = last_message(&app);
    assert!(
        report.contains("[err]"),
        "the removal must be refused: {report}"
    );
    assert!(
        report.contains("/plugins remove mongodb"),
        "the refusal must name the plugin's own removal command: {report}"
    );
    assert!(
        report.contains("enable") || report.contains("/mcp disconnect"),
        "the refusal must say enable/disable still works: {report}"
    );
    let config = read_json(&external);
    assert!(
        config["mcpServers"].get("db").is_some(),
        "the plugin's declaration must be untouched: {config}"
    );
    assert_eq!(
        config["mcpServers"]["keep"]["command"], "keep-mcp",
        "the sibling server must survive: {config}"
    );
}

/// The same refusal applies to a Codex-dialect plugin that declares its servers
/// inline under the `mcp_servers` spelling.
#[tokio::test]
async fn mcp_remove_refuses_an_inline_codex_plugin_server() {
    let (_guard, _env, temp) = enter_project(None);
    let store = temp.path().join(".config/ragent/plugins");
    let plugin = store.join("inlineplugin");
    std::fs::create_dir_all(plugin.join(".claude-plugin")).expect("manifest dir");
    let manifest = plugin.join(".claude-plugin/plugin.json");
    std::fs::write(
        &manifest,
        br#"{"name":"inlineplugin","version":"1.0.0","mcp_servers":{"db":{"command":"npx"},"keep":{"command":"keep-mcp"}}}"#,
    )
    .expect("write manifest");
    std::fs::write(
        store.join("_state.json"),
        br#"{"plugins":{"inlineplugin":{"enabled":true}}}"#,
    )
    .expect("write state ledger");
    let mut app = app();

    app.execute_slash_command("/mcp remove inlineplugin.db")
        .await;

    let report = last_message(&app);
    assert!(
        report.contains("[err]"),
        "the removal must be refused: {report}"
    );
    assert!(
        report.contains("/plugins remove inlineplugin"),
        "report: {report}"
    );
    let config = read_json(&manifest);
    assert!(
        config["mcp_servers"].get("db").is_some(),
        "the inline declaration must be untouched: {config}"
    );
    assert_eq!(
        config["mcp_servers"]["keep"]["command"], "keep-mcp",
        "the sibling server must survive: {config}"
    );
}

/// `/mcp` names the plugin's external file, not the manifest, as the defining
/// file of a bridged server.
#[tokio::test]
async fn mcp_list_reports_the_plugin_external_file() {
    let (_guard, _env, temp) = enter_project(None);
    let store = temp.path().join(".config/ragent/plugins");
    let plugin = store.join("mongodb");
    std::fs::create_dir_all(plugin.join(".claude-plugin")).expect("manifest dir");
    std::fs::write(
        plugin.join(".claude-plugin/plugin.json"),
        br#"{"name":"mongodb","version":"1.0.0","mcpServers":"./mcp.json"}"#,
    )
    .expect("write manifest");
    std::fs::write(
        plugin.join("mcp.json"),
        br#"{"mcpServers":{"db":{"command":"npx"}}}"#,
    )
    .expect("write mcp.json");
    std::fs::write(
        store.join("_state.json"),
        br#"{"plugins":{"mongodb":{"enabled":true}}}"#,
    )
    .expect("write state ledger");
    let mut app = app();

    app.execute_slash_command("/mcp").await;

    let report = last_message(&app);
    assert!(report.contains("mongodb.db"), "report: {report}");
    assert!(
        report.contains(&plugin.join("mcp.json").display().to_string()),
        "the external mcp.json must be the reported source: {report}"
    );
}

/// `/mcp config` names each discovered server's defining file, so a user can
/// see which file `/mcp remove` would prune before running it.
#[tokio::test]
async fn mcp_config_lists_the_defining_file_column() {
    let locations = vec![ragent_agent::mcp::McpDiscoveryLocation {
        label: "ragent-native".to_string(),
        path: Some(std::path::PathBuf::from(
            "/home/user/.config/ragent/servers",
        )),
        exists: true,
        servers: vec![ragent_agent::mcp::discovery::DiscoveredMcpServer {
            id: "registry-one".to_string(),
            name: "Registry One".to_string(),
            executable: std::path::PathBuf::from("/usr/bin/mcp-registry-one"),
            args: vec!["--root".to_string()],
            env: std::collections::HashMap::new(),
            source: ragent_agent::mcp::discovery::McpDiscoverySource::McpRegistry {
                registry_dir: std::path::PathBuf::from("/home/user/.config/ragent/servers"),
            },
            definition_file: Some(std::path::PathBuf::from(
                "/home/user/.config/ragent/servers/registry-one/server.json",
            )),
        }],
    }];

    let report = ragent_tui::app::render_mcp_config_for_tests(&locations);

    assert!(report.contains("From: /mcp config"), "report: {report}");
    assert!(
        report.contains("Defined in"),
        "the discovered-server table must name the defining file: {report}"
    );
    assert!(
        report.contains("servers/registry-one/server.json"),
        "the defining file must be listed: {report}"
    );
}

/// The discover panel paints the defining file beside each row, and widens past
/// its former 90 columns so an absolute path is readable.
#[tokio::test]
async fn mcp_discover_panel_paints_the_definition_file() {
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    let (_guard, _env, _temp) = enter_project(None);
    let mut app = app();
    app.mcp_discover = Some(ragent_tui::app::McpDiscoverState {
        servers: vec![ragent_agent::mcp::discovery::DiscoveredMcpServer {
            id: "registry-one".to_string(),
            name: "Registry One".to_string(),
            executable: std::path::PathBuf::from("/usr/bin/mcp-registry-one"),
            args: Vec::new(),
            env: std::collections::HashMap::new(),
            source: ragent_agent::mcp::discovery::McpDiscoverySource::McpRegistry {
                registry_dir: std::path::PathBuf::from("/opt/registry"),
            },
            definition_file: Some(std::path::PathBuf::from(
                "/home/user/.config/ragent/servers/registry-one/server.json",
            )),
        }],
        enabled: vec![true],
        selected: 0,
        scroll_offset: 0,
        feedback: None,
    });
    // Keep the panel open (Esc would dismiss it) but leave the slash menu quiet.
    app.slash_menu = None;

    let backend = TestBackend::new(170, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, &mut app))
        .expect("render app");
    let buffer = terminal.backend().buffer();
    let painted: String = (0..40)
        .map(|y| {
            (0..170)
                .map(|x| {
                    buffer
                        .cell((x, y))
                        .map(|c| c.symbol().to_string())
                        .unwrap_or_default()
                })
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(painted.contains("Defined in"), "painted:\n{painted}");
    // The path is longer than the old 90-column panel could show, so seeing its
    // tail proves the dialog widened rather than clipping it away.
    assert!(
        painted.contains("servers/registry-one/server.json"),
        "the defining file must be painted:\n{painted}"
    );
}
