//! Shared test-support helpers for `ragent-tui` integration tests and benches.
//!
//! Provides [`make_app`] - the standard `App` constructor used across ~20 test
//! files and 3 bench files.  Previously copy-pasted verbatim into every test.

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

/// Build an [`App`] backed by an in-memory database.
///
/// Creates a full `SessionProcessor`, provider/tool registries, and permission
/// checker wired to an in-memory `Storage` instance.  The agent is resolved as
/// `"general"` with default config.
///
/// Files that need a customised `App` (e.g. with a shared `EventBus`, a custom
/// `Storage`, or a debug flag) should define their own local helper.
#[allow(dead_code)]
#[must_use]
pub fn make_app() -> App {
    ragent_tui::app::use_scratch_log_root_for_tests();
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
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
        agent_manager: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
        team_manager: Default::default(),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
        code_index: std::sync::OnceLock::new(),
        project_code_indexes: Default::default(),
        project_tool_registries: Default::default(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
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
        auto_approve: false,
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
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");

    let mut app = App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        Arc::unwrap_or_clone(agent_info),
        false,
        std::path::PathBuf::new(),
    );
    // The session sidebar (MULTISESSION M5) is on by default and would take
    // the left columns of every rendered layout; tests that are not about the
    // sidebar keep the full-width chat layout they were written against.
    // Sidebar tests turn it back on.
    app.sidebar.visible = false;
    app
}

/// Makes `root` a permitted session root for `app`, as a `dirs.allowed_roots`
/// entry would, by injecting a config into the processor's cache.
///
/// The injected config is honoured only while no `RAGENT_CONFIG*` override
/// is set, and the launch directory comes from the process cwd; both are
/// process-global, so a caller holds `crate::global_env_lock()` for as long
/// as it relies on the permitted roots.
#[allow(dead_code)]
pub fn allow_session_root(app: &App, root: &std::path::Path) {
    let mut config = ragent_config::Config::default();
    config.dirs.allowed_roots = vec![root.display().to_string()];
    *app.session_processor.cached_config.lock() =
        Some(ragent_agent::session::processor::CachedConfig {
            config: Arc::new(config),
            file_mtimes: Vec::new(),
            env_overrides_present: false,
        });
}
