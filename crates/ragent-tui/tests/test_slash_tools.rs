//! `/tools` and `/thinking` TUI dispatch tests.
//!
//! `/tools` lists the whole registered catalogue - the family visibility
//! switches it used to read and write are gone. `/thinking` is a read-only
//! report over the active model's supported levels.
#![allow(unsafe_code)]
#![cfg(test)]
// The isolation harness holds a process-global mutex across the async test body
// so cwd/env mutations cannot interleave.
#![allow(clippy::await_holding_lock)]

use ragent_tui::App;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

/// An app in an isolated project with a scratch config.
fn isolated_app() -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    let app = app_with_session();
    (guard, env, temp, app)
}

// ---------------------------------------------------------------------------
// /tools
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tools_bare_help_and_show_list() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/tools").await;
    assert_eq!(app.status, "tools help");

    app.execute_slash_command("/tools show").await;
    assert_eq!(app.status, "tools");

    app.execute_slash_command("/tools list").await;
    assert_eq!(app.status, "tools");
}

#[tokio::test]
async fn tools_list_includes_every_family_tool() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/tools list").await;
    assert_eq!(app.status, "tools");
    let text = last_message(&app);
    // Tools from the former `github`, `teams`, `agents`, `plan` and
    // `codeindex` families are advertised like any other tool.
    assert!(text.contains("github_list_issues"), "text: {text}");
    assert!(text.contains("team_create"), "text: {text}");
    assert!(text.contains("new_agent"), "text: {text}");
    assert!(text.contains("EnterPlanMode"), "text: {text}");
    assert!(text.contains("codeindex_search"), "text: {text}");
}

#[tokio::test]
async fn tools_invalid_subcommand_is_rejected() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/tools not-a-subcommand").await;
    assert_eq!(app.status, "tools error");

    app.execute_slash_command("/tools github").await;
    assert_eq!(app.status, "tools error");

    app.execute_slash_command("/tools teams on").await;
    assert_eq!(app.status, "tools error");
}

#[tokio::test]
async fn tools_too_many_args_shows_usage() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/tools list extra").await;
    assert_eq!(app.status, "tools error");
}

// ---------------------------------------------------------------------------
// /thinking
// ---------------------------------------------------------------------------

#[tokio::test]
async fn thinking_without_model_warns() {
    let mut app = app_session();
    app.selected_model = None;
    app.agent_info.model = None;
    app.execute_slash_command("/thinking").await;
    assert!(app.status.contains("No model selected"));
}

#[tokio::test]
async fn thinking_invalid_setting_shows_usage() {
    let mut app = app_session();
    // Provide a selected model so the early no-model guard is bypassed; the
    // invalid setting is rejected before any level lookup.
    app.selected_model = Some("ollama/llama3".to_string());
    app.execute_slash_command("/thinking nonsense").await;
    assert_eq!(app.status, "Usage: /thinking [auto|off|low|medium|high]");
}

#[tokio::test]
async fn thinking_report_without_resolvable_model() {
    let mut app = app_session();
    // A model that the provider registry cannot resolve: the supported set is
    // empty, so the bare report renders "unknown".
    app.selected_model = Some("nonexistent-provider/nonexistent-model".to_string());
    app.execute_slash_command("/thinking").await;
    assert_eq!(app.status, "thinking");
    assert!(last_message(&app).contains("Supported:"));
}

/// An app with a session but no shared isolation (used by the read-only
/// `/thinking` cases that never touch the config).
fn app_session() -> App {
    let mut app = support::make_app();
    app.session_id = Some("slash-thinking".to_string());
    // Neutralise the ambient provider config so the selected model is the only
    // source considered.
    app.configured_provider = None;
    app
}
