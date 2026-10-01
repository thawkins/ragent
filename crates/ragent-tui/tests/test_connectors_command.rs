//! T-010 (spec `connectors`, FR-004, FR-006, FR-023): `/connectors` registration,
//! the autocomplete menu, and the `help` / bare / unknown-subcommand usage block.
//!
//! The usage-block *content* and the pure parse-and-run glue are asserted
//! crate-side (`crates/ragent-connectors/tests/test_connector_commands.rs`); here
//! we assert the TUI wiring: the command is registered, autocompletes, and the
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
fn connectors_registered_in_slash_commands() {
    let def = ragent_tui::app::SLASH_COMMANDS
        .iter()
        .find(|cmd| cmd.trigger == "connectors")
        .expect("/connectors must be registered in SLASH_COMMANDS (FR-004)");
    for sub in [
        "list",
        "search",
        "claude",
        "add",
        "remove",
        "enable",
        "disable",
        "connect",
        "disconnect",
        "auth",
        "test",
        "stores",
        "help",
    ] {
        assert!(
            def.description.contains(sub),
            "the /connectors description must advertise `{sub}`: {}",
            def.description
        );
    }
}

#[test]
fn connectors_suggestions_list_all_subcommands() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, _temp) = enter_temp_dir();
    let mut app = support::make_app();

    app.input = "/connectors".to_string();
    app.update_slash_menu();
    let menu = app
        .slash_menu
        .as_ref()
        .expect("/connectors prefix must open the slash menu");
    let entry = menu
        .matches
        .iter()
        .find(|m| m.trigger == "connectors")
        .expect("menu must contain the connectors entry");
    for sub in [
        "list",
        "search",
        "claude",
        "add",
        "remove",
        "enable",
        "disable",
        "connect",
        "disconnect",
        "auth",
        "test",
        "stores",
        "help",
    ] {
        assert!(
            entry.suggestions.iter().any(|s| s == sub),
            "autocomplete must offer `{sub}`: {:?}",
            entry.suggestions
        );
    }
    for flag in ["--verbose", "--category", "--force", "--check", "--refresh"] {
        assert!(
            entry.suggestions.iter().any(|s| s == flag),
            "autocomplete must offer `{flag}`: {:?}",
            entry.suggestions
        );
    }
    // The menu is seeded from the shared token list (FR-004), so it cannot
    // drift from the usage block: assert the exact seed.
    assert_eq!(
        entry.suggestions,
        ragent_connectors::autocomplete_tokens(),
        "autocomplete must be seeded from the shared connectors token list"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn help_bare_and_unknown_all_render_the_usage_block() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, _temp) = enter_temp_dir();
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    for invocation in ["/connectors help", "/connectors", "/connectors bogus"] {
        app.execute_slash_command(invocation).await;
        let text = last_text(&app);
        // FR-006/FR-017: all three print the usage block with a `/connectors`
        // attribution.
        assert!(
            text.contains("From: /connectors"),
            "`{invocation}` must attribute its output: {text}"
        );
        assert!(
            text.contains("command reference"),
            "`{invocation}` must render the usage block heading: {text}"
        );
        assert!(
            text.contains("Sources accepted by `/connectors add`"),
            "`{invocation}` must document the source forms: {text}"
        );
        for needle in ["https://", "--force", "--category", "--check"] {
            assert!(
                text.contains(needle),
                "`{invocation}` must mention `{needle}`: {text}"
            );
        }
        assert!(
            text.is_ascii(),
            "`{invocation}` output must be ASCII only: {text}"
        );
    }
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn help_creates_no_files() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    for invocation in ["/connectors help", "/connectors", "/connectors bogus"] {
        app.execute_slash_command(invocation).await;
    }

    // FR-017: help creates or modifies no files, and in particular must not
    // create the connector store.
    assert!(
        !temp.path().join(".ragent").join("connectors").exists(),
        "help must not create the connector store directory"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn stores_report_is_attributed_ascii_and_offline() {
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/connectors stores").await;

    let raw = last_text(&app);
    let text = flat_text(&app);
    assert!(raw.contains("From: /connectors stores"), "{raw}");
    assert!(
        text.contains("claude: [default] https://"),
        "the compiled default endpoint must be tagged default: {text}"
    );
    assert!(raw.is_ascii(), "the report must be ASCII only: {raw}");
    assert!(
        !temp.path().join(".ragent").join("connectors").exists(),
        "reporting the stores must create no connector store"
    );
}

#[tokio::test]
#[allow(clippy::await_holding_lock)]
async fn master_switch_disables_the_subsystem() {
    // FR-021: `connectors.enabled: false` makes `/connectors list` report the
    // disabled subsystem and discover nothing; `help` remains available.
    let _lock = cwd_test_lock().lock().unwrap_or_else(|e| e.into_inner());
    let (_guard, temp) = enter_temp_dir();
    std::fs::write(
        temp.path().join(".ragent").join("ragent.json"),
        r#"{ "connectors": { "enabled": false } }"#,
    )
    .expect("config writable");

    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/connectors list").await;
    let listed = last_text(&app);
    assert!(
        listed.contains("[err]") && listed.contains("disabled"),
        "list must report the disabled subsystem: {listed}"
    );
    assert!(
        !temp.path().join(".ragent").join("connectors").exists(),
        "a disabled subsystem must discover nothing"
    );

    app.execute_slash_command("/connectors help").await;
    let help = last_text(&app);
    assert!(
        help.contains("command reference"),
        "help must still render while disabled: {help}"
    );
}
