//! T-018 (spec `osinfo`, FR-013): `/osinfo` slash-command registration tests.
//!
//! Verifies the `SLASH_COMMANDS` entry, the autocomplete suggestion map, the
//! parameter hint, and the `/help` command index. Dispatch behaviour
//! (`show`/`help`/unknown) is covered by the T-024 tests once the dispatch arm
//! lands.

use support::make_app;

mod support;

#[test]
fn test_osinfo_registered_in_slash_commands() {
    let found = ragent_tui::app::SLASH_COMMANDS
        .iter()
        .find(|cmd| cmd.trigger == "osinfo");
    let def = found.expect("osinfo must be registered in SLASH_COMMANDS (FR-013)");
    assert!(
        def.description.contains("show") && def.description.contains("help"),
        "description must advertise the show and help subcommands: {}",
        def.description
    );
    assert!(
        def.description.contains("--no-probe"),
        "description must advertise the --no-probe flag: {}",
        def.description
    );
}

#[test]
fn test_osinfo_suggestions_offer_show_and_help() {
    let mut app = make_app();
    app.input = "/osinfo".to_string();
    app.update_slash_menu();
    let menu = app
        .slash_menu
        .as_ref()
        .expect("/osinfo prefix must open the slash menu");
    let entry = menu
        .matches
        .iter()
        .find(|m| m.trigger == "osinfo")
        .expect("menu must contain the osinfo entry");
    assert_eq!(
        entry.suggestions,
        vec![
            "show".to_string(),
            "--no-probe".to_string(),
            "help".to_string()
        ],
        "osinfo autocomplete suggestions must be [show, --no-probe, help]"
    );
    assert_eq!(
        entry.parameter_hint.as_deref(),
        Some("[show [--no-probe]|help]"),
        "osinfo parameter hint must list the show/--no-probe/help forms"
    );
}

#[test]
fn test_osinfo_slash_menu_matches_prefix() {
    // Typing the bare `/os` prefix must surface the osinfo trigger (prefix
    // match path of `update_slash_menu`).
    let mut app = make_app();
    app.input = "/os".to_string();
    app.update_slash_menu();
    let menu = app
        .slash_menu
        .as_ref()
        .expect("/os prefix must open the slash menu");
    let triggers: Vec<&str> = menu.matches.iter().map(|e| e.trigger.as_str()).collect();
    assert!(
        triggers.contains(&"osinfo"),
        "menu matches must include osinfo, got: {triggers:?}"
    );
}

#[tokio::test]
async fn test_help_index_lists_osinfo() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());

    app.execute_slash_command("/help").await;

    let text = app
        .messages
        .last()
        .expect("help should create a message")
        .text_content();
    assert!(
        text.contains("/osinfo"),
        "/help should list the /osinfo command, got: {text}"
    );
}
