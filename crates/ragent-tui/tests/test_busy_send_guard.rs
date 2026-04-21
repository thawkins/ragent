//! Regression tests for blocking new prompt submission while the app is busy.

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ragent_core::{
    agent,
    config::StreamConfig,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::{
    App,
    input::{InputAction, handle_key},
};

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
        stream_config: StreamConfig::default(),
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

#[test]
fn test_enter_is_ignored_while_processing() {
    let mut app = make_app();
    app.is_processing = true;
    app.input = "hello".to_string();

    let action = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(action.is_none(), "busy app should not emit a send action");
    assert_eq!(app.input, "hello");
    assert_eq!(app.status, "busy - wait for the current turn to finish");
}

#[test]
fn test_enter_still_submits_when_idle() {
    let mut app = make_app();
    app.input = "hello".to_string();

    let action = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match action {
        Some(InputAction::SendMessage(text)) => assert_eq!(text, "hello"),
        _ => panic!("expected SendMessage action"),
    }
}
