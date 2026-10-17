//! `/slash` arms that persist state to the config file or the activity log.
//!
//! Covers `/init config`, `/dirs`, `/bash`, `/editlog`, `/alog` and
//! `/security`. Every test runs inside an isolated tempdir project with an
//! isolated `XDG_CONFIG_HOME`, so the writes land in the tempdir and never in
//! the developer's real config. Env mutation is `unsafe` in edition 2024; the
//! workspace denies `unsafe_code`, so this target opts back in explicitly.
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

/// An app in a fresh isolated project with an empty scratch config.
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
// /init config
// ---------------------------------------------------------------------------

#[tokio::test]
async fn init_config_creates_then_reports_existing() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/init config").await;
    assert_eq!(app.status, "init config: created");

    // A second invocation finds the file and skips.
    app.execute_slash_command("/init config").await;
    assert_eq!(app.status, "init config: already exists");
}

#[tokio::test]
async fn init_help_renders_usage() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/init help").await;
    assert_eq!(app.status, "init: help");
    assert!(last_message(&app).contains("/init config"));
}

// ---------------------------------------------------------------------------
// /dirs
// ---------------------------------------------------------------------------

#[tokio::test]
async fn dirs_add_and_remove_allowlist() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/dirs add allow src/**/*.rs")
        .await;
    assert!(
        last_message(&app).contains("Added `src/**/*.rs`"),
        "{}",
        last_message(&app)
    );

    app.execute_slash_command("/dirs remove allow src/**/*.rs")
        .await;
    assert!(last_message(&app).contains("Removed `src/**/*.rs`"));
}

#[tokio::test]
async fn dirs_add_and_remove_denylist() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/dirs add deny secrets/**").await;
    assert!(last_message(&app).contains("Added `secrets/**`"));

    app.execute_slash_command("/dirs remove deny secrets/**")
        .await;
    assert!(last_message(&app).contains("Removed `secrets/**`"));
}

#[tokio::test]
async fn dirs_add_and_remove_roots() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/dirs add roots ../other").await;
    assert!(last_message(&app).contains("allowed_roots"));

    app.execute_slash_command("/dirs remove roots ../other")
        .await;
    assert!(last_message(&app).contains("allowed_roots"));
}

#[tokio::test]
async fn dirs_missing_pattern_shows_usage() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/dirs add allow").await;
    assert!(last_message(&app).contains("Usage"));
}

#[tokio::test]
async fn dirs_unknown_list_type_and_subcommand() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/dirs add badtype thing").await;
    assert!(last_message(&app).contains("Unknown list type"));

    app.execute_slash_command("/dirs frobnicate").await;
    assert!(last_message(&app).contains("Unknown subcommand"));
}

#[tokio::test]
async fn dirs_remove_missing_entry_warns() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/dirs remove allow never-added")
        .await;
    assert!(last_message(&app).contains("was not in the"));
}

// ---------------------------------------------------------------------------
// /bash
// ---------------------------------------------------------------------------

#[tokio::test]
async fn bash_add_and_remove_allowlist() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/bash add allow curl").await;
    assert!(last_message(&app).contains("allowlist"));

    app.execute_slash_command("/bash remove allow curl").await;
    assert!(last_message(&app).contains("Removed `curl`"));
}

#[tokio::test]
async fn bash_add_and_remove_denylist() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/bash add deny dangerous-pattern")
        .await;
    assert!(last_message(&app).contains("denylist"));

    app.execute_slash_command("/bash remove deny dangerous-pattern")
        .await;
    assert!(last_message(&app).contains("Removed `dangerous-pattern`"));
}

#[tokio::test]
async fn bash_missing_entry_shows_usage() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/bash add allow").await;
    assert!(last_message(&app).contains("Usage"));
}

#[tokio::test]
async fn bash_unknown_list_type_and_subcommand() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/bash add badtype thing").await;
    assert!(last_message(&app).contains("Unknown list type"));

    app.execute_slash_command("/bash frobnicate").await;
    assert!(last_message(&app).contains("Unknown subcommand"));
}

// ---------------------------------------------------------------------------
// /editlog
// ---------------------------------------------------------------------------

#[tokio::test]
async fn editlog_on_off_status_help() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/editlog on").await;
    assert_eq!(app.status, "editlog: enabled");

    app.execute_slash_command("/editlog status").await;
    assert_eq!(app.status, "editlog: on");

    app.execute_slash_command("/editlog off").await;
    assert_eq!(app.status, "editlog: disabled");

    app.execute_slash_command("/editlog help").await;
    assert_eq!(app.status, "editlog: help");
}

#[tokio::test]
async fn editlog_show_and_analyse_without_logs() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/editlog show").await;
    assert_eq!(app.status, "editlog: no logs");

    app.execute_slash_command("/editlog analyse").await;
    assert_eq!(app.status, "editlog: no logs");
}

#[tokio::test]
async fn editlog_clear_reports_count() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/editlog clear").await;
    assert_eq!(app.status, "editlog: cleared");
    assert!(last_message(&app).contains("Cleared"));
}

#[tokio::test]
async fn editlog_unknown_subcommand_shows_usage() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/editlog frobnicate").await;
    assert_eq!(app.status, "editlog: usage");
}

// ---------------------------------------------------------------------------
// /alog (read-only inspection arms)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_config_list_status_and_help() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/alog help").await;
    assert_eq!(app.status, "alog: help");

    app.execute_slash_command("/alog config").await;
    assert_eq!(app.status, "alog: config");

    app.execute_slash_command("/alog list").await;
    assert_eq!(app.status, "alog: list");

    app.execute_slash_command("/alog status").await;
    assert_eq!(app.status, "alog: status");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_delete_without_run_id_reports_error() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/alog delete").await;
    assert_eq!(app.status, "alog: error");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn alog_export_without_run_id_reports_error() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/alog export").await;
    assert_eq!(app.status, "alog: error");
}

// ---------------------------------------------------------------------------
// /security
// ---------------------------------------------------------------------------

#[tokio::test]
async fn security_on_off_and_revert() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/security on").await;
    assert_eq!(app.status, "security: on");

    app.execute_slash_command("/security status").await;
    assert_eq!(app.status, "security: status");

    app.execute_slash_command("/security off").await;
    assert_eq!(app.status, "security: off");

    app.execute_slash_command("/security revert").await;
    assert_eq!(app.status, "security: off");
}
