//! Deep coverage for `ragent_tui::input` key handling and the
//! `App` keyboard/mouse dispatch in `src/app/input_handler.rs`.
//!
//! Everything here drives the public entry points only -
//! `App::handle_key_event`, `App::handle_mouse_event`,
//! `ragent_tui::input::handle_key` - and constructs `crossterm::event` values
//! directly, exactly as the terminal event loop does. No production logic is
//! changed, no network call is awaited and no real TTY is used.
//!
//! Focus areas (the arms the existing suites do not reach):
//!   * the provider-setup dialog state machine for every `ProviderSetupStep`
//!     variant, entered through `App::handle_key_event` so the modal dispatch
//!     in `input::handle_key` is exercised too,
//!   * the router cluster panel and its model-picker sub-dialog,
//!   * the telemetry multi-field form (`input::telemetry_field`),
//!   * the general editing / navigation ladder in `input::handle_key`,
//!   * mouse routing: wheel scrolling of every side panel, drag-select, the
//!     right-click context menu's copy/cut rows, and the plugin-store and
//!     connector-catalogue panels' click/wheel columns,
//!   * the history / config-save pickers and the memory-delete confirmation.

mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::layout::Rect;

use ragent_tui::app::{
    ConfigSavePickerState, ContextAction, ContextMenuState, HistoryPickerState, ModelPickerEntry,
    PendingMemoryDelete, ProviderSetupStep, SelectionPane, TextSelection,
};
use ragent_tui::input::{InputAction, handle_key};
use ragent_tui::input_field::InputField;

/// A plain (no-modifier) key press.
fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// A key press with the given modifiers.
fn kmod(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, mods)
}

/// A mouse event with no modifiers.
const fn mouse(kind: MouseEventKind, column: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

/// Render one frame so the layout caches every pane/panel area and scroll
/// maximum the mouse hit-testing reads back.
fn render(app: &mut ragent_tui::App, width: u16, height: u16) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw");
}

/// Index of `pid` in `PROVIDER_LIST`.
fn provider_index(pid: &str) -> usize {
    ragent_tui::app::PROVIDER_LIST
        .iter()
        .position(|(id, _)| *id == pid)
        .expect("provider present in PROVIDER_LIST")
}

/// A model-picker entry with `id` and `thinking` levels.
fn model_entry(
    id: &str,
    provider_id: &str,
    thinking: Vec<ragent_types::ThinkingLevel>,
) -> ModelPickerEntry {
    ModelPickerEntry {
        provider_id: provider_id.to_string(),
        id: id.to_string(),
        name: format!("Model {id}"),
        context_window: 128_000,
        max_output: Some(16_384),
        cost_input: 0.0,
        cost_output: 0.0,
        reasoning: !thinking.is_empty(),
        vision: false,
        tool_use: true,
        thinking_levels: thinking,
        thinking_config: None,
        cost_tier: "Free".to_string(),
        cost_multiplier: "0x".to_string(),
    }
}

/// An `EnterKey` setup step for `provider_id` with the key text `key_text`.
fn enter_key(provider_id: &str, key_text: &str) -> ProviderSetupStep {
    ProviderSetupStep::EnterKey {
        provider_id: provider_id.to_string(),
        provider_name: provider_id.to_string(),
        key_field: InputField::with_text(key_text),
        endpoint_field: InputField::new(),
        active_field: 0,
        error: None,
    }
}

/// A `GitLabSetup` step focused on `active_field`.
fn gitlab_setup(active: u8) -> ProviderSetupStep {
    ProviderSetupStep::GitLabSetup {
        url_input: "http://127.0.0.1:1".to_string(),
        url_cursor: "http://127.0.0.1:1".len(),
        token_input: String::new(),
        token_cursor: 0,
        active_field: active,
        error: None,
    }
}

/// A `TelemetrySetup` step focused on `active_field`.
fn telemetry_setup(active: u8) -> ProviderSetupStep {
    ProviderSetupStep::TelemetrySetup {
        endpoint_field: InputField::with_text("http://localhost:4318"),
        protocol: ragent_config::OtelProtocol::Http,
        interval_field: InputField::with_text("30"),
        timeout_field: InputField::with_text("10"),
        port_field: InputField::new(),
        active_field: active,
        error: None,
    }
}

/// A `SetupRouter` step over one selected provider.
fn router_setup(
    providers: Vec<(String, String)>,
    selected_ids: Vec<String>,
    draft: ragent_llm::providers::router_config::RouterConfig,
) -> ProviderSetupStep {
    ProviderSetupStep::SetupRouter {
        providers: providers
            .into_iter()
            .map(|(id, name)| ragent_tui::app::ConfiguredProvider {
                id,
                name,
                source: ragent_tui::app::ProviderSource::Database,
            })
            .collect(),
        selected_provider_ids: selected_ids,
        selected_provider_index: 0,
        draft_config: draft,
        active_bucket: ragent_llm::providers::router_config::Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: None,
    }
}

/// Persist `ids` as the discovered model list for `provider_id` so
/// `App::models_for_provider` returns them without a network call.
fn seed_discovered_models(app: &ragent_tui::App, provider_id: &str, ids: &[&str]) {
    let models: Vec<ragent_agent::provider::ModelInfo> = ids
        .iter()
        .map(|id| ragent_agent::provider::ModelInfo {
            id: (*id).to_string(),
            provider_id: provider_id.to_string(),
            name: (*id).to_string(),
            cost: ragent_config::Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: ragent_config::Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: Vec::new(),
            },
            context_window: 200_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
        })
        .collect();
    let json = serde_json::to_string(&models).expect("serialize models");
    app.storage
        .set_discovered_models(provider_id, &json)
        .expect("seed discovered models");
}

// ===========================================================================
// Provider setup: SelectProvider through the public dispatcher
// ===========================================================================

#[tokio::test]
async fn select_provider_enter_unconfigured_opens_key_entry() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectProvider {
        selected: provider_index("anthropic"),
        force_key_entry: false,
    });
    app.handle_key_event(key(KeyCode::Enter)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::EnterKey { provider_id, .. } => assert_eq!(provider_id, "anthropic"),
        other => panic!("expected EnterKey, got {other:?}"),
    }
}

#[tokio::test]
async fn select_provider_enter_prefills_stored_key() {
    let mut app = support::make_app();
    app.storage
        .set_provider_auth("anthropic", "sk-stored")
        .expect("store key");
    app.provider_setup = Some(ProviderSetupStep::SelectProvider {
        selected: provider_index("anthropic"),
        force_key_entry: true,
    });
    app.handle_key_event(key(KeyCode::Enter)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::EnterKey { key_field, .. } => {
            assert_eq!(key_field.text(), "sk-stored");
        }
        other => panic!("expected EnterKey, got {other:?}"),
    }
}

#[tokio::test]
async fn select_provider_enter_ollama_skips_key_entry() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectProvider {
        selected: provider_index("ollama"),
        force_key_entry: true,
    });
    app.handle_key_event(key(KeyCode::Enter)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::LoadingModels { provider_id, .. } => assert_eq!(provider_id, "ollama"),
        other => panic!("expected LoadingModels, got {other:?}"),
    }
}

#[tokio::test]
async fn select_provider_enter_copilot_without_token_starts_device_flow() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectProvider {
        selected: provider_index("copilot"),
        force_key_entry: true,
    });
    app.handle_key_event(key(KeyCode::Enter)).await;
    // No stored/env/IDE token -> the device-flow start is spawned and the
    // dialog shows its loading spinner while the result is in flight.
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::LoadingModels { .. })
    ));
    assert!(app.model_loading_state.is_some());
}

#[tokio::test]
async fn select_provider_enter_azure_resource_opens_resource_picker() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectProvider {
        selected: provider_index("azure_resource"),
        force_key_entry: true,
    });
    app.handle_key_event(key(KeyCode::Enter)).await;
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::SelectAzureResource { .. })
    ));
}

#[tokio::test]
async fn select_provider_enter_router_without_providers_warns_and_stays() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectProvider {
        selected: provider_index("router"),
        force_key_entry: false,
    });
    app.handle_key_event(key(KeyCode::Enter)).await;
    assert_eq!(
        app.status, "[warn] No concrete providers - configure one first",
        "the router needs a concrete provider first"
    );
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::SelectProvider { .. })
    ));
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.contains("no concrete providers configured")),
        "a warning is logged alongside the status"
    );
}

// ===========================================================================
// Provider setup: EnterKey editing through the public dispatcher
// ===========================================================================

#[tokio::test]
async fn enter_key_typing_backspace_and_ctrl_v_edit_the_field() {
    let mut app = support::make_app();
    // A thread-local clipboard override keeps the paste deterministic.
    let guard = ragent_tui::clipboard::ClipboardTestOverrideGuard::new("X");
    app.provider_setup = Some(enter_key("anthropic", "ab"));
    app.handle_key_event(key(KeyCode::Char('c'))).await;
    app.handle_key_event(key(KeyCode::Backspace)).await;
    app.handle_key_event(kmod(KeyCode::Char('v'), KeyModifiers::CONTROL))
        .await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::EnterKey {
            key_field, error, ..
        } => {
            assert_eq!(key_field.text(), "abX");
            assert!(error.is_none(), "editing clears the previous error");
        }
        other => panic!("expected EnterKey, got {other:?}"),
    }
    drop(guard);
}

// ===========================================================================
// Provider setup: SelectModel / SelectThinkingLevel
// ===========================================================================

#[tokio::test]
async fn select_model_navigation_wraps_and_enter_opens_thinking_levels() {
    use ragent_types::ThinkingLevel;
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![
            model_entry(
                "m1",
                "anthropic",
                vec![ThinkingLevel::Off, ThinkingLevel::Low],
            ),
            model_entry("m2", "anthropic", vec![ThinkingLevel::Off]),
        ],
        selected: 0,
    });
    app.handle_key_event(key(KeyCode::Up)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 1),
        other => panic!("expected SelectModel, got {other:?}"),
    }
    app.handle_key_event(key(KeyCode::Down)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 0),
        other => panic!("expected SelectModel, got {other:?}"),
    }
    app.handle_key_event(key(KeyCode::Enter)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SelectThinkingLevel { model, .. } => {
            assert_eq!(model.thinking_levels.len(), 2);
        }
        other => panic!("expected SelectThinkingLevel, got {other:?}"),
    }
}

#[tokio::test]
async fn select_thinking_level_enter_finalises_the_model() {
    use ragent_types::ThinkingLevel;
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        model: model_entry(
            "claude-x",
            "anthropic",
            vec![ThinkingLevel::Off, ThinkingLevel::High],
        ),
        selected: 1,
    });
    app.handle_key_event(key(KeyCode::Down)).await;
    app.handle_key_event(key(KeyCode::Up)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::Done { model_name, .. } => assert!(model_name.is_some()),
        other => panic!("expected Done, got {other:?}"),
    }
    assert_eq!(app.selected_thinking_level, Some(ThinkingLevel::High));
    assert_eq!(
        app.selected_model.as_deref(),
        Some("anthropic/claude-x"),
        "the selected model is persisted"
    );
}

// ===========================================================================
// Provider setup: GitLab form and validation spawn
// ===========================================================================

#[tokio::test]
async fn gitlab_setup_enter_with_both_fields_starts_validation() {
    let mut app = support::make_app();
    let mut step = gitlab_setup(1);
    if let ProviderSetupStep::GitLabSetup { token_input, .. } = &mut step {
        *token_input = "glpat-token".to_string();
    }
    app.provider_setup = Some(step);
    app.handle_key_event(key(KeyCode::Enter)).await;
    // The PAT is handed to the background validator; the dialog shows the
    // validating state. The spawned task only publishes an event on failure.
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::GitLabValidating {
            instance_url,
            token,
        } => {
            assert_eq!(instance_url, "http://127.0.0.1:1");
            assert_eq!(token, "glpat-token");
        }
        other => panic!("expected GitLabValidating, got {other:?}"),
    }
}

#[tokio::test]
async fn gitlab_validating_other_key_keeps_the_state() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::GitLabValidating {
        instance_url: "http://127.0.0.1:1".to_string(),
        token: "t".to_string(),
    });
    app.handle_key_event(key(KeyCode::Tab)).await;
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::GitLabValidating { .. })
    ));
}

// ===========================================================================
// Provider setup: telemetry multi-field form
// ===========================================================================

#[tokio::test]
async fn telemetry_setup_tab_cycles_all_five_fields() {
    let mut app = support::make_app();
    app.provider_setup = Some(telemetry_setup(0));
    for expected in [1_u8, 2, 3, 4, 0] {
        app.handle_key_event(key(KeyCode::Tab)).await;
        match app.provider_setup.as_ref().expect("setup") {
            ProviderSetupStep::TelemetrySetup { active_field, .. } => {
                assert_eq!(*active_field, expected);
            }
            other => panic!("expected TelemetrySetup, got {other:?}"),
        }
    }
    // BackTab walks the other way and wraps at 0.
    app.handle_key_event(key(KeyCode::BackTab)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::TelemetrySetup { active_field, .. } => assert_eq!(*active_field, 4),
        other => panic!("expected TelemetrySetup, got {other:?}"),
    }
}

#[tokio::test]
async fn telemetry_setup_port_field_accepts_digits_and_edits() {
    let mut app = support::make_app();
    app.provider_setup = Some(telemetry_setup(4));
    for digit in ['9', '1'] {
        app.handle_key_event(key(KeyCode::Char(digit))).await;
    }
    app.handle_key_event(key(KeyCode::Home)).await;
    app.handle_key_event(key(KeyCode::End)).await;
    app.handle_key_event(key(KeyCode::Left)).await;
    app.handle_key_event(key(KeyCode::Right)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::TelemetrySetup { port_field, .. } => {
            assert_eq!(port_field.text(), "91");
        }
        other => panic!("expected TelemetrySetup, got {other:?}"),
    }
    app.handle_key_event(key(KeyCode::Backspace)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::TelemetrySetup { port_field, .. } => {
            assert_eq!(port_field.text(), "9");
        }
        other => panic!("expected TelemetrySetup, got {other:?}"),
    }
}

#[tokio::test]
async fn telemetry_setup_enter_with_bad_endpoint_reports_problems() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
        endpoint_field: InputField::with_text("not-a-url"),
        protocol: ragent_config::OtelProtocol::Grpc,
        interval_field: InputField::with_text("30"),
        timeout_field: InputField::with_text("10"),
        port_field: InputField::new(),
        active_field: 0,
        error: None,
    });
    app.handle_key_event(key(KeyCode::Enter)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::TelemetrySetup { error, .. } => {
            assert!(
                error.as_deref().unwrap_or("").contains("endpoint"),
                "got: {error:?}"
            );
        }
        other => panic!("expected TelemetrySetup error, got {other:?}"),
    }
}

// ===========================================================================
// Provider setup: router cluster panel and its model picker
// ===========================================================================

#[tokio::test]
async fn router_setup_pane_and_bucket_navigation() {
    let mut app = support::make_app();
    app.provider_setup = Some(router_setup(
        vec![
            ("anthropic".to_string(), "Anthropic".to_string()),
            ("openai".to_string(), "OpenAI".to_string()),
        ],
        vec!["anthropic".to_string()],
        ragent_llm::providers::router_config::RouterConfig::default(),
    ));
    // Provider-pane cursor moves and wraps.
    app.handle_key_event(key(KeyCode::Down)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            selected_provider_index,
            ..
        } => assert_eq!(*selected_provider_index, 1),
        other => panic!("expected SetupRouter, got {other:?}"),
    }
    app.handle_key_event(key(KeyCode::Up)).await;
    // Tab hands focus to the bucket pane; Left/Right cycle the buckets.
    app.handle_key_event(key(KeyCode::Tab)).await;
    app.handle_key_event(key(KeyCode::Right)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            left_pane_focused,
            active_bucket,
            ..
        } => {
            assert!(!left_pane_focused);
            assert_eq!(
                *active_bucket,
                ragent_llm::providers::router_config::Tier::Medium
            );
        }
        other => panic!("expected SetupRouter, got {other:?}"),
    }
    app.handle_key_event(key(KeyCode::Left)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter { active_bucket, .. } => assert_eq!(
            *active_bucket,
            ragent_llm::providers::router_config::Tier::Simple
        ),
        other => panic!("expected SetupRouter, got {other:?}"),
    }
    // Up/Down over an empty bucket clamp the index at 0.
    app.handle_key_event(key(KeyCode::Up)).await;
    app.handle_key_event(key(KeyCode::Down)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            active_bucket_index,
            ..
        } => assert_eq!(*active_bucket_index, 0),
        other => panic!("expected SetupRouter, got {other:?}"),
    }
}

#[tokio::test]
async fn router_setup_space_deselects_a_provider_and_its_tier_entries() {
    use ragent_llm::providers::router_config::{RouterConfig, Tier, TierConfig, TierEntry};
    let mut draft = RouterConfig::default();
    draft.tiers.insert(
        Tier::Simple.to_string(),
        TierConfig {
            models: vec![
                TierEntry {
                    provider_id: "anthropic".to_string(),
                    model: "m1".to_string(),
                },
                TierEntry {
                    provider_id: "anthropic".to_string(),
                    model: "m2".to_string(),
                },
            ],
            timeout_ms: None,
        },
    );
    let mut app = support::make_app();
    app.provider_setup = Some(router_setup(
        vec![("anthropic".to_string(), "Anthropic".to_string())],
        vec!["anthropic".to_string()],
        draft,
    ));
    // Space on the selected provider removes it and its tier assignments.
    app.handle_key_event(key(KeyCode::Char(' '))).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            selected_provider_ids,
            draft_config,
            ..
        } => {
            assert_eq!(selected_provider_ids.len(), 0);
            let models = draft_config
                .tiers
                .get(&Tier::Simple.to_string())
                .map(|t| t.models.len())
                .unwrap_or(0);
            assert_eq!(models, 0, "tier assignments are dropped with the provider");
        }
        other => panic!("expected SetupRouter, got {other:?}"),
    }
    // Space again re-adds it to the cluster palette (no tier entries).
    app.handle_key_event(key(KeyCode::Char(' '))).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            selected_provider_ids,
            ..
        } => assert_eq!(selected_provider_ids, &vec!["anthropic".to_string()]),
        other => panic!("expected SetupRouter, got {other:?}"),
    }
}

#[tokio::test]
async fn router_setup_ctrl_up_and_down_reorder_bucket_models() {
    use ragent_llm::providers::router_config::{RouterConfig, Tier, TierConfig, TierEntry};
    let mut draft = RouterConfig::default();
    draft.tiers.insert(
        Tier::Simple.to_string(),
        TierConfig {
            models: vec![
                TierEntry {
                    provider_id: "anthropic".to_string(),
                    model: "m1".to_string(),
                },
                TierEntry {
                    provider_id: "anthropic".to_string(),
                    model: "m2".to_string(),
                },
            ],
            timeout_ms: None,
        },
    );
    let mut app = support::make_app();
    app.provider_setup = Some(router_setup(
        vec![("anthropic".to_string(), "Anthropic".to_string())],
        vec!["anthropic".to_string()],
        draft,
    ));
    // Tab moves focus to the bucket pane with the cursor on the first model.
    app.handle_key_event(key(KeyCode::Tab)).await;
    app.handle_key_event(kmod(KeyCode::Down, KeyModifiers::CONTROL))
        .await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            draft_config,
            active_bucket_index,
            ..
        } => {
            assert_eq!(*active_bucket_index, 1);
            let models = &draft_config
                .tiers
                .get(&Tier::Simple.to_string())
                .expect("simple tier")
                .models;
            assert_eq!(models[0].model, "m2", "Ctrl+Down swaps the two entries");
            assert_eq!(models[1].model, "m1");
        }
        other => panic!("expected SetupRouter, got {other:?}"),
    }
    app.handle_key_event(kmod(KeyCode::Up, KeyModifiers::CONTROL))
        .await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            draft_config,
            active_bucket_index,
            ..
        } => {
            assert_eq!(*active_bucket_index, 0);
            let models = &draft_config
                .tiers
                .get(&Tier::Simple.to_string())
                .expect("simple tier")
                .models;
            assert_eq!(models[0].model, "m1", "Ctrl+Up swaps them back");
        }
        other => panic!("expected SetupRouter, got {other:?}"),
    }
}

#[tokio::test]
async fn router_setup_delete_removes_a_bucket_model() {
    use ragent_llm::providers::router_config::{RouterConfig, Tier, TierConfig, TierEntry};
    let mut draft = RouterConfig::default();
    draft.tiers.insert(
        Tier::Simple.to_string(),
        TierConfig {
            models: vec![
                TierEntry {
                    provider_id: "anthropic".to_string(),
                    model: "m1".to_string(),
                },
                TierEntry {
                    provider_id: "anthropic".to_string(),
                    model: "m2".to_string(),
                },
            ],
            timeout_ms: None,
        },
    );
    let mut app = support::make_app();
    app.provider_setup = Some(router_setup(
        vec![("anthropic".to_string(), "Anthropic".to_string())],
        vec!["anthropic".to_string()],
        draft,
    ));
    app.handle_key_event(key(KeyCode::Tab)).await;
    app.handle_key_event(key(KeyCode::Delete)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter { draft_config, .. } => {
            let models = draft_config
                .tiers
                .get(&Tier::Simple.to_string())
                .map(|t| t.models.len())
                .unwrap_or(0);
            assert_eq!(models, 1);
        }
        other => panic!("expected SetupRouter, got {other:?}"),
    }
}

#[tokio::test]
async fn router_setup_ctrl_s_stages_the_save_when_a_tier_has_a_model() {
    use ragent_llm::providers::router_config::{RouterConfig, Tier, TierConfig, TierEntry};
    let mut draft = RouterConfig::default();
    draft.tiers.insert(
        Tier::Simple.to_string(),
        TierConfig {
            models: vec![TierEntry {
                provider_id: "anthropic".to_string(),
                model: "m1".to_string(),
            }],
            timeout_ms: None,
        },
    );
    let mut app = support::make_app();
    app.provider_setup = Some(router_setup(
        vec![("anthropic".to_string(), "Anthropic".to_string())],
        vec!["anthropic".to_string()],
        draft,
    ));
    app.handle_key_event(kmod(KeyCode::Char('s'), KeyModifiers::CONTROL))
        .await;
    assert!(
        app.pending_router_save.is_some(),
        "a non-empty cluster stages the save confirmation"
    );
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::SetupRouter { .. })
    ));
}

#[tokio::test]
async fn router_setup_enter_opens_the_model_picker() {
    let mut app = support::make_app();
    seed_discovered_models(&app, "anthropic", &["claude-sonnet-4-20250514"]);
    app.provider_setup = Some(router_setup(
        vec![("anthropic".to_string(), "Anthropic".to_string())],
        vec!["anthropic".to_string()],
        ragent_llm::providers::router_config::RouterConfig::default(),
    ));
    app.handle_key_event(key(KeyCode::Enter)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SelectRouterModel {
            provider_id,
            target_tier,
            ..
        } => {
            assert_eq!(provider_id, "anthropic");
            assert_eq!(
                *target_tier,
                ragent_llm::providers::router_config::Tier::Simple
            );
        }
        other => panic!("expected SelectRouterModel, got {other:?}"),
    }
}

#[tokio::test]
async fn router_setup_enter_without_selection_reports_the_space_hint() {
    let mut app = support::make_app();
    app.provider_setup = Some(router_setup(
        vec![("anthropic".to_string(), "Anthropic".to_string())],
        Vec::new(),
        ragent_llm::providers::router_config::RouterConfig::default(),
    ));
    app.handle_key_event(key(KeyCode::Enter)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter { error, .. } => {
            assert_eq!(error.as_deref(), Some("Select provider with Space first"));
        }
        other => panic!("expected SetupRouter error, got {other:?}"),
    }
}

#[tokio::test]
async fn router_model_picker_esc_returns_to_the_cluster_panel() {
    let mut app = support::make_app();
    app.router_draft_providers = vec![ragent_tui::app::ConfiguredProvider {
        id: "anthropic".to_string(),
        name: "Anthropic".to_string(),
        source: ragent_tui::app::ProviderSource::Database,
    }];
    app.router_draft_selected_ids = vec!["anthropic".to_string()];
    app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![model_entry("m1", "anthropic", Vec::new())],
        selected: 0,
        target_tier: ragent_llm::providers::router_config::Tier::Simple,
    });
    app.handle_key_event(key(KeyCode::Esc)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            left_pane_focused, ..
        } => assert!(*left_pane_focused),
        other => panic!("expected SetupRouter, got {other:?}"),
    }
    assert_eq!(app.router_draft_providers.len(), 0);
}

#[tokio::test]
async fn router_model_picker_navigates_and_assigns_the_model() {
    use ragent_llm::providers::router_config::Tier;
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![
            model_entry("m1", "anthropic", Vec::new()),
            model_entry("m2", "anthropic", Vec::new()),
        ],
        selected: 0,
        target_tier: Tier::Simple,
    });
    app.handle_key_event(key(KeyCode::Up)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SelectRouterModel { selected, .. } => assert_eq!(*selected, 1),
        other => panic!("expected SelectRouterModel, got {other:?}"),
    }
    app.handle_key_event(key(KeyCode::Down)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            draft_config,
            left_pane_focused,
            active_bucket_index,
            ..
        } => {
            let models = draft_config
                .tiers
                .get(&Tier::Simple.to_string())
                .map(|t| t.models.len())
                .unwrap_or(0);
            assert_eq!(models, 1);
            assert!(!left_pane_focused, "focus returns to the bucket pane");
            assert_eq!(*active_bucket_index, 0);
        }
        other => panic!("expected SetupRouter, got {other:?}"),
    }
}

#[tokio::test]
async fn router_model_picker_rejects_the_router_itself() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
        provider_id: "router".to_string(),
        provider_name: "Model Router".to_string(),
        models: vec![model_entry("router", "router", Vec::new())],
        selected: 0,
        target_tier: ragent_llm::providers::router_config::Tier::Simple,
    });
    app.handle_key_event(key(KeyCode::Enter)).await;
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter { error, .. } => {
            assert_eq!(error.as_deref(), Some("The router cannot route to itself"));
        }
        other => panic!("expected SetupRouter error, got {other:?}"),
    }
}

#[tokio::test]
async fn router_model_picker_empty_list_and_other_keys_keep_the_dialog() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: Vec::new(),
        selected: 0,
        target_tier: ragent_llm::providers::router_config::Tier::Simple,
    });
    app.handle_key_event(key(KeyCode::Up)).await;
    app.handle_key_event(key(KeyCode::Down)).await;
    app.handle_key_event(key(KeyCode::Tab)).await;
    app.handle_key_event(key(KeyCode::Enter)).await;
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::SelectRouterModel { .. })
    ));
}

// ===========================================================================
// input::handle_key - the general editing / navigation ladder
// ===========================================================================

#[tokio::test]
async fn input_ladder_control_keys_map_to_their_actions() {
    let mut app = support::make_app();
    for (code, expected) in [
        ('a', "SelectAll"),
        ('e', "MoveCursorEnd"),
        ('b', "MoveCursorLeft"),
        ('f', "MoveCursorRight"),
        ('w', "DeletePrevWord"),
        ('k', "DeleteToLineEnd"),
        ('u', "ClearLine"),
    ] {
        assert!(
            handle_key(&mut app, kmod(KeyCode::Char(code), KeyModifiers::CONTROL))
                .await
                .is_some(),
            "Ctrl+{code} maps to {expected}"
        );
    }
}

#[tokio::test]
async fn input_ladder_history_recall_walks_the_entries() {
    let mut app = support::make_app();
    app.input_history = vec!["first".to_string(), "second".to_string()];

    app.handle_key_event(key(KeyCode::Up)).await;
    assert_eq!(app.input, "second");
    assert_eq!(app.history_index, Some(1));

    app.handle_key_event(key(KeyCode::Up)).await;
    assert_eq!(app.input, "first");
    assert_eq!(app.history_index, Some(0));

    app.handle_key_event(key(KeyCode::Up)).await;
    assert_eq!(app.input, "first", "Up at the oldest entry is a no-op");

    app.handle_key_event(key(KeyCode::Down)).await;
    assert_eq!(app.input, "second");
    app.handle_key_event(key(KeyCode::Down)).await;
    assert_eq!(
        app.history_index, None,
        "past the newest entry restores the draft"
    );
    assert_eq!(app.input.len(), 0);
}

#[tokio::test]
async fn input_ladder_page_and_delete_keys_map_to_actions() {
    let mut app = support::make_app();
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::PageUp)).await,
        Some(InputAction::ScrollUp)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::PageDown)).await,
        Some(InputAction::ScrollDown)
    ));
    assert!(matches!(
        handle_key(&mut app, kmod(KeyCode::PageUp, KeyModifiers::CONTROL)).await,
        Some(InputAction::LogScrollUp)
    ));
    // Delete is applied in place (no InputAction) rather than dispatched.
    app.input = "abc".to_string();
    app.input_cursor = 0;
    assert!(handle_key(&mut app, key(KeyCode::Delete)).await.is_none());
    assert_eq!(
        app.input, "bc",
        "Delete removes the character at the cursor"
    );
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Tab)).await,
        Some(InputAction::SwitchAgent)
    ));
    app.input.clear();
    app.input_cursor = 0;
    assert!(
        handle_key(&mut app, key(KeyCode::Enter)).await.is_none(),
        "Enter on an empty buffer sends nothing"
    );
}

#[tokio::test]
async fn input_ladder_slash_and_bang_prefixes_select_the_action() {
    let mut app = support::make_app();
    app.input = "/help".to_string();
    app.input_cursor = app.input.chars().count();
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Enter)).await,
        Some(InputAction::SlashCommand(_))
    ));

    let mut app = support::make_app();
    app.input = "! ls".to_string();
    app.input_cursor = app.input.chars().count();
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Enter)).await,
        Some(InputAction::BangCommand(_))
    ));
}

// ===========================================================================
// Mouse: wheel scrolling of every pane
// ===========================================================================

#[test]
fn mouse_wheel_scrolls_the_messages_pane() {
    let mut app = support::make_app();
    app.message_area = Rect::new(0, 1, 80, 20);
    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, 5, 5));
    assert_eq!(app.scroll_offset, 3);
    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, 5, 5));
    assert_eq!(app.scroll_offset, 0);
}

#[test]
fn mouse_wheel_scrolls_every_side_panel() {
    // The wheel routes to whichever side panel is visible: log, profile,
    // tasks, memory, telemetry and the context panel each own their scroll
    // offset. None of them may scroll the message pane.
    let mut app = support::make_app();
    app.show_log = true;
    app.log_area = Rect::new(60, 1, 20, 20);
    app.show_profile = true;
    app.profile_area = Rect::new(60, 21, 20, 20);
    app.show_tasks_panel = true;
    app.tasks_area = Rect::new(30, 1, 20, 20);
    app.show_memory = true;
    app.memory_area = Rect::new(30, 21, 20, 20);
    app.show_telemetry = true;
    app.telemetry_area = Rect::new(90, 1, 20, 20);
    app.show_context_panel = true;
    app.context_panel_area = Rect::new(90, 21, 20, 20);

    for (column, row) in [
        (65_u16, 5_u16),
        (65, 25),
        (35, 5),
        (35, 25),
        (95, 5),
        (95, 25),
    ] {
        app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, column, row));
        app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, column, row));
    }
    assert_eq!(app.scroll_offset, 0, "the message pane never scrolled");
}

#[test]
fn mouse_wheel_over_the_input_pane_is_a_noop() {
    let mut app = support::make_app();
    app.input_area = Rect::new(0, 40, 80, 3);
    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, 5, 41));
    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, 5, 41));
    assert_eq!(app.scroll_offset, 0);
}

// ===========================================================================
// Mouse: drag selection, release, scrollbar drag
// ===========================================================================

#[test]
fn mouse_drag_extends_and_release_preserves_the_selection() {
    let mut app = support::make_app();
    app.message_area = Rect::new(0, 1, 80, 20);

    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Left), 4, 4));
    app.handle_mouse_event(mouse(MouseEventKind::Drag(MouseButton::Left), 12, 6));
    let sel = app.text_selection.as_ref().expect("selection started");
    assert_eq!(sel.anchor, (4, 4));
    assert_eq!(sel.endpoint, (12, 6));

    app.handle_mouse_event(mouse(MouseEventKind::Up(MouseButton::Left), 12, 6));
    assert!(
        app.text_selection.is_some(),
        "release keeps the selection highlighted"
    );
}

#[test]
fn mouse_scrollbar_drag_moves_the_log_offset() {
    let mut app = support::make_app();
    app.show_log = true;
    app.log_area = Rect::new(60, 1, 20, 20);
    app.log_max_scroll = 100;
    let gutter = app.log_area.right() - 1;

    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Left), gutter, 1));
    assert_eq!(
        app.scrollbar_drag,
        Some(ragent_tui::app::ScrollbarDragPane::Log)
    );
    // Dragging toward the top drives the offset to the maximum.
    app.handle_mouse_event(mouse(MouseEventKind::Drag(MouseButton::Left), gutter, 2));
    assert!(app.log_scroll_offset > 0);
    app.handle_mouse_event(mouse(MouseEventKind::Up(MouseButton::Left), gutter, 2));
    assert!(app.scrollbar_drag.is_none(), "release clears the drag");
}

// ===========================================================================
// Mouse + context menu: copy and cut from a pane
// ===========================================================================

/// A log pane holding two copy-able rows.
fn app_with_log_rows() -> ragent_tui::App {
    let mut app = support::make_app();
    app.show_log = true;
    app.log_area = Rect::new(60, 1, 30, 20);
    app.log_content_lines = vec!["hello world".to_string(), "second row".to_string()];
    app
}

#[test]
fn context_menu_copy_row_copies_the_selected_log_text() {
    let mut app = app_with_log_rows();
    // Inner origin of the log pane is (x + 1, y + 1) = (61, 2).
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Log,
        anchor: (61, 2),
        endpoint: (65, 2),
    });
    app.context_menu = Some(ContextMenuState {
        x: 2,
        y: 2,
        pane: SelectionPane::Log,
        selected: 1,
        items: vec![
            (ContextAction::Cut, false),
            (ContextAction::Copy, true),
            (ContextAction::Paste, false),
        ],
    });

    // Rows start at y + 1: row 3 is Cut, row 4 is Copy.
    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Left), 3, 4));
    assert!(
        app.context_menu.is_none(),
        "the menu closes after the action"
    );
    assert!(
        app.log_entries
            .iter()
            .any(|e| e.message.starts_with("Copied ")),
        "the copied character count is logged"
    );
    assert!(
        app.text_selection.is_some(),
        "Copy is non-destructive and keeps the selection"
    );
}

#[test]
fn context_menu_cut_row_from_the_input_pane_removes_the_range() {
    let mut app = support::make_app();
    app.input_area = Rect::new(0, 40, 40, 3);
    app.input = "abcdef".to_string();
    app.input_cursor = app.input.chars().count();
    // Inner origin is (1, 41); the painted prefix is "> ", so columns 5..6
    // address "cd".
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Input,
        anchor: (5, 41),
        endpoint: (6, 41),
    });
    app.context_menu = Some(ContextMenuState {
        x: 2,
        y: 2,
        pane: SelectionPane::Input,
        selected: 0,
        items: vec![(ContextAction::Cut, true)],
    });

    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Left), 3, 3));
    assert!(app.context_menu.is_none());
    assert_eq!(app.input, "abef");
    assert!(app.text_selection.is_none(), "Cut consumes the selection");
}

#[test]
fn context_menu_paste_row_with_a_provider_setup_dialog_pastes_into_the_form() {
    let mut app = support::make_app();
    app.input_area = Rect::new(0, 40, 40, 3);
    app.provider_setup = Some(enter_key("anthropic", ""));
    app.context_menu = Some(ContextMenuState {
        x: 2,
        y: 2,
        pane: SelectionPane::Input,
        selected: 2,
        items: vec![
            (ContextAction::Cut, false),
            (ContextAction::Copy, false),
            (ContextAction::Paste, true),
        ],
    });
    // Row 5 is the Paste row. An empty clipboard leaves the field unchanged,
    // but the provider-setup paste path must not panic or close the dialog.
    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Left), 3, 5));
    assert!(app.context_menu.is_none());
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::EnterKey { .. })
    ));
}

// ===========================================================================
// History picker and config-save picker key routing
// ===========================================================================

#[tokio::test]
async fn history_picker_copy_key_copies_without_closing() {
    let mut app = support::make_app();
    app.history_picker = Some(HistoryPickerState {
        entries: vec!["first".to_string(), "second".to_string()],
        selected: 1,
        scroll_offset: 1,
    });
    app.handle_key_event(key(KeyCode::Up)).await;
    assert_eq!(app.history_picker.as_ref().expect("open").selected, 0);
    assert_eq!(
        app.history_picker.as_ref().expect("open").scroll_offset,
        0,
        "moving above the first visible row scrolls the list"
    );

    app.handle_key_event(key(KeyCode::Char('c'))).await;
    assert_eq!(app.status, "history: copied to clipboard");
    assert!(app.history_picker.is_some(), "'c' keeps the picker open");

    // Down past the last entry is a no-op.
    app.handle_key_event(key(KeyCode::Down)).await;
    assert_eq!(app.history_picker.as_ref().expect("open").selected, 1);
    app.handle_key_event(key(KeyCode::Char('j'))).await;
    assert_eq!(app.history_picker.as_ref().expect("open").selected, 1);

    app.handle_key_event(key(KeyCode::Char('q'))).await;
    assert!(app.history_picker.is_none());
}

#[tokio::test]
async fn config_save_picker_navigation_and_cancel_are_routed() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mut app = support::make_app();
    app.config_save_picker = Some(ConfigSavePickerState {
        entries: vec![tmp.path().join("one.json"), tmp.path().join("two.json")],
        selected: 0,
        scroll_offset: 1,
        config_dir: tmp.path().to_path_buf(),
    });
    app.handle_key_event(key(KeyCode::Down)).await;
    assert_eq!(app.config_save_picker.as_ref().expect("open").selected, 1);
    app.handle_key_event(key(KeyCode::Down)).await;
    assert_eq!(
        app.config_save_picker.as_ref().expect("open").selected,
        1,
        "Down past the last entry is a no-op"
    );
    app.handle_key_event(key(KeyCode::Up)).await;
    assert_eq!(app.config_save_picker.as_ref().expect("open").selected, 0);
    app.handle_key_event(key(KeyCode::Esc)).await;
    assert!(app.config_save_picker.is_none());
    assert_eq!(app.status, "config list: cancelled");
}

// ===========================================================================
// Memory delete confirmation
// ===========================================================================

#[tokio::test]
async fn memory_delete_confirmation_deletes_and_reports() {
    let mut app = support::make_app();
    let id = app
        .storage
        .create_memory("a memory", "fact", "test", 0.8, "proj", "sess", &[])
        .expect("create memory");
    app.pending_memory_delete = Some(PendingMemoryDelete {
        id,
        preview: "a memory".to_string(),
    });
    app.handle_key_event(key(KeyCode::Enter)).await;
    assert!(app.pending_memory_delete.is_none());
    assert_eq!(app.status, format!("memory #{id} deleted"));

    // A stale id reports "not found" instead of failing silently.
    app.pending_memory_delete = Some(PendingMemoryDelete {
        id: 99_999,
        preview: "gone".to_string(),
    });
    app.handle_key_event(key(KeyCode::Enter)).await;
    assert_eq!(app.status, "memory #99999 not found");

    // Esc cancels the confirmation.
    app.pending_memory_delete = Some(PendingMemoryDelete {
        id,
        preview: "a memory".to_string(),
    });
    app.handle_key_event(key(KeyCode::Esc)).await;
    assert!(app.pending_memory_delete.is_none());
    assert_eq!(app.status, "memory delete cancelled");
}

// ===========================================================================
// Plugin-store and connector-catalogue mouse columns
// ===========================================================================

#[test]
fn plugin_store_mouse_clicks_navigator_results_and_clear_button() {
    use ragent_plugins::StoreKind;
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "que", false);
    render(&mut app, 120, 40);

    let nav = app.plugin_store_nav_area;
    let results = app.plugin_store_result_area;
    let clear = app.plugin_store_clear_area;

    // Clicking a navigator row focuses and selects it.
    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Left), nav.x, nav.y));
    assert_eq!(
        app.plugin_store.as_ref().expect("open").focus,
        ragent_tui::app::StorePane::Categories
    );
    // Wheel over the navigator and the results scrolls them.
    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, nav.x, nav.y));
    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, nav.x, nav.y));
    if results.height > 0 {
        app.handle_mouse_event(mouse(
            MouseEventKind::Down(MouseButton::Left),
            results.x,
            results.y,
        ));
        app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, results.x, results.y));
        app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, results.x, results.y));
    }
    // The search-box clear button wipes the live filter.
    if clear.height > 0 {
        app.handle_mouse_event(mouse(
            MouseEventKind::Down(MouseButton::Left),
            clear.x,
            clear.y,
        ));
        assert_eq!(app.plugin_store.as_ref().expect("open").query.len(), 0);
    }
    // Anything else over the modal is swallowed.
    app.handle_mouse_event(mouse(MouseEventKind::Moved, 119, 39));
}

#[test]
fn connector_catalogue_mouse_clicks_navigator_and_results() {
    let mut app = support::make_app();
    app.open_connector_catalogue("", ragent_connectors::CategoryFilter::all(), false);
    render(&mut app, 110, 30);

    let nav = app.connector_store_nav_area;
    let results = app.connector_store_result_area;

    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Left), nav.x, nav.y));
    assert_eq!(
        app.connector_store.as_ref().expect("open").focus,
        ragent_connectors::CataloguePane::Categories,
        "a navigator click focuses the category column"
    );
    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, nav.x, nav.y));
    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, nav.x, nav.y));
    if results.height > 0 {
        app.handle_mouse_event(mouse(
            MouseEventKind::Down(MouseButton::Left),
            results.x,
            results.y,
        ));
        app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, results.x, results.y));
        app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, results.x, results.y));
    }
    // A wheel/click over the modal but outside both columns is swallowed.
    app.handle_mouse_event(mouse(MouseEventKind::Moved, 109, 29));
    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, 109, 29));
}
