//! Shared [`AppState`] fixture for the `ragent-server` integration tests.
//!
//! Every server test that drives the axum router in process needs the same
//! in-memory storage, event bus, session manager and `SessionProcessor`. Before
//! this module existed each of the seven test files carried its own
//! ~64-line copy of `test_state()`, so a field added to `AppState` or
//! `SessionProcessor` had to be threaded through seven separate places.
//!
//! This module is declared once in `tests/all.rs`, so every child test module
//! reaches it as `crate::support::...`. Call sites that need a non-default
//! value (a config with automations enabled, a bigger event-bus capacity, a
//! test-owned research root) adjust the returned state rather than copying the
//! constructor:
//!
//! ```ignore
//! let mut state = crate::support::test_state("token");
//! *state.config.write().await = crate::support::config_with_automations();
//! ```

#![allow(dead_code)]

use std::collections::HashMap;
use std::sync::Arc;

use ragent_agent::Config;
use ragent_agent::event::EventBus;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::ProviderRegistry;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool::ToolRegistry;
use ragent_server::routes::AppState;

/// Build a minimal [`AppState`] suitable for driving the router in process.
///
/// The storage is in memory, the provider and tool registries are empty, and
/// the research vault root is a private directory under `target/temp/`, so a
/// test run never rewrites the checked-in research tree.
///
/// # Arguments
///
/// * `token` - the bearer token the returned state will accept.
///
/// # Returns
///
/// A ready-to-use [`AppState`]. Its `config` is [`Config::default()`]; a test
/// that needs a different configuration writes the lock before building the
/// router (see the module docs).
#[must_use]
pub fn test_state(token: &str) -> AppState {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let event_bus = Arc::new(EventBus::new(16));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(ProviderRegistry::new()),
        tool_registry: Arc::new(ToolRegistry::new()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
        code_index: std::sync::OnceLock::new(),
        stream_config: Default::default(),
        extraction_engine: std::sync::OnceLock::new(),
        auto_approve: false,
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
            std::collections::HashMap::new(),
        )),
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
        pending_verdicts: ragent_agent::session::pending::PendingVerdicts::new(),
        bg_service: std::sync::OnceLock::new(),
    });
    AppState {
        event_bus,
        config: Arc::new(tokio::sync::RwLock::new(Config::default())),
        storage,
        session_processor: processor,
        auth_token: token.to_string(),
        rate_limiter: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        coordinator: None,
        research_runs: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        research_root: std::sync::Arc::new(crate::research_sandbox::isolated_research_root()),
    }
}
