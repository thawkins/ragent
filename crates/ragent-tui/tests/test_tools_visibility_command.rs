//! Regression tests for `/tools <switch> on|off`.

use std::sync::{Arc, Mutex, OnceLock};

use ragent_core::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;

fn make_app() -> App {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));
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
        stream_config: ragent_core::config::StreamConfig::default(),
        auto_approve: false,
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
    )
}

struct CwdGuard(std::path::PathBuf);

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.0);
    }
}

fn enter_temp_config_dir() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    std::fs::create_dir_all(temp.path().join(".ragent")).expect("create .ragent");
    temp
}

fn cwd_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

#[test]
fn test_slash_tools_toggle_persists_and_updates_hidden_registry() {
    let _lock = cwd_test_lock().lock().expect("cwd lock");
    let original_cwd = std::env::current_dir().expect("cwd");
    let _guard = CwdGuard(original_cwd);
    let _temp = enter_temp_config_dir();

    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.tool_visibility = ragent_core::config::ToolVisibilityConfig::default();

    assert!(
        app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "codeindex_search")
    );

    app.execute_slash_command("/tools codeindex off");

    assert!(!app.tool_visibility.codeindex);
    assert_eq!(app.status, "tools: codeindex off");
    assert!(
        app.messages
            .last()
            .expect("message")
            .text_content()
            .contains("`codeindex` visibility is now **off**")
    );
    assert!(
        !app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "codeindex_search")
    );

    let cfg = ragent_core::config::Config::load().expect("load saved config");
    assert!(!cfg.tool_visibility.codeindex);
}

#[test]
fn test_slash_codeindex_off_updates_visibility_and_config() {
    let _lock = cwd_test_lock().lock().expect("cwd lock");
    let original_cwd = std::env::current_dir().expect("cwd");
    let _guard = CwdGuard(original_cwd);
    let _temp = enter_temp_config_dir();

    let mut app = make_app();
    app.session_id = Some("test-session".to_string());
    app.code_index_enabled = true;
    app.tool_visibility.codeindex = true;

    assert!(
        app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "codeindex_search")
    );

    app.execute_slash_command("/codeindex off");

    assert!(!app.code_index_enabled);
    assert!(!app.tool_visibility.codeindex);
    assert_eq!(app.status, "codeindex: off");
    assert!(
        !app.session_processor
            .tool_registry
            .definitions()
            .iter()
            .any(|d| d.name == "codeindex_search")
    );

    let cfg = ragent_core::config::Config::load().expect("load saved config");
    assert!(!cfg.code_index.enabled);
    assert!(!cfg.tool_visibility.codeindex);
}
