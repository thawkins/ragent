//! Tests for test_session_processor.rs

//! Tests for session processor error propagation paths (Task 5.2).

use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, atomic::AtomicBool};
use std::time::Duration;

use anyhow::Result;
use futures::stream;
use serde_json::Value;
use tokio::time::timeout;

use ragent_core::agent::{AgentInfo, ModelRef};
use ragent_core::config::StreamConfig;
use ragent_core::event::{Event, EventBus, FinishReason};
use ragent_core::llm::{ChatRequest, LlmClient, StreamEvent};
use ragent_core::permission::PermissionChecker;
use ragent_core::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_core::session::SessionManager;
use ragent_core::session::processor::{OLLAMA_TOOL_GUIDANCE, SessionProcessor};
use ragent_core::storage::Storage;
use ragent_core::tool::ToolRegistry;

#[tokio::test]
async fn test_session_processor_errors_when_model_missing() {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(16));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let provider_registry = Arc::new(ProviderRegistry::new());
    let tool_registry = Arc::new(ToolRegistry::new());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));

    let processor = SessionProcessor {
        session_manager: session_manager.clone(),
        provider_registry,
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: StreamConfig::default(),
    };

    let tempdir = tempfile::tempdir().unwrap();
    let session = session_manager
        .create_session(PathBuf::from(tempdir.path()))
        .unwrap();

    let mut rx = event_bus.subscribe();

    let agent = AgentInfo::new("test", "Test agent");
    let cancel = Arc::new(AtomicBool::new(false));

    let err = processor
        .process_message(&session.id, "hi", &agent, cancel)
        .await;
    assert!(err.is_err());
    assert!(
        err.unwrap_err()
            .to_string()
            .contains("has no model configured")
    );

    // Consume events until AgentError is observed (MessageStart may arrive first).
    let mut found = false;
    for _ in 0..4 {
        let ev = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for event")
            .expect("failed to receive event");
        match ev {
            Event::AgentError { session_id, error } => {
                assert_eq!(session_id, session.id);
                assert!(error.contains("has no model configured"));
                found = true;
                break;
            }
            _ => continue,
        }
    }
    assert!(found, "expected AgentError event but none received");
}

#[tokio::test]
async fn test_session_processor_errors_when_provider_missing() {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(16));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let provider_registry = Arc::new(ProviderRegistry::new());
    let tool_registry = Arc::new(ToolRegistry::new());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));

    let processor = SessionProcessor {
        session_manager: session_manager.clone(),
        provider_registry,
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: StreamConfig::default(),
    };

    let tempdir = tempfile::tempdir().unwrap();
    let session = session_manager
        .create_session(PathBuf::from(tempdir.path()))
        .unwrap();

    let mut rx = event_bus.subscribe();

    let mut agent = AgentInfo::new("test", "Test agent");
    agent.model = Some(ModelRef {
        provider_id: "missing".to_string(),
        model_id: "m".to_string(),
    });

    let cancel = Arc::new(AtomicBool::new(false));

    let err = processor
        .process_message(&session.id, "hi", &agent, cancel)
        .await;
    assert!(err.is_err());
    assert!(
        err.unwrap_err()
            .to_string()
            .contains("Provider 'missing' not found")
    );

    // Consume events until AgentError is observed (MessageStart may arrive first).
    let mut found = false;
    for _ in 0..4 {
        let ev = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for event")
            .expect("failed to receive event");
        match ev {
            Event::AgentError { session_id, error } => {
                assert_eq!(session_id, session.id);
                assert!(error.contains("Provider 'missing' not found"));
                found = true;
                break;
            }
            _ => continue,
        }
    }
    assert!(found, "expected AgentError event but none received");
}

struct MockOllamaProvider {
    captured_system: Arc<tokio::sync::Mutex<Option<String>>>,
}

struct MockOllamaClient {
    captured_system: Arc<tokio::sync::Mutex<Option<String>>>,
}

#[async_trait::async_trait]
impl Provider for MockOllamaProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn name(&self) -> &'static str {
        "Ollama"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "gemma4:26b".to_string(),
            provider_id: "ollama".to_string(),
            name: "Gemma 4 26B".to_string(),
            cost: Default::default(),
            capabilities: Default::default(),
            context_window: 8_192,
            max_output: None,
        }]
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &std::collections::HashMap<String, Value>,
    ) -> Result<Box<dyn LlmClient>> {
        Ok(Box::new(MockOllamaClient {
            captured_system: Arc::clone(&self.captured_system),
        }))
    }
}

#[async_trait::async_trait]
impl LlmClient for MockOllamaClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<std::pin::Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        *self.captured_system.lock().await = request.system;
        Ok(Box::pin(stream::iter(vec![StreamEvent::Finish {
            reason: FinishReason::Stop,
        }])))
    }
}

#[tokio::test]
async fn test_session_processor_injects_ollama_read_guidance() {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(16));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let captured_system = Arc::new(tokio::sync::Mutex::new(None));

    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(MockOllamaProvider {
        captured_system: Arc::clone(&captured_system),
    }));

    let provider_registry = Arc::new(provider_registry);
    let tool_registry = Arc::new(ToolRegistry::new());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));

    let processor = SessionProcessor {
        session_manager: session_manager.clone(),
        provider_registry,
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: StreamConfig::default(),
    };

    let tempdir = tempfile::tempdir().unwrap();
    let session = session_manager
        .create_session(PathBuf::from(tempdir.path()))
        .unwrap();

    let mut agent = AgentInfo::new("test", "Test agent");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "gemma4:26b".to_string(),
    });

    let cancel = Arc::new(AtomicBool::new(false));

    processor
        .process_message(&session.id, "hi", &agent, cancel)
        .await
        .unwrap();

    let system_prompt = captured_system.lock().await.clone().unwrap();
    assert!(system_prompt.contains(OLLAMA_TOOL_GUIDANCE.trim()));
}

#[tokio::test]
async fn test_session_processor_injects_git_and_readme_context() {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    let event_bus = Arc::new(EventBus::new(16));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let captured_system = Arc::new(tokio::sync::Mutex::new(None));

    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(MockOllamaProvider {
        captured_system: Arc::clone(&captured_system),
    }));

    let provider_registry = Arc::new(provider_registry);
    let tool_registry = Arc::new(ToolRegistry::new());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));

    let processor = SessionProcessor {
        session_manager: session_manager.clone(),
        provider_registry,
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: StreamConfig::default(),
    };

    let tempdir = tempfile::tempdir().unwrap();
    Command::new("git")
        .arg("init")
        .current_dir(tempdir.path())
        .output()
        .expect("git init");
    std::fs::write(tempdir.path().join("README.md"), "# hello\nmore text\n").unwrap();
    let session = session_manager
        .create_session(PathBuf::from(tempdir.path()))
        .unwrap();

    let mut agent = AgentInfo::new("test", "Test agent");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "gemma4:26b".to_string(),
    });

    let cancel = Arc::new(AtomicBool::new(false));

    processor
        .process_message(&session.id, "hi", &agent, cancel)
        .await
        .unwrap();

    let system_prompt = captured_system.lock().await.clone().unwrap();
    assert!(system_prompt.contains("Git Context"));
    assert!(system_prompt.contains("README"));
}
