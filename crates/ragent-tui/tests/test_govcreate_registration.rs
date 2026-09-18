//! T-010 (spec `govdoc`, FR-001, NFR-001): `/spec govcreate` discoverability.
//!
//! Verifies that the `/spec` registry entry advertises `govcreate`, that the
//! `/spec` autocomplete suggestions offer it, and that the parameter hint
//! lists it. Dispatch behaviour is owned by T-011 and the runner by T-012.

use support::make_app;

mod support;

#[test]
fn test_govcreate_advertised_in_spec_registry_entry() {
    let def = ragent_tui::app::SLASH_COMMANDS
        .iter()
        .find(|cmd| cmd.trigger == "spec")
        .expect("/spec must be registered in SLASH_COMMANDS");
    assert!(
        def.description.contains("govcreate"),
        "the /spec description must mention govcreate (FR-001): {}",
        def.description
    );
}

#[test]
fn test_govcreate_in_spec_autocomplete_suggestions() {
    let mut app = make_app();
    app.input = "/spec".to_string();
    app.input_cursor = app.input.chars().count();
    app.update_slash_menu();

    let menu = app
        .slash_menu
        .as_ref()
        .expect("typing /spec must open the slash menu");
    let entry = menu
        .matches
        .iter()
        .find(|m| m.trigger == "spec")
        .expect("the menu must contain the spec entry");

    assert!(
        entry.suggestions.iter().any(|s| s == "govcreate"),
        "spec suggestions must offer govcreate: {:?}",
        entry.suggestions
    );
    let hint = entry
        .parameter_hint
        .as_deref()
        .expect("the spec entry must carry a parameter hint");
    assert!(
        hint.contains("govcreate"),
        "spec parameter hint must list govcreate: {hint}"
    );
}
