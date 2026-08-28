//! Tests for the `/cron add` slash command positional parameter parsing.
//!
//! Verifies the positional form: `/cron add <cronname> <agent> <schedule> "<prompt>"`.
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
use ragent_tui::App;

/// Build an [`App`] backed by an in-memory database.
fn make_app() -> App {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    make_app_with_storage(storage)
}

#[allow(clippy::type_complexity)]
fn make_app_with_storage(storage: Arc<Storage>) -> App {
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
        agent_manager: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
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
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        skill_registry_cache: parking_lot::Mutex::new(None),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        Arc::unwrap_or_clone(agent_info),
        false,
        std::path::PathBuf::new(),
    )
}

/// Return the text of the last assistant message appended to the app.
fn last_text(app: &App) -> String {
    app.messages.last().unwrap().text_content()
}

#[test]
fn test_cron_add_with_named_id_every_repeat() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron add nightly general every 30m \"Run tests\"");
    assert_eq!(app.status, "cron: added");
    let out = last_text(&app);
    assert!(out.contains("`nightly`"), "ID should be nightly: {out}");
    assert!(out.contains("`general`"), "agent should be general: {out}");
    assert!(out.contains("Run tests"), "prompt should be present: {out}");
    // Stored event uses the cronname as id.
    let row = app
        .storage
        .get_cron_event("nightly")
        .expect("query")
        .expect("event should exist");
    assert_eq!(row.agent_type, "general");
    assert_eq!(row.prompt, "Run tests");
}

#[test]
fn test_cron_add_with_named_id_from_repeat() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command(
        "/cron add morning general from 2025-01-15T09:00:00Z every 30m \"Run tests\"",
    );
    assert_eq!(app.status, "cron: added");
    let out = last_text(&app);
    assert!(out.contains("`morning`"), "ID should be morning: {out}");
    assert!(out.contains("2025-01-15T09:00:00Z"), "schedule: {out}");
    let row = app
        .storage
        .get_cron_event("morning")
        .expect("query")
        .expect("event should exist");
    assert_eq!(row.schedule_raw, "from 2025-01-15T09:00:00Z every 30m");
}

#[test]
fn test_cron_add_missing_cronname_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    // No cronname — prompt is quoted, so before_prompt has at most 1 token.
    app.execute_slash_command("/cron add general \"Run tests\"");
    assert_eq!(app.status, "cron: add missing agent");
    let out = last_text(&app);
    assert!(
        out.contains("Missing agent and schedule expression"),
        "msg: {out}"
    );
}

#[test]
fn test_cron_add_missing_schedule_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron add nightly general \"Run tests\"");
    assert_eq!(app.status, "cron: add missing schedule");
    let out = last_text(&app);
    assert!(out.contains("Missing schedule expression"), "msg: {out}");
}

#[test]
fn test_cron_add_missing_prompt_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron add nightly general every 30m Run tests");
    assert_eq!(app.status, "cron: add missing prompt");
    let out = last_text(&app);
    assert!(
        out.contains("prompt must be enclosed in double quotes"),
        "msg: {out}"
    );
}

#[test]
fn test_cron_add_empty_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron add");
    assert_eq!(app.status, "cron: add usage");
    let out = last_text(&app);
    assert!(
        out.contains("Usage: `/cron add <cronname> <agent> <schedule>"),
        "msg: {out}"
    );
}

#[test]
fn test_cron_add_uses_cronname_as_id_not_autogenerated() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron add myid general every 1h \"hello\"");
    let out = last_text(&app);
    // The ID must be exactly the supplied cronname, not cron-<sessionid>.
    assert!(out.contains("`myid`"), "ID should be myid: {out}");
    assert!(
        !out.contains("cron-"),
        "ID should not be auto-generated: {out}"
    );
}

#[test]
fn test_cron_help_shows_positional_params() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron help");
    assert_eq!(app.status, "cron: help");
    let out = last_text(&app);
    assert!(
        out.contains("`add` parameters (positional)"),
        "help table: {out}"
    );
    assert!(
        out.contains("`cronname` | Sets the event ID"),
        "help table: {out}"
    );
}

#[test]
fn test_cron_add_natural_time_5pm() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron add evening general at 5pm \"Run tests\"");
    assert_eq!(app.status, "cron: added");
    let out = last_text(&app);
    assert!(out.contains("`evening`"), "ID should be evening: {out}");
    let row = app
        .storage
        .get_cron_event("evening")
        .expect("query")
        .expect("event should exist");
    assert_eq!(row.schedule_raw, "at 5pm");
}

#[test]
fn test_cron_add_natural_time_5_30pm() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron add evening general at 5:30pm \"Run tests\"");
    assert_eq!(app.status, "cron: added");
    let out = last_text(&app);
    assert!(out.contains("`evening`"), "ID should be evening: {out}");
    let row = app
        .storage
        .get_cron_event("evening")
        .expect("query")
        .expect("event should exist");
    assert_eq!(row.schedule_raw, "at 5:30pm");
}

#[test]
fn test_cron_add_natural_time_24_hour() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron add evening general at 17:00 \"Run tests\"");
    assert_eq!(app.status, "cron: added");
    let row = app
        .storage
        .get_cron_event("evening")
        .expect("query")
        .expect("event should exist");
    assert_eq!(row.schedule_raw, "at 17:00");
}

#[test]
fn test_cron_add_natural_time_from_5pm_every_1h() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron add evening general from 5pm every 1h \"Run tests\"");
    assert_eq!(app.status, "cron: added");
    let out = last_text(&app);
    assert!(out.contains("`evening`"), "ID should be evening: {out}");
    let row = app
        .storage
        .get_cron_event("evening")
        .expect("query")
        .expect("event should exist");
    assert_eq!(row.schedule_raw, "from 5pm every 1h");
}

#[test]
fn test_cron_add_natural_time_5pm_tomorrow() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron add evening general at 5pm tomorrow \"Run tests\"");
    assert_eq!(app.status, "cron: added");
    let row = app
        .storage
        .get_cron_event("evening")
        .expect("query")
        .expect("event should exist");
    assert_eq!(row.schedule_raw, "at 5pm tomorrow");
}

#[test]
fn test_cron_add_natural_time_invalid_time() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron add evening general at 13pm \"Run tests\"");
    assert_eq!(app.status, "cron: add parse error");
    let out = last_text(&app);
    assert!(
        out.contains("Failed to parse schedule"),
        "should show parse error: {out}"
    );
}

#[test]
fn test_cron_help_shows_natural_language_timestamps() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron help");
    assert_eq!(app.status, "cron: help");
    let out = last_text(&app);
    assert!(
        out.contains("natural-language shortcuts"),
        "help should mention natural-language shortcuts: {out}"
    );
}
#[test]
fn test_cron_detail_shows_full_prompt() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    let long_prompt =
        "This is a very long prompt that exceeds the 40 character preview limit used by list.";
    app.execute_slash_command(&format!(
        "/cron add nightly general every 30m \"{long_prompt}\""
    ));
    assert_eq!(app.status, "cron: added");

    app.execute_slash_command("/cron detail nightly");
    assert_eq!(app.status, "cron: detail");
    let out = last_text(&app);
    assert!(out.contains("`nightly`"), "should show ID: {out}");
    assert!(out.contains("`general`"), "should show agent: {out}");
    assert!(out.contains("every 30m"), "should show schedule: {out}");
    // The full prompt must appear, not a truncated preview.
    assert!(
        out.contains(long_prompt),
        "should contain the full untruncated prompt: {out}"
    );
    assert!(
        !out.contains("…"),
        "should not contain truncation marker: {out}"
    );
}

#[test]
fn test_cron_detail_not_found() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron detail nonexistent");
    assert_eq!(app.status, "cron: detail not found");
    let out = last_text(&app);
    assert!(out.contains("not found"), "should report not found: {out}");
}

#[test]
fn test_cron_detail_no_id_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron detail");
    assert_eq!(app.status, "cron: detail usage");
    let out = last_text(&app);
    assert!(
        out.contains("Usage: `/cron detail <event_id>`"),
        "should show usage: {out}"
    );
}

#[test]
fn test_cron_help_lists_detail_subcommand() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron help");
    let out = last_text(&app);
    assert!(
        out.contains("`detail`"),
        "help should list the detail sub-command: {out}"
    );
    assert!(
        out.contains("/cron detail <event_id>"),
        "help should show detail usage: {out}"
    );
}

#[test]
fn test_cron_disable_then_enable() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron add nightly general every 30m \"Run tests\"");
    assert_eq!(app.status, "cron: added");

    // Should start enabled.
    let row = app
        .storage
        .get_cron_event("nightly")
        .expect("query")
        .expect("event should exist");
    assert!(row.enabled, "new event should be enabled by default");

    // Disable it.
    app.execute_slash_command("/cron disable nightly");
    assert_eq!(app.status, "cron: disabled");
    let out = last_text(&app);
    assert!(out.contains("disabled"), "should say disabled: {out}");
    let row = app
        .storage
        .get_cron_event("nightly")
        .expect("query")
        .expect("event should exist");
    assert!(!row.enabled, "event should be disabled after /cron disable");

    // Re-enable it.
    app.execute_slash_command("/cron enable nightly");
    assert_eq!(app.status, "cron: enabled");
    let out = last_text(&app);
    assert!(out.contains("enabled"), "should say enabled: {out}");
    let row = app
        .storage
        .get_cron_event("nightly")
        .expect("query")
        .expect("event should exist");
    assert!(row.enabled, "event should be enabled after /cron enable");
}

#[test]
fn test_cron_enable_not_found() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron enable nonexistent");
    assert_eq!(app.status, "cron: not found");
    let out = last_text(&app);
    assert!(out.contains("not found"), "should report not found: {out}");
}

#[test]
fn test_cron_disable_no_id_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron disable");
    assert_eq!(app.status, "cron: disable usage");
    let out = last_text(&app);
    assert!(
        out.contains("Usage: `/cron disable <event_id>`"),
        "should show usage: {out}"
    );
}

#[test]
fn test_cron_enable_no_id_shows_usage() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron enable");
    assert_eq!(app.status, "cron: enable usage");
    let out = last_text(&app);
    assert!(
        out.contains("Usage: `/cron enable <event_id>`"),
        "should show usage: {out}"
    );
}

#[test]
fn test_cron_help_lists_enable_disable() {
    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.execute_slash_command("/cron help");
    let out = last_text(&app);
    assert!(
        out.contains("`enable`"),
        "help should list the enable sub-command: {out}"
    );
    assert!(
        out.contains("/cron enable <event_id>"),
        "help should show enable usage: {out}"
    );
    assert!(
        out.contains("`disable`"),
        "help should list the disable sub-command: {out}"
    );
    assert!(
        out.contains("/cron disable <event_id>"),
        "help should show disable usage: {out}"
    );
}
