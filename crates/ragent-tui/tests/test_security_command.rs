//! `/security` TUI dispatch tests (spec `openhands` T-015; FR-006, FR-016,
//! FR-017).
//!
//! Verifies the `handle_security_command` dispatch arm through the public
//! `App::execute_slash_command` entry point, including the `/security-analyzer`
//! alias the test plan spells, and the unknown-subcommand usage path. The
//! help/usage paths never load the on-disk config, so no cwd / env isolation is
//! needed.

use ragent_tui::App;

#[path = "support/mod.rs"]
mod support;

/// Return the text of the most recently appended message.
fn last_message_text(app: &App) -> String {
    app.messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default()
}

#[tokio::test]
async fn test_security_help_renders_usage() {
    let mut app = support::make_app();
    app.execute_slash_command("/security help").await;

    let text = last_message_text(&app);
    assert!(
        text.contains("From: /security help") && text.contains("/security on"),
        "help must render the usage table, got: {text}"
    );
    assert_eq!(app.status, "security: help", "/security help status");
}

#[tokio::test]
async fn test_security_analyzer_alias_dispatches() {
    let mut app = support::make_app();
    // The test plan spells the command `/security-analyzer`; it must resolve to
    // the same handler as `/security`.
    app.execute_slash_command("/security-analyzer help").await;

    let text = last_message_text(&app);
    assert!(
        text.contains("From: /security help"),
        "the /security-analyzer alias must dispatch, got: {text}"
    );
    assert_eq!(app.status, "security: help");
}

#[tokio::test]
async fn test_security_unknown_subcommand_usage() {
    let mut app = support::make_app();
    app.execute_slash_command("/security frobnicate").await;

    let text = last_message_text(&app);
    assert!(
        text.contains("Unknown subcommand") && text.contains("on|off|status|revert|help"),
        "an unknown subcommand must print usage, got: {text}"
    );
    assert_eq!(app.status, "security: usage");
}
