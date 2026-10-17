//! `/router` TUI dispatch tests.
//!
//! Exercises the `/router` match arm inside `App::execute_slash_command`
//! through the public entry point. The router family is read-mostly: `help`,
//! `status`, `tiers`, `weights`, `boundaries` and `test` never touch disk,
//! while `reload` reads the project/global `ragent.json`. `on`/`off` only flip
//! the in-memory `router_enabled` flag.
//!
//! Isolation is only needed for `reload`, which is driven from an isolated
//! tempdir project so it never reads the developer's real config.
#![allow(unsafe_code)]
#![cfg(test)]
// The isolation harness holds a process-global mutex across the async test body
// so cwd/env mutations cannot interleave.
#![allow(clippy::await_holding_lock)]

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

// ---------------------------------------------------------------------------
// help / status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn help_renders_subcommand_table() {
    let mut app = app_with_session();
    app.execute_slash_command("/router help").await;
    assert_eq!(app.status, "router: help");
    let text = last_message(&app);
    for needle in [
        "/router on",
        "/router off",
        "/router status",
        "/router tiers",
        "/router weights",
        "/router boundaries",
        "/router test",
        "/router reload",
    ] {
        assert!(text.contains(needle), "help missing {needle}: {text}");
    }
}

#[tokio::test]
async fn bare_router_is_help() {
    let mut app = app_with_session();
    app.execute_slash_command("/router").await;
    assert_eq!(app.status, "router: help");
}

#[tokio::test]
async fn unknown_subcommand_reports_unknown() {
    let mut app = app_with_session();
    app.execute_slash_command("/router frobnicate").await;
    assert_eq!(app.status, "router: unknown");
    assert!(last_message(&app).contains("Unknown subcommand"));
}

// ---------------------------------------------------------------------------
// on / off / status
// ---------------------------------------------------------------------------

#[tokio::test]
async fn on_enables_router() {
    let mut app = app_with_session();
    app.router_enabled = false;
    app.execute_slash_command("/router on").await;
    assert!(app.router_enabled, "router_enabled should be true");
    assert_eq!(app.status, "router: on");
}

#[tokio::test]
async fn off_disables_router() {
    let mut app = app_with_session();
    app.router_enabled = true;
    app.execute_slash_command("/router off").await;
    assert!(!app.router_enabled, "router_enabled should be false");
    assert_eq!(app.status, "router: off");
}

#[tokio::test]
async fn status_reports_enabled_and_tier() {
    let mut app = app_with_session();
    app.router_enabled = true;
    app.router_current_tier = Some("MEDIUM".to_string());
    app.execute_slash_command("/router status").await;
    assert_eq!(app.status, "router: status");
    let text = last_message(&app);
    assert!(
        text.contains("enabled"),
        "status should say enabled: {text}"
    );
    assert!(
        text.contains("MEDIUM"),
        "status should show the tier: {text}"
    );
}

// ---------------------------------------------------------------------------
// tiers / weights / boundaries
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tiers_lists_all_tiers() {
    let mut app = app_with_session();
    app.execute_slash_command("/router tiers").await;
    assert_eq!(app.status, "router: tiers");
    assert_ne!(last_message(&app), "");
}

#[tokio::test]
async fn weights_lists_dimensions() {
    let mut app = app_with_session();
    app.execute_slash_command("/router weights").await;
    assert_eq!(app.status, "router: weights");
    assert!(last_message(&app).contains("Weight"));
}

#[tokio::test]
async fn weights_override_is_not_supported() {
    let mut app = app_with_session();
    app.execute_slash_command("/router weights set complexity 0.5")
        .await;
    assert_eq!(app.status, "router: weights not supported");
}

#[tokio::test]
async fn boundaries_lists_thresholds() {
    let mut app = app_with_session();
    app.execute_slash_command("/router boundaries").await;
    assert_eq!(app.status, "router: boundaries");
    assert!(last_message(&app).contains("SIMPLE"));
}

#[tokio::test]
async fn boundaries_override_is_not_supported() {
    let mut app = app_with_session();
    app.execute_slash_command("/router boundaries set simple_medium 0.3")
        .await;
    assert_eq!(app.status, "router: boundaries not supported");
}

// ---------------------------------------------------------------------------
// test
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_without_prompt_shows_usage() {
    let mut app = app_with_session();
    let before = app.status.clone();
    app.execute_slash_command("/router test").await;
    // The empty-prompt arm prints usage and leaves the status untouched.
    assert_eq!(app.status, before, "empty /router test must not set status");
    assert!(last_message(&app).contains("Usage"));
}

#[tokio::test]
async fn test_with_prompt_classifies_and_records_tier() {
    let mut app = app_with_session();
    app.execute_slash_command("/router test summarise the release notes")
        .await;
    assert!(
        app.status.starts_with("router: test ->"),
        "expected a classified tier, got {}",
        app.status
    );
    assert!(app.router_current_tier.is_some());
}

// ---------------------------------------------------------------------------
// reload
// ---------------------------------------------------------------------------

#[tokio::test]
async fn reload_without_config_fails_cleanly() {
    // Construct the app first (which may materialise a default project config),
    // then remove every config file so reload finds neither project nor global.
    let (_guard, _env, _temp) = enter_project("{}");
    let mut app = app_with_session();
    std::fs::remove_file(
        std::env::current_dir()
            .expect("cwd")
            .join(".ragent")
            .join("ragent.json"),
    )
    .expect("remove project config");
    app.execute_slash_command("/router reload").await;
    assert_eq!(app.status, "router: reload failed");
}

#[tokio::test]
async fn reload_with_router_section_succeeds() {
    let config = r#"{
      "provider": {
        "router": { "enabled": true }
      }
    }"#;
    let (_guard, _env, _temp) = enter_project(config);
    let mut app = app_with_session();
    app.router_enabled = false;
    app.execute_slash_command("/router reload").await;
    assert_eq!(app.status, "router: reloaded");
    assert!(
        app.router_enabled,
        "reload must adopt the file's enabled flag"
    );
    assert!(app.router_current_tier.is_none(), "reload resets the tier");
}

#[tokio::test]
async fn reload_without_router_section_reports_no_config() {
    let config = r#"{ "provider": { "anthropic": {} } }"#;
    let (_guard, _env, _temp) = enter_project(config);
    let mut app = app_with_session();
    app.execute_slash_command("/router reload").await;
    assert_eq!(app.status, "router: no config");
}

#[tokio::test]
async fn reload_with_malformed_json_reports_parse_error() {
    let (_guard, _env, _temp) = enter_project("{ this is not json");
    let mut app = app_with_session();
    app.execute_slash_command("/router reload").await;
    assert_eq!(app.status, "router: reload parse error");
}

#[tokio::test]
async fn help_never_touches_config_state() {
    let mut app = app_with_session();
    app.router_enabled = true;
    app.execute_slash_command("/router help").await;
    assert!(app.router_enabled, "help must not change the enabled flag");
}
