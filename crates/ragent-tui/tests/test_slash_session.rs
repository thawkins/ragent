//! `/plan`, `/autopilot`, `/memory` and session-side `/slash` arms.
//!
//! Covers dispatch arms that mutate live session state (autonomous mode, role
//! delegation, the memory panel) but never touch the config file, so they run
//! in the repository cwd without an isolation harness.
#![cfg(test)]

use ragent_tui::App;

#[path = "support/mod.rs"]
mod support;

/// Build an app with a live session.
fn app() -> App {
    let mut app = support::make_app();
    app.session_id = Some("slash-session".to_string());
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
// plan
// ---------------------------------------------------------------------------

#[tokio::test]
async fn plan_help_renders_usage() {
    let mut app = app();
    app.execute_slash_command("/plan help").await;
    assert_eq!(app.status, "plan: help");
    assert!(last_message(&app).contains("/plan <task description>"));
}

#[tokio::test]
async fn plan_without_task_shows_usage() {
    let mut app = app();
    app.execute_slash_command("/plan").await;
    assert_eq!(app.status, "plan: usage");
}

#[tokio::test]
async fn plan_with_task_delegates_to_plan_agent() {
    let mut app = app();
    app.execute_slash_command("/plan refactor the storage layer")
        .await;
    assert!(
        app.status.starts_with("agent: plan"),
        "expected plan delegation, got {}",
        app.status
    );
    assert_eq!(app.agent_name, "plan");
    // The delegated task appears in the transcript as a user message.
    assert!(
        app.messages
            .iter()
            .any(|m| m.text_content().contains("refactor the storage layer"))
    );
}

// ---------------------------------------------------------------------------
// autopilot
// ---------------------------------------------------------------------------

#[tokio::test]
async fn autopilot_help_renders_table() {
    let mut app = app();
    app.execute_slash_command("/autopilot help").await;
    assert_eq!(app.status, "autopilot: help");
    assert!(last_message(&app).contains("/autopilot on"));
}

#[tokio::test]
async fn autopilot_bare_is_help() {
    let mut app = app();
    app.execute_slash_command("/autopilot").await;
    assert_eq!(app.status, "autopilot: help");
}

#[tokio::test]
async fn autopilot_on_with_budgets_then_status_then_off() {
    let mut app = app();
    app.execute_slash_command("/autopilot on --max-tokens 5000 --max-time 60")
        .await;
    assert!(app.autopilot_enabled, "autopilot should be enabled");
    assert_eq!(app.autopilot_token_budget, Some(5000));
    assert_eq!(app.autopilot_time_limit_secs, Some(60));
    assert!(app.autopilot_started_at.is_some());

    app.execute_slash_command("/autopilot status").await;
    assert!(last_message(&app).contains("Autopilot: **ON**"));

    app.execute_slash_command("/autopilot off").await;
    assert!(!app.autopilot_enabled, "autopilot should be disabled");
    assert_eq!(app.autopilot_token_budget, None);
    assert_eq!(app.autopilot_time_limit_secs, None);
    assert!(app.autopilot_started_at.is_none());
}

#[tokio::test]
async fn autopilot_on_rejects_non_numeric_budget() {
    let mut app = app();
    app.execute_slash_command("/autopilot on --max-tokens abc")
        .await;
    assert_eq!(app.status, "autopilot: invalid argument");
    assert!(
        !app.autopilot_enabled,
        "invalid arg must not enable autopilot"
    );
    assert!(last_message(&app).contains("expected a positive integer"));
}

#[tokio::test]
async fn autopilot_unknown_subcommand_shows_usage() {
    let mut app = app();
    app.execute_slash_command("/autopilot frobnicate").await;
    assert!(last_message(&app).contains("Usage:"));
}

// ---------------------------------------------------------------------------
// memory
// ---------------------------------------------------------------------------

#[tokio::test]
async fn memory_show_toggles_panel_and_reports_count() {
    let mut app = app();
    let before = app.show_memory;
    app.execute_slash_command("/memory show").await;
    assert_ne!(app.show_memory, before, "memory panel should toggle");
    assert!(last_message(&app).contains("Structured memories"));
}

#[tokio::test]
async fn memory_clear_opens_confirmation() {
    let mut app = app();
    app.execute_slash_command("/memory clear").await;
    assert!(
        app.memory_clear_confirm_open,
        "confirmation dialog should open"
    );
    assert_eq!(app.status, "confirm clearing memory");
}

#[tokio::test]
async fn memory_help_and_unknown_print_usage() {
    let mut app = app();
    app.execute_slash_command("/memory help").await;
    assert!(last_message(&app).contains("/memory show"));

    app.execute_slash_command("/memory frobnicate").await;
    assert!(last_message(&app).contains("/memory show"));
}

// ---------------------------------------------------------------------------
// spawn
// ---------------------------------------------------------------------------

#[tokio::test]
async fn spawn_help_bare_and_empty_prompt() {
    let mut app = app();
    app.execute_slash_command("/spawn help").await;
    assert_eq!(app.status, "spawn: help");

    // Bare `/spawn` is help too.
    app.execute_slash_command("/spawn").await;
    assert_eq!(app.status, "spawn: help");

    // An agent name with no prompt is the usage error.
    app.execute_slash_command("/spawn general").await;
    assert_eq!(app.status, "spawn: usage");
    assert!(last_message(&app).contains("Usage"));
}

#[tokio::test]
async fn spawn_unknown_agent_is_rejected() {
    let mut app = app();
    app.execute_slash_command("/spawn no-such-agent-xyz do something")
        .await;
    assert!(
        last_message(&app).contains("Unknown") || app.status.contains("unknown"),
        "unknown agent must be rejected: status={} msg={}",
        app.status,
        last_message(&app)
    );
}

// ---------------------------------------------------------------------------
// log / task panels
// ---------------------------------------------------------------------------

#[tokio::test]
async fn log_toggles_panel_and_help() {
    let mut app = app();
    app.show_log = false;
    app.execute_slash_command("/log").await;
    assert!(app.show_log, "log panel should toggle on");
    assert_eq!(app.status, "log panel visible");

    app.execute_slash_command("/log help").await;
    assert_eq!(app.status, "log: help");
}

#[tokio::test]
async fn log_unknown_subcommand_prints_usage() {
    let mut app = app();
    app.execute_slash_command("/log frobnicate").await;
    assert_eq!(app.status, "log: usage");
}

#[tokio::test]
async fn task_toggles_panel_and_help() {
    let mut app = app();
    app.show_tasks_panel = false;
    app.execute_slash_command("/task").await;
    assert!(app.show_tasks_panel, "tasks panel should toggle on");

    app.execute_slash_command("/task help").await;
    assert!(last_message(&app).contains("/task"));
}

// ---------------------------------------------------------------------------
// startup with recorded timings
// ---------------------------------------------------------------------------

#[tokio::test]
async fn startup_reports_recorded_timings() {
    let mut app = app();
    let mut timings = ragent_agent::StartupTimings::new();
    timings.record("test stage", std::time::Duration::from_millis(5));
    timings.finish();
    app.startup_timings = Some(timings);
    app.execute_slash_command("/startup").await;
    assert_eq!(app.status, "startup: timings shown");
    assert!(last_message(&app).contains("test stage"));
}

// ---------------------------------------------------------------------------
// queue (mutating arms not covered elsewhere)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn queue_help_and_unknown() {
    let mut app = app();
    app.execute_slash_command("/queue help").await;
    assert_eq!(app.status, "queue: help");

    app.execute_slash_command("/queue frobnicate").await;
    assert_eq!(app.status, "queue: unknown");
}
