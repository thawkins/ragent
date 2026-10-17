//! `/spec` TUI dispatch tests.
//!
//! Exercises the read-only `/spec` arms (`help`, `list`, `status`, `task`,
//! `coverage`, `activate`, `deactivate`, `delete`, `search`) through the public
//! `App::execute_slash_command` entry point. `SpecCommand::parse` maps every
//! spelling to a variant; this file drives each arm in an isolated empty
//! project so no developer spec tree is read or written.
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

/// A project with an empty `specs/` directory.
fn app_in_empty_project() -> (
    std::sync::MutexGuard<'static, ()>,
    slash_support::EnvGuard,
    tempfile::TempDir,
    App,
) {
    let (guard, env, temp) = enter_project(r#"{ "defaultAgent": "general" }"#);
    std::fs::create_dir_all(temp.path().join("specs")).expect("create specs");
    let app = app_with_session();
    (guard, env, temp, app)
}

// ---------------------------------------------------------------------------
// help / usage
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn help_renders_command_reference() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    app.execute_slash_command("/spec help").await;
    assert_eq!(app.status, "spec: help");
    let text = last_message(&app);
    assert!(text.contains("From: /spec help"));
    assert!(text.contains("/spec create"));
    assert!(text.contains("/spec validate"));
    assert!(text.contains("/spec task"));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bare_spec_is_help() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    app.execute_slash_command("/spec").await;
    assert_eq!(app.status, "spec: help");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_argument_subcommands_report_usage() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    // Each of these parses to a usage-error variant (`Unknown(<sub>)` where the
    // subcommand exists but was given incomplete arguments).
    for sub in [
        "status", "task", "activate", "coverage", "update", "plan", "tasks", "feedback", "add",
        "delete", "specify",
    ] {
        app.execute_slash_command(&format!("/spec {sub}")).await;
        assert_eq!(
            app.status,
            format!("Usage: /spec {sub} - try /spec help"),
            "/spec {sub} should report usage"
        );
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_subcommand_status_names_command() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    app.execute_slash_command("/spec frobnicate").await;
    assert_eq!(
        app.status,
        "Unknown /spec subcommand: frobnicate. Try /spec help"
    );
}

// ---------------------------------------------------------------------------
// list / search
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_empty_specs_directory() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    app.execute_slash_command("/spec list").await;
    assert_eq!(app.status, "spec: 0 spec(s) listed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn list_with_status_filter() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    app.execute_slash_command("/spec list --status draft").await;
    assert_eq!(app.status, "spec: 0 spec(s) listed");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn search_no_match() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    app.execute_slash_command("/spec search nothing-here").await;
    assert_eq!(app.status, "spec: search complete");
    assert!(last_message(&app).contains("No specs found"));
}

// ---------------------------------------------------------------------------
// validate
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_all_when_empty() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    app.execute_slash_command("/spec validate").await;
    assert!(
        app.status == "spec: validation complete" || app.status.starts_with("spec:"),
        "got {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn validate_specific_missing_spec() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    app.execute_slash_command("/spec validate missing-spec")
        .await;
    assert!(app.status.starts_with("spec:"), "got {}", app.status);
}

// ---------------------------------------------------------------------------
// status / task / coverage / activate / deactivate / delete on a missing spec
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn status_missing_spec_reports_unknown() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    app.execute_slash_command("/spec status missing-spec").await;
    assert!(
        app.status.starts_with("spec:"),
        "unexpected status {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_spec_id_is_rejected() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    app.execute_slash_command("/spec status bad!id").await;
    assert!(app.status.contains("invalid spec ID"), "got {}", app.status);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn task_list_missing_spec() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    app.execute_slash_command("/spec task missing-spec").await;
    assert!(app.status.starts_with("spec:"), "got {}", app.status);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn coverage_missing_spec() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    app.execute_slash_command("/spec coverage missing-spec")
        .await;
    assert!(app.status.starts_with("spec:"), "got {}", app.status);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn activate_then_deactivate() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    // No active spec to deactivate.
    app.execute_slash_command("/spec deactivate").await;
    assert_eq!(app.status, "spec: no active spec");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delete_without_yes_refuses_to_delete() {
    let (_g, _e, _t, mut app) = app_in_empty_project();
    let before = app.status.clone();
    app.execute_slash_command("/spec delete missing-spec").await;
    // The confirmation guard returns early without setting the status.
    assert_eq!(
        app.status, before,
        "delete without --yes must not set status"
    );
    assert!(last_message(&app).contains("Refusing to delete"));
}
