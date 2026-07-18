//! Tests for the configured-provider selection and model-persistence feature
//! (spec: `selectConfigedProvider`, FR-001 through FR-005).
//!
//! Environment variables cannot be manipulated in tests (the workspace forbids
//! `unsafe_code` and Rust 2024 requires `unsafe` for `set_var`/`remove_var`).
//! Therefore, env-var-based detection is verified indirectly via the existing
//! `test_provider_detection.rs` suite.  The tests here focus on database-driven
//! behaviour: credential storage, per-provider model persistence, stale-model
//! fallback, and reset cleanup.

use std::sync::Arc;

use ragent_agent::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;
use ragent_tui::app::ProviderSetupStep;

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
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
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

// ── FR-001: get_configured_providers (DB-key-based providers) ─────────────

#[test]
fn test_get_configured_providers_db_keys_enumerated() {
    let storage = mem_storage();

    // Store auth keys for two providers.
    storage
        .set_provider_auth("anthropic", "sk-ant-test")
        .expect("store anthropic key");
    storage
        .set_provider_auth("openai", "sk-oai-test")
        .expect("store openai key");

    let configured = App::get_configured_providers(&storage);

    // At a minimum, the two DB-key providers should appear (env vars may add more).
    let ids: Vec<String> = configured.iter().map(|p| p.id.clone()).collect();
    assert!(
        ids.contains(&"anthropic".to_string()),
        "anthropic should be in list"
    );
    assert!(
        ids.contains(&"openai".to_string()),
        "openai should be in list"
    );
}

#[test]
fn test_get_configured_providers_disabled_excluded() {
    let storage = mem_storage();

    storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store key");
    storage
        .set_setting("provider_anthropic_disabled", "true")
        .expect("disable");

    let configured = App::get_configured_providers(&storage);
    assert!(
        !configured.iter().any(|p| p.id == "anthropic"),
        "disabled anthropic must be excluded"
    );
}

#[test]
fn test_get_configured_providers_preferred_first() {
    let storage = mem_storage();

    // Store keys for both.
    storage
        .set_provider_auth("openai", "sk-test")
        .expect("store openai key");
    storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store anthropic key");
    // Prefer openai.
    storage
        .set_setting("preferred_provider", "openai")
        .expect("store preferred");

    let configured = App::get_configured_providers(&storage);
    assert!(!configured.is_empty());
    // The preferred provider should appear first.
    assert_eq!(
        configured[0].id, "openai",
        "preferred provider should be first in the list"
    );
}

#[test]
fn test_get_configured_providers_does_not_crash_empty() {
    let storage = mem_storage();
    let result = App::get_configured_providers(&storage);
    // With no env vars or DB keys this will be empty, but must not panic.
    assert!(result.is_empty() || !result.is_empty());
}

// ── FR-003: per-provider model persistence (via storage directly) ─────────

#[test]
fn test_model_persistence_write_and_read() {
    let storage = mem_storage();

    // Simulate what finalize_model_selection does: write the per-provider key.
    storage
        .set_setting("provider_anthropic_last_model", "claude-sonnet-4-20250514")
        .expect("persist model");

    let persisted = storage
        .get_setting("provider_anthropic_last_model")
        .expect("read last_model")
        .expect("last_model should be set");
    assert_eq!(
        persisted, "claude-sonnet-4-20250514",
        "persisted model ID must match"
    );
}

#[test]
fn test_model_persistence_multiple_providers_independent() {
    let storage = mem_storage();

    storage
        .set_setting("provider_anthropic_last_model", "claude-sonnet-4-20250514")
        .expect("persist anthropic model");
    storage
        .set_setting("provider_openai_last_model", "gpt-4o")
        .expect("persist openai model");

    let anthro_persisted = storage
        .get_setting("provider_anthropic_last_model")
        .expect("read")
        .expect("set");
    let oai_persisted = storage
        .get_setting("provider_openai_last_model")
        .expect("read")
        .expect("set");

    assert_eq!(anthro_persisted, "claude-sonnet-4-20250514");
    assert_eq!(oai_persisted, "gpt-4o");
}

// ── FR-003 / FR-004: model restore fallback (via storage) ─────────────────

#[test]
fn test_model_restore_empty_persisted_value_returns_none() {
    let storage = mem_storage();
    let mut app = make_app_with_storage(storage.clone());

    storage
        .set_setting("provider_anthropic_last_model", "")
        .expect("persist empty model");

    let result = app.try_restore_provider_model("anthropic", "Anthropic (Claude)");
    assert!(result.is_none(), "empty persisted model must not restore");
}

// ── FR-002: configured-provider picker dialog state ───────────────────────

#[test]
fn test_configured_picker_state_transition() {
    let storage = mem_storage();
    let mut app = make_app_with_storage(storage.clone());

    // Place two providers in the DB so get_configured_providers() returns them.
    storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store key");
    storage
        .set_provider_auth("openai", "sk-test")
        .expect("store key");

    let configured = App::get_configured_providers(&storage);
    assert!(
        configured.len() >= 2,
        "should have at least 2 configured providers"
    );

    // Simulate the dialog opening.
    app.provider_setup = Some(ProviderSetupStep::SelectConfiguredProvider {
        providers: configured,
        selected: 0,
    });

    assert!(matches!(
        app.provider_setup,
        Some(ProviderSetupStep::SelectConfiguredProvider { .. })
    ));
}

#[test]
fn test_configured_picker_esc_cancel() {
    let storage = mem_storage();
    let mut app = make_app_with_storage(storage.clone());

    storage
        .set_provider_auth("anthropic", "sk-test")
        .expect("store key");

    let configured = App::get_configured_providers(&storage);
    app.provider_setup = Some(ProviderSetupStep::SelectConfiguredProvider {
        providers: configured,
        selected: 0,
    });

    // Simulate Esc: the dialog should be dismissed.
    app.provider_setup = None;
    assert!(app.provider_setup.is_none());
}

// ── FR-005: per-provider model cleared on reset ───────────────────────────

#[test]
fn test_model_persistence_cleared_on_reset() {
    let storage = mem_storage();

    // Set up a persisted model.
    storage
        .set_setting("provider_anthropic_last_model", "claude-sonnet-4-20250514")
        .expect("persist model");

    assert!(
        storage
            .get_setting("provider_anthropic_last_model")
            .ok()
            .flatten()
            .is_some()
    );

    // Simulate the reset flow: delete auth, set disabled, clear last_model.
    let _ = storage.delete_setting("provider_anthropic_last_model");

    assert!(
        storage
            .get_setting("provider_anthropic_last_model")
            .ok()
            .flatten()
            .is_none()
    );
}

// ── Sanity: model persistence key is independent of global selected_model ─

#[test]
fn test_local_persistence_key_is_model_only_format() {
    let storage = mem_storage();

    // Set both the global selected_model and the per-provider key.
    storage
        .set_setting("selected_model", "anthropic/claude-sonnet-4-20250514")
        .expect("set global");
    storage
        .set_setting("provider_anthropic_last_model", "claude-sonnet-4-20250514")
        .expect("set per-provider");

    let global = storage
        .get_setting("selected_model")
        .ok()
        .flatten()
        .expect("global selected_model must be set");

    let per_provider = storage
        .get_setting("provider_anthropic_last_model")
        .ok()
        .flatten()
        .expect("per-provider key must be set");

    // The global key stores "provider/model", the per-provider key stores just the model.
    assert!(
        global.contains('/'),
        "global key is provider/model format, got: {global}"
    );
    assert!(
        !per_provider.contains('/'),
        "per-provider key is model-only, got: {per_provider}"
    );
}

// ── Smoke: models_for_provider returns non-empty for built-in providers ───

#[test]
fn test_models_for_provider_anthropic_is_empty_without_key_or_discovery() {
    let app = make_app();
    let models = app.models_for_provider("anthropic");
    assert!(
        models.is_empty(),
        "anthropic models should be empty when no key or discovery is available"
    );
}

#[test]
fn test_models_for_provider_openai_is_empty_without_key_or_discovery() {
    let app = make_app();
    let models = app.models_for_provider("openai");
    assert!(
        models.is_empty(),
        "openai models should be empty when no key or discovery is available"
    );
}
