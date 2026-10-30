//! Tests for the `/telemetry` slash-command family.
//!
//! These tests exercise the non-interactive subcommands (`help`, `counters`,
//! and input validation) without launching a full terminal backend.

#[path = "support/mod.rs"]
mod support;

use ragent_config::OtelProtocol;
use ragent_tui::App;
use ragent_tui::app::ProviderSetupStep;

fn last_message_text(app: &App) -> String {
    app.messages
        .last()
        .map(|m| {
            m.parts
                .iter()
                .filter_map(|p| match p {
                    ragent_agent::message::MessagePart::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<String>()
        })
        .unwrap_or_default()
}

/// A test [`App`] built on the shared crate-wide fixture.
fn make_app() -> App {
    support::make_app()
}

#[test]
fn test_telemetry_help_is_added_to_slash_commands() {
    use ragent_tui::app::SLASH_COMMANDS;
    assert!(SLASH_COMMANDS.iter().any(|cmd| cmd.trigger == "telemetry"));
}

#[tokio::test]
async fn test_telemetry_help_appends_help_message() {
    let mut app = make_app();
    app.execute_slash_command("/telemetry help").await;
    let last = last_message_text(&app);
    assert!(
        last.contains("/telemetry on"),
        "help should mention /telemetry on: {last}"
    );
    assert!(
        last.contains("/telemetry setup"),
        "help should mention /telemetry setup: {last}"
    );
    assert!(
        last.contains("/telemetry counters"),
        "help should mention /telemetry counters: {last}"
    );
}

#[tokio::test]
async fn test_telemetry_counters_appends_catalogue() {
    let mut app = make_app();
    app.execute_slash_command("/telemetry counters").await;
    let last = last_message_text(&app);
    assert!(
        last.contains("Usage metrics"),
        "counters should list Usage metrics: {last}"
    );
    assert!(
        last.contains("Performance metrics"),
        "counters should list Performance metrics: {last}"
    );
    assert!(
        last.contains("Cost metrics"),
        "counters should list Cost metrics: {last}"
    );
    assert!(
        last.contains("Effectiveness metrics"),
        "counters should list Effectiveness metrics: {last}"
    );
    assert!(
        last.contains("ragent.llm.requests"),
        "counters should include ragent.llm.requests: {last}"
    );
    assert!(
        last.contains("*Counter*"),
        "counters should include Counter type label: {last}"
    );
    assert!(
        last.contains("*UpDownCounter*"),
        "counters should include UpDownCounter type label: {last}"
    );
    assert!(
        last.contains("*Gauge*"),
        "counters should include Gauge type label: {last}"
    );
    assert!(
        last.contains("*Histogram*"),
        "counters should include Histogram type label: {last}"
    );
    assert!(
        last.contains("Traces"),
        "counters should mention Traces section: {last}"
    );
}

#[tokio::test]
async fn test_telemetry_setup_opens_dialog_with_defaults() {
    let mut app = make_app();
    app.execute_slash_command("/telemetry setup").await;
    let step = app
        .provider_setup
        .as_ref()
        .expect("setup should open provider_setup dialog");
    match step {
        ProviderSetupStep::TelemetrySetup {
            endpoint_field,
            protocol,
            interval_field,
            timeout_field,
            port_field,
            active_field,
            error,
        } => {
            assert_eq!(endpoint_field.text(), "http://localhost:4318");
            assert_eq!(*protocol, OtelProtocol::Http);
            assert_eq!(interval_field.text(), "30");
            assert_eq!(timeout_field.text(), "10");
            assert_eq!(port_field.text(), "");
            assert_eq!(*active_field, 0);
            assert!(error.is_none());
        }
        _ => panic!("expected TelemetrySetup dialog, got {step:?}"),
    }
}

#[tokio::test]
async fn test_telemetry_unknown_subcommand_shows_usage() {
    let mut app = make_app();
    app.execute_slash_command("/telemetry frobnicate").await;
    let last = last_message_text(&app);
    assert!(
        last.contains("Usage: `/telemetry help|on|off|setup|counters`"),
        "unknown subcommand should show usage: {last}"
    );
}

#[tokio::test]
async fn test_telemetry_panel_slash_command_toggles_panel() {
    let mut app = make_app();
    assert!(!app.show_telemetry);
    app.execute_slash_command("/telemetry_panel").await;
    assert!(app.show_telemetry);
    assert!(!app.show_log);
    assert!(!app.show_profile);
    assert!(!app.show_tasks_panel);
    assert!(!app.show_memory);
    assert_eq!(app.status, "telemetry panel visible");

    app.execute_slash_command("/telemetry_panel").await;
    assert!(!app.show_telemetry);
    assert_eq!(app.status, "telemetry panel hidden");
}

#[test]
fn test_telemetry_panel_is_in_slash_commands() {
    use ragent_tui::app::SLASH_COMMANDS;
    assert!(
        SLASH_COMMANDS
            .iter()
            .any(|cmd| cmd.trigger == "telemetry_panel")
    );
}
