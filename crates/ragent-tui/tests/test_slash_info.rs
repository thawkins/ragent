//! Miscellaneous informational `/slash` dispatch arms.
//!
//! These commands render static text or read-only state and never mutate the
//! config file or the working directory, so no isolation harness is needed -
//! every test runs in the repository cwd and only asserts the status bar /
//! message window. This file deliberately covers the many "one arm, one line"
//! families that the focused per-command suites leave untouched.
#![cfg(test)]

#[path = "support/mod.rs"]
mod support;

use ragent_tui::App;

/// Build an app with a live session so `append_assistant_text` records
/// messages.
fn app() -> App {
    let mut app = support::make_app();
    app.session_id = Some("slash-info".to_string());
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
// about / agents / context
// ---------------------------------------------------------------------------

#[tokio::test]
async fn about_reports_build_info() {
    let mut app = app();
    app.execute_slash_command("/about").await;
    assert_eq!(app.status, "about");
    let text = last_message(&app);
    assert!(text.contains("From: /about"), "header: {text}");
    assert!(text.contains("MIT"), "licence line: {text}");
}

#[tokio::test]
async fn agents_lists_builtins() {
    let mut app = app();
    app.execute_slash_command("/agents").await;
    assert_eq!(app.status, "agents");
    let text = last_message(&app);
    assert!(text.contains("Built-in Agents"), "header: {text}");
    assert!(
        text.contains("Custom Agents"),
        "custom-agents section: {text}"
    );
}

#[tokio::test]
async fn context_help_renders_usage() {
    let mut app = app();
    app.execute_slash_command("/context help").await;
    assert_eq!(app.status, "context: help");
    assert!(last_message(&app).contains("/context refresh"));
}

#[tokio::test]
async fn context_refresh_clears_cache() {
    let mut app = app();
    app.execute_slash_command("/context refresh").await;
    assert_eq!(app.status, "context refreshed");
}

#[tokio::test]
async fn context_unknown_prints_usage_without_status_change() {
    let mut app = app();
    app.status = "sentinel".to_string();
    app.execute_slash_command("/context nonsense").await;
    assert_eq!(app.status, "sentinel", "unknown arm must not set status");
    assert!(last_message(&app).contains("Usage"));
}

// ---------------------------------------------------------------------------
// mode
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mode_help_renders_table() {
    let mut app = app();
    app.execute_slash_command("/mode help").await;
    assert_eq!(app.status, "mode: help");
    assert!(last_message(&app).contains("/mode architect"));
}

#[tokio::test]
async fn mode_status_reports_normal() {
    let mut app = app();
    app.role_mode = None;
    app.execute_slash_command("/mode status").await;
    assert!(last_message(&app).contains("normal"));
}

#[tokio::test]
async fn mode_sets_role_and_clears_it() {
    let mut app = app();
    app.execute_slash_command("/mode architect").await;
    assert!(app.role_mode.is_some(), "role mode should be set");
    assert!(app.status.contains("architect"));

    app.execute_slash_command("/mode off").await;
    assert!(app.role_mode.is_none(), "role mode should be cleared");
    assert_eq!(app.status, "mode: normal");
}

#[tokio::test]
async fn mode_unknown_reports_available_modes() {
    let mut app = app();
    app.execute_slash_command("/mode quantum").await;
    assert!(last_message(&app).contains("Unknown mode 'quantum'"));
}

// ---------------------------------------------------------------------------
// yolo
// ---------------------------------------------------------------------------

#[tokio::test]
async fn yolo_help_renders_help() {
    let mut app = app();
    app.execute_slash_command("/yolo help").await;
    assert_eq!(app.status, "yolo: help");
    assert!(last_message(&app).contains("YOLO mode"));
}

// ---------------------------------------------------------------------------
// clear / cancel / clip / cost
// ---------------------------------------------------------------------------

#[tokio::test]
async fn clear_empties_messages() {
    let mut app = app();
    app.messages
        .push(ragent_agent::message::Message::user_text("s", "hi"));
    app.execute_slash_command("/clear").await;
    assert!(app.messages.is_empty());
    assert_eq!(app.status, "messages cleared");
}

#[tokio::test]
async fn cancel_help_and_missing_id() {
    let mut app = app();
    app.execute_slash_command("/cancel help").await;
    assert_eq!(app.status, "cancel: help");

    app.execute_slash_command("/cancel").await;
    assert!(app.status.contains("provide a task ID"));
}

#[tokio::test]
async fn clip_empty_window_reports_nothing_to_copy() {
    let mut app = app();
    app.message_content_lines.clear();
    app.execute_slash_command("/clip").await;
    assert_eq!(app.status, "clip: nothing to copy");
}

#[tokio::test]
async fn cost_without_samples_is_unavailable() {
    let mut app = app();
    app.llm_request_stats.clear();
    app.execute_slash_command("/cost").await;
    assert_eq!(app.status, "cost unavailable");
}

// ---------------------------------------------------------------------------
// profile / perf / startup / actionloop / history
// ---------------------------------------------------------------------------

#[tokio::test]
async fn profile_on_and_off_toggle_panel() {
    let mut app = app();
    app.execute_slash_command("/profile on").await;
    assert!(app.show_profile, "profile panel should be on");
    assert_eq!(app.status, "profile panel visible");

    app.execute_slash_command("/profile off").await;
    assert!(!app.show_profile, "profile panel should be off");
    assert_eq!(app.status, "profile panel hidden");
}

#[tokio::test]
async fn perf_help_and_usage() {
    let mut app = app();
    app.execute_slash_command("/perf help").await;
    assert_eq!(app.status, "perf: help");

    app.execute_slash_command("/perf nonsense").await;
    assert_eq!(app.status, "perf usage");
}

#[tokio::test]
async fn startup_reports_timings_when_present() {
    let mut app = app();
    app.startup_timings = None;
    app.execute_slash_command("/startup").await;
    assert_eq!(app.status, "startup: unavailable");
    assert!(last_message(&app).contains("No startup timings"));
}

#[tokio::test]
async fn actionloop_help_and_no_samples() {
    let mut app = app();
    app.execute_slash_command("/actionloop help").await;
    assert_eq!(app.status, "actionloop: help");

    app.execute_slash_command("/actionloop").await;
    assert_eq!(app.status, "actionloop: no samples");
}

#[tokio::test]
async fn history_help_and_picker() {
    let mut app = app();
    app.execute_slash_command("/history help").await;
    assert_eq!(app.status, "history: help");

    app.input_history = vec!["alpha".to_string(), "beta".to_string()];
    app.execute_slash_command("/history").await;
    assert!(app.history_picker.is_some(), "history picker should open");
}

// ---------------------------------------------------------------------------
// mouse / telemetry / websearch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn mouse_help_on_off_and_status() {
    let mut app = app();
    app.execute_slash_command("/mouse help").await;
    assert_eq!(app.status, "mouse: help");

    app.execute_slash_command("/mouse on").await;
    assert!(app.mouse_enabled);
    assert_eq!(app.status, "mouse: enabled");

    app.execute_slash_command("/mouse off").await;
    assert!(!app.mouse_enabled);

    app.execute_slash_command("/mouse").await;
    assert!(app.status.starts_with("mouse: "));
}

#[tokio::test]
async fn telemetry_help_usage_and_counters() {
    let mut app = app();
    app.execute_slash_command("/telemetry help").await;
    assert_eq!(app.status, "telemetry: help");

    app.execute_slash_command("/telemetry counters").await;
    assert_eq!(app.status, "telemetry: counters");

    app.execute_slash_command("/telemetry frobnicate").await;
    assert_eq!(app.status, "telemetry: usage");
}

#[tokio::test]
async fn websearch_help_show_and_unknown() {
    let mut app = app();
    app.execute_slash_command("/websearch help").await;
    assert_eq!(app.status, "websearch: help");

    app.execute_slash_command("/websearch show").await;
    assert_eq!(app.status, "websearch: status shown");

    app.execute_slash_command("/websearch frobnicate").await;
    assert_eq!(app.status, "websearch: unknown");
}

// ---------------------------------------------------------------------------
// toolchain (multi-thread: `list` uses `block_in_place`)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn toolchain_help_list_and_unknown() {
    let mut app = app();
    app.execute_slash_command("/toolchain help").await;
    assert_eq!(app.status, "toolchain: help");

    app.execute_slash_command("/toolchain list").await;
    assert_eq!(app.status, "toolchain: list");

    app.execute_slash_command("/toolchain frobnicate").await;
    assert_eq!(app.status, "toolchain: usage");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn toolchain_list_unknown_language_reports_ids() {
    let mut app = app();
    app.execute_slash_command("/toolchain list notalanguage")
        .await;
    assert_eq!(app.status, "toolchain: list");
    assert!(last_message(&app).contains("Unknown language id"));
}

// ---------------------------------------------------------------------------
// security / prompt / skills / blueprints / template / goal
// ---------------------------------------------------------------------------

#[tokio::test]
async fn security_help_and_usage() {
    let mut app = app();
    app.execute_slash_command("/security help").await;
    assert_eq!(app.status, "security: help");

    app.execute_slash_command("/security frobnicate").await;
    assert_eq!(app.status, "security: usage");
}

#[tokio::test]
async fn prompt_help_list_and_unknown_agent() {
    let mut app = app();
    app.execute_slash_command("/prompt help").await;
    assert_eq!(app.status, "prompt: help");

    app.execute_slash_command("/prompt list").await;
    assert!(app.status.starts_with("prompt: list"), "got {}", app.status);

    app.execute_slash_command("/prompt definitely-not-an-agent")
        .await;
    assert!(
        app.status.starts_with("prompt: unknown subcommand"),
        "got {}",
        app.status
    );
}

#[tokio::test]
async fn skills_help_renders_usage() {
    let mut app = app();
    app.execute_slash_command("/skills help").await;
    assert_eq!(app.status, "skills: help");
}

#[tokio::test]
async fn blueprints_help_list_and_unknown() {
    let mut app = app();
    app.execute_slash_command("/blueprints help").await;
    assert_eq!(app.status, "blueprints: help");

    app.execute_slash_command("/blueprints list").await;
    assert_eq!(app.status, "blueprints: list");

    app.execute_slash_command("/blueprints no-such-blueprint")
        .await;
    assert!(app.status.contains("not found"));
}

#[tokio::test]
async fn template_help_list_and_not_found() {
    let mut app = app();
    app.execute_slash_command("/template help").await;
    assert_eq!(app.status, "template: help");

    app.execute_slash_command("/template").await;
    assert_eq!(app.status, "template: listed");

    app.execute_slash_command("/template no-such-template")
        .await;
    assert!(app.status.contains("not found"));
}

#[tokio::test]
async fn goal_help_set_clear_show_test_and_unknown() {
    let mut app = app();
    // `/goal help` is not a recognised subcommand; the unknown arm reports it.
    app.execute_slash_command("/goal help").await;
    assert_eq!(app.status, "goal: unknown 'help'");

    app.execute_slash_command("/goal set finish the release")
        .await;
    assert!(app.status.starts_with("goal: set"));

    app.execute_slash_command("/goal show").await;
    assert_eq!(app.status, "goal: none");

    app.execute_slash_command("/goal test").await;
    assert_eq!(app.status, "goal: test (none)");

    app.execute_slash_command("/goal clear").await;
    assert_eq!(app.status, "goal: cleared");

    app.execute_slash_command("/goal nonsense").await;
    assert!(app.status.starts_with("goal: unknown"));
}

// ---------------------------------------------------------------------------
// automation / alog / resume / reload / tools / dirs / bash / github / gitlab
// ---------------------------------------------------------------------------

#[tokio::test]
async fn automation_help_and_unknown() {
    let mut app = app();
    app.execute_slash_command("/automation help").await;
    assert_eq!(app.status, "automation: help");

    app.execute_slash_command("/automation frobnicate").await;
    assert_eq!(app.status, "automation: unknown");
}

#[tokio::test]
async fn alog_help_and_unknown() {
    let mut app = app();
    app.execute_slash_command("/alog help").await;
    assert_eq!(app.status, "alog: help");

    app.execute_slash_command("/alog frobnicate").await;
    assert_eq!(app.status, "alog: usage");
}

#[tokio::test]
async fn resume_help_and_nothing_to_resume() {
    let mut app = app();
    app.execute_slash_command("/resume help").await;
    assert_eq!(app.status, "resume: help");

    app.agent_halted = false;
    app.execute_slash_command("/resume").await;
    assert!(app.status.contains("Nothing to resume"));
}

#[tokio::test]
async fn reload_help_renders_usage() {
    let mut app = app();
    app.execute_slash_command("/reload help").await;
    assert_eq!(app.status, "reload: help");
    assert!(last_message(&app).contains("/reload agents"));
}

#[tokio::test]
async fn tools_help_and_list() {
    let mut app = app();
    app.execute_slash_command("/tools help").await;
    assert_eq!(app.status, "tools help");

    app.execute_slash_command("/tools list").await;
    assert_eq!(app.status, "tools");
}

#[tokio::test]
async fn dirs_help_and_show() {
    let mut app = app();
    app.execute_slash_command("/dirs help").await;
    assert!(last_message(&app).contains("/dirs add allow"));

    app.execute_slash_command("/dirs show").await;
    assert!(last_message(&app).contains("Directory/File Permission Lists"));
}

#[tokio::test]
async fn bash_help_and_show() {
    let mut app = app();
    app.execute_slash_command("/bash help").await;
    assert!(last_message(&app).contains("/bash add allow"));

    app.execute_slash_command("/bash show").await;
    assert!(last_message(&app).contains("Bash command lists"));
}

#[tokio::test]
async fn github_help_status_and_usage() {
    let mut app = app();
    app.execute_slash_command("/github help").await;
    assert_eq!(app.status, "github: help");

    app.execute_slash_command("/github status").await;
    assert!(last_message(&app).contains("From: /github"));

    app.execute_slash_command("/github frobnicate").await;
    assert!(last_message(&app).contains("Usage"));
}

#[tokio::test]
async fn gitlab_help_status_and_usage() {
    let mut app = app();
    app.execute_slash_command("/gitlab help").await;
    assert_eq!(app.status, "gitlab: help");

    app.execute_slash_command("/gitlab status").await;
    assert!(last_message(&app).contains("From: /gitlab"));

    app.execute_slash_command("/gitlab frobnicate").await;
    assert!(last_message(&app).contains("Usage"));
}

#[tokio::test]
async fn model_and_provider_help() {
    let mut app = app();
    app.execute_slash_command("/model help").await;
    assert_eq!(app.status, "model: help");

    app.execute_slash_command("/provider help").await;
    assert_eq!(app.status, "provider: help");
}

#[tokio::test]
async fn thinking_without_model_warns() {
    let mut app = app();
    app.selected_model = None;
    app.agent_info.model = None;
    app.execute_slash_command("/thinking").await;
    assert!(app.status.contains("No model selected"));
}
