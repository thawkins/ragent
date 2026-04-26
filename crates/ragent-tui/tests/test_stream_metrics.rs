//! Regression tests for status-bar context and stream byte metrics.

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
use ragent_tui::{
    App,
    app::{ConfiguredProvider, ProviderSource},
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
fn test_usage_display_shows_pct_then_context_window_size() {
    let mut app = make_app();
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama_cloud".to_string(),
        name: "Ollama Cloud".to_string(),
        source: ProviderSource::Database,
    });
    app.selected_model = Some("ollama_cloud/kimi-k2.6".to_string());
    app.selected_model_ctx_window = Some(200_000);
    app.last_input_tokens = 50_000;

    let (label, unknown) = app.usage_display();

    assert!(!unknown);
    assert_eq!(label, "ctx: 25% 50K/200K");
}

#[test]
fn test_request_started_resets_inbound_and_sets_outbound_bytes() {
    let mut app = make_app();
    app.session_id = Some("session-1".to_string());
    app.stream_in_bytes = 321;

    app.handle_event(Event::RequestStarted {
        session_id: "session-1".to_string(),
        outbound_bytes: 4096,
    });
    app.handle_event(Event::TextDelta {
        session_id: "session-1".to_string(),
        text: "hello".to_string(),
    });

    assert_eq!(app.stream_out_bytes, 4096);
    assert_eq!(app.stream_in_bytes, 5);
}
