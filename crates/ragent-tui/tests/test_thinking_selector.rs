//! Tests for the model-picker thinking-level selector flow.

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
use ragent_tui::app::{ModelPickerEntry, ProviderSetupStep};
use ragent_types::{ThinkingConfig, ThinkingLevel};

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

fn reasoning_entry() -> ModelPickerEntry {
    ModelPickerEntry {
        id: "claude-sonnet-4-20250514".to_string(),
        name: "Claude Sonnet 4".to_string(),
        context_window: 200_000,
        max_output: Some(64_000),
        cost_input: 3.0,
        cost_output: 15.0,
        reasoning: true,
        vision: true,
        tool_use: true,
        thinking_levels: vec![
            ThinkingLevel::Auto,
            ThinkingLevel::Off,
            ThinkingLevel::Low,
            ThinkingLevel::Medium,
            ThinkingLevel::High,
        ],
        thinking_config: Some(ThinkingConfig::new(ThinkingLevel::Auto)),
        cost_tier: "Premium".to_string(),
        cost_multiplier: "1x".to_string(),
    }
}

#[test]
fn test_model_selection_opens_thinking_selector_for_reasoning_models() {
    let mut app = make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![reasoning_entry()],
        selected: 0,
    });

    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match app.provider_setup.as_ref() {
        Some(ProviderSetupStep::SelectThinkingLevel {
            provider_id,
            provider_name,
            model,
            selected,
        }) => {
            assert_eq!(provider_id, "anthropic");
            assert_eq!(provider_name, "Anthropic");
            assert_eq!(model.id, "claude-sonnet-4-20250514");
            assert_eq!(
                *selected, 0,
                "Model-configured thinking should be pre-selected by default"
            );
        }
        other => panic!("expected thinking selector, got {other:?}"),
    }
}

#[test]
fn test_thinking_selector_confirm_persists_selected_level() {
    let mut app = make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        model: reasoning_entry(),
        selected: 4,
    });

    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert_eq!(
        app.selected_model.as_deref(),
        Some("anthropic/claude-sonnet-4-20250514")
    );
    assert_eq!(app.selected_thinking_level, Some(ThinkingLevel::High));
    assert_eq!(
        app.storage
            .get_setting("thinking_level")
            .expect("thinking setting read"),
        Some("high".to_string())
    );
    match app.provider_setup.as_ref() {
        Some(ProviderSetupStep::Done {
            provider_name,
            model_name,
        }) => {
            assert_eq!(provider_name, "Anthropic");
            assert_eq!(model_name.as_deref(), Some("Claude Sonnet 4"));
        }
        other => panic!("expected done step, got {other:?}"),
    }
}
