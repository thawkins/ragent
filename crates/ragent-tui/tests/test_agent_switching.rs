//! Tests for test_agent_switching.rs

//! Tests for TUI agent switching (TASK-005).
//!
//! Verifies that cycling through agents updates the agent name, status bar,
//! log entries, and publishes `AgentSwitched` events when a session is active.

use std::sync::Arc;

use ragent_core::{
    agent,
    event::{Event, EventBus},
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;

/// Build an [`App`] backed by an in-memory database.
fn make_app(event_bus: Arc<EventBus>) -> App {
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

/// Helper: simulate a `SwitchAgent` action via a Tab key press.
fn press_tab(app: &mut App) {
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    let key = KeyEvent::new(KeyCode::Tab, KeyModifiers::empty());
    app.handle_key_event(key);
}

#[test]
fn test_agent_switch_cycles_through_agents() {
    let bus = Arc::new(EventBus::default());
    let mut app = make_app(bus);

    let agent_count = app.cycleable_agents.len();
    assert!(agent_count > 1, "need at least two cycleable agents");

    let first = app.agent_name.clone();
    press_tab(&mut app);
    let second = app.agent_name.clone();
    assert_ne!(first, second, "agent should change on Tab");

    // Cycle back to the first agent
    for _ in 1..agent_count {
        press_tab(&mut app);
    }
    assert_eq!(app.agent_name, first, "should wrap around to first agent");
}

#[test]
fn test_agent_switch_updates_status_bar() {
    let bus = Arc::new(EventBus::default());
    let mut app = make_app(bus);

    press_tab(&mut app);
    assert!(
        app.status.starts_with("agent: "),
        "status bar should show agent name, got: {}",
        app.status
    );
    assert!(
        app.status.contains(&app.agent_name),
        "status bar should contain the new agent name"
    );
}

#[test]
fn test_agent_switch_pushes_log_entry() {
    let bus = Arc::new(EventBus::default());
    let mut app = make_app(bus);

    assert!(app.log_entries.is_empty(), "log should be empty initially");

    press_tab(&mut app);
    assert_eq!(app.log_entries.len(), 1, "one log entry expected");

    let entry = &app.log_entries[0];
    assert!(
        entry.message.starts_with("Switched to:"),
        "log message should start with 'Switched to:', got: {}",
        entry.message
    );
    assert!(
        entry.message.contains(&app.agent_name),
        "log message should contain agent name"
    );
}

#[test]
fn test_agent_switch_publishes_event_with_session() {
    let bus = Arc::new(EventBus::default());
    let mut rx = bus.subscribe();
    let mut app = make_app(bus);

    // Set a session ID so the event gets published
    app.session_id = Some("test-session-1".to_string());

    let prev_name = app.agent_name.clone();
    press_tab(&mut app);
    let new_name = app.agent_name.clone();

    // Drain the event
    let event = rx.try_recv().expect("should receive AgentSwitched event");
    match event {
        Event::AgentSwitched {
            session_id,
            from,
            to,
        } => {
            assert_eq!(session_id, "test-session-1");
            assert_eq!(from, prev_name);
            assert_eq!(to, new_name);
        }
        other => panic!("expected AgentSwitched, got: {:?}", other),
    }
}

#[test]
fn test_agent_switch_no_event_without_session() {
    let bus = Arc::new(EventBus::default());
    let mut rx = bus.subscribe();
    let mut app = make_app(bus);

    // No session set — should not publish
    assert!(app.session_id.is_none());
    press_tab(&mut app);

    // No event should be received
    assert!(
        rx.try_recv().is_err(),
        "no event should be published without an active session"
    );
}

#[test]
fn test_agent_switch_updates_agent_info() {
    let bus = Arc::new(EventBus::default());
    let mut app = make_app(bus);

    let initial_agent = app.agent_info.clone();
    press_tab(&mut app);

    assert_ne!(
        app.agent_info.name, initial_agent.name,
        "agent_info should change"
    );
    assert_eq!(
        app.agent_info.name, app.agent_name,
        "agent_info.name and agent_name should match"
    );
}

#[test]
fn test_agent_switch_index_tracking() {
    let bus = Arc::new(EventBus::default());
    let mut app = make_app(bus);

    let initial_index = app.current_agent_index;
    press_tab(&mut app);

    let expected = (initial_index + 1) % app.cycleable_agents.len();
    assert_eq!(
        app.current_agent_index, expected,
        "index should increment with wrap"
    );
}

#[test]
fn test_agent_switch_full_cycle_names() {
    let bus = Arc::new(EventBus::default());
    let mut app = make_app(bus);

    let count = app.cycleable_agents.len();
    let expected_names: Vec<String> = app
        .cycleable_agents
        .iter()
        .map(|a| a.name.clone())
        .collect();

    // Cycle through starting from current index
    let start = app.current_agent_index;
    let mut seen = Vec::new();
    for i in 0..count {
        let idx = (start + i) % count;
        assert_eq!(app.agent_name, expected_names[idx]);
        seen.push(app.agent_name.clone());
        press_tab(&mut app);
    }

    // After full cycle, back to start
    assert_eq!(app.agent_name, expected_names[start]);
    // All cycleable agents should have been visited
    for name in &expected_names {
        assert!(seen.contains(name), "agent '{}' was not visited", name);
    }
}

#[test]
fn test_cycleable_agents_excludes_hidden() {
    let bus = Arc::new(EventBus::default());
    let app = make_app(bus);

    let all_agents = agent::create_builtin_agents();
    assert!(
        all_agents.iter().any(|a| a.hidden),
        "should have hidden agents"
    );

    for agent in &app.cycleable_agents {
        assert!(
            !agent.hidden,
            "cycleable agents should not contain hidden agent '{}'",
            agent.name
        );
    }
}
