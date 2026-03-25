//! Tests for test_session_resume.rs

//! Tests for CLI session resume (TASK-007).
//!
//! Verifies that `App::load_session` correctly restores session state
//! including messages, screen mode, status bar, and log entries.

use std::sync::Arc;

use ragent_core::{
    agent,
    event::EventBus,
    message::Message,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;
use ragent_tui::app::{LogLevel, ScreenMode};

/// Build an [`App`] and its shared [`SessionManager`] backed by in-memory storage.
fn make_app_with_manager() -> (App, Arc<SessionManager>) {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager: session_manager.clone(),
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    let app = App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        agent_info,
        false,
    );
    (app, session_manager)
}

#[test]
fn test_load_session_restores_messages() {
    let (mut app, mgr) = make_app_with_manager();
    let dir = std::env::current_dir().unwrap_or_default();
    let session = mgr.create_session(dir).expect("create session");

    // Insert some messages
    let m1 = Message::user_text(&session.id, "Hello");
    let m2 = Message::user_text(&session.id, "World");
    mgr.storage().create_message(&m1).unwrap();
    mgr.storage().create_message(&m2).unwrap();

    // Load the session
    app.load_session(&session.id).unwrap();

    assert_eq!(app.messages.len(), 2, "should load 2 messages");
    assert_eq!(app.messages[0].text_content(), "Hello");
    assert_eq!(app.messages[1].text_content(), "World");
}

#[test]
fn test_load_session_sets_session_id() {
    let (mut app, mgr) = make_app_with_manager();
    let dir = std::env::current_dir().unwrap_or_default();
    let session = mgr.create_session(dir).expect("create session");

    assert!(app.session_id.is_none());
    app.load_session(&session.id).unwrap();

    assert_eq!(app.session_id.as_deref(), Some(session.id.as_str()));
}

#[test]
fn test_load_session_switches_to_chat_screen() {
    let (mut app, mgr) = make_app_with_manager();
    let dir = std::env::current_dir().unwrap_or_default();
    let session = mgr.create_session(dir).expect("create session");

    assert_eq!(app.current_screen, ScreenMode::Home);
    app.load_session(&session.id).unwrap();

    assert_eq!(app.current_screen, ScreenMode::Chat);
}

#[test]
fn test_load_session_updates_status() {
    let (mut app, mgr) = make_app_with_manager();
    let dir = std::env::current_dir().unwrap_or_default();
    let session = mgr.create_session(dir).expect("create session");

    let m1 = Message::user_text(&session.id, "test");
    mgr.storage().create_message(&m1).unwrap();

    app.load_session(&session.id).unwrap();

    assert!(
        app.status.contains("resumed"),
        "status should mention resumed: {}",
        app.status
    );
    assert!(
        app.status.contains("1 messages"),
        "status should mention message count: {}",
        app.status
    );
}

#[test]
fn test_load_session_pushes_log_entry() {
    let (mut app, mgr) = make_app_with_manager();
    let dir = std::env::current_dir().unwrap_or_default();
    let session = mgr.create_session(dir).expect("create session");

    app.load_session(&session.id).unwrap();

    assert_eq!(app.log_entries.len(), 1);
    assert_eq!(app.log_entries[0].level, LogLevel::Info);
    assert!(app.log_entries[0].message.contains("Resumed session"));
}

#[test]
fn test_load_session_unknown_id_returns_error() {
    let (mut app, _mgr) = make_app_with_manager();
    let result = app.load_session("nonexistent-session-id");

    assert!(result.is_err(), "should fail for unknown session");
    let err = result.unwrap_err().to_string();
    assert!(
        err.contains("not found"),
        "error should mention not found: {}",
        err
    );
}

#[test]
fn test_load_session_empty_history() {
    let (mut app, mgr) = make_app_with_manager();
    let dir = std::env::current_dir().unwrap_or_default();
    let session = mgr.create_session(dir).expect("create session");

    app.load_session(&session.id).unwrap();

    assert!(app.messages.is_empty(), "no messages in a fresh session");
    assert!(app.status.contains("0 messages"));
}

#[test]
fn test_load_session_updates_cwd() {
    let (mut app, mgr) = make_app_with_manager();
    let dir = std::path::PathBuf::from("/tmp/test-project");
    let session = mgr.create_session(dir).expect("create session");

    app.load_session(&session.id).unwrap();

    assert_eq!(app.cwd, "/tmp/test-project");
}

#[test]
fn test_load_session_preserves_agent() {
    let (mut app, mgr) = make_app_with_manager();
    let dir = std::env::current_dir().unwrap_or_default();
    let session = mgr.create_session(dir).expect("create session");

    let original_agent = app.agent_name.clone();
    app.load_session(&session.id).unwrap();

    assert_eq!(
        app.agent_name, original_agent,
        "agent should not change on resume"
    );
}
