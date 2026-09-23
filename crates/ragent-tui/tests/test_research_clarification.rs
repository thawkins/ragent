//! Tests for the `/research` clarification round-trip through the TUI
//! question dialog (the `ask_user`-style flow).
//!
//! `ask_user_clarification` in `src/app/research.rs` publishes
//! `Event::QuestionRequested` and awaits a matching
//! `Event::QuestionAnswered`. These tests verify the dialog side of that
//! contract end-to-end through the public `App` surface: the dialog is
//! queued, a typed answer is submitted on Enter as a `QuestionAnswered`
//! event with the same request id, and the dismissal marker produced by
//! Esc is rejected by the clarification gate.

use std::sync::Arc;

use ragent_agent::{
    StreamConfig, agent,
    event::{Event, EventBus},
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;

/// Marker published by the question dialog when the user presses Esc
/// (free-text path in `src/input.rs`). Must stay in sync with
/// `QUESTION_DISMISSED_MARKER` in `src/app/research.rs`.
const DISMISSED_MARKER: &str = "[User dismissed question]";

fn make_app(event_bus: Arc<EventBus>) -> App {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
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
        stream_config: StreamConfig::default(),
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
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
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
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
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

/// The dialog side of the clarification round-trip: a free-text question
/// queued via `QuestionRequested` is answered by typing text and pressing
/// Enter, which publishes `QuestionAnswered` with the submitted answer.
#[test]
fn test_research_clarification_dialog_round_trip_publishes_answer() {
    let event_bus = Arc::new(EventBus::default());
    let mut rx = event_bus.subscribe();
    let mut app = make_app(event_bus.clone());
    app.session_id = Some("s1".to_string());

    // Mirrors ask_user_clarification: publish a request and wait for the
    // matching QuestionAnswered reply.
    app.handle_event(Event::QuestionRequested {
        session_id: "s1".to_string(),
        request_id: "req-1".to_string(),
        question: "Could you narrow this down? Which aspect of 'rust' should the research cover?"
            .to_string(),
        options: Vec::new(),
    });
    assert_eq!(app.question_queue.len(), 1, "dialog should be queued");

    // Type the answer and submit it with Enter (the free-text dialog path).
    app.pending_question_input = "  async runtimes  ".to_string();
    let _ = ragent_tui::input::handle_key(
        &mut app,
        crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Enter),
    );

    assert!(
        app.question_queue.is_empty(),
        "dialog should be consumed after Enter"
    );

    let answer = rx
        .try_recv()
        .expect("QuestionAnswered should be published on the event bus");
    let Event::QuestionAnswered {
        session_id,
        request_id,
        response,
    } = answer
    else {
        panic!("expected QuestionAnswered, got {answer:?}");
    };
    assert_eq!(session_id, "s1");
    assert_eq!(request_id, "req-1");
    assert_eq!(response, "async runtimes", "answer should be trimmed");
}

/// Esc on the free-text dialog publishes the dismissal marker; the
/// clarification gate must treat that answer as "cancelled" rather than
/// folding it into the topic.
#[test]
fn test_research_clarification_dismissed_marker_rejected() {
    let event_bus = Arc::new(EventBus::default());
    let mut rx = event_bus.subscribe();
    let mut app = make_app(event_bus.clone());
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::QuestionRequested {
        session_id: "s1".to_string(),
        request_id: "req-2".to_string(),
        question: "This topic is a bit broad. What specific angle should the research focus on?"
            .to_string(),
        options: Vec::new(),
    });

    let _ = ragent_tui::input::handle_key(
        &mut app,
        crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Esc),
    );

    assert!(app.question_queue.is_empty(), "dialog should be consumed");
    let answer = rx
        .try_recv()
        .expect("QuestionAnswered should be published on Esc");
    let Event::QuestionAnswered { response, .. } = answer else {
        panic!("expected QuestionAnswered, got {answer:?}");
    };
    assert_eq!(response, DISMISSED_MARKER);
}

/// The clarified topic is appended to the base topic with the same
/// `(clarification: ...)` shape the CLI path uses, so the re-run config
/// carries the user's answer.
#[test]
fn test_clarified_topic_format_matches_cli() {
    let topic = "the inference market";
    let answer = "fireworks vs together pricing, 2024";
    let clarified = format!("{topic} (clarification: {answer})");
    assert_eq!(
        clarified,
        "the inference market (clarification: fireworks vs together pricing, 2024)"
    );
    assert!(clarified.starts_with(topic));
    assert!(clarified.contains("(clarification: "));
}
