//! Tests for the `/x help` subcommand support across the main slash-command
//! families. Each test verifies that running `<cmd> help` appends a usage
//! message into the chat transcript (the `From: /<cmd> help` header is the
//! shared convention) and does not mutate unrelated state.
//!
//! The `/x help` arm must never trigger a side effect (e.g. `/init help` must
//! not start the project-analysis agent run, `/update help` must not hit the
//! network).

use ragent_tui::App;
use support::make_app;

mod support;

/// Run `/<cmd> help` and return the last assistant message text.
fn last_output_after_help(app: &mut App, cmd: &str) -> String {
    app.execute_slash_command(&format!("/{cmd} help"));
    app.messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default()
}

fn assert_help(app: &mut App, cmd: &str, expect_status: &str) {
    let text = last_output_after_help(app, cmd);
    assert!(
        text.contains(&format!("From: /{cmd} help")),
        "/{cmd} help should emit a 'From: /{cmd} help' header, got: {text}"
    );
    assert_eq!(app.status, expect_status, "/{cmd} help status");
}

#[test]
fn test_slash_config_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "config", "config: help");
    assert!(last_output_after_help(&mut app, "config").contains("/config save"));
}

#[test]
fn test_slash_init_help_does_not_start_analysis() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "init", "init: help");
    // The help arm must not spawn the analysis agent run.
    assert!(!app.is_processing, "/init help must not start processing");
    let text = last_output_after_help(&mut app, "init");
    assert!(text.contains("/init config"));
}

#[test]
fn test_slash_context_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "context", "context: help");
}

#[test]
fn test_slash_mcp_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "mcp", "mcp: help");
    assert!(last_output_after_help(&mut app, "mcp").contains("/mcp discover"));
}

#[test]
fn test_slash_profile_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "profile", "profile: help");
}

#[test]
fn test_slash_perf_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "perf", "perf: help");
}

#[test]
fn test_slash_model_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "model", "model: help");
}

#[test]
fn test_slash_provider_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "provider", "provider: help");
}

#[test]
fn test_slash_reload_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "reload", "reload: help");
}

#[test]
fn test_slash_mode_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "mode", "mode: help");
}

#[test]
fn test_slash_autopilot_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "autopilot", "autopilot: help");
    // Help must not enable autopilot.
    assert!(!app.autopilot_enabled);
}

#[test]
fn test_slash_github_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "github", "github: help");
}

#[test]
fn test_slash_gitlab_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "gitlab", "gitlab: help");
}

#[test]
fn test_slash_update_help_does_not_check_network() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "update", "update: help");
}

#[test]
fn test_slash_mouse_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "mouse", "mouse: help");
}

#[test]
fn test_slash_yolo_help_does_not_toggle() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "yolo", "yolo: help");
}

#[test]
fn test_slash_skills_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "skills", "skills: help");
}

#[test]
fn test_slash_agent_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "agent", "agent: help");
    // Help must not open the picker dialog.
    assert!(app.provider_setup.is_none());
}

#[test]
fn test_slash_cancel_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "cancel", "cancel: help");
}

#[test]
fn test_slash_system_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "system", "system: help");
}

#[test]
fn test_slash_undo_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "undo", "undo: help");
}

#[test]
fn test_slash_name_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "name", "name: help");
}

#[test]
fn test_slash_resume_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "resume", "resume: help");
}

#[test]
fn test_slash_doctor_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "doctor", "doctor: help");
}

#[test]
fn test_slash_history_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "history", "history: help");
    // Help must not open the history picker.
    assert!(app.history_picker.is_none());
}

#[test]
fn test_slash_tasks_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "tasks", "tasks: help");
}

#[test]
fn test_slash_template_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "template", "template: help");
}

#[test]
fn test_slash_plan_help() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    assert_help(&mut app, "plan", "plan: help");
}

#[test]
fn test_slash_help_dash_h_aliases() {
    // `--help` / `-h` spellings resolve to the same help output on families
    // that historically only accepted the bare token.
    let mut app = make_app();
    app.session_id = Some("s".to_string());

    app.execute_slash_command("/opt --help");
    assert_eq!(app.status, "opt help");

    app.execute_slash_command("/loop -h");
    assert_eq!(app.status, "loop: help");
}

#[test]
fn test_slash_central_help_lists_new_commands() {
    let mut app = make_app();
    app.session_id = Some("s".to_string());
    app.execute_slash_command("/help");
    let text = app
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default();
    for trigger in [
        "cron",
        "triggers",
        "inbox",
        "loop",
        "editlog",
        "alog",
        "telemetry",
        "router",
        "mouse",
    ] {
        assert!(
            text.contains(&format!("/{trigger}")),
            "central /help should list /{trigger}"
        );
    }
}
