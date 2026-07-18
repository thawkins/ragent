//! Tests for the `/telemetry` slash-command family.
//!
//! These tests exercise the non-interactive subcommands (`help`, `counters`,
//! and input validation) without launching a full terminal backend.

use std::collections::HashMap;
use std::sync::Arc;

use ragent_agent::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
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

fn make_app() -> App {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let event_bus = Arc::new(EventBus::default());
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(HashMap::new())),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        agent_info,
        false,
        std::path::PathBuf::new(),
    )
}

#[test]
fn test_telemetry_help_is_added_to_slash_commands() {
    use ragent_tui::app::SLASH_COMMANDS;
    assert!(SLASH_COMMANDS.iter().any(|cmd| cmd.trigger == "telemetry"));
}

#[test]
fn test_telemetry_help_appends_help_message() {
    let mut app = make_app();
    app.execute_slash_command("/telemetry help");
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

#[test]
fn test_telemetry_counters_appends_catalogue() {
    let mut app = make_app();
    app.execute_slash_command("/telemetry counters");
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

#[test]
fn test_telemetry_setup_opens_dialog_with_defaults() {
    let mut app = make_app();
    app.execute_slash_command("/telemetry setup");
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

#[test]
fn test_telemetry_unknown_subcommand_shows_usage() {
    let mut app = make_app();
    app.execute_slash_command("/telemetry frobnicate");
    let last = last_message_text(&app);
    assert!(
        last.contains("Usage: `/telemetry help|on|off|setup|counters`"),
        "unknown subcommand should show usage: {last}"
    );
}

#[test]
fn test_telemetry_panel_slash_command_toggles_panel() {
    let mut app = make_app();
    assert!(!app.show_telemetry);
    app.execute_slash_command("/telemetry_panel");
    assert!(app.show_telemetry);
    assert!(!app.show_log);
    assert!(!app.show_profile);
    assert!(!app.show_todo);
    assert!(!app.show_memory);
    assert_eq!(app.status, "telemetry panel visible");

    app.execute_slash_command("/telemetry_panel");
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
