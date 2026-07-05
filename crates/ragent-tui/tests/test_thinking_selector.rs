//! Tests for the model-picker thinking-level selector flow.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_tui::app::{ModelPickerEntry, ProviderSetupStep};
use ragent_types::{ThinkingConfig, ThinkingLevel};

#[path = "support/mod.rs"]
mod support;

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
    let mut app = support::make_app();
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
    let mut app = support::make_app();
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
