//! Tests for multiple-choice question dialog support.

use std::sync::Arc;

use ragent_core::{
    agent,
    config::StreamConfig,
    event::{Event, EventBus},
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::{App, layout};
use ratatui::{Terminal, backend::TestBackend};

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
        true,
    )
}

fn render_app_to_string(app: &mut App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render dialog");

    let backend = terminal.backend();
    let buffer = backend.buffer();
    let mut text = String::new();
    let area = buffer.area();
    for y in 0..area.height {
        for x in 0..area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

#[test]
fn test_multiple_choice_question_renders_options() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::QuestionRequested {
        session_id: "s1".to_string(),
        request_id: "r1".to_string(),
        question: "Which provider?".to_string(),
        options: vec![
            "anthropic".to_string(),
            "openai".to_string(),
            "ollama".to_string(),
        ],
    });

    let text = render_app_to_string(&mut app);

    // The question text should be visible.
    assert!(text.contains("Which provider?"));

    // All three options should be rendered.
    assert!(text.contains("anthropic"));
    assert!(text.contains("openai"));
    assert!(text.contains("ollama"));

    // Navigation hint should be visible.
    assert!(text.contains("to navigate"));
}

#[test]
fn test_multiple_choice_question_shows_selected_option() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::QuestionRequested {
        session_id: "s1".to_string(),
        request_id: "r1".to_string(),
        question: "Pick one".to_string(),
        options: vec!["A".to_string(), "B".to_string()],
    });

    let text = render_app_to_string(&mut app);

    // First option should have the selection indicator (▶).
    assert!(text.contains("▶ A"));
}

#[test]
fn test_multiple_choice_selection_navigates_down() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::QuestionRequested {
        session_id: "s1".to_string(),
        request_id: "r1".to_string(),
        question: "Pick".to_string(),
        options: vec!["first".to_string(), "second".to_string()],
    });

    // Simulate pressing Down (Char('j')).
    let _ = ragent_tui::input::handle_key(
        &mut app,
        crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Char('j')),
    );

    let text = render_app_to_string(&mut app);

    // Second option should now be selected.
    assert!(text.contains("▶ second"));
}

#[test]
fn test_free_text_question_does_not_show_options() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::QuestionRequested {
        session_id: "s1".to_string(),
        request_id: "r1".to_string(),
        question: "Type your answer".to_string(),
        options: vec![],
    });

    let text = render_app_to_string(&mut app);

    // Should show the free-text input hint.
    assert!(text.contains("Enter to submit"));
    assert!(!text.contains("↑/↓"));
}
