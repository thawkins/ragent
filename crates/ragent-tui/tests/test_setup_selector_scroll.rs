//! Tests that every model/provider selector scrolls to keep its cursor on screen.
//!
//! The setup dialogs share one rule ([`ragent_tui::layout`]'s window helpers):
//! the painted slice starts where it must so the highlighted row is visible, and
//! a clipped list says so in its legend. These tests render through the real
//! `layout::render` entry point at terminal sizes small enough to clip, so a
//! selector that silently hides its tail (the regression this guards) fails on
//! what the user actually sees.
//!
//! They also pin the wheel path: a notch over the open dialog steps the cursor
//! through the same handler the keyboard uses, and a notch outside it is
//! swallowed so nothing behind the modal scrolls.

use crossterm::event::{KeyModifiers, MouseEvent, MouseEventKind};
use ragent_llm::providers::router_config::{RouterConfig, Tier, TierConfig, TierEntry};
use ragent_tui::App;
use ragent_tui::app::{ConfiguredProvider, ModelPickerEntry, ProviderSetupStep, ProviderSource};
use ragent_types::{ThinkingConfig, ThinkingLevel};
use ratatui::{Terminal, backend::TestBackend};

#[path = "support/mod.rs"]
mod support;

fn entry(id: &str, name: &str) -> ModelPickerEntry {
    ModelPickerEntry {
        provider_id: "anthropic".to_string(),
        id: id.to_string(),
        name: name.to_string(),
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

fn painted(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw");
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(ratatui::buffer::Cell::symbol)
        .collect()
}

fn configured(id: &str) -> ConfiguredProvider {
    ConfiguredProvider {
        id: id.to_string(),
        name: format!("provider-{id:02}"),
        source: ProviderSource::Database,
    }
}

fn wheel(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

#[test]
fn test_azure_resource_list_scrolls_to_the_selected_deployment() {
    let mut app = support::make_app();
    let entries: Vec<_> = (0..40)
        .map(
            |i| ragent_llm::providers::azure_resource::AzureResourceEntry {
                id: format!("res-{i}"),
                name: format!("deployment-{i:02}"),
                endpoint: "https://example.openai.azure.com".to_string(),
                api_key: None,
                api_key_env: None,
                context_window: None,
                capabilities: None,
                thinking: None,
                api_type: None,
            },
        )
        .collect();
    app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
        entries,
        selected: 39,
        error: None,
    });

    // A short terminal that cannot hold forty deployments: without a window the
    // selected row (and every row before the last thirteen) is clipped away.
    let text = painted(&mut app, 120, 24);
    assert!(
        text.contains("deployment-39"),
        "selected deployment must be painted: {text}"
    );
    assert!(
        text.contains("Showing"),
        "a clipped list must state its window: {text}"
    );
    assert!(
        !text.contains("deployment-00"),
        "the window must have scrolled past the first row: {text}"
    );
}

#[test]
fn test_thinking_level_selector_scrolls_to_the_selected_level() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        model: entry("claude-sonnet-4", "Claude Sonnet 4"),
        selected: 4,
    });

    // Twelve rows of terminal leaves room for one two-line level only.
    let text = painted(&mut app, 120, 12);
    assert!(
        text.contains("Use the deepest reasoning budget"),
        "the selected level's description must be painted: {text}"
    );
    assert!(
        text.contains("Showing"),
        "a clipped level list must state its window: {text}"
    );
}

#[test]
fn test_router_bucket_scrolls_to_the_selected_model() {
    let mut app = support::make_app();
    let models: Vec<TierEntry> = (0..20)
        .map(|i| TierEntry {
            provider: "anthropic".to_string(),
            model: format!("model-{i:02}"),
        })
        .collect();
    let mut draft = RouterConfig::default();
    draft.tiers.insert(
        Tier::Simple.to_string(),
        TierConfig {
            models,
            timeout_ms: None,
        },
    );
    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
        providers: vec![configured("anthropic")],
        selected_provider_ids: vec!["anthropic".to_string()],
        selected_provider_index: 0,
        draft_config: draft,
        active_bucket: Tier::Simple,
        active_bucket_index: 19,
        left_pane_focused: false,
        error: None,
    });

    let text = painted(&mut app, 120, 40);
    assert!(
        text.contains("model-19"),
        "the selected bucket entry must be painted: {text}"
    );
    assert!(
        !text.contains("model-00"),
        "the bucket must have scrolled past its first entry: {text}"
    );
}

#[test]
fn test_router_provider_pane_scrolls_to_the_selected_provider() {
    let mut app = support::make_app();
    let providers: Vec<ConfiguredProvider> =
        (0..30).map(|i| configured(&format!("{i:02}"))).collect();
    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
        providers,
        selected_provider_ids: Vec::new(),
        selected_provider_index: 29,
        draft_config: RouterConfig::default(),
        active_bucket: Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: None,
    });

    let text = painted(&mut app, 120, 40);
    assert!(
        text.contains("provider-29"),
        "the selected provider must be painted: {text}"
    );
    assert!(
        !text.contains("provider-00"),
        "the pane must have scrolled past its first provider: {text}"
    );
    assert!(
        text.contains(" of 30"),
        "a clipped provider pane must state its window: {text}"
    );
}

#[test]
fn test_wheel_over_the_model_picker_steps_the_cursor() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![
            entry("a", "Model A"),
            entry("b", "Model B"),
            entry("c", "Model C"),
        ],
        selected: 0,
    });
    // Render first: the dialog records the rect the wheel is hit-tested against.
    let area = {
        painted(&mut app, 120, 40);
        app.provider_setup_area
    };
    assert!(area.width > 0 && area.height > 0, "dialog area recorded");

    app.handle_mouse_event(wheel(MouseEventKind::ScrollDown, area.x + 2, area.y + 2));
    match app.provider_setup.as_ref().expect("picker still open") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 1),
        other => panic!("expected SelectModel, got {other:?}"),
    }

    app.handle_mouse_event(wheel(MouseEventKind::ScrollUp, area.x + 2, area.y + 2));
    match app.provider_setup.as_ref().expect("picker still open") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 0),
        other => panic!("expected SelectModel, got {other:?}"),
    }
}

#[test]
fn test_wheel_outside_the_setup_dialog_never_scrolls_the_chat() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![entry("a", "Model A"), entry("b", "Model B")],
        selected: 0,
    });
    painted(&mut app, 120, 40);
    let before = app.scroll_offset;

    app.handle_mouse_event(wheel(MouseEventKind::ScrollDown, 0, 0));
    assert_eq!(
        app.scroll_offset, before,
        "the modal swallows a wheel outside it, so the transcript must not move"
    );
    match app.provider_setup.as_ref().expect("picker still open") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 0),
        other => panic!("expected SelectModel, got {other:?}"),
    }
}

#[test]
fn test_wheel_over_the_success_card_never_dismisses_it() {
    // `Done` closes on any key, so a wheel over it must not be translated into a
    // keystroke: the confirmation stays until the user presses a key.
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::Done {
        provider_name: "Anthropic".to_string(),
        model_name: Some("claude-sonnet-4".to_string()),
    });
    let area = {
        painted(&mut app, 120, 40);
        app.provider_setup_area
    };

    app.handle_mouse_event(wheel(MouseEventKind::ScrollDown, area.x + 2, area.y + 2));
    assert!(
        matches!(app.provider_setup, Some(ProviderSetupStep::Done { .. })),
        "a wheel notch must not dismiss the success card"
    );
}
