//! T-014 (spec `plugins`, FR-006, FR-014): `/plugins` registration, the
//! autocomplete menu, and the `help` / bare / unknown-subcommand usage block.
//!
//! The usage-block *content* is asserted crate-side on the pure
//! `ragent_plugins::render_help` (see `ragent-plugins` `test_help`); here we
//! assert the TUI wiring: the command is registered, autocompletes, and the
//! three invocation forms reach the help renderer and touch no files.

use std::sync::{Mutex, OnceLock};

#[path = "support/mod.rs"]
mod support;

/// RAII cwd guard so a temp working directory is restored on drop.
struct CwdGuard(std::path::PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

/// Serialise cwd-mutating tests in this binary.
fn cwd_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

/// Enter a fresh temp working directory with an empty `.ragent/`.
fn enter_temp_dir() -> (CwdGuard, tempfile::TempDir) {
    let original = std::env::current_dir().expect("cwd");
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    std::fs::create_dir_all(temp.path().join(".ragent")).expect("create .ragent");
    (CwdGuard(original), temp)
}

/// The rendered text of the most recent assistant message.
fn last_text(app: &ragent_tui::App) -> String {
    app.messages
        .last()
        .expect("an assistant message must have been appended")
        .text_content()
}

/// The rendered text of the most recent assistant message with every whitespace
/// run folded to a single space, so a long endpoint URL that the message window
/// markdown-wrapped onto its own line can still be asserted as one cell.
fn flat_text(app: &ragent_tui::App) -> String {
    last_text(app)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[test]
fn plugins_registered_in_slash_commands() {
    let def = ragent_tui::app::SLASH_COMMANDS
        .iter()
        .find(|cmd| cmd.trigger == "plugins")
        .expect("/plugins must be registered in SLASH_COMMANDS (FR-006)");
    for sub in [
        "list", "add", "remove", "enable", "disable", "test", "stores", "help",
    ] {
        assert!(
            def.description.contains(sub),
            "the /plugins description must advertise `{sub}`: {}",
            def.description
        );
    }
}

#[test]
fn plugins_suggestions_list_all_subcommands() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, _temp) = enter_temp_dir();
    let mut app = support::make_app();

    app.input = "/plugins".to_string();
    app.update_slash_menu();
    let menu = app
        .slash_menu
        .as_ref()
        .expect("/plugins prefix must open the slash menu");
    let entry = menu
        .matches
        .iter()
        .find(|m| m.trigger == "plugins")
        .expect("menu must contain the plugins entry");
    for sub in [
        "list", "add", "remove", "enable", "disable", "test", "stores", "help",
    ] {
        assert!(
            entry.suggestions.iter().any(|s| s == sub),
            "autocomplete must offer `{sub}`: {:?}",
            entry.suggestions
        );
    }
    assert!(
        entry.suggestions.iter().any(|s| s == "--verbose"),
        "autocomplete must offer `--verbose` for `list`: {:?}",
        entry.suggestions
    );
    assert!(
        entry.suggestions.iter().any(|s| s == "--force"),
        "autocomplete must offer `--force` for `add`: {:?}",
        entry.suggestions
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn help_bare_and_unknown_all_render_the_usage_block() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, _temp) = enter_temp_dir();
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    for invocation in ["/plugins help", "/plugins", "/plugins bogus"] {
        app.execute_slash_command(invocation).await;
        let text = last_text(&app);
        // FR-014: all three print the usage block with a `/plugins` attribution.
        assert!(
            text.contains("From: /plugins"),
            "`{invocation}` must attribute its output: {text}"
        );
        // The rendered window may re-wrap the markdown table, so assert on the
        // stable section heading and a couple of unambiguous tokens rather than
        // the exact table cells (exact content is pinned crate-side).
        assert!(
            text.contains("command reference"),
            "`{invocation}` must render the usage block heading: {text}"
        );
        assert!(
            text.contains("Sources accepted by `/plugins add`"),
            "`{invocation}` must document the source forms: {text}"
        );
        for needle in ["https://", "--force", "--verbose"] {
            assert!(
                text.contains(needle),
                "`{invocation}` must mention `{needle}`: {text}"
            );
        }
        // ASCII only (project convention: no non-ASCII in output).
        assert!(
            text.is_ascii(),
            "`{invocation}` output must be ASCII only: {text}"
        );
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn stores_reports_both_endpoints_as_default_without_a_stores_block() {
    // FR-031/FR-038: with no `plugins.stores` block, `/plugins stores` lists
    // both stores, tags each `default`, and names its compiled https endpoint.
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/plugins stores").await;
    let raw = last_text(&app);
    let text = flat_text(&app);

    assert!(raw.contains("From: /plugins stores"), "{raw}");
    // The message window re-renders markdown bullets as `*` and wraps a long
    // endpoint onto its own line, so assert on the whitespace-folded
    // `token: [tag] endpoint` cells rather than the bullet glyph.
    for token in ["codex", "claude"] {
        assert!(
            text.contains(&format!("{token}: [default] https://")),
            "`{token}` must be tagged default with an https endpoint: {text}"
        );
    }
    assert!(raw.is_ascii(), "the report must be ASCII only: {raw}");
    assert!(
        !temp.path().join(".ragent").join("plugins").exists(),
        "reporting the stores must create no plugin store"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn stores_tags_a_configured_endpoint_as_config() {
    // FR-031: a non-empty `plugins.stores.<name>.url` override is tagged
    // `config` and names its URL; the other store keeps its default.
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();
    std::fs::write(
        temp.path().join(".ragent").join("ragent.json"),
        r#"{ "plugins": { "stores": { "codex": { "url": "https://cfg.example/codex.json" } } } }"#,
    )
    .expect("config writable");

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/plugins stores").await;
    let text = flat_text(&app);

    assert!(
        text.contains("codex: [config] https://cfg.example/codex.json"),
        "an override must be tagged config with its URL: {text}"
    );
    assert!(
        text.contains("claude: [default] https://"),
        "the unconfigured store keeps its default: {text}"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn help_creates_no_files() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    for invocation in ["/plugins help", "/plugins", "/plugins bogus"] {
        app.execute_slash_command(invocation).await;
    }

    // FR-014: help creates or modifies no files. In particular it must not
    // create the plugin store (nor load any plugin).
    assert!(
        !temp.path().join(".ragent").join("plugins").exists(),
        "help must not create the plugin store directory"
    );
}

/// Write a minimal Codex-dialect plugin into a source directory.
fn write_codex_plugin(dir: &std::path::Path, id: &str) {
    std::fs::create_dir_all(dir).expect("plugin dir");
    std::fs::write(
        dir.join("codex-plugin.json"),
        format!(r#"{{ "id": "{id}", "name": "{id}", "version": "1.0.0", "entry": "index.js" }}"#),
    )
    .expect("manifest");
    std::fs::write(
        dir.join("index.js"),
        "ragent.register_tool({ name: \"ping\", description: \"Ping\", parameters: { type: \"object\" }, handler: function(){ return \"pong\"; } });",
    )
    .expect("entry");
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn list_reports_a_discovered_plugin_as_disabled() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();
    write_codex_plugin(
        &temp
            .path()
            .join(".ragent")
            .join("plugins")
            .join("codex-weather"),
        "codex-weather",
    );
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;
    let listed = last_text(&app);
    assert!(
        listed.contains("codex-weather"),
        "list must show the discovered plugin: {listed}"
    );
    assert!(
        listed.contains("disabled"),
        "a freshly added plugin is disabled (FR-016): {listed}"
    );
}

/// Write a minimal Claude-dialect plugin that declares one MCP server.
fn write_claude_mcp_plugin(dir: &std::path::Path, id: &str) {
    std::fs::create_dir_all(dir.join(".claude-plugin")).expect("plugin dir");
    std::fs::write(
        dir.join(".claude-plugin/plugin.json"),
        format!(r#"{{ "name": "{id}", "version": "1.0.0", "mcpServers": "./mcp.json" }}"#),
    )
    .expect("manifest");
    std::fs::write(
        dir.join("mcp.json"),
        format!(
            r#"{{ "mcpServers": {{ "{id}": {{ "command": "npx", "args": ["-y", "server"] }} }} }}"#
        ),
    )
    .expect("mcp config");
}

/// FR-030: `/plugins list` must show how many MCP servers a plugin installs and
/// how many tools those servers provide, in the table.
#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn list_shows_mcp_server_and_tool_counts() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();
    write_claude_mcp_plugin(
        &temp.path().join(".ragent").join("plugins").join("mongodb"),
        "mongodb",
    );
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;
    let listed = last_text(&app);
    assert!(
        listed.contains("MCP Tools"),
        "the list table must carry an MCP Tools column: {listed}"
    );
    assert!(
        listed.contains("MCP server")
            || listed
                .lines()
                .any(|l| l.contains("mongodb") && l.trim_end().ends_with("| 1   | ?         |")),
        "the mongodb row must report one MCP server: {listed}"
    );
    assert!(
        listed.contains("mcp [mongodb.mongodb"),
        "the contributions block must name the bridged MCP server: {listed}"
    );
}

/// FR-030: the table's `MCP Tools` cell is the LIVE tool count from the shared
/// MCP client once the server has connected — `?` (unknown, not zero) is only
/// for a server whose tool list the surface cannot see.
#[tokio::test(flavor = "multi_thread")]
#[allow(clippy::await_holding_lock)]
async fn list_shows_live_mcp_tool_count_after_connect() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();
    write_claude_mcp_plugin(
        &temp.path().join(".ragent").join("plugins").join("mongodb"),
        "mongodb",
    );
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    // `App::new` captured the pre-test cwd; point it at the temp project so the
    // plugin scan sees the staged store.
    app.cwd_path = temp.path().to_path_buf();

    // Stand in for the completed startup connect loop: the bridged server is
    // connected and advertises two tools.
    let mut client = ragent_agent::mcp::McpClient::new();
    let tools = ["find", "count"]
        .iter()
        .map(|name| ragent_agent::mcp::McpToolDef {
            name: (*name).to_string(),
            description: "tool".to_string(),
            parameters: serde_json::json!({ "type": "object" }),
        })
        .collect();
    client.register_connected_for_tests("mongodb.mongodb", tools);
    app.session_processor
        .mcp_client
        .set(std::sync::Arc::new(tokio::sync::RwLock::new(client)))
        .map_err(|_| ())
        .expect("mcp client set once");
    let processor = std::sync::Arc::clone(&app.session_processor);
    app.adopt_mcp_client_state(&processor).await;

    app.execute_slash_command("/plugins list").await;
    let listed = last_text(&app);
    assert!(
        listed
            .lines()
            .any(|l| l.contains("mongodb") && l.trim_end().ends_with("| 1   | 2         |")),
        "the mongodb row must report the live tool count, not '?': {listed}"
    );
    assert!(
        listed.contains("mcp [mongodb.mongodb (2 tools)]"),
        "the contributions block must report the live tool count: {listed}"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn add_from_a_local_directory_installs_and_reports_enabled() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();

    // Stage a source directory *outside* the store.
    let src = temp.path().join("src-plugin");
    write_codex_plugin(&src, "codex-weather");

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command(&format!("/plugins add {}", src.display()))
        .await;
    let added = last_text(&app);
    assert!(added.contains("[ok]"), "add must succeed: {added}");
    assert!(
        added.contains("codex-weather"),
        "add must report the plugin id: {added}"
    );

    // Installed into the project store and recorded enabled (FR-007).
    assert!(
        temp.path()
            .join(".ragent")
            .join("plugins")
            .join("codex-weather")
            .join("codex-plugin.json")
            .exists(),
        "the plugin must be installed into the store"
    );
    app.execute_slash_command("/plugins list").await;
    let listed = last_text(&app);
    assert!(
        listed.contains("enabled"),
        "an added plugin is enabled until disabled: {listed}"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn test_harness_reports_per_step_results_without_touching_the_session() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();
    write_codex_plugin(
        &temp
            .path()
            .join(".ragent")
            .join("plugins")
            .join("codex-weather"),
        "codex-weather",
    );
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins test codex-weather")
        .await;
    let report = last_text(&app);
    assert!(
        report.contains("[ ok ]"),
        "the harness must report ok steps: {report}"
    );
    assert!(
        report.contains("live session untouched"),
        "the harness must confirm the live session is untouched: {report}"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn enable_loads_the_plugin_and_disable_unloads_it() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();
    write_codex_plugin(
        &temp
            .path()
            .join(".ragent")
            .join("plugins")
            .join("codex-weather"),
        "codex-weather",
    );
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    // FR-011: enable loads it and states the declared permissions.
    app.execute_slash_command("/plugins enable codex-weather")
        .await;
    let enabled = last_text(&app);
    assert!(enabled.contains("[ok]"), "enable must succeed: {enabled}");
    assert!(
        enabled.contains("Declared permissions"),
        "enable must state the declared permissions: {enabled}"
    );

    // FR-012: disable unloads it and confirms deregistration.
    app.execute_slash_command("/plugins disable codex-weather")
        .await;
    let disabled = last_text(&app);
    assert!(
        disabled.contains("[ok]") && disabled.contains("deregistered"),
        "disable must confirm deregistration: {disabled}"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn master_switch_disables_the_subsystem() {
    // Acceptance criterion 8: `plugins.enabled: false` makes `/plugins list`
    // report the disabled subsystem and discover nothing (FR-016).
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();
    write_codex_plugin(
        &temp
            .path()
            .join(".ragent")
            .join("plugins")
            .join("codex-weather"),
        "codex-weather",
    );
    std::fs::write(
        temp.path().join(".ragent").join("ragent.json"),
        r#"{ "plugins": { "enabled": false } }"#,
    )
    .expect("config writable");

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/plugins list").await;
    let listed = last_text(&app);
    assert!(
        listed.contains("[err]") && listed.contains("disabled"),
        "list must report the disabled subsystem: {listed}"
    );
    assert!(
        !listed.contains("codex-weather"),
        "no plugin rows while the subsystem is disabled: {listed}"
    );
}
