//! Regression tests for slash-menu escape handling.

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_core::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;
use ragent_tui::input;

fn make_app() -> App {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_core::config::StreamConfig::default(),
        auto_approve: false,
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
    )
}

fn esc_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
}

#[test]
fn test_slash_menu_escape_closes_menu_and_preserves_input_cursor() {
    let mut app = make_app();
    app.input = "/mod".to_string();
    app.input_cursor = app.input.chars().count();
    app.update_slash_menu();

    assert!(app.slash_menu.is_some());

    let action = input::handle_key(&mut app, esc_key());

    assert!(action.is_none());
    assert!(app.slash_menu.is_none());
    assert_eq!(app.input, "/mod");
    assert_eq!(app.input_cursor, app.input_len_chars());
}

#[test]
fn test_slash_menu_escape_with_invalid_cursor_clamps_to_input_length() {
    let mut app = make_app();
    app.input = "/mod".to_string();
    app.input_cursor = 10;
    app.update_slash_menu();

    assert!(app.slash_menu.is_some());
    assert!(app.input_cursor > app.input_len_chars());

    let action = input::handle_key(&mut app, esc_key());

    assert!(action.is_none());
    assert!(app.slash_menu.is_none());
    assert_eq!(app.input, "/mod");
    assert_eq!(app.input_cursor, app.input_len_chars());
}
