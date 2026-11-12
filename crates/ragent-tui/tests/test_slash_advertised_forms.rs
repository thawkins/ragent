//! Regression tests: slash-command help, descriptions and autocomplete only
//! advertise forms the dispatcher handles (issues #26, #27, #28, #43).

mod support;

use ragent_tui::app::{SLASH_COMMANDS, StatusMessage};
use support::make_app;

fn description(trigger: &str) -> &'static str {
    SLASH_COMMANDS
        .iter()
        .find(|c| c.trigger == trigger)
        .map(|c| c.description)
        .expect("command registered")
}

#[tokio::test]
async fn test_status_lists_history_newest_first() {
    let mut app = make_app();
    app.session.session_id = Some("s1".to_string());
    app.status_history
        .push(StatusMessage::working("first step"));
    app.status_history
        .push(StatusMessage::working("second step"));

    app.execute_slash_command("/status").await;

    assert_eq!(app.status, "status: history shown");
    let text = app.session.messages.last().expect("output").text_content();
    let first = text.find("first step").expect("first listed");
    let second = text.find("second step").expect("second listed");
    assert!(second < first, "newest first: {text}");
}

#[tokio::test]
async fn test_status_clear_empties_history() {
    let mut app = make_app();
    app.session.session_id = Some("s1".to_string());
    app.status_history.push(StatusMessage::working("step"));

    app.execute_slash_command("/status clear").await;

    assert!(app.status_history.is_empty());
    assert_eq!(app.status, "status: history cleared");
}

#[tokio::test]
async fn test_status_empty_history_says_so() {
    let mut app = make_app();
    app.session.session_id = Some("s1".to_string());
    app.status_history.clear();

    app.execute_slash_command("/status").await;

    let text = app.session.messages.last().expect("output").text_content();
    assert!(text.contains("No status messages recorded yet."), "{text}");
}

#[tokio::test]
async fn test_router_help_omits_unhandled_stats_and_tier() {
    let mut app = make_app();
    app.session.session_id = Some("s1".to_string());

    app.execute_slash_command("/router help").await;

    let text = app.session.messages.last().expect("output").text_content();
    assert!(!text.contains("/router stats"), "{text}");
    assert!(!text.contains("/router tier "), "{text}");
    assert!(!description("router").contains("stats"));
    assert!(
        !app.get_command_suggestions("router")
            .contains(&"stats".to_string())
    );
}

#[test]
fn test_team_does_not_advertise_open() {
    let app = make_app();
    assert!(!description("team").contains("open"));
    let suggestions = app.get_command_suggestions("team");
    for missing in ["open", "list", "spawn"] {
        assert!(
            !suggestions.contains(&missing.to_string()),
            "/team suggests unhandled `{missing}`: {suggestions:?}"
        );
    }
}

#[tokio::test]
async fn test_i18n_help_describes_bare_command_as_status() {
    let mut app = make_app();
    app.session.session_id = Some("s1".to_string());

    app.execute_slash_command("/i18n help").await;

    let text = app.session.messages.last().expect("output").text_content();
    assert!(
        text.contains("bare `/i18n` shows the status"),
        "help must match the dispatcher: {text}"
    );
}

/// The `commands_info` catalog serialises its own subcommand lists; they must
/// match the TUI autocomplete for the commands fixed here (#27, #28).
#[test]
fn test_catalog_subcommands_match_tui_suggestions() {
    let app = make_app();
    for trigger in ["router", "team", "teams"] {
        let entry = ragent_agent::tool::command_catalog::COMMAND_CATALOG
            .iter()
            .find(|e| e.trigger == trigger)
            .expect("catalog entry");
        let mut catalog: Vec<String> = entry.subcommands.iter().map(|s| (*s).to_string()).collect();
        let mut tui = app.get_command_suggestions(trigger);
        catalog.sort();
        tui.sort();
        assert_eq!(catalog, tui, "/{trigger} catalog vs TUI suggestions");
    }
}
