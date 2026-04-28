//! Integration test for thinking propagation through the session pipeline.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::EventBus;
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::{SessionManager, processor::SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::{Capabilities, Cost};
use ragent_types::{ThinkingConfig, ThinkingLevel};

#[derive(Clone)]
struct MockProvider {
    captured_requests: Arc<Mutex<Vec<ChatRequest>>>,
}

struct MockClient {
    captured_requests: Arc<Mutex<Vec<ChatRequest>>>,
}

#[async_trait::async_trait]
impl LlmClient for MockClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        self.captured_requests
            .lock()
            .expect("captured requests lock")
            .push(request);
        Ok(Box::pin(stream::iter(vec![
            StreamEvent::TextDelta {
                text: "ok".to_string(),
            },
            StreamEvent::Finish {
                reason: LlmFinishReason::Stop,
            },
        ])))
    }
}

#[async_trait::async_trait]
impl Provider for MockProvider {
    fn id(&self) -> &str {
        "ollama"
    }

    fn name(&self) -> &str {
        "Mock Ollama"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: "qwen3:latest".to_string(),
            provider_id: "ollama".to_string(),
            name: "Qwen3".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                reasoning: true,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![ThinkingLevel::Auto, ThinkingLevel::Off],
            },
            context_window: 128_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: Some(ThinkingConfig::new(ThinkingLevel::Auto)),
        }]
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &HashMap<String, serde_json::Value>,
    ) -> Result<Box<dyn LlmClient>> {
        Ok(Box::new(MockClient {
            captured_requests: Arc::clone(&self.captured_requests),
        }))
    }
}

#[tokio::test]
async fn test_process_message_forwards_agent_thinking_to_chat_request() {
    let captured_requests = Arc::new(Mutex::new(Vec::new()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(MockProvider {
        captured_requests: Arc::clone(&captured_requests),
    }));

    let event_bus = Arc::new(EventBus::new(32));
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));
    let processor = SessionProcessor {
        session_manager: session_manager.clone(),
        provider_registry: Arc::new(provider_registry),
        tool_registry,
        permission_checker,
        event_bus,
        task_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::config::StreamConfig::default(),
        auto_approve: false,
    };

    let working_dir = tempfile::tempdir().expect("tempdir");
    let session = session_manager
        .create_session(working_dir.path().to_path_buf())
        .expect("session should be created");

    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });
    agent.thinking = Some(ThinkingConfig::new(ThinkingLevel::High));

    let reply = processor
        .process_message(
            &session.id,
            "hello",
            &agent,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("process_message should succeed");

    assert_eq!(reply.text_content(), "ok");
    let captured = captured_requests.lock().expect("captured requests lock");
    assert_eq!(captured.len(), 1);
    assert_eq!(
        captured[0].thinking,
        Some(ThinkingConfig::new(ThinkingLevel::High))
    );
}
