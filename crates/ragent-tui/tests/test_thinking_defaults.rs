//! Tests for config-driven thinking defaults in the TUI model picker and status label.

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
use ragent_types::{ThinkingConfig, ThinkingLevel};

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

fn cwd_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn enter_temp_config_dir() -> tempfile::TempDir {
    let temp = tempfile::tempdir().expect("tempdir");
    std::env::set_current_dir(temp.path()).expect("set cwd");
    std::fs::create_dir_all(temp.path().join(".ragent")).expect("create .ragent");
    temp
}

#[test]
fn test_models_for_provider_applies_model_and_provider_thinking_defaults() {
    let _lock = cwd_test_lock()
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    let original_cwd = std::env::current_dir().expect("current dir");
    let _guard = CwdGuard(original_cwd);
    let temp = enter_temp_config_dir();
    std::fs::write(
        temp.path().join(".ragent/ragent.json"),
        r#"{
            "provider": {
                "anthropic": {
                    "thinking": {
                        "enabled": true,
                        "level": "low"
                    },
                    "models": {
                        "claude-sonnet-4-20250514": {
                            "thinking": {
                                "enabled": true,
                                "level": "high"
                            }
                        }
                    }
                }
            }
        }"#,
    )
    .expect("write config");

    let app = make_app();
    let models = app.models_for_provider("anthropic");

    let sonnet = models
        .iter()
        .find(|entry| entry.id == "claude-sonnet-4-20250514")
        .expect("claude-sonnet-4-20250514 should exist");
    assert_eq!(
        sonnet.thinking_config,
        Some(ThinkingConfig::new(ThinkingLevel::High))
    );

    let haiku = models
        .iter()
        .find(|entry| entry.id == "claude-3-5-haiku-latest")
        .expect("claude-3-5-haiku-latest should exist");
    assert_eq!(
        haiku.thinking_config,
        Some(ThinkingConfig::new(ThinkingLevel::Low))
    );
}

#[test]
fn test_provider_label_prefers_agent_default_over_config_thinking() {
    let _lock = cwd_test_lock()
        .lock()
        .unwrap_or_else(|err| err.into_inner());
    let original_cwd = std::env::current_dir().expect("current dir");
    let _guard = CwdGuard(original_cwd);
    let temp = enter_temp_config_dir();
    std::fs::write(
        temp.path().join(".ragent/ragent.json"),
        r#"{
            "provider": {
                "anthropic": {
                    "thinking": {
                        "enabled": true,
                        "level": "high"
                    }
                }
            }
        }"#,
    )
    .expect("write config");

    let mut app = make_app();
    app.agent_info = agent::resolve_agent("ask", &Default::default()).expect("resolve ask agent");
    app.selected_model = Some("anthropic/claude-sonnet-4-20250514".to_string());

    let label = app
        .provider_model_label()
        .expect("provider label should exist");
    assert!(label.contains("claude-sonnet-4-20250514"));
    assert!(label.ends_with("[thinking: off]"));
}
