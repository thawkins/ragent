//! Tests for the provider/model setup dialog chrome (`/provider`, `/model`).
//!
//! The setup dialogs share one composition: a slate accent frame, a heading strip,
//! a pinned reversed-video column band, a cursor-filled zebra-striped list, and a
//! key-hint footer. These tests pin the parts that make the dialogs easier to read
//! (column alignment, the configured-provider count, the feature legend, the
//! explicit scroll legend) without giving up any of the behaviour the pickers had.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_tui::app::{ConfiguredProvider, ModelPickerEntry, ProviderSetupStep};
use ragent_tui::input_field::InputField;
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

fn painted(app: &mut ragent_tui::App, width: u16, height: u16) -> String {
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

fn render_setup(step: ProviderSetupStep, width: u16, height: u16) -> String {
    let mut app = support::make_app();
    app.provider_setup = Some(step);
    painted(&mut app, width, height)
}

#[test]
fn test_select_provider_heading_states_the_configured_count() {
    // A fresh in-memory store has no credentials, so the heading reports 0 of the
    // full registry: setup progress is legible before anything is chosen.
    let text = render_setup(
        ProviderSetupStep::SelectProvider {
            selected: 0,
            force_key_entry: false,
        },
        120,
        40,
    );
    assert!(text.contains("Select a Provider"), "heading: {text}");
    // The count tracks the live registry, so it is asserted against the constant
    // rather than a literal: adding a provider must not break this test.
    let expected = format!("of {} configured", ragent_tui::app::PROVIDER_LIST.len());
    assert!(text.contains(&expected), "count lane {expected}: {text}");
}

#[test]
fn test_select_provider_rows_align_under_the_column_band() {
    // The band pins Provider/Id/Kind/Status above the rows; both the heading labels
    // and the provider ids must be painted, so the columns are readable.
    let text = render_setup(
        ProviderSetupStep::SelectProvider {
            selected: 3,
            force_key_entry: false,
        },
        120,
        40,
    );
    for column in ["Provider", "Id", "Kind", "Status"] {
        assert!(text.contains(column), "band label {column} missing: {text}");
    }
    assert!(text.contains("generic_openai"), "provider id lane: {text}");
    assert!(text.contains("[local]"), "ollama badge lane: {text}");
}

#[test]
fn test_select_provider_scroll_legend_states_the_window() {
    // A 20-row terminal cannot hold all 14 providers plus the chrome, so the legend
    // must say how much is above and below.
    let text = render_setup(
        ProviderSetupStep::SelectProvider {
            selected: 13,
            force_key_entry: false,
        },
        120,
        20,
    );
    assert!(text.contains("more above"), "legend: {text}");
    assert!(text.contains("more below"), "legend: {text}");
    assert!(
        text.contains("navigate"),
        "key hints stay with the legend: {text}"
    );
}

#[test]
fn test_select_model_footer_legends_the_feature_column() {
    // `R`/`V`/`T` are terse; the footer spells them out so the column is readable.
    let text = render_setup(
        ProviderSetupStep::SelectModel {
            provider_id: "anthropic".to_string(),
            provider_name: "Anthropic".to_string(),
            models: vec![entry("claude-sonnet-4", "Claude Sonnet 4")],
            selected: 0,
        },
        120,
        40,
    );
    assert!(text.contains("R reasoning"), "feature legend: {text}");
    assert!(text.contains("V vision"), "feature legend: {text}");
    assert!(text.contains("T tools"), "feature legend: {text}");
    assert!(text.contains("Claude Sonnet 4"), "model row: {text}");
    assert!(text.contains("200K"), "context column: {text}");
    // The selection indicator survives inside the model cell.
    assert!(text.contains("> Claude Sonnet 4"), "cursor row: {text}");
}

#[test]
fn test_select_model_heading_names_the_provider_and_model_count() {
    let text = render_setup(
        ProviderSetupStep::SelectModel {
            provider_id: "anthropic".to_string(),
            provider_name: "Anthropic".to_string(),
            models: vec![
                entry("claude-sonnet-4", "Claude Sonnet 4"),
                entry("claude-haiku", "Claude Haiku"),
            ],
            selected: 0,
        },
        120,
        40,
    );
    assert!(text.contains("Select Model"), "title: {text}");
    assert!(text.contains("Anthropic"), "provider in title: {text}");
    assert!(text.contains("2 models"), "model count: {text}");
    assert!(text.contains("Enter selects"), "heading hint: {text}");
}

#[test]
fn test_select_model_showing_legend_leads_the_footer_when_clipped() {
    let models: Vec<ModelPickerEntry> = (0..40)
        .map(|i| entry(&format!("m{i}"), &format!("Model {i}")))
        .collect();
    let text = render_setup(
        ProviderSetupStep::SelectModel {
            provider_id: "anthropic".to_string(),
            provider_name: "Anthropic".to_string(),
            models,
            selected: 35,
        },
        120,
        24,
    );
    // `Showing` must survive even though the footer is the first thing a narrow
    // dialog truncates, so it is painted before the key hints.
    assert!(text.contains("Showing"), "window position: {text}");
    assert!(text.contains("navigate"), "key hints: {text}");
}

#[test]
fn test_select_model_empty_notice_keeps_its_guidance() {
    let text = render_setup(
        ProviderSetupStep::SelectModel {
            provider_id: "anthropic".to_string(),
            provider_name: "Anthropic".to_string(),
            models: Vec::new(),
            selected: 0,
        },
        120,
        40,
    );
    assert!(
        text.contains("No models are currently available for this provider."),
        "empty notice: {text}"
    );
    assert!(text.contains("Esc cancel"), "escape hint: {text}");
}

#[test]
fn test_thinking_selector_marks_the_cursor_row_and_keeps_descriptions() {
    let text = render_setup(
        ProviderSetupStep::SelectThinkingLevel {
            provider_id: "anthropic".to_string(),
            provider_name: "Anthropic".to_string(),
            model: entry("claude-sonnet-4", "Claude Sonnet 4"),
            selected: 4,
        },
        120,
        40,
    );
    assert!(
        text.contains("Select thinking for Claude Sonnet 4"),
        "{text}"
    );
    assert!(text.contains("High"), "level label: {text}");
    assert!(
        text.contains("Use the deepest reasoning budget"),
        "level description: {text}"
    );
}

#[tokio::test]
async fn test_thinking_selector_selection_still_persists_a_level() {
    // The restyle must not touch the state machine: Enter on the highlighted level
    // still records it and advances to the confirmation card.
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        model: entry("claude-sonnet-4", "Claude Sonnet 4"),
        selected: 2,
    });
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE))
        .await;
    assert_eq!(app.selected_thinking_level, Some(ThinkingLevel::Low));
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::Done { .. })
    ));
}

#[test]
fn test_switch_provider_rows_name_where_the_key_came_from() {
    let text = render_setup(
        ProviderSetupStep::SelectConfiguredProvider {
            providers: vec![
                ConfiguredProvider {
                    id: "anthropic".to_string(),
                    name: "Anthropic (Claude)".to_string(),
                    source: ragent_tui::app::ProviderSource::Database,
                },
                ConfiguredProvider {
                    id: "openai".to_string(),
                    name: "OpenAI (GPT)".to_string(),
                    source: ragent_tui::app::ProviderSource::EnvVar,
                },
            ],
            selected: 0,
        },
        120,
        40,
    );
    assert!(text.contains("Switch Provider"), "title: {text}");
    assert!(text.contains("2 configured"), "count lane: {text}");
    assert!(text.contains("stored"), "database source lane: {text}");
    assert!(text.contains("env"), "env-var source lane: {text}");
    assert!(text.contains("[ok]"), "credential lane: {text}");
}

#[test]
fn test_show_provider_config_lists_ids_behind_the_names() {
    let text = render_setup(
        ProviderSetupStep::ShowProviderConfig {
            providers: vec![ConfiguredProvider {
                id: "openai".to_string(),
                name: "OpenAI (GPT)".to_string(),
                source: ragent_tui::app::ProviderSource::Database,
            }],
            selected: 0,
        },
        120,
        40,
    );
    assert!(
        text.contains("Show Provider Configuration"),
        "heading: {text}"
    );
    assert!(text.contains("OpenAI (GPT)"), "provider name: {text}");
    assert!(text.contains("openai"), "provider id: {text}");
    assert!(text.contains("Enter show config"), "key hint: {text}");
}

#[test]
fn test_reset_provider_keeps_its_warning_colour_and_marks_the_active_provider() {
    let mut app = support::make_app();
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama".to_string(),
        name: "Ollama (Local)".to_string(),
        source: ragent_tui::app::ProviderSource::Database,
    });
    app.provider_setup = Some(ProviderSetupStep::ResetProvider { selected: 0 });

    let text = painted(&mut app, 120, 40);
    assert!(
        text.contains("Reset Provider Credentials"),
        "heading: {text}"
    );
    assert!(text.contains("* = active provider"), "legend: {text}");
    assert!(text.contains("[local]"), "ollama badge: {text}");
    assert!(text.contains("Enter reset"), "key hint: {text}");

    // The destructive dialog keeps its yellow frame.
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, &mut app))
        .expect("draw");
    let buffer = terminal.backend().buffer();
    assert!(
        buffer
            .content
            .iter()
            .any(|cell| cell.fg == ratatui::style::Color::Yellow && cell.symbol() == "─"),
        "reset dialog frame must stay yellow"
    );
}

#[test]
fn test_enter_key_dialog_names_the_provider_in_the_frame_title() {
    let text = render_setup(
        ProviderSetupStep::EnterKey {
            provider_id: "generic_openai".to_string(),
            provider_name: "Generic OpenAI API".to_string(),
            key_field: InputField::with_text("sk-abcdefghijklmnop"),
            endpoint_field: InputField::new(),
            active_field: 0,
            error: None,
        },
        120,
        40,
    );
    assert!(text.contains("Enter API Key"), "title: {text}");
    assert!(
        text.contains("Generic OpenAI API"),
        "provider in title: {text}"
    );
    assert!(text.contains("Endpoint URL"), "endpoint field: {text}");
    // The key must stay masked (SEC-ragent-tui-002).
    assert!(
        !text.contains("sk-abcdefghijklmnop"),
        "the API key must not be painted verbatim: {text}"
    );
}

#[test]
fn test_select_agent_and_azure_list_keep_their_markers() {
    let mut app = support::make_app();
    app.current_agent_index = 1;
    app.provider_setup = Some(ProviderSetupStep::SelectAgent {
        agents: vec![
            ("general".to_string(), "General helper".to_string(), false),
            ("coder".to_string(), "Codes things".to_string(), true),
        ],
        selected: 1,
    });
    let text = painted(&mut app, 120, 40);
    assert!(text.contains("Select an Agent"), "heading: {text}");
    assert!(text.contains("[custom]"), "custom marker: {text}");
    assert!(text.contains('*'), "in-use marker: {text}");
    assert!(text.contains("Codes things"), "agent description: {text}");

    let text = render_setup(
        ProviderSetupStep::SelectAzureResource {
            entries: Vec::new(),
            selected: 0,
            error: None,
        },
        120,
        40,
    );
    assert!(text.contains("Select Azure Resource"), "title: {text}");
    assert!(text.contains("No resources found."), "empty notice: {text}");
}
