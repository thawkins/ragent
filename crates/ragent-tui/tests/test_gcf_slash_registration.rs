//! T-008 (spec `gcf`, FR-003): `/gcf` registration and autocomplete tests.
//!
//! Verifies the registry entry, the autocomplete suggestion map, and the
//! parameter hint. Dispatch behaviour (on/off/show/help) is covered by
//! T-010 tests once the dispatch arm lands.

use support::make_app;

mod support;

#[test]
fn test_gcf_registered_in_slash_commands() {
    let found = ragent_tui::app::SLASH_COMMANDS
        .iter()
        .find(|cmd| cmd.trigger == "gcf");
    let def = found.expect("gcf must be registered in SLASH_COMMANDS (FR-003)");
    assert!(
        def.description.contains("on|off|show|help"),
        "description must advertise all subcommands: {}",
        def.description
    );
}

#[test]
fn test_gcf_suggestions_offer_on_off_show_help() {
    let mut app = make_app();
    app.input = "/gcf".to_string();
    app.update_slash_menu();
    let menu = app
        .slash_menu
        .as_ref()
        .expect("/gcf prefix must open the slash menu");
    let entry = menu
        .matches
        .iter()
        .find(|m| m.trigger == "gcf")
        .expect("menu must contain the gcf entry");
    assert_eq!(
        entry.suggestions,
        vec![
            "on".to_string(),
            "off".to_string(),
            "show".to_string(),
            "help".to_string()
        ],
        "gcf autocomplete suggestions must be exactly [on, off, show, help]"
    );
    // The entry also carries the parameter hint listing every subcommand.
    assert_eq!(
        entry.parameter_hint.as_deref(),
        Some("[on|off|show|help]"),
        "gcf parameter hint must list all subcommands"
    );
}

#[test]
fn test_gcf_slash_menu_matches_gcf_prefix() {
    // Typing the bare `/g` prefix must surface the gcf trigger in the menu
    // (prefix match path of `update_slash_menu`).
    let mut app = make_app();
    app.input = "/g".to_string();
    app.update_slash_menu();
    let menu = app
        .slash_menu
        .as_ref()
        .expect("/g prefix must open the slash menu");
    let triggers: Vec<&str> = menu.matches.iter().map(|e| e.trigger.as_str()).collect();
    assert!(
        triggers.contains(&"gcf"),
        "menu matches must include gcf, got: {triggers:?}"
    );
}
