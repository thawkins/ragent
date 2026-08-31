//! Tests for `/research cluster` slash-command parsing and validation.

use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use ragent_agent::{
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;

fn make_app() -> App {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
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
        agent_manager: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
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
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
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
        skill_registry_cache: parking_lot::Mutex::new(None),
    });
    let agent_info = ragent_agent::agent::resolve_agent("general", &Default::default())
        .expect("resolve general agent");
    let mut app = App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        Arc::unwrap_or_clone(agent_info),
        false,
        std::path::PathBuf::new(),
    );
    // Pick a model that resolves without runtime discovery so the cluster
    // command can build a ModelRef and proceed to the async phase.
    app.selected_model = Some("gemini/gemini-2.0-flash".to_string());
    app.selected_model_ctx_window = Some(1_048_576);
    app
}

struct CwdGuard {
    prev: std::path::PathBuf,
    _lock: MutexGuard<'static, ()>,
    _temp: Option<tempfile::TempDir>,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.prev);
    }
}

fn cwd_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn cwd_lock() -> MutexGuard<'static, ()> {
    let lock = cwd_test_lock().lock();
    match lock {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    }
}

fn enter_with_cwd() -> CwdGuard {
    let _lock = cwd_lock();
    let prev = std::env::current_dir().expect("current dir");
    let temp = tempfile::TempDir::new().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set_current_dir");
    CwdGuard {
        prev,
        _lock,
        _temp: Some(temp),
    }
}

#[tokio::test]
async fn test_research_cluster_valid_sources() {
    let _guard = enter_with_cwd();
    std::fs::create_dir_all("research/foo/sources").expect("create dirs");
    std::fs::write("research/foo/sources/web-01.md", "body").expect("write source");

    let mut app = make_app();
    app.session_id = Some("s1".to_string());
    app.execute_slash_command("/research cluster foo");

    assert!(app.status.starts_with("[wait]"), "status: {}", app.status);
    assert!(
        app.status.contains("reading sources"),
        "status: {}",
        app.status
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("Request accepted for `foo`"), "{text}");
    assert!(text.contains("force=false"), "{text}");
}

#[tokio::test]
async fn test_research_cluster_valid_sources_force() {
    let _guard = enter_with_cwd();
    std::fs::create_dir_all("research/bar/sources").expect("create dirs");
    std::fs::write("research/bar/sources/web-01.md", "body").expect("write source");

    let mut app = make_app();
    app.session_id = Some("s2".to_string());
    app.execute_slash_command("/research cluster bar --force");

    assert!(app.status.starts_with("[wait]"), "status: {}", app.status);
    assert!(
        app.status.contains("reading sources"),
        "status: {}",
        app.status
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("force=true"), "{text}");
}

#[tokio::test]
async fn test_research_cluster_missing_folder() {
    let _guard = enter_with_cwd();

    let mut app = make_app();
    app.session_id = Some("s3".to_string());

    app.execute_slash_command("/research cluster missing");

    assert_eq!(app.status, "research: cluster 'missing' folder missing");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("research folder `research/missing` does not exist"),
        "{text}"
    );
}

#[tokio::test]
async fn test_research_cluster_no_sources_folder() {
    let _guard = enter_with_cwd();
    std::fs::create_dir_all("research/no-sources").expect("create dirs");

    let mut app = make_app();
    app.session_id = Some("s4".to_string());

    app.execute_slash_command("/research cluster no-sources");

    assert_eq!(app.status, "research: cluster 'no-sources' no sources");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("`research/no-sources/sources/` folder not found"),
        "{text}"
    );
}

#[tokio::test]
async fn test_research_cluster_empty_sources() {
    let _guard = enter_with_cwd();
    std::fs::create_dir_all("research/empty/sources").expect("create dirs");

    let mut app = make_app();
    app.session_id = Some("s5".to_string());

    app.execute_slash_command("/research cluster empty");

    assert_eq!(app.status, "research: cluster 'empty' empty sources");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("`research/empty/sources/` is empty"),
        "{text}"
    );
}

#[tokio::test]
async fn test_research_cluster_rejects_existing_concepts_without_force() {
    let _guard = enter_with_cwd();
    std::fs::create_dir_all("research/existing/sources").expect("create dirs");
    std::fs::write("research/existing/sources/web-01.md", "body").expect("write source");
    std::fs::write("research/existing/CONCEPTS.md", "old").expect("write concepts");

    let mut app = make_app();
    app.session_id = Some("s6".to_string());

    app.execute_slash_command("/research cluster existing");

    assert_eq!(app.status, "research: cluster 'existing' already clustered");
    let text = app.messages.last().unwrap().text_content();
    assert!(
        text.contains("`research/existing/CONCEPTS.md` already exists"),
        "{text}"
    );
    assert!(text.contains("`--force`"), "{text}");
}

#[tokio::test]
async fn test_research_cluster_force_overwrites_existing_concepts() {
    let _guard = enter_with_cwd();
    std::fs::create_dir_all("research/forced/sources").expect("create dirs");
    std::fs::write("research/forced/sources/web-01.md", "body").expect("write source");
    std::fs::write("research/forced/CONCEPTS.md", "old").expect("write concepts");

    let mut app = make_app();
    app.session_id = Some("s7".to_string());
    app.execute_slash_command("/research cluster forced --force");

    assert!(app.status.starts_with("[wait]"), "status: {}", app.status);
    assert!(
        app.status.contains("reading sources"),
        "status: {}",
        app.status
    );
    let text = app.messages.last().unwrap().text_content();
    assert!(text.contains("force=true"), "{text}");
    assert!(text.contains("Request accepted for `forced`"), "{text}");
}
