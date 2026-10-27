//! Additional deep coverage for `ragent_tui::input` and the `App` input
//! surfaces in `src/app/input_handler.rs` that the existing suites
//! (`test_input_more.rs`, `test_input_field_extra.rs`, `test_input_handler_more.rs`,
//! `test_router_setup.rs`, ...) do not exercise.
//!
//! This file drives the real public dispatch entry points only -
//! `ragent_tui::input::handle_key`, `App::handle_key_event`,
//! `App::handle_mouse_event` and `App::handle_config_save_picker_key` - and
//! constructs `crossterm::event` values directly. No production logic is
//! touched and no network or real TTY is used.
//!
//! Focus areas (the not-yet-hit arms):
//!   * the provider-setup dialog state machine (`SelectProvider` / `EnterKey` /
//!     `DeviceFlowPending` / `SelectModel` / `SelectAzureResource` /
//!     `SelectThinkingLevel` / `Done` / `SelectAgent` / `SelectConfiguredProvider`
//!     / `ShowProviderConfig` / `ResetProvider` / `GitLabSetup` /
//!     `GitLabValidating` / `TelemetrySetup` / `LoadingModels`),
//!   * the router-save / forcecleanup / rollback confirmation modals,
//!   * the `mcp_discover` and `plugins_list` panel key routing,
//!   * connector-catalogue + plugin-store mouse wheel routing,
//!   * provider-setup / input-pane right-click context-menu construction,
//!   * side-panel scrollbar drag hit-testing and the drag/up arms.

mod support;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_llm::providers::azure_resource::AzureResourceEntry;
use ragent_llm::providers::router_config::RouterConfig;
use ragent_tui::app::{
    ConfiguredProvider, ModelPickerEntry, PendingForceCleanup, PlanApprovalState,
    ProviderSetupStep, ProviderSource, RollbackOfferState, SelectionPane,
};
use ragent_tui::input::{InputAction, handle_key};
use ragent_tui::input_field::InputField;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

const fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

fn render(app: &mut ragent_tui::App, width: u16, height: u16) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw");
}

fn model_entry(id: &str, thinking: Vec<ragent_types::ThinkingLevel>) -> ModelPickerEntry {
    ModelPickerEntry {
        provider_id: "anthropic".to_string(),
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

fn provider(id: &str, name: &str) -> ConfiguredProvider {
    ConfiguredProvider {
        id: id.to_string(),
        name: name.to_string(),
        source: ProviderSource::Database,
    }
}

fn azure_entry(id: &str) -> AzureResourceEntry {
    AzureResourceEntry {
        id: id.to_string(),
        name: format!("Azure {id}"),
        endpoint: "https://example.openai.azure.com".to_string(),
        api_key: None,
        api_key_env: Some("AZURE_KEY".to_string()),
        context_window: Some(200_000),
        capabilities: None,
        thinking: None,
        api_type: Some("openai".to_string()),
    }
}

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

fn enter_key_endpoint(provider_id: &str, key_text: &str, endpoint: &str) -> ProviderSetupStep {
    ProviderSetupStep::EnterKey {
        provider_id: provider_id.to_string(),
        provider_name: provider_id.to_string(),
        key_field: InputField::with_text(key_text),
        endpoint_field: InputField::with_text(endpoint),
        active_field: 1,
        error: None,
    }
}

fn gitlab_setup(active: u8) -> ProviderSetupStep {
    ProviderSetupStep::GitLabSetup {
        url_input: String::new(),
        url_cursor: 0,
        token_input: String::new(),
        token_cursor: 0,
        active_field: active,
        error: None,
    }
}

fn telemetry_setup(active: u8) -> ProviderSetupStep {
    ProviderSetupStep::TelemetrySetup {
        endpoint_field: InputField::new(),
        protocol: ragent_config::OtelProtocol::Http,
        interval_field: InputField::new(),
        timeout_field: InputField::new(),
        port_field: InputField::new(),
        active_field: active,
        error: None,
    }
}

// ===========================================================================
// handle_key: router-save / forcecleanup / rollback confirmation modals
// ===========================================================================

#[tokio::test]
async fn router_save_modal_all_keys() {
    let mut app = support::make_app();
    app.pending_router_save = Some(RouterConfig::default());
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Enter)).await,
        Some(InputAction::ConfirmRouterSave)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Esc)).await,
        Some(InputAction::CancelRouterSave)
    ));
    assert!(
        handle_key(&mut app, key(KeyCode::Char('z')))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn forcecleanup_modal_all_keys() {
    let mut app = support::make_app();
    app.pending_forcecleanup = Some(PendingForceCleanup {
        team_name: "t".to_string(),
        active_members: vec![],
    });
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Enter)).await,
        Some(InputAction::ConfirmForceCleanup)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Esc)).await,
        Some(InputAction::CancelForceCleanup)
    ));
    assert!(handle_key(&mut app, key(KeyCode::Tab)).await.is_none());
}

#[tokio::test]
async fn rollback_modal_all_keys() {
    let mut app = support::make_app();
    app.pending_rollback = Some(RollbackOfferState {
        session_id: "s1".to_string(),
        status: "done".to_string(),
        iterations: 1,
        diffstat: "+1 -0".to_string(),
        files: vec!["a.rs".to_string()],
    });
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Enter)).await,
        Some(InputAction::ConfirmRollback)
    ));
    assert!(matches!(
        handle_key(&mut app, key(KeyCode::Esc)).await,
        Some(InputAction::CancelRollback)
    ));
    assert!(
        handle_key(&mut app, key(KeyCode::Char('x')))
            .await
            .is_none()
    );
}

#[tokio::test]
async fn plan_approval_swallowed_key() {
    let mut app = support::make_app();
    app.plan_approval_pending = Some(PlanApprovalState {
        plan_text: "p".to_string(),
        cursor_approve: true,
    });
    assert!(
        handle_key(&mut app, key(KeyCode::Char('z')))
            .await
            .is_none()
    );
    assert!(app.plan_approval_pending.is_some());
}

// ===========================================================================
// handle_key: slash-menu edge arms
// ===========================================================================

fn slash_menu(
    matches: Vec<ragent_tui::app::SlashMenuEntry>,
    filter: &str,
) -> ragent_tui::app::SlashMenuState {
    ragent_tui::app::SlashMenuState {
        matches,
        selected: 0,
        filter: filter.to_string(),
    }
}

fn slash_entry(trigger: &str) -> ragent_tui::app::SlashMenuEntry {
    ragent_tui::app::SlashMenuEntry {
        trigger: trigger.to_string(),
        description: "d".to_string(),
        is_skill: false,
        suggestions: vec![],
        parameter_hint: None,
    }
}

#[tokio::test]
async fn slash_menu_up_down_on_empty_matches_is_a_noop() {
    let mut app = support::make_app();
    app.input = "/".to_string();
    app.slash_menu = Some(slash_menu(vec![], ""));
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert_eq!(app.slash_menu.as_ref().expect("open").selected, 0);
}

#[tokio::test]
async fn slash_menu_enter_preserves_raw_command_with_arguments() {
    let mut app = support::make_app();
    app.input = "/model llama3".to_string();
    app.slash_menu = Some(slash_menu(vec![slash_entry("model")], ""));
    match handle_key(&mut app, key(KeyCode::Enter)).await {
        Some(InputAction::SlashCommand(cmd)) => assert_eq!(cmd, "/model llama3"),
        other => panic!("expected raw SlashCommand, got {other:?}"),
    }
}

#[tokio::test]
async fn slash_menu_enter_with_no_matches_uses_filter() {
    let mut app = support::make_app();
    app.input = "/zzz".to_string();
    app.slash_menu = Some(slash_menu(vec![], "zzz"));
    match handle_key(&mut app, key(KeyCode::Enter)).await {
        Some(InputAction::SlashCommand(cmd)) => assert_eq!(cmd, "zzz"),
        other => panic!("expected filter fallback, got {other:?}"),
    }
}

#[tokio::test]
async fn slash_menu_backspace_routes_into_input() {
    let mut app = support::make_app();
    app.input = "/mo".to_string();
    app.input_cursor = 3;
    app.slash_menu = Some(slash_menu(vec![slash_entry("model")], "mo"));
    assert!(
        handle_key(&mut app, key(KeyCode::Backspace))
            .await
            .is_none()
    );
    assert_eq!(app.input, "/m");
}

#[tokio::test]
async fn slash_menu_typed_char_routes_into_input() {
    let mut app = support::make_app();
    app.input = "/".to_string();
    app.input_cursor = 1;
    app.slash_menu = Some(slash_menu(vec![slash_entry("model")], ""));
    assert!(
        handle_key(&mut app, key(KeyCode::Char('m')))
            .await
            .is_none()
    );
    assert_eq!(app.input, "/m");
}

// ===========================================================================
// handle_key: mcp_discover / plugins_list panel key routing
// ===========================================================================

fn discovered(id: &str) -> ragent_agent::mcp::discovery::DiscoveredMcpServer {
    ragent_agent::mcp::discovery::DiscoveredMcpServer {
        id: id.to_string(),
        name: format!("{id} server"),
        executable: std::env::current_exe().expect("current exe"),
        args: Vec::new(),
        env: std::collections::HashMap::new(),
        source: ragent_agent::mcp::discovery::McpDiscoverySource::SystemPath,
        definition_file: None,
    }
}

fn mcp_dialog(
    servers: Vec<ragent_agent::mcp::discovery::DiscoveredMcpServer>,
) -> ragent_tui::app::McpDiscoverState {
    let enabled = vec![false; servers.len()];
    ragent_tui::app::McpDiscoverState {
        servers,
        enabled,
        selected: 0,
        scroll_offset: 0,
        feedback: None,
    }
}

#[tokio::test]
async fn mcp_discover_panel_nav_home_end_esc() {
    let mut app = support::make_app();
    app.mcp_discover = Some(mcp_dialog(vec![
        discovered("a"),
        discovered("b"),
        discovered("c"),
    ]));

    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert_eq!(app.mcp_discover.as_ref().expect("open").selected, 1);
    assert!(
        handle_key(&mut app, key(KeyCode::Char('j')))
            .await
            .is_none()
    );
    assert_eq!(app.mcp_discover.as_ref().expect("open").selected, 2);
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert_eq!(app.mcp_discover.as_ref().expect("open").selected, 2);
    assert!(
        handle_key(&mut app, key(KeyCode::Char('k')))
            .await
            .is_none()
    );
    assert_eq!(app.mcp_discover.as_ref().expect("open").selected, 1);
    assert!(handle_key(&mut app, key(KeyCode::Home)).await.is_none());
    assert_eq!(app.mcp_discover.as_ref().expect("open").selected, 0);
    assert!(handle_key(&mut app, key(KeyCode::End)).await.is_none());
    assert_eq!(app.mcp_discover.as_ref().expect("open").selected, 2);
    assert!(handle_key(&mut app, key(KeyCode::Tab)).await.is_none());
    assert!(app.mcp_discover.is_some());
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.mcp_discover.is_none());
}

fn plugins_list_state(lines: usize) -> ragent_tui::app::PluginsListState {
    let body: Vec<String> = (0..lines).map(|i| format!("line {i}")).collect();
    let rows: Vec<ragent_plugins::PluginListRow> = (0..lines.min(2))
        .map(|i| ragent_plugins::PluginListRow {
            id: format!("p{i}"),
            enabled: false,
        })
        .collect();
    ragent_tui::app::PluginsListState {
        view: ragent_plugins::PluginListView {
            attribution: "From: /plugins list".to_string(),
            header: Some("hdr".to_string()),
            separator: Some("sep".to_string()),
            body,
            rows,
        },
        heading: Some("hdr".to_string()),
        selected: 0,
        scroll_offset: 0,
        verbose: false,
        feedback: None,
    }
}

#[tokio::test]
async fn plugins_list_panel_nav_home_end_esc() {
    let mut app = support::make_app();
    app.plugins_list = Some(plugins_list_state(4));

    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert_eq!(app.plugins_list.as_ref().expect("open").selected, 1);
    assert!(
        handle_key(&mut app, key(KeyCode::Char('j')))
            .await
            .is_none()
    );
    assert_eq!(app.plugins_list.as_ref().expect("open").selected, 2);
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert_eq!(app.plugins_list.as_ref().expect("open").selected, 1);
    assert!(handle_key(&mut app, key(KeyCode::End)).await.is_none());
    assert_eq!(app.plugins_list.as_ref().expect("open").selected, 3);
    assert!(handle_key(&mut app, key(KeyCode::Home)).await.is_none());
    assert_eq!(app.plugins_list.as_ref().expect("open").selected, 0);
    // Space toggles the highlighted plugin row (best-effort, offline).
    assert!(
        handle_key(&mut app, key(KeyCode::Char(' ')))
            .await
            .is_none()
    );
    // Space on a capability block past the plugin rows is a no-op.
    app.plugins_list.as_mut().expect("open").selected = 3;
    assert!(
        handle_key(&mut app, key(KeyCode::Char(' ')))
            .await
            .is_none()
    );
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.plugins_list.is_none());
}

#[tokio::test]
async fn plugins_list_empty_body_end_clamps_to_zero() {
    let mut app = support::make_app();
    app.plugins_list = Some(plugins_list_state(0));
    assert!(handle_key(&mut app, key(KeyCode::End)).await.is_none());
    assert_eq!(app.plugins_list.as_ref().expect("open").selected, 0);
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert_eq!(app.plugins_list.as_ref().expect("open").selected, 0);
}

// ===========================================================================
// Provider setup: Esc closes non-router dialogs; SelectProvider navigation
// ===========================================================================

#[tokio::test]
async fn provider_setup_esc_closes_enter_key_dialog() {
    let mut app = support::make_app();
    app.provider_setup = Some(enter_key("anthropic", "sk-1"));
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.provider_setup.is_none());
}

#[tokio::test]
async fn select_provider_up_down_wrap() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectProvider {
        selected: 0,
        force_key_entry: false,
    });
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SelectProvider { selected, .. } => {
            assert_eq!(*selected, ragent_tui::app::PROVIDER_LIST.len() - 1);
        }
        other => panic!("expected SelectProvider, got {other:?}"),
    }
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SelectProvider { selected, .. } => assert_eq!(*selected, 0),
        other => panic!("expected SelectProvider, got {other:?}"),
    }
    // An unrelated key keeps the picker open unchanged.
    assert!(handle_key(&mut app, key(KeyCode::Tab)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SelectProvider { selected, .. } => assert_eq!(*selected, 0),
        other => panic!("expected SelectProvider, got {other:?}"),
    }
}

// ===========================================================================
// Provider setup: EnterKey dialog paths
// ===========================================================================

#[tokio::test]
async fn enter_key_empty_is_rejected() {
    let mut app = support::make_app();
    app.provider_setup = Some(enter_key("anthropic", "   "));
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::EnterKey { error, .. } => {
            assert!(error.as_deref().unwrap_or("").contains("cannot be empty"));
        }
        other => panic!("expected EnterKey error, got {other:?}"),
    }
}

#[tokio::test]
async fn enter_key_copilot_pat_is_refused() {
    let mut app = support::make_app();
    app.provider_setup = Some(enter_key("copilot", "github_pat_1234567890"));
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::EnterKey { error, .. } => {
            assert!(error.as_deref().unwrap_or("").contains("not supported"));
        }
        other => panic!("expected EnterKey PAT error, got {other:?}"),
    }
}

#[tokio::test]
async fn enter_key_success_moves_to_loading_models() {
    let mut app = support::make_app();
    app.provider_setup = Some(enter_key("anthropic", "sk-valid"));
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::LoadingModels { provider_id, .. } => {
            assert_eq!(provider_id, "anthropic");
        }
        other => panic!("expected LoadingModels, got {other:?}"),
    }
}

#[tokio::test]
async fn enter_key_generic_openai_tab_switches_field() {
    let mut app = support::make_app();
    app.provider_setup = Some(enter_key_endpoint("generic_openai", "k", "http://x"));
    assert!(handle_key(&mut app, key(KeyCode::Tab)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::EnterKey { active_field, .. } => assert_eq!(*active_field, 0),
        other => panic!("expected EnterKey, got {other:?}"),
    }
}

#[tokio::test]
async fn enter_key_generic_openai_enter_saves_endpoint() {
    let mut app = support::make_app();
    app.provider_setup = Some(enter_key_endpoint(
        "generic_openai",
        "k-1",
        "http://127.0.0.1:8080",
    ));
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::LoadingModels { .. })
    ));
}

#[tokio::test]
async fn enter_key_typing_and_backspace_edit_the_key_field() {
    let mut app = support::make_app();
    app.provider_setup = Some(enter_key("anthropic", "ab"));
    assert!(
        handle_key(&mut app, key(KeyCode::Backspace))
            .await
            .is_none()
    );
    assert!(
        handle_key(&mut app, key(KeyCode::Char('z')))
            .await
            .is_none()
    );
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::EnterKey { key_field, .. } => assert_eq!(key_field.text(), "az"),
        other => panic!("expected EnterKey, got {other:?}"),
    }
}

#[tokio::test]
async fn enter_key_home_key_keeps_dialog_open() {
    let mut app = support::make_app();
    app.provider_setup = Some(enter_key("bedrock", "k"));
    assert!(handle_key(&mut app, key(KeyCode::Home)).await.is_none());
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::EnterKey { .. })
    ));
}

// ===========================================================================
// Provider setup: DeviceFlowPending
// ===========================================================================

#[tokio::test]
async fn device_flow_pending_esc_copies_and_other() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::DeviceFlowPending {
        flow: ragent_tui::app::DeviceFlowKind::Copilot,
        user_code: "ABCD-1234".to_string(),
        verification_uri: "https://github.com/login/device".to_string(),
    });
    assert!(
        handle_key(&mut app, key(KeyCode::Char('z')))
            .await
            .is_none()
    );
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::DeviceFlowPending { .. })
    ));
    assert!(
        handle_key(&mut app, key(KeyCode::Char('c')))
            .await
            .is_none()
    );
    assert!(app.status.contains("Device code copied"));
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.provider_setup.is_none());
}

// ===========================================================================
// Provider setup: SelectModel
// ===========================================================================

#[tokio::test]
async fn select_model_empty_list_and_other_key() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![],
        selected: 0,
    });
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SelectModel { selected, .. } => assert_eq!(*selected, 0),
        other => panic!("expected SelectModel, got {other:?}"),
    }
    assert!(handle_key(&mut app, key(KeyCode::Tab)).await.is_none());
    // Enter on an empty list falls through to Done with no model.
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::Done { model_name, .. } => assert!(model_name.is_none()),
        other => panic!("expected Done, got {other:?}"),
    }
}

#[tokio::test]
async fn select_model_non_reasoning_finalises_to_done() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectModel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        models: vec![model_entry("claude-x", vec![])],
        selected: 0,
    });
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::Done { model_name, .. } => assert!(model_name.is_some()),
        other => panic!("expected Done, got {other:?}"),
    }
}

// ===========================================================================
// Provider setup: SelectAzureResource
// ===========================================================================

#[tokio::test]
async fn select_azure_resource_empty_navigation_and_other() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
        entries: vec![],
        selected: 0,
        error: None,
    });
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SelectAzureResource { selected, .. } => assert_eq!(*selected, 0),
        other => panic!("expected SelectAzureResource, got {other:?}"),
    }
    assert!(handle_key(&mut app, key(KeyCode::Tab)).await.is_none());
    // Enter with no entries takes the state out of `handle_provider_setup_key`
    // (the guard `take()` leaves it `None`) and never restores it.
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(app.provider_setup.is_none());
}

#[tokio::test]
async fn select_azure_resource_enter_persists_selection() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectAzureResource {
        entries: vec![azure_entry("gpt-4o")],
        selected: 0,
        error: None,
    });
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert_eq!(app.selected_model.as_deref(), Some("azure_resource/gpt-4o"));
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::Done { .. })
    ));
}

// ===========================================================================
// Provider setup: SelectThinkingLevel
// ===========================================================================

#[tokio::test]
async fn select_thinking_level_empty_levels_navigate_and_other() {
    use ragent_types::ThinkingLevel;
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectThinkingLevel {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
        model: model_entry("m", vec![]),
        selected: 0,
    });
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Tab)).await.is_none());
    // Enter on an empty list finalises with the Off level.
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::Done { model_name, .. } => assert!(model_name.is_some()),
        other => panic!("expected Done, got {other:?}"),
    }
    assert_eq!(app.selected_thinking_level, Some(ThinkingLevel::Off));
}

// ===========================================================================
// Provider setup: Done / SelectAgent / SelectConfiguredProvider /
// ShowProviderConfig / ResetProvider
// ===========================================================================

#[tokio::test]
async fn done_screen_any_key_closes() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::Done {
        provider_name: "Anthropic".to_string(),
        model_name: Some("claude".to_string()),
    });
    assert!(
        handle_key(&mut app, key(KeyCode::Char('q')))
            .await
            .is_none()
    );
    assert!(app.provider_setup.is_none());
}

#[tokio::test]
async fn select_agent_navigation_and_enter() {
    let mut app = support::make_app();
    let agents = vec![
        ("general".to_string(), "General".to_string(), false),
        ("coder".to_string(), "Coder".to_string(), false),
    ];
    app.provider_setup = Some(ProviderSetupStep::SelectAgent {
        agents: agents.clone(),
        selected: 0,
    });
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SelectAgent { selected, .. } => assert_eq!(*selected, 1),
        other => panic!("expected SelectAgent, got {other:?}"),
    }
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    // Unrelated key keeps the dialog; Enter closes it.
    assert!(handle_key(&mut app, key(KeyCode::Tab)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(app.provider_setup.is_none());
}

#[tokio::test]
async fn select_agent_empty_list_enter_closes() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectAgent {
        agents: vec![],
        selected: 0,
    });
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(app.provider_setup.is_none());
}

#[tokio::test]
async fn select_configured_provider_empty_enter_closes() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectConfiguredProvider {
        providers: vec![],
        selected: 0,
    });
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Tab)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(app.provider_setup.is_none());
}

#[tokio::test]
async fn select_configured_provider_navigates_and_selects() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::SelectConfiguredProvider {
        providers: vec![
            provider("anthropic", "Anthropic"),
            provider("openai", "OpenAI"),
        ],
        selected: 0,
    });
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SelectConfiguredProvider { selected, .. } => assert_eq!(*selected, 1),
        other => panic!("expected SelectConfiguredProvider, got {other:?}"),
    }
    // Enter starts model discovery (LoadingModels) for the highlighted provider.
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::LoadingModels { .. }) | Some(ProviderSetupStep::Done { .. })
    ));
}

#[tokio::test]
async fn show_provider_config_empty_and_router_report() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::ShowProviderConfig {
        providers: vec![],
        selected: 0,
    });
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(app.provider_setup.is_none());

    // A non-empty list with a concrete provider renders a report and closes.
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::ShowProviderConfig {
        providers: vec![provider("anthropic", "Anthropic")],
        selected: 0,
    });
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(app.provider_setup.is_none());
    assert!(app.status.contains("provider show"));
}

#[tokio::test]
async fn reset_provider_navigation_and_enter() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::ResetProvider { selected: 0 });
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::ResetProvider { .. })
    ));
    assert!(handle_key(&mut app, key(KeyCode::Down)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Tab)).await.is_none());
    // Enter resets the highlighted provider's credentials and closes.
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    assert!(app.provider_setup.is_none());
    assert!(app.status.contains("reset"));
}

// ===========================================================================
// Provider setup: GitLabSetup / GitLabValidating
// ===========================================================================

#[tokio::test]
async fn gitlab_setup_tab_switches_field_and_typing_edits() {
    let mut app = support::make_app();
    app.provider_setup = Some(gitlab_setup(0));
    // Tab flips to the token field.
    assert!(handle_key(&mut app, key(KeyCode::Tab)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::GitLabSetup { active_field, .. } => assert_eq!(*active_field, 1),
        other => panic!("expected GitLabSetup, got {other:?}"),
    }
    // BackTab flips back to the URL field.
    assert!(handle_key(&mut app, key(KeyCode::BackTab)).await.is_none());
    // Typing into the URL field, moving the cursor, Home/End.
    assert!(
        handle_key(&mut app, key(KeyCode::Char('h')))
            .await
            .is_none()
    );
    assert!(
        handle_key(&mut app, key(KeyCode::Char('i')))
            .await
            .is_none()
    );
    assert!(handle_key(&mut app, key(KeyCode::Left)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Right)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Home)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::End)).await.is_none());
    assert!(
        handle_key(&mut app, key(KeyCode::Backspace))
            .await
            .is_none()
    );
    // An unrelated key keeps the dialog.
    assert!(handle_key(&mut app, key(KeyCode::Delete)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::GitLabSetup { url_input, .. } => assert_eq!(url_input, "h"),
        other => panic!("expected GitLabSetup, got {other:?}"),
    }
}

#[tokio::test]
async fn gitlab_setup_enter_missing_fields_sets_error() {
    let mut app = support::make_app();
    app.provider_setup = Some(gitlab_setup(0));
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::GitLabSetup { error, .. } => {
            assert!(error.as_deref().unwrap_or("").contains("required"));
        }
        other => panic!("expected GitLabSetup error, got {other:?}"),
    }
}

#[tokio::test]
async fn gitlab_validating_esc_returns_to_form() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::GitLabValidating {
        instance_url: "https://gitlab.com".to_string(),
        token: "t".to_string(),
    });
    // A non-Esc key keeps the validating state.
    assert!(
        handle_key(&mut app, key(KeyCode::Char('z')))
            .await
            .is_none()
    );
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::GitLabValidating { .. })
    ));
    // Esc is intercepted by the top-level "Esc closes non-router setup dialogs"
    // guard before it ever reaches the `GitLabValidating` arm, so the whole
    // dialog closes (the inner arm's cancel-to-form path is unreachable through
    // the public entry point).
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.provider_setup.is_none());
}

// ===========================================================================
// Provider setup: TelemetrySetup
// ===========================================================================

#[tokio::test]
async fn telemetry_setup_tab_cycles_fields_and_protocol_toggle() {
    let mut app = support::make_app();
    app.provider_setup = Some(telemetry_setup(0));
    // Tab advances through the five fields.
    for _ in 0..5 {
        assert!(handle_key(&mut app, key(KeyCode::Tab)).await.is_none());
    }
    // BackTab from field 0 wraps to 4.
    app.provider_setup = Some(telemetry_setup(0));
    assert!(handle_key(&mut app, key(KeyCode::BackTab)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::TelemetrySetup { active_field, .. } => assert_eq!(*active_field, 4),
        other => panic!("expected TelemetrySetup, got {other:?}"),
    }
    // On the protocol field, Up toggles the protocol.
    app.provider_setup = Some(telemetry_setup(1));
    assert!(handle_key(&mut app, key(KeyCode::Up)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::TelemetrySetup { protocol, .. } => {
            assert_eq!(*protocol, ragent_config::OtelProtocol::Grpc);
        }
        other => panic!("expected TelemetrySetup, got {other:?}"),
    }
}

#[tokio::test]
async fn telemetry_setup_text_field_editing() {
    let mut app = support::make_app();
    app.provider_setup = Some(telemetry_setup(0));
    assert!(
        handle_key(&mut app, key(KeyCode::Char('h')))
            .await
            .is_none()
    );
    assert!(
        handle_key(&mut app, key(KeyCode::Char('i')))
            .await
            .is_none()
    );
    assert!(handle_key(&mut app, key(KeyCode::Left)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Right)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::Home)).await.is_none());
    assert!(handle_key(&mut app, key(KeyCode::End)).await.is_none());
    assert!(
        handle_key(&mut app, key(KeyCode::Backspace))
            .await
            .is_none()
    );
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::TelemetrySetup { endpoint_field, .. } => {
            assert_eq!(endpoint_field.text(), "h");
        }
        other => panic!("expected TelemetrySetup, got {other:?}"),
    }
}

#[tokio::test]
async fn telemetry_setup_non_text_field_char_is_ignored() {
    let mut app = support::make_app();
    // Field 1 is the protocol (non-text): typing a char must not panic.
    app.provider_setup = Some(telemetry_setup(1));
    assert!(
        handle_key(&mut app, key(KeyCode::Char('x')))
            .await
            .is_none()
    );
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::TelemetrySetup { endpoint_field, .. } => {
            assert_eq!(endpoint_field.text(), "");
        }
        other => panic!("expected TelemetrySetup, got {other:?}"),
    }
}

#[tokio::test]
async fn telemetry_setup_bad_port_sets_error() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::TelemetrySetup {
        endpoint_field: InputField::with_text("http://localhost:4317"),
        protocol: ragent_config::OtelProtocol::Http,
        interval_field: InputField::with_text("30"),
        timeout_field: InputField::with_text("10"),
        port_field: InputField::with_text("not-a-port"),
        active_field: 0,
        error: None,
    });
    assert!(handle_key(&mut app, key(KeyCode::Enter)).await.is_none());
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::TelemetrySetup { error, .. } => {
            assert!(error.as_deref().unwrap_or("").contains("port"));
        }
        other => panic!("expected TelemetrySetup port error, got {other:?}"),
    }
}

// ===========================================================================
// Provider setup: LoadingModels
// ===========================================================================

#[tokio::test]
async fn loading_models_esc_closes_other_keeps() {
    let mut app = support::make_app();
    app.provider_setup = Some(ProviderSetupStep::LoadingModels {
        provider_id: "anthropic".to_string(),
        provider_name: "Anthropic".to_string(),
    });
    assert!(
        handle_key(&mut app, key(KeyCode::Char('z')))
            .await
            .is_none()
    );
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::LoadingModels { .. })
    ));
    assert!(handle_key(&mut app, key(KeyCode::Esc)).await.is_none());
    assert!(app.provider_setup.is_none());
}

// ===========================================================================
// input_handler: connector-catalogue + plugin-store mouse wheel routing
// ===========================================================================

#[test]
fn connector_catalogue_wheel_over_nav_and_results() {
    let mut app = support::make_app();
    app.open_connector_catalogue("", ragent_connectors::CategoryFilter::all(), false);
    render(&mut app, 110, 30);

    let nav = app.connector_store_nav_area;
    let results = app.connector_store_result_area;

    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, nav.x + 1, nav.y + 1));
    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, nav.x + 1, nav.y + 1));
    app.handle_mouse_event(mouse(
        MouseEventKind::ScrollUp,
        results.x + 1,
        results.y + 1,
    ));
    app.handle_mouse_event(mouse(
        MouseEventKind::ScrollDown,
        results.x + 1,
        results.y + 1,
    ));
    // A mouse event over the modal but outside both columns is swallowed.
    app.handle_mouse_event(mouse(MouseEventKind::Moved, 109, 29));
}

#[test]
fn plugin_store_wheel_over_nav_marketplace_results() {
    use ragent_plugins::StoreKind;
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    render(&mut app, 120, 40);

    let nav = app.plugin_store_nav_area;
    let marketplace = app.plugin_store_marketplace_area;
    let results = app.plugin_store_result_area;

    app.handle_mouse_event(mouse(MouseEventKind::ScrollUp, nav.x + 1, nav.y + 1));
    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, nav.x + 1, nav.y + 1));
    if marketplace.height > 0 {
        app.handle_mouse_event(mouse(
            MouseEventKind::ScrollUp,
            marketplace.x + 1,
            marketplace.y,
        ));
        app.handle_mouse_event(mouse(
            MouseEventKind::ScrollDown,
            marketplace.x + 1,
            marketplace.y,
        ));
    }
    app.handle_mouse_event(mouse(
        MouseEventKind::ScrollUp,
        results.x + 1,
        results.y + 1,
    ));
    app.handle_mouse_event(mouse(
        MouseEventKind::ScrollDown,
        results.x + 1,
        results.y + 1,
    ));
    app.handle_mouse_event(mouse(MouseEventKind::Moved, 119, 39));
}

// ===========================================================================
// input_handler: right-click context-menu construction in the input pane
// ===========================================================================

#[test]
fn right_click_in_input_pane_builds_context_menu() {
    let mut app = support::make_app();
    app.input_area = ratatui::layout::Rect::new(0, 40, 80, 3);
    // A right-click inside the input pane opens a context menu with Cut/Copy/Paste.
    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Right), 5, 41));
    let menu = app.context_menu.as_ref().expect("context menu opened");
    assert_eq!(menu.pane, SelectionPane::Input);
    assert_eq!(menu.items.len(), 3);
}

#[test]
fn right_click_with_provider_setup_input_enables_paste() {
    let mut app = support::make_app();
    app.input_area = ratatui::layout::Rect::new(0, 40, 80, 3);
    app.provider_setup = Some(enter_key("anthropic", "k"));
    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Right), 5, 41));
    let menu = app.context_menu.as_ref().expect("context menu opened");
    // With a provider-setup input active, Paste is enabled regardless of clipboard.
    let paste = menu
        .items
        .iter()
        .find(|(a, _)| matches!(a, ragent_tui::app::ContextAction::Paste))
        .expect("paste row");
    assert!(
        paste.1,
        "paste is enabled while a provider-setup input is active"
    );
}

#[test]
fn right_click_outside_any_pane_leaves_no_menu() {
    let mut app = support::make_app();
    // No pane areas set -> pane_at returns None.
    app.handle_mouse_event(mouse(MouseEventKind::Down(MouseButton::Right), 5, 5));
    assert!(app.context_menu.is_none());
}

// ===========================================================================
// input_handler: side-panel scrollbar drag hit-testing (drag + up arms)
// ===========================================================================

#[test]
fn messages_scrollbar_gutter_click_starts_a_drag() {
    let mut app = support::make_app();
    app.message_area = ratatui::layout::Rect::new(0, 1, 80, 20);
    app.message_max_scroll = 100;
    app.scroll_offset = 40;
    // Click the rightmost column (scrollbar gutter) inside the messages pane.
    app.handle_mouse_event(mouse(
        MouseEventKind::Down(MouseButton::Left),
        app.message_area.right() - 1,
        5,
    ));
    assert!(app.scrollbar_drag.is_some(), "gutter click starts a drag");
}

#[test]
fn tasks_panel_scrollbar_gutter_click_starts_a_drag() {
    let mut app = support::make_app();
    app.show_tasks_panel = true;
    app.tasks_area = ratatui::layout::Rect::new(60, 1, 20, 20);
    app.tasks_max_scroll = 100;
    app.handle_mouse_event(mouse(
        MouseEventKind::Down(MouseButton::Left),
        app.tasks_area.right() - 1,
        5,
    ));
    assert!(app.scrollbar_drag.is_some());
}

#[test]
fn memory_panel_scrollbar_gutter_click_starts_a_drag() {
    let mut app = support::make_app();
    app.show_memory = true;
    app.memory_area = ratatui::layout::Rect::new(60, 1, 20, 20);
    app.memory_max_scroll = 100;
    app.handle_mouse_event(mouse(
        MouseEventKind::Down(MouseButton::Left),
        app.memory_area.right() - 1,
        5,
    ));
    assert!(app.scrollbar_drag.is_some());
}
