//! Remaining `/slash` dispatch arms not covered by the focused suites.
//!
//! Covers the small informational/usage arms of `/system`, `/provider`,
//! `/model`, `/webapi`, `/reload`, `/cron`, `/mcp`, `/queue`, `/loop`,
//! `/research`, `/swarm` and `/thinking`. All are read-only or usage-only, so
//! they run in the repository cwd without isolation.
#![cfg(test)]

use ragent_tui::App;

#[path = "support/mod.rs"]
mod support;

/// Build an app with a live session.
fn app() -> App {
    let mut app = support::make_app();
    app.session_id = Some("slash-misc".to_string());
    app
}

/// Text of the most recent message in the message window.
fn last_message(app: &App) -> String {
    app.messages
        .last()
        .map(ragent_types::message::Message::text_content)
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// /system
// ---------------------------------------------------------------------------

#[tokio::test]
async fn system_help_sets_prompt_and_shows_current() {
    let mut app = app();
    app.execute_slash_command("/system help").await;
    assert_eq!(app.status, "system: help");

    // No prompt set yet -> the no-arg form reports that.
    app.agent_info.prompt = None;
    app.execute_slash_command("/system").await;
    assert_eq!(app.status, "No system prompt set");

    // Setting a prompt stores it and flips the status.
    app.execute_slash_command("/system You are terse.").await;
    assert_eq!(app.status, "system prompt updated");
    assert_eq!(app.agent_info.prompt.as_deref(), Some("You are terse."));

    // The no-arg form now echoes the stored prompt.
    app.execute_slash_command("/system").await;
    assert!(last_message(&app).contains("Current system prompt"));
}

// ---------------------------------------------------------------------------
// /provider
// ---------------------------------------------------------------------------

#[tokio::test]
async fn provider_help_and_usage() {
    let mut app = app();
    app.execute_slash_command("/provider help").await;
    assert_eq!(app.status, "provider: help");

    app.execute_slash_command("/provider nonsense").await;
    assert_eq!(app.status, "provider: usage");
}

#[tokio::test]
async fn provider_empty_opens_setup() {
    let mut app = app();
    app.provider_setup = None;
    app.execute_slash_command("/provider").await;
    assert!(
        app.provider_setup.is_some(),
        "bare /provider must open the setup dialog"
    );
}

// ---------------------------------------------------------------------------
// /model
// ---------------------------------------------------------------------------

#[tokio::test]
async fn model_unknown_arg_shows_usage() {
    let mut app = app();
    app.execute_slash_command("/model nonsense").await;
    assert_eq!(app.status, "Usage: /model [show]");
}

#[tokio::test]
async fn model_show_without_model_warns() {
    let mut app = app();
    app.selected_model = None;
    app.agent_info.model = None;
    app.execute_slash_command("/model show").await;
    assert_eq!(app.status, "[warn] No active model selected");
}

// ---------------------------------------------------------------------------
// /webapi
// ---------------------------------------------------------------------------

#[tokio::test]
async fn webapi_help_renders_endpoints() {
    let mut app = app();
    app.execute_slash_command("/webapi help").await;
    let text = last_message(&app);
    assert!(text.contains("Web API"), "endpoint table: {text}");
    assert!(text.contains("/health"), "health row: {text}");
}

#[tokio::test]
async fn webapi_disable_when_not_running() {
    let mut app = app();
    app.webapi_server = None;
    app.execute_slash_command("/webapi disable").await;
    assert!(last_message(&app).contains("not running"));
}

#[tokio::test]
async fn webapi_unknown_subcommand_shows_usage() {
    let mut app = app();
    app.execute_slash_command("/webapi frobnicate").await;
    assert!(last_message(&app).contains("Usage: `/webapi"));
}

// ---------------------------------------------------------------------------
// /reload (sub-arms + unknown)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn reload_sub_arms_and_unknown() {
    let mut app = app();
    for sub in ["config", "mcp", "skills", "agents"] {
        app.execute_slash_command(&format!("/reload {sub}")).await;
        assert_eq!(app.status, "reload", "/reload {sub} status");
        assert!(last_message(&app).contains("From: /reload"));
    }
    app.execute_slash_command("/reload frobnicate").await;
    assert_eq!(app.status, "reload");
    assert!(last_message(&app).contains("Unknown subcommand"));
}

// ---------------------------------------------------------------------------
// /cron (help + unknown)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cron_help_and_unknown() {
    let mut app = app();
    app.execute_slash_command("/cron help").await;
    assert_eq!(app.status, "cron: help");
    assert!(last_message(&app).contains("/cron add"));

    app.execute_slash_command("/cron frobnicate").await;
    assert_eq!(app.status, "cron: unknown");
}

// ---------------------------------------------------------------------------
// /mcp (help + connect/disconnect without id)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mcp_help_and_usage_forms() {
    let mut app = app();
    app.execute_slash_command("/mcp help").await;
    assert_eq!(app.status, "mcp: help");

    app.execute_slash_command("/mcp connect").await;
    assert_eq!(app.status, "Usage: /mcp connect <id>");

    app.execute_slash_command("/mcp disconnect").await;
    assert_eq!(app.status, "Usage: /mcp disconnect <id>");
}

// ---------------------------------------------------------------------------
// /queue (list/clear/next)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn queue_list_empty_and_clear() {
    let mut app = app();
    app.execute_slash_command("/queue list").await;
    assert_eq!(app.status, "queue: list empty");

    app.execute_slash_command("/queue clear").await;
    assert_eq!(app.status, "queue: cleared");
}

#[tokio::test]
async fn queue_next_empty() {
    let mut app = app();
    app.execute_slash_command("/queue next").await;
    assert_eq!(app.status, "queue: next empty");
}

// ---------------------------------------------------------------------------
// /loop / /research / /swarm help
// ---------------------------------------------------------------------------

#[tokio::test]
async fn loop_help_renders_usage() {
    let mut app = app();
    app.execute_slash_command("/loop help").await;
    assert!(last_message(&app).contains("From: /loop help"));
}

#[tokio::test]
async fn loop_opens_setup_on_bare_form() {
    let mut app = app();
    app.execute_slash_command("/loop").await;
    assert!(
        app.loop_setup.is_some(),
        "bare /loop must open the setup dialog"
    );
}

#[tokio::test]
async fn loop_invalid_flag_reports_error() {
    let mut app = app();
    app.execute_slash_command("/loop general --max-steps").await;
    assert_eq!(app.status, "loop: invalid argument");
}

#[tokio::test]
async fn research_help_renders_usage() {
    let mut app = app();
    app.execute_slash_command("/research help").await;
    assert_eq!(app.status, "research: help");
    assert!(
        last_message(&app).contains("ragent research"),
        "help body: {}",
        last_message(&app)
    );
}

#[tokio::test]
async fn swarm_help_renders_usage() {
    let mut app = app();
    app.execute_slash_command("/swarm help").await;
    assert!(last_message(&app).contains("/swarm"));
}

#[tokio::test]
async fn swarm_status_without_swarm() {
    let mut app = app();
    app.swarm_state = None;
    app.execute_slash_command("/swarm status").await;
    assert_ne!(app.status, "");
}

// ---------------------------------------------------------------------------
// /connectors / /plugins usage
// ---------------------------------------------------------------------------

#[tokio::test]
async fn connectors_help_and_unknown() {
    let mut app = app();
    app.execute_slash_command("/connectors help").await;
    assert_eq!(app.status, "connectors");

    app.execute_slash_command("/connectors frobnicate").await;
    assert_eq!(app.status, "connectors");
}

#[tokio::test]
async fn plugins_help_and_unknown() {
    let mut app = app();
    app.execute_slash_command("/plugins help").await;
    assert_eq!(app.status, "plugins");

    app.execute_slash_command("/plugins frobnicate").await;
    assert_eq!(app.status, "plugins");
}
