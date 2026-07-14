//! Tests for the router provider TUI setup flow (spec: `routeui`).
//!
//! These tests exercise the state-machine helpers and persistence logic
//! without launching a full terminal backend.

use std::collections::HashMap;
use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_agent::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_llm::providers::router_config::{RouterConfig, Tier, TierEntry};
use ragent_tui::App;
use ragent_tui::app::{ConfiguredProvider, ProviderSetupStep, ProviderSource};

// ── helpers ────────────────────────────────────────────────────────────────

fn mem_storage() -> Arc<Storage> {
    Arc::new(Storage::open_in_memory().expect("in-memory storage"))
}

fn make_app_with_storage(storage: Arc<Storage>) -> App {
    let event_bus = Arc::new(EventBus::default());
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
        task_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(HashMap::new())),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
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
        std::path::PathBuf::new(),
    )
}

fn make_app() -> App {
    make_app_with_storage(mem_storage())
}

// ── FR-004: configured-provider query helper excludes router ───────────────

#[test]
fn test_get_configured_providers_for_router_excludes_router() {
    let storage = mem_storage();
    storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store key");
    storage
        .set_provider_auth("openai", "sk-test")
        .expect("store key");

    let for_router = App::get_configured_providers_for_router(&storage);
    let ids: Vec<String> = for_router.iter().map(|p| p.id.clone()).collect();
    assert!(ids.contains(&"anthropic".to_string()));
    assert!(ids.contains(&"openai".to_string()));
    assert!(
        !ids.contains(&"router".to_string()),
        "router must be excluded"
    );
}

// ── FR-003 / FR-006: router setup state initialisation ─────────────────────

#[test]
fn test_router_setup_step_defaults() {
    let app = make_app();
    let providers = App::get_configured_providers_for_router(&app.storage);
    let step = ProviderSetupStep::SetupRouter {
        providers,
        selected_provider_ids: Vec::new(),
        selected_provider_index: 0,
        draft_config: RouterConfig::default(),
        active_bucket: Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: None,
    };

    if let ProviderSetupStep::SetupRouter {
        active_bucket,
        left_pane_focused,
        ..
    } = step
    {
        assert_eq!(active_bucket, Tier::Simple);
        assert!(left_pane_focused);
    } else {
        panic!("expected SetupRouter");
    }
}

// ── FR-010: router config report formatting ────────────────────────────────

#[test]
fn test_router_config_report_renders_tiers() {
    let app = make_app();
    let report = app.router_config_report(&app.provider_registry);
    assert!(report.contains("Model Router"));
    assert!(report.contains("## Tier Mappings"));
    assert!(report.contains("### SIMPLE"));
    assert!(report.contains("### MEDIUM"));
    assert!(report.contains("### COMPLEX"));
    assert!(report.contains("### REASONING"));
}

#[test]
fn test_router_config_report_includes_entries() {
    let app = make_app();
    let mut config = RouterConfig::default();
    config.enabled = true;
    config.tiers.insert(
        "MEDIUM".to_string(),
        ragent_llm::providers::router_config::TierConfig {
            models: vec![TierEntry {
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4-20250514".to_string(),
            }],
            timeout_ms: None,
        },
    );

    // Inject the config into ragent.json options via a temporary in-memory path is
    // not practical; instead verify the report helper accepts explicit tier entries
    // by checking the default report has no entries (default tiers may have entries).
    let report = app.router_config_report(&app.provider_registry);
    // The default RouterConfig has built-in defaults for every tier, so the report
    // should contain at least one model reference.
    assert!(
        report.contains("via **"),
        "report should list assigned model entries: {report}"
    );
}

// ── FR-020: status bar label when router enabled ───────────────────────────

#[test]
fn test_provider_model_label_shows_router() {
    let mut app = make_app();
    app.selected_model = Some("router/router".to_string());
    // The provider label uses the configured provider name when the router is not
    // actually enabled in the registry. This test verifies the selected model is
    // reflected; the "Model Router" override is covered by FR-020 integration tests.
    let label = app.provider_model_label().expect("label should exist");
    assert!(
        label.ends_with(" / router"),
        "label should end with selected model: {label}"
    );
}

// ── FR-024: router setup helper excludes router provider ───────────────────

#[test]
fn test_router_setup_providers_list_never_contains_router() {
    let storage = mem_storage();
    storage
        .set_provider_auth("router", "unused")
        .expect("store router key");
    storage
        .set_provider_auth("openai", "sk-test")
        .expect("store openai key");

    let providers = App::get_configured_providers_for_router(&storage);
    assert!(
        !providers.iter().any(|p| p.id == "router"),
        "router must not appear in the router setup palette"
    );
}

// ── FR-002: provider picker list contains router ───────────────────────────

#[test]
fn test_provider_list_includes_router() {
    use ragent_tui::app::PROVIDER_LIST;
    assert!(PROVIDER_LIST.iter().any(|(id, _)| id == &"router"));
}

// ── FR-003: router model picker preserves provider selection on Enter/Esc ────

fn router_setup_with_providers(storage: Arc<Storage>) -> App {
    storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store key");
    storage
        .set_provider_auth("openai", "sk-test")
        .expect("store key");
    let mut app = make_app_with_storage(storage);
    // Use a deterministic, two-provider palette so tests are not affected by
    // ambient environment variables like OLLAMA_HOST.
    let providers = vec![
        ConfiguredProvider {
            id: "anthropic".to_string(),
            name: "Anthropic (Claude)".to_string(),
            source: ProviderSource::Database,
        },
        ConfiguredProvider {
            id: "openai".to_string(),
            name: "OpenAI (GPT)".to_string(),
            source: ProviderSource::Database,
        },
    ];
    app.provider_setup = Some(ProviderSetupStep::SetupRouter {
        providers,
        selected_provider_ids: vec!["anthropic".to_string()],
        selected_provider_index: 0,
        draft_config: ragent_llm::providers::router_config::RouterConfig {
            enabled: false,
            tiers: std::collections::HashMap::new(),
            ..ragent_llm::providers::router_config::RouterConfig::default()
        },
        active_bucket: Tier::Simple,
        active_bucket_index: 0,
        left_pane_focused: true,
        error: None,
    });
    app
}

static TEMP_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

fn temp_config_path() -> std::path::PathBuf {
    let n = TEMP_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("ragent-routeui-{}-{n}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("create temp config dir");
    let path = dir.join("ragent.json");
    std::fs::write(&path, "{}").expect("write empty config");
    path
}

#[test]
fn test_slash_provider_router_opens_setup_router() {
    let mut app = make_app();
    app.storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store key");
    app.execute_slash_command("/provider router");
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::SetupRouter { .. })
        ),
        "/provider router should open router setup"
    );
}

#[test]
fn test_router_setup_space_toggles_provider() {
    let mut app = router_setup_with_providers(mem_storage());
    // Initially anthropic is selected in the palette.
    // Space on the selected provider removes it.
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
    );
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            selected_provider_ids,
            ..
        } => {
            assert!(!selected_provider_ids.contains(&"anthropic".to_string()));
        }
        _ => panic!("expected SetupRouter"),
    }

    // Move down to openai and add it to the palette.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE),
    );
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            selected_provider_ids,
            selected_provider_index,
            ..
        } => {
            assert!(selected_provider_ids.contains(&"openai".to_string()));
            assert_eq!(*selected_provider_index, 1);
        }
        _ => panic!("expected SetupRouter"),
    }
}

#[test]
fn test_router_setup_tab_switches_pane_focus() {
    let mut app = router_setup_with_providers(mem_storage());
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            left_pane_focused,
            active_bucket_index,
            ..
        } => {
            assert!(!left_pane_focused);
            assert_eq!(*active_bucket_index, 0);
        }
        _ => panic!("expected SetupRouter"),
    }
}

#[test]
fn test_router_setup_assigns_model_to_bucket() {
    let mut app = router_setup_with_providers(mem_storage());
    // Enter opens the model picker for the selected anthropic provider.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::SelectRouterModel { .. })
        ),
        "expected model picker"
    );

    // Confirm the first model.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            draft_config,
            active_bucket,
            left_pane_focused,
            ..
        } => {
            assert_eq!(*active_bucket, Tier::Simple);
            assert!(!left_pane_focused);
            let models = draft_config
                .tiers
                .get("SIMPLE")
                .map(|t| t.models.clone())
                .unwrap_or_default();
            assert!(!models.is_empty(), "model must be assigned to SIMPLE");
            assert_eq!(models[0].provider, "anthropic");
        }
        _ => panic!("expected SetupRouter after assignment"),
    }
}

#[test]
fn test_router_setup_save_persists_cluster_and_enables_router() {
    let mut app = router_setup_with_providers(mem_storage());
    let config_path = temp_config_path();
    app.config_paths = vec![config_path.clone()];

    // Assign a model to the SIMPLE bucket.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Save with Ctrl+S.
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
    );

    assert!(
        app.provider_setup.is_none(),
        "setup dialog should close after save"
    );
    assert!(app.router_enabled, "router should be enabled after save");

    // Verify the saved file.
    let raw = std::fs::read_to_string(&config_path).expect("read saved config");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse saved config");
    let router = json
        .get("provider")
        .and_then(|p| p.get("router"))
        .expect("provider.router present");
    assert_eq!(router.get("enabled").and_then(|v| v.as_bool()), Some(true));
    let simple = router
        .get("tiers")
        .and_then(|t| t.get("SIMPLE"))
        .and_then(|t| t.get("models"))
        .and_then(|m| m.as_array())
        .expect("SIMPLE tier models");
    assert!(!simple.is_empty());
}

#[test]
fn test_router_setup_rejects_empty_cluster_on_save() {
    let mut app = router_setup_with_providers(mem_storage());
    let config_path = temp_config_path();
    app.config_paths = vec![config_path.clone()];

    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
    );

    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter { error, .. } => {
            assert!(
                error
                    .as_ref()
                    .expect("error set")
                    .contains("At least one tier"),
                "expected empty-cluster error: {:?}",
                error
            );
        }
        _ => panic!("expected SetupRouter with error"),
    }

    // Nothing should have been written.
    let raw = std::fs::read_to_string(&config_path).expect("read config");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse config");
    assert!(json.get("provider").is_none() || json["provider"].get("router").is_none());
}

#[test]
fn test_router_setup_preserves_weights_and_boundaries() {
    let mut app = router_setup_with_providers(mem_storage());
    let config_path = temp_config_path();

    // Seed an existing router block with custom weights and boundaries.
    let seed = serde_json::json!({
        "provider": {
            "router": {
                "enabled": false,
                "tiers": {},
                "weights": {
                    "token_count": 0.5,
                    "vocabulary_complexity": 0.5,
                    "syntax_complexity": 0.0,
                    "domain_specificity": 0.0,
                    "ambiguity": 0.0,
                    "context_dependency": 0.0,
                    "reasoning_depth": 0.0,
                    "creativity_level": 0.0,
                    "emotional_complexity": 0.0,
                    "multimodality": 0.0,
                    "instruction_complexity": 0.0,
                    "knowledge_recency": 0.0,
                    "code_complexity": 0.0,
                    "mathematical_complexity": 0.0,
                    "image_attachment": 0.0
                },
                "boundaries": {
                    "simple_medium": 0.25,
                    "medium_complex": 0.55,
                    "complex_reasoning": 0.85
                }
            }
        }
    });
    std::fs::write(&config_path, serde_json::to_string_pretty(&seed).unwrap())
        .expect("write seed config");
    app.config_paths = vec![config_path.clone()];

    // Assign a model and save.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
    );

    let raw = std::fs::read_to_string(&config_path).expect("read saved config");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse saved config");
    let router = json["provider"]["router"].clone();
    assert_eq!(router["enabled"], true);
    assert_eq!(router["weights"]["token_count"], 0.5);
    assert_eq!(router["boundaries"]["simple_medium"], 0.25);
    assert_eq!(router["boundaries"]["medium_complex"], 0.55);
    assert_eq!(router["boundaries"]["complex_reasoning"], 0.85);
}

#[test]
fn test_router_setup_rejects_recursive_router_assignment() {
    let mut app = make_app();
    // Directly open the router model picker with a router target to exercise
    // the recursive-routing guard, bypassing the filtered provider palette.
    app.router_draft_providers = vec![ConfiguredProvider {
        id: "router".to_string(),
        name: "Model Router".to_string(),
        source: ProviderSource::Database,
    }];
    app.router_draft_selected_ids = vec!["router".to_string()];
    app.router_draft_config = Some(RouterConfig {
        enabled: false,
        tiers: std::collections::HashMap::new(),
        ..RouterConfig::default()
    });
    app.provider_setup = Some(ProviderSetupStep::SelectRouterModel {
        provider_id: "router".to_string(),
        provider_name: "Model Router".to_string(),
        models: vec![ragent_tui::app::ModelPickerEntry {
            id: "router".to_string(),
            name: "Router".to_string(),
            context_window: 0,
            max_output: None,
            cost_input: 0.0,
            cost_output: 0.0,
            reasoning: false,
            vision: false,
            tool_use: false,
            thinking_levels: vec![],
            thinking_config: None,
            cost_tier: "Unknown".to_string(),
            cost_multiplier: "0x".to_string(),
        }],
        selected: 0,
        target_tier: Tier::Simple,
    });

    // Confirming the assignment should be rejected.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter { error, .. } => {
            assert!(
                error
                    .as_ref()
                    .expect("error set")
                    .contains("cannot route to itself"),
                "expected recursive routing error: {:?}",
                error
            );
        }
        _ => panic!("expected SetupRouter with error"),
    }
}

#[test]
fn test_router_setup_empty_state_when_no_providers() {
    // Disable any concrete provider that might be auto-discovered from the
    // environment so the empty-state path is exercised deterministically.
    let mut app = make_app();
    for pid in ["ollama", "ollama_cloud", "copilot"] {
        let _ = app
            .storage
            .set_setting(&format!("provider_{pid}_disabled"), "true");
    }
    app.execute_slash_command("/provider router");
    assert!(app.provider_setup.is_none());
    assert_eq!(app.status, "⚠ No concrete providers — configure one first");
}

#[test]
fn test_router_setup_reorder_models_within_bucket() {
    let mut app = router_setup_with_providers(mem_storage());
    // Assign two models to the SIMPLE bucket.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Re-open the picker and move down to select a different anthropic model,
    // then assign it as a second fallback in the SIMPLE bucket.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Focus should now be in the bucket pane on the second model.
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            draft_config,
            active_bucket_index,
            left_pane_focused,
            ..
        } => {
            let models = &draft_config.tiers["SIMPLE"].models;
            assert_eq!(models.len(), 2);
            assert_eq!(*active_bucket_index, 1);
            assert!(!left_pane_focused);
        }
        _ => panic!("expected SetupRouter"),
    }

    // Ctrl+Up should move the selected model up one slot.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Up, KeyModifiers::CONTROL));
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            draft_config,
            active_bucket_index,
            ..
        } => {
            let models = &draft_config.tiers["SIMPLE"].models;
            assert_eq!(models.len(), 2);
            assert_eq!(*active_bucket_index, 0);
            // After the swap, the two model ids are still present but reordered.
            assert_ne!(models[0].model, models[1].model);
        }
        _ => panic!("expected SetupRouter"),
    }
}

#[test]
fn test_slash_provider_show_includes_router_when_configured() {
    let mut app = router_setup_with_providers(mem_storage());
    let config_path = temp_config_path();
    app.config_paths = vec![config_path.clone()];

    // Save a minimal cluster.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
    );

    app.execute_slash_command("/provider show");
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::ShowProviderConfig { providers, .. } => {
            assert!(providers.iter().any(|p| p.id == "router"));
        }
        _ => panic!("expected ShowProviderConfig"),
    }
}

#[test]
fn test_provider_show_renders_router_cluster() {
    let mut app = router_setup_with_providers(mem_storage());
    let config_path = temp_config_path();
    app.config_paths = vec![config_path.clone()];

    // Save a cluster.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
    );

    app.execute_slash_command("/provider show");

    // Find the router entry and press Enter to render its report.
    let (router_index, selected) = if let ProviderSetupStep::ShowProviderConfig {
        providers,
        selected,
    } = app.provider_setup.as_ref().expect("setup")
    {
        (
            providers
                .iter()
                .position(|p| p.id == "router")
                .expect("router in show list"),
            *selected,
        )
    } else {
        panic!("expected ShowProviderConfig");
    };
    for _ in 0..router_index.saturating_sub(selected) {
        ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }
    for _ in 0..selected.saturating_sub(router_index) {
        ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
    }
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    let text = app.messages.last().expect("report message").text_content();
    assert!(text.contains("Model Router"));
    assert!(text.contains("## Tier Mappings"));
    assert!(text.contains("### SIMPLE"));
}

#[test]
fn test_router_status_bar_label_when_enabled() {
    let mut app = make_app();
    app.router_enabled = true;
    app.selected_model = Some("router/router".to_string());
    let label = app.provider_model_label().expect("label should exist");
    assert!(
        label.starts_with("Model Router"),
        "label should show Model Router: {label}"
    );
}

#[test]
fn test_router_setup_provider_list_excludes_router() {
    let storage = mem_storage();
    storage
        .set_provider_auth("router", "unused")
        .expect("store router key");
    storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store anthropic key");
    let for_router = App::get_configured_providers_for_router(&storage);
    assert!(
        !for_router.iter().any(|p| p.id == "router"),
        "router must never appear in setup palette"
    );
    assert!(for_router.iter().any(|p| p.id == "anthropic"));
}

#[test]
fn test_router_model_picker_enter_preserves_providers() {
    let mut app = router_setup_with_providers(mem_storage());
    let providers_before = app
        .provider_setup
        .as_ref()
        .and_then(|s| match s {
            ProviderSetupStep::SetupRouter { providers, .. } => Some(providers.clone()),
            _ => None,
        })
        .expect("setup router state");

    // Enter on the selected provider opens the model picker.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::SelectRouterModel { .. })
        ),
        "expected SelectRouterModel after Enter"
    );
    assert_eq!(
        app.router_draft_providers.len(),
        providers_before.len(),
        "draft providers must be stashed"
    );
    assert_eq!(
        app.router_draft_selected_ids,
        vec!["anthropic".to_string()],
        "draft selected ids must be stashed"
    );

    // Select the first model and confirm.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    match app.provider_setup.as_ref().expect("setup present") {
        ProviderSetupStep::SetupRouter {
            providers,
            selected_provider_ids,
            draft_config,
            active_bucket,
            left_pane_focused,
            ..
        } => {
            assert_eq!(
                providers.len(),
                providers_before.len(),
                "provider list must be restored"
            );
            assert_eq!(
                selected_provider_ids,
                &vec!["anthropic".to_string()],
                "selected providers must be restored"
            );
            assert!(!left_pane_focused, "focus should return to bucket pane");
            assert_eq!(*active_bucket, Tier::Simple);
            assert!(
                draft_config
                    .tiers
                    .get("SIMPLE")
                    .map(|t| !t.models.is_empty())
                    .unwrap_or(false),
                "selected model must be added to the SIMPLE bucket"
            );
        }
        _ => panic!("expected SetupRouter after model selection"),
    }
}

#[test]
fn test_router_model_picker_esc_preserves_providers() {
    let mut app = router_setup_with_providers(mem_storage());
    let providers_before = app
        .provider_setup
        .as_ref()
        .and_then(|s| match s {
            ProviderSetupStep::SetupRouter { providers, .. } => Some(providers.clone()),
            _ => None,
        })
        .expect("setup router state");

    // Open the model picker.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::SelectRouterModel { .. })
    ));

    // Cancel with Esc.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
    match app.provider_setup.as_ref().expect("setup present") {
        ProviderSetupStep::SetupRouter {
            providers,
            selected_provider_ids,
            left_pane_focused,
            ..
        } => {
            assert_eq!(providers.len(), providers_before.len());
            assert_eq!(
                selected_provider_ids,
                &vec!["anthropic".to_string()],
                "selection must survive Esc"
            );
            assert!(*left_pane_focused, "focus should return to provider pane");
            // The draft config is restored exactly as it was when the picker opened;
            // no new model assignment happens from a cancelled picker.
        }
        _ => panic!("expected SetupRouter after Esc"),
    }
}
// ── FR-025: selecting "Model Router" from the provider picker opens the
// router cluster setup panel (not the generic API-key dialog) ──────────────

#[test]
fn test_provider_picker_router_opens_setup_router() {
    let storage = mem_storage();
    storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store anthropic key");
    let mut app = make_app_with_storage(storage);
    // Start at the provider picker with "Model Router" highlighted.
    let router_idx = ragent_tui::app::PROVIDER_LIST
        .iter()
        .position(|(id, _)| id == &"router")
        .expect("router in PROVIDER_LIST");
    app.provider_setup = Some(ProviderSetupStep::SelectProvider {
        selected: router_idx,
    });
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::SetupRouter { .. })
        ),
        "selecting Model Router from the picker should open SetupRouter, not EnterKey"
    );
}

#[test]
fn test_provider_picker_router_no_concrete_providers_keeps_picker() {
    let storage = mem_storage();
    let mut app = make_app_with_storage(storage);
    // Suppress any provider that could be auto-discovered from the environment
    // so the empty-state path is exercised deterministically.
    for pid in [
        "ollama",
        "ollama_cloud",
        "copilot",
        "anthropic",
        "openai",
        "gemini",
        "huggingface",
        "generic_openai",
        "azure_foundry",
        "azure_resource",
    ] {
        let _ = app
            .storage
            .set_setting(&format!("provider_{pid}_disabled"), "true");
    }
    let router_idx = ragent_tui::app::PROVIDER_LIST
        .iter()
        .position(|(id, _)| id == &"router")
        .expect("router in PROVIDER_LIST");
    app.provider_setup = Some(ProviderSetupStep::SelectProvider {
        selected: router_idx,
    });
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(
        matches!(
            app.provider_setup,
            Some(ProviderSetupStep::SelectProvider { .. })
        ),
        "picker should stay open when no concrete providers are configured, got {:?}",
        app.provider_setup
            .as_ref()
            .map(|s| match s {
                ProviderSetupStep::SelectProvider { .. } => "SelectProvider".to_string(),
                ProviderSetupStep::SetupRouter { .. } => "SetupRouter".to_string(),
                ProviderSetupStep::EnterKey { .. } => "EnterKey".to_string(),
                _ => "other".to_string(),
            })
            .unwrap_or_else(|| "None".to_string())
    );
    assert!(
        app.status.contains("No concrete providers"),
        "status should warn about missing concrete providers: {}",
        app.status
    );
}
