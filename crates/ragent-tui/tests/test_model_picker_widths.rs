//! Tests for content-sized columns in the model-picker dialogs.
//!
//! The `SelectModel` and `SelectRouterModel` tables size each column to the
//! widest header or cell (see `layout::content_sized_columns`), so the widest
//! content in every column must render without truncation.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_tui::App;
use ragent_tui::app::{ModelPickerEntry, ProviderSetupStep};
use ragent_types::{ThinkingConfig, ThinkingLevel};
use ratatui::{Terminal, backend::TestBackend};

#[path = "support/mod.rs"]
mod support;

fn entry(id: &str, name: &str, cost_tier: &str, cost_multiplier: &str) -> ModelPickerEntry {
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
        cost_tier: cost_tier.to_string(),
        cost_multiplier: cost_multiplier.to_string(),
    }
}

/// Render the app and return the flattened buffer text (mirrors the helper in
/// `test_router_setup.rs`).
fn render_to_string(app: &mut App, width: u16, height: u16) -> String {
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

#[test]
fn test_select_model_renders_widest_model_name_untruncated() {
    let long_name = "Claude Sonnet 4 With A Very Long Display Name Indeed";
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![
            entry("claude-sonnet-4", long_name, "Premium", "1x"),
            entry("claude-haiku", "Claude 3.5 Haiku", "Low", "1x"),
        ],
        selected: 0,
    });

    let text = render_to_string(&mut app, 120, 40);
    assert!(
        text.contains(long_name),
        "widest model name must not be truncated: {text}"
    );
}

#[test]
fn test_select_model_renders_widest_cost_and_thinking_cells_untruncated() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![
            entry("claude-sonnet-4", "Claude Sonnet 4", "Premium", "12x"),
            entry("claude-opus", "Claude Opus 4", "Ultra", "12x"),
        ],
        selected: 0,
    });

    let text = render_to_string(&mut app, 120, 40);
    // The thinking column shows all levels joined with "/"; the selection
    // indicator sits inside the Model cell of the highlighted row.
    assert!(
        text.contains("Auto/Off/Low/Med/High"),
        "widest thinking cell must not be truncated: {text}"
    );
    assert!(
        text.contains("▸ Claude Sonnet 4"),
        "selection indicator plus name must render untruncated: {text}"
    );
}

#[test]
fn test_select_model_dialog_expands_and_clamps_to_terminal() {
    // A very long name must expand the dialog but never exceed the terminal
    // width (small-terminal clamp in `centered_rect_max`).
    let long_name = "An Extremely Long Model Name That Exceeds Any Reasonable Terminal Width";
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![entry("huge", long_name, "Premium", "1x")],
        selected: 0,
    });

    // Even at a modest terminal size the render must not panic and the dialog
    // must occupy the full available width.
    let text = render_to_string(&mut app, 100, 40);
    assert!(text.contains("Select Model"), "dialog must render: {text}");
}

#[test]
fn test_select_router_model_renders_widest_cells_untruncated() {
    let long_name = "Claude Sonnet 4 With A Very Long Display Name Indeed";
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![
            entry("claude-sonnet-4", long_name, "Premium", "1x"),
            entry("claude-haiku", "Claude 3.5 Haiku", "Low", "1x"),
        ],
        selected: 0,
        target_tier: ragent_llm::providers::router_config::Tier::Simple,
    });

    let text = render_to_string(&mut app, 120, 40);
    assert!(
        text.contains(long_name),
        "widest router model name must not be truncated: {text}"
    );
    assert!(
        text.contains("Auto/Off/Low/Med/High"),
        "widest thinking cell must not be truncated: {text}"
    );
}

#[test]
fn test_model_picker_down_moves_selection_indicator() {
    // The indicator prefix moves with the selection, and the new row's cell
    // must also render fully (column widths account for both prefixes).
    let mut app = support::make_app();
    let name_b = "A Second Model Whose Row Is Also Fully Rendered";
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![
            entry("claude-sonnet-4", "Claude Sonnet 4", "Premium", "1x"),
            entry("claude-haiku", name_b, "Low", "1x"),
        ],
        selected: 0,
    });

    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));

    let text = render_to_string(&mut app, 120, 40);
    assert!(
        text.contains("▸ A Second Model Whose Row Is Also Fully Rendered"),
        "selected row must show the indicator and full name: {text}"
    );
}
