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

#[test]
fn plugins_registered_in_slash_commands() {
    let def = ragent_tui::app::SLASH_COMMANDS
        .iter()
        .find(|cmd| cmd.trigger == "plugins")
        .expect("/plugins must be registered in SLASH_COMMANDS (FR-006)");
    for sub in ["list", "add", "remove", "enable", "disable", "test", "help"] {
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
    for sub in ["list", "add", "remove", "enable", "disable", "test", "help"] {
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

#[test]
fn help_bare_and_unknown_all_render_the_usage_block() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, _temp) = enter_temp_dir();
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    for invocation in ["/plugins help", "/plugins", "/plugins bogus"] {
        app.execute_slash_command(invocation);
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

#[test]
fn help_creates_no_files() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    for invocation in ["/plugins help", "/plugins", "/plugins bogus"] {
        app.execute_slash_command(invocation);
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

#[test]
fn list_reports_a_discovered_plugin_as_disabled() {
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

    app.execute_slash_command("/plugins list");
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

#[test]
fn add_from_a_local_directory_installs_and_stays_disabled() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();

    // Stage a source directory *outside* the store.
    let src = temp.path().join("src-plugin");
    write_codex_plugin(&src, "codex-weather");

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command(&format!("/plugins add {}", src.display()));
    let added = last_text(&app);
    assert!(added.contains("[ok]"), "add must succeed: {added}");
    assert!(
        added.contains("codex-weather"),
        "add must report the plugin id: {added}"
    );

    // Installed into the project store and still disabled (FR-007).
    assert!(
        temp.path()
            .join(".ragent")
            .join("plugins")
            .join("codex-weather")
            .join("codex-plugin.json")
            .exists(),
        "the plugin must be installed into the store"
    );
    app.execute_slash_command("/plugins list");
    let listed = last_text(&app);
    assert!(
        listed.contains("disabled"),
        "an added plugin stays disabled until enabled: {listed}"
    );
}

#[test]
fn test_harness_reports_per_step_results_without_touching_the_session() {
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

    app.execute_slash_command("/plugins test codex-weather");
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

#[test]
fn enable_loads_the_plugin_and_disable_unloads_it() {
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
    app.execute_slash_command("/plugins enable codex-weather");
    let enabled = last_text(&app);
    assert!(enabled.contains("[ok]"), "enable must succeed: {enabled}");
    assert!(
        enabled.contains("Declared permissions"),
        "enable must state the declared permissions: {enabled}"
    );

    // FR-012: disable unloads it and confirms deregistration.
    app.execute_slash_command("/plugins disable codex-weather");
    let disabled = last_text(&app);
    assert!(
        disabled.contains("[ok]") && disabled.contains("deregistered"),
        "disable must confirm deregistration: {disabled}"
    );
}

#[test]
fn master_switch_disables_the_subsystem() {
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

    app.execute_slash_command("/plugins list");
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
