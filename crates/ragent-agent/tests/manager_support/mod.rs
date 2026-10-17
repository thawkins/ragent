//! Shared helper for constructing a real `TeamManager` over a stub
//! `SessionProcessor` so the manager's disk-backed coordination paths
//! (tasks, plan approval, adoption, hooks) can be exercised from an external
//! test crate without a live model or provider.

use std::collections::HashMap;
use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::team::TeamManager;
use ragent_agent::tool::create_default_registry;
use ragent_config::Config as RagentConfig;
use tempfile::TempDir;

/// Build a `SessionProcessor` with in-memory storage and no provider. Only the
/// `session_manager` / `event_bus` fields are ever touched by the team manager.
pub fn stub_processor(event_bus: Arc<EventBus>) -> Arc<SessionProcessor> {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let config: RagentConfig = serde_json::from_str("{}").expect("valid config JSON");

    Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(ragent_agent::provider::ProviderRegistry::new()),
        tool_registry: Arc::new(create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        tool_repeat_guard: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
        code_index: std::sync::OnceLock::new(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(HashMap::new()),
        cached_config: parking_lot::Mutex::new(Some(
            ragent_agent::session::processor::CachedConfig {
                config: Arc::new(config),
                file_mtimes: Vec::new(),
                env_overrides_present: false,
            },
        )),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: Default::default(),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(HashMap::new()),
    })
}

/// A `TeamManager` together with the temp dir that owns its team directory.
///
/// The manager holds the (strong) `Arc<EventBus>` and a `Weak` processor; the
/// returned `TempDir` must be kept alive for the test body.
pub fn team_manager(
    team_name: &str,
    lead_session: &str,
    project_local: bool,
) -> (Arc<TeamManager>, TempDir, Arc<EventBus>) {
    let bus = Arc::new(EventBus::new(256));
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(workspace.join(".ragent")).expect("create .ragent");

    let mut store =
        ragent_agent::team::TeamStore::create(team_name, "old-lead", &workspace, project_local)
            .expect("create team");
    // Give the team one member so adoption / plan logic has something to act on.
    store
        .add_member(ragent_agent::team::TeamMember::new(
            "alice", "tm-001", "general",
        ))
        .expect("add member");
    let team_dir = store.dir.clone();
    drop(store);

    let processor = stub_processor(bus.clone());
    let manager = Arc::new(TeamManager::new(
        team_name,
        lead_session,
        team_dir,
        processor,
        bus.clone(),
    ));
    (manager, tmp, bus)
}
