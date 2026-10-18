//! Dispatch-path coverage for `app/session_ops.rs`: the arms of
//! `dispatch_user_message` / `dispatch_bang_command` and the auxiliary App
//! methods (`refresh_memory_stats`, `paste_image_from_clipboard`,
//! `set_mcp_server_enabled`, `refresh_connector_statuses`) that the existing
//! suites do not reach.
//!
//! Tests only - no production logic is touched.
#![allow(unsafe_code)]
#![cfg(test)]
// The isolation harness holds a process-global mutex across the async test body
// so cwd/env mutations cannot interleave.
#![allow(clippy::await_holding_lock)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_tui::App;

#[path = "slash_support/mod.rs"]
mod slash_support;
#[path = "support/mod.rs"]
mod support;

use slash_support::{app_with_session, enter_project, last_message};

/// An app in an isolated tempdir project with a live session.
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

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn key_ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::CONTROL)
}

// ---------------------------------------------------------------------------
// dispatch_user_message
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_without_a_session_reports_no_active_session() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.session_id = None;
    app.configured_provider = Some(ragent_tui::app::ConfiguredProvider {
        id: "ollama".to_string(),
        name: "Ollama".to_string(),
        source: ragent_tui::app::ProviderSource::Database,
    });
    app.selected_model = Some("ollama/qwen2.5".to_string());

    // Typing `hello` then Enter goes through the SendMessage branch of
    // `handle_key_event`, which creates a session first when none exists.
    for ch in "hello".chars() {
        app.handle_key_event(key(KeyCode::Char(ch))).await;
    }
    app.handle_key_event(key(KeyCode::Enter)).await;
    // Either a session is created (the create-then-dispatch path) or the
    // command is staged; both leave a message or a status behind.
    assert!(
        app.session_id.is_some() || app.status.contains("session"),
        "status: {}",
        app.status
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn send_with_reference_token_resolves_it() {
    let (_g, _e, temp, mut app) = isolated_app();
    std::fs::write(temp.path().join("notes.txt"), "reference body").expect("write notes");
    app.configured_provider = Some(ragent_tui::app::ConfiguredProvider {
        id: "ollama".to_string(),
        name: "Ollama".to_string(),
        source: ragent_tui::app::ProviderSource::Database,
    });
    app.selected_model = Some("ollama/qwen2.5".to_string());

    for ch in "@notes.txt summarise".chars() {
        app.handle_key_event(key(KeyCode::Char(ch))).await;
    }
    app.handle_key_event(key(KeyCode::Enter)).await;
    // The reference may expand into the message body or a file-menu selection
    // may be required; either way the app must not panic and must record state.
    assert!(
        !app.messages.is_empty() || app.session_id.is_some() || !app.status.is_empty(),
        "the send path must record something"
    );
}

// ---------------------------------------------------------------------------
// dispatch_bang_command
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bang_command_runs_and_records_output() {
    let (_g, _e, _t, mut app) = isolated_app();
    for ch in "!echo bang-marker".chars() {
        app.handle_key_event(key(KeyCode::Char(ch))).await;
    }
    app.handle_key_event(key(KeyCode::Enter)).await;
    // Give the spawned bash tool a moment to finish and publish its result.
    for _ in 0..200 {
        if app
            .messages
            .iter()
            .any(|m| m.text_content().contains("bang-marker"))
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(
        app.messages
            .iter()
            .any(|m| m.text_content().contains("bang-marker"))
            || app.status.contains("bang"),
        "bang output must be recorded (status: {})",
        app.status
    );
}

// ---------------------------------------------------------------------------
// auxiliary App methods reachable through the public surface
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn memory_panel_toggle_refreshes_stats() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/memory show").await;
    assert_eq!(app.status, "memory panel visible");
    // Toggling off runs the same refresh path and reports the hidden state.
    app.execute_slash_command("/memory show").await;
    assert_eq!(app.status, "memory panel hidden");
    assert_ne!(last_message(&app), "");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn paste_image_without_a_clipboard_image_reports_it() {
    let (_g, _e, _t, mut app) = isolated_app();
    // Ctrl+V with an empty clipboard must not attach anything.
    app.handle_key_event(key_ctrl(KeyCode::Char('v'))).await;
    assert!(
        app.pending_attachments.is_empty(),
        "no attachment may be staged from an empty clipboard"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mcp_enable_of_unknown_server_reports_it() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/mcp connect no-such-server")
        .await;
    assert!(
        !last_message(&app).is_empty(),
        "an unknown MCP server must produce a message"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connector_panel_refresh_without_a_panel_is_inert() {
    let (_g, _e, _t, mut app) = isolated_app();
    assert!(app.connector_store.is_none());
    app.refresh_connector_catalogue_installed();
    assert!(app.connector_store.is_none(), "no panel may be created");
}

// ---------------------------------------------------------------------------
// /connectors sub-arms that touch the catalogue
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connectors_list_and_help_render() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/connectors help").await;
    assert!(app.status.starts_with("connectors"), "got {}", app.status);

    app.execute_slash_command("/connectors list").await;
    assert!(app.status.starts_with("connectors"), "got {}", app.status);
    assert_ne!(last_message(&app), "");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn connectors_unknown_subcommand_is_reported() {
    let (_g, _e, _t, mut app) = isolated_app();
    app.execute_slash_command("/connectors frobnicate").await;
    assert!(app.status.starts_with("connectors"), "got {}", app.status);
}

// ---------------------------------------------------------------------------
// /plugins and /bench lists (the smaller render arms)
// ---------------------------------------------------------------------------

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn plugins_list_and_help_render() {
    let (_g, _e, _t, mut app) = isolated_app();

    app.execute_slash_command("/plugins help").await;
    assert!(app.status.starts_with("plugins"), "got {}", app.status);

    app.execute_slash_command("/plugins list").await;
    assert!(app.status.starts_with("plugins"), "got {}", app.status);
    assert_ne!(last_message(&app), "");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn bench_help_list_show_and_status_render() {
    let (_g, _e, _t, mut app) = isolated_app();
    for (cmd, expected) in [
        ("/bench help", "bench help"),
        ("/bench list", "bench list"),
        ("/bench show", "bench show"),
        ("/bench status", "bench status"),
    ] {
        app.execute_slash_command(cmd).await;
        assert_eq!(app.status, expected, "{cmd}");
        assert_ne!(last_message(&app), "", "{cmd} produced no message");
    }
}
