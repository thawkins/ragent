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
use ratatui::{Terminal, backend::TestBackend};

fn discovered_model(provider_id: &str, id: &str, name: &str) -> ragent_agent::provider::ModelInfo {
    ragent_agent::provider::ModelInfo {
        id: id.to_string(),
        provider_id: provider_id.to_string(),
        name: name.to_string(),
        cost: ragent_config::Cost {
            input: 0.0,
            output: 0.0,
        },
        capabilities: ragent_config::Capabilities {
            reasoning: false,
            streaming: true,
            vision: true,
            tool_use: true,
            thinking_levels: Vec::new(),
        },
        context_window: 200_000,
        max_output: Some(8_192),
        request_multiplier: None,
        thinking_config: None,
    }
}

fn seed_router_provider_models(storage: &Storage) {
    let anthropic = serde_json::to_string(&vec![
        discovered_model("anthropic", "claude-sonnet-4-20250514", "Claude Sonnet 4"),
        discovered_model("anthropic", "claude-3-5-haiku-latest", "Claude 3.5 Haiku"),
    ])
    .expect("serialize anthropic models");
    storage
        .set_discovered_models("anthropic", &anthropic)
        .expect("seed anthropic discovered models");

    let openai = serde_json::to_string(&vec![
        discovered_model("openai", "gpt-4o", "GPT-4o"),
        discovered_model("openai", "gpt-4o-mini", "GPT-4o Mini"),
    ])
    .expect("serialize openai models");
    storage
        .set_discovered_models("openai", &openai)
        .expect("seed openai discovered models");
}

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
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
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
    let mut app = make_app();
    let config = RouterConfig {
        enabled: true,
        tiers: {
            let mut tiers = HashMap::new();
            tiers.insert(
                "MEDIUM".to_string(),
                ragent_llm::providers::router_config::TierConfig {
                    models: vec![TierEntry {
                        provider: "anthropic".to_string(),
                        model: "claude-sonnet-4-20250514".to_string(),
                    }],
                    timeout_ms: None,
                },
            );
            tiers
        },
        ..RouterConfig::default()
    };

    // `router_config_report` reads the router block from the on-disk
    // `ragent.json` referenced by `app.config_paths` (FR-010), so persist the
    // constructed config under `provider.router` in a temp file before
    // generating the report.
    let config_path = temp_config_path();
    let wrapper = serde_json::json!({ "provider": { "router": config } });
    std::fs::write(
        &config_path,
        serde_json::to_vec(&wrapper).expect("serialize router config wrapper"),
    )
    .expect("write temp router config");
    app.config_paths = vec![config_path];

    let report = app.router_config_report(&app.provider_registry);
    assert!(
        report.contains("via **"),
        "report should list assigned model entries: {report}"
    );
    assert!(
        report.contains("claude-sonnet-4-20250514"),
        "report should name the assigned model: {report}"
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
    seed_router_provider_models(&storage);
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
    // The model picker uses the real registry models, so include only
    // providers that have models available in the default registry.
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
    app.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));

    // Confirm the save modal with Enter.
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(
        app.provider_setup.is_none(),
        "setup dialog should close after save"
    );
    assert!(app.router_enabled, "router should be enabled after save");

    // The router cluster should also become the active model/provider.
    assert_eq!(
        app.selected_model.as_deref(),
        Some("router/router"),
        "router should be selected as the active model"
    );
    assert_eq!(
        app.configured_provider.as_ref().map(|p| p.id.as_str()),
        Some("router"),
        "router should be the configured provider"
    );
    assert_eq!(
        app.storage
            .get_setting("preferred_provider")
            .ok()
            .flatten()
            .as_deref(),
        Some("router"),
        "preferred_provider should be router"
    );

    // Verify the saved file.
    let raw = std::fs::read_to_string(&config_path).expect("read saved config");
    let json: serde_json::Value = serde_json::from_str(&raw).expect("parse saved config");
    let router = json
        .get("provider")
        .and_then(|p| p.get("router"))
        .expect("provider.router present");
    assert_eq!(
        router.get("enabled").and_then(serde_json::Value::as_bool),
        Some(true)
    );
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
                "expected empty-cluster error: {error:?}"
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
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

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
                "expected recursive routing error: {error:?}"
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
    app.config_paths = vec![config_path];

    // Save a minimal cluster.
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

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
    app.config_paths = vec![config_path];

    // Save a cluster.
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

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
fn test_router_setup_restores_router_state_at_startup() {
    let mut app = router_setup_with_providers(mem_storage());
    let config_path = temp_config_path();
    app.config_paths = vec![config_path];

    // Assign a model and save the cluster.
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Simulate a fresh session by resetting in-memory state and re-running the
    // same restore logic `App::new` performs at startup.
    app.router_enabled = false;
    app.selected_model = Some("router/router".to_string());
    app.configured_provider = None;
    app.restore_router_state();

    assert!(app.router_enabled, "restore should enable the router");
    assert_eq!(
        app.configured_provider.as_ref().map(|p| p.id.as_str()),
        Some("router"),
        "restore should set the configured provider to router"
    );
    assert!(
        app.provider_model_label()
            .expect("label should exist")
            .starts_with("Model Router"),
        "status bar should show Model Router after restore"
    );

    // The provider registry's RouterProvider should reflect the saved config.
    let registry_config = app
        .provider_registry
        .get_as_any("router")
        .and_then(|p| {
            p.downcast_ref::<ragent_llm::providers::router::RouterProvider>()
                .map(ragent_agent::provider::router::RouterProvider::config)
        })
        .expect("router provider in registry");
    assert!(registry_config.enabled, "registry router should be enabled");
    assert!(
        registry_config.tiers.contains_key("SIMPLE"),
        "saved SIMPLE tier should be loaded into the registry"
    );
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
                    .is_some_and(|t| !t.models.is_empty()),
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
        app.provider_setup.as_ref().map_or_else(
            || "None".to_string(),
            |s| match s {
                ProviderSetupStep::SelectProvider { .. } => "SelectProvider".to_string(),
                ProviderSetupStep::SetupRouter { .. } => "SetupRouter".to_string(),
                ProviderSetupStep::EnterKey { .. } => "EnterKey".to_string(),
                _ => "other".to_string(),
            }
        )
    );
    assert!(
        app.status.contains("No concrete providers"),
        "status should warn about missing concrete providers: {}",
        app.status
    );
}

// ── FR-006 (revised): buckets render in a 2×2 grid with full tier names ──────

/// Render the router setup panel and return the flattened buffer text so tests
/// can assert on what the user actually sees.
fn render_router_setup_to_string(app: &mut App, width: u16, height: u16) -> String {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("draw");
    let cells = terminal.backend().buffer().content.clone();
    cells.iter().map(ratatui::buffer::Cell::symbol).collect()
}

#[test]
fn test_router_setup_renders_full_tier_names_in_bucket_titles() {
    let mut app = router_setup_with_providers(mem_storage());
    // Move focus to the bucket pane so the right-hand titles render.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));

    let text = render_router_setup_to_string(&mut app, 120, 40);
    // The full tier names must appear in the bucket titles, not single chars.
    assert!(
        text.contains("SIMPLE"),
        "bucket title should show full tier name SIMPLE: {text}"
    );
    assert!(
        text.contains("MEDIUM"),
        "bucket title should show full tier name MEDIUM: {text}"
    );
    assert!(
        text.contains("COMPLEX"),
        "bucket title should show full tier name COMPLEX: {text}"
    );
    assert!(
        text.contains("REASONING"),
        "bucket title should show full tier name REASONING: {text}"
    );
}

#[test]
fn test_router_setup_bucket_displays_retained_model_properties() {
    let mut app = router_setup_with_providers(mem_storage());

    // Assign the first anthropic model to the SIMPLE bucket.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Render and confirm the bucket shows retained model properties (context
    // window + features). The anthropic default catalog advertises context
    // windows and tool-use, so the "ctx" and "feat" property labels must appear.
    let text = render_router_setup_to_string(&mut app, 120, 40);
    assert!(
        text.contains("ctx "),
        "bucket should display the retained context-window property: {text}"
    );
    assert!(
        text.contains("feat "),
        "bucket should display the retained features property: {text}"
    );
    assert!(
        text.contains("think "),
        "bucket should display the retained thinking-levels property: {text}"
    );
    assert!(
        text.contains("cost "),
        "bucket should display the retained cost-tier property: {text}"
    );
}

#[test]
fn test_router_model_picker_renders_property_columns() {
    let mut app = router_setup_with_providers(mem_storage());
    // Open the model picker for the selected anthropic provider.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::SelectRouterModel { .. })
    ));

    // The picker table must render the same column headers as the standard
    // model picker so properties are visible for sub-model selection.
    let text = render_router_setup_to_string(&mut app, 120, 40);
    assert!(
        text.contains("Model"),
        "picker should have Model column: {text}"
    );
    assert!(
        text.contains("Context"),
        "picker should have Context column: {text}"
    );
    assert!(
        text.contains("Cost"),
        "picker should have Cost column: {text}"
    );
    assert!(
        text.contains("Thinking"),
        "picker should have Thinking column: {text}"
    );
    assert!(
        text.contains("Features"),
        "picker should have Features column: {text}"
    );
}

// ── Re-opening setup seeds the draft from the persisted config ───────────

#[test]
fn test_router_setup_reopen_seeds_draft_from_persisted_config() {
    let mut app = router_setup_with_providers(mem_storage());
    let config_path = temp_config_path();
    app.config_paths = vec![config_path];

    // Assign a model to the SIMPLE bucket (anthropic is pre-selected in the
    // palette) and save the cluster.
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    app.handle_key_event(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL));
    app.handle_key_event(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    assert!(app.provider_setup.is_none(), "setup closed after save");

    // Re-open the router setup via `/provider router`. The draft should be
    // seeded from the just-saved `provider.router` block rather than starting
    // from an empty cluster.
    app.execute_slash_command("/provider router");

    match app.provider_setup.as_ref().expect("setup reopened") {
        ProviderSetupStep::SetupRouter {
            selected_provider_ids,
            draft_config,
            ..
        } => {
            assert!(
                selected_provider_ids.contains(&"anthropic".to_string()),
                "anthropic should be pre-checked from the saved cluster: {selected_provider_ids:?}"
            );
            let simple = draft_config
                .tiers
                .get("SIMPLE")
                .expect("SIMPLE tier should be seeded from the saved config");
            assert!(
                !simple.models.is_empty(),
                "SIMPLE bucket should contain the saved model assignment"
            );
            assert!(
                simple.models.iter().any(|e| e.provider == "anthropic"),
                "saved SIMPLE assignment should reference anthropic: {:?}",
                simple.models
            );
        }
        other => panic!("expected SetupRouter, got {other:?}"),
    }
}

#[test]
fn test_router_setup_reopen_without_saved_config_uses_empty_draft() {
    // First-time setup: no `provider.router` block on disk. Re-opening (or
    // opening for the first time) must still present four empty buckets rather
    // than the built-in default tier models.
    let mut app = make_app();
    app.storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store key");
    app.config_paths = vec![temp_config_path()];

    app.execute_slash_command("/provider router");

    match app.provider_setup.as_ref().expect("setup opened") {
        ProviderSetupStep::SetupRouter {
            selected_provider_ids,
            draft_config,
            ..
        } => {
            assert!(
                selected_provider_ids.is_empty(),
                "no providers should be pre-checked without a saved config"
            );
            for tier in [Tier::Simple, Tier::Medium, Tier::Complex, Tier::Reasoning] {
                let cfg = draft_config.tiers.get(&tier.to_string());
                let empty = cfg.map_or(true, |t| t.models.is_empty());
                assert!(empty, "{tier} bucket should be empty on first-time setup");
            }
        }
        other => panic!("expected SetupRouter, got {other:?}"),
    }
}

#[test]
fn test_router_setup_delete_removes_selected_model_in_bucket() {
    let mut app = router_setup_with_providers(mem_storage());
    // Assign two models to the SIMPLE bucket.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Re-open the picker and assign a second model to SIMPLE.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Sanity check: two models present and bucket pane is focused.
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            draft_config,
            active_bucket_index,
            left_pane_focused,
            ..
        } => {
            assert_eq!(draft_config.tiers["SIMPLE"].models.len(), 2);
            assert_eq!(*active_bucket_index, 1);
            assert!(!left_pane_focused);
        }
        _ => panic!("expected SetupRouter"),
    }

    // Delete the selected (second) model.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            draft_config,
            active_bucket_index,
            left_pane_focused,
            ..
        } => {
            assert!(!left_pane_focused);
            let models = &draft_config.tiers["SIMPLE"].models;
            assert_eq!(models.len(), 1, "Delete should remove the selected model");
            assert_eq!(
                *active_bucket_index, 0,
                "cursor should move up when last item removed"
            );
        }
        _ => panic!("expected SetupRouter"),
    }
}

#[test]
fn test_router_setup_delete_does_nothing_in_provider_pane() {
    let mut app = router_setup_with_providers(mem_storage());
    // Focus is initially in the left provider pane.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            selected_provider_ids,
            left_pane_focused,
            ..
        } => {
            assert!(*left_pane_focused);
            assert!(
                !selected_provider_ids.is_empty(),
                "Delete in provider pane should be a no-op"
            );
        }
        _ => panic!("expected SetupRouter"),
    }
}

#[test]
fn test_router_setup_delete_on_last_model_adjusts_index() {
    let mut app = router_setup_with_providers(mem_storage());
    // Assign one model to the SIMPLE bucket and move cursor to it.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Sanity: one model at index 0, bucket pane focused.
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            draft_config,
            active_bucket_index,
            left_pane_focused,
            ..
        } => {
            assert_eq!(draft_config.tiers["SIMPLE"].models.len(), 1);
            assert_eq!(*active_bucket_index, 0);
            assert!(!left_pane_focused);
        }
        _ => panic!("expected SetupRouter"),
    }

    // Delete the only model; index should remain 0.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Delete, KeyModifiers::NONE));
    match app.provider_setup.as_ref().expect("setup") {
        ProviderSetupStep::SetupRouter {
            draft_config,
            active_bucket_index,
            ..
        } => {
            assert!(draft_config.tiers["SIMPLE"].models.is_empty());
            assert_eq!(*active_bucket_index, 0);
        }
        _ => panic!("expected SetupRouter"),
    }
}

// ── Regression: router save confirmation modal must render on top ────────────

#[test]
fn test_router_save_confirmation_renders_above_setup_dialog() {
    let mut app = router_setup_with_providers(mem_storage());
    // Assign the first model to the SIMPLE bucket so the draft has content.
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    ragent_tui::input::handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Press Ctrl+S to arm the save confirmation modal.
    ragent_tui::input::handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL),
    );
    assert!(
        app.pending_router_save.is_some(),
        "pending_router_save should be set after Ctrl+S"
    );

    let text = render_router_setup_to_string(&mut app, 120, 40);
    assert!(
        text.contains("Save Router Configuration"),
        "save confirmation title should be visible above the router setup dialog: {text}"
    );
    assert!(
        text.contains("Enter save  Esc cancel"),
        "save dialog hint should be visible: {text}"
    );
}

// ── Regression: selecting an already-configured Model Router from the provider
// picker must open the router setup UI, not the single-entry model picker. ───

#[test]
fn test_provider_picker_already_configured_router_opens_setup_router() {
    let storage = mem_storage();
    storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store anthropic key");
    // Storing any router credential makes `get_configured_providers` treat the
    // router as already configured, which previously caused the provider
    // picker to fall through to the generic model-loading flow.
    storage
        .set_provider_auth("router", "unused")
        .expect("store router key");
    let mut app = make_app_with_storage(storage);

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
        "selecting an already-configured Model Router should open SetupRouter, not the model picker"
    );
}
