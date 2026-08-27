//! Integration test for T-012: per-run cost summary event.
//!
//! Drives `SessionProcessor::process_message` with a mock provider whose
//! client emits `StreamEvent::Usage`, then asserts that `Event::RunCostSummary`
//! is published with the expected token totals and estimated cost.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::cost::builtin_prices;
use ragent_agent::event::{Event, EventBus};
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
            StreamEvent::Usage {
                input_tokens: 100,
                output_tokens: 50,
            },
            StreamEvent::Finish {
                reason: LlmFinishReason::Stop,
            },
        ])))
    }
}

#[async_trait::async_trait]
impl Provider for MockProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn name(&self) -> &'static str {
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

    fn as_any_static(&self) -> &(dyn std::any::Any + 'static) {
        self
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

fn make_processor(event_bus: Arc<EventBus>) -> (SessionProcessor, std::path::PathBuf) {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(MockProvider {
        captured_requests: Arc::new(Mutex::new(Vec::new())),
    }));

    let processor = SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
    };

    let tmp = tempfile::tempdir().expect("tempdir");
    (processor, tmp.path().to_path_buf())
}

#[tokio::test]
async fn test_process_message_publishes_run_cost_summary() {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir) = make_processor(event_bus.clone());
    let session_manager = processor.session_manager.clone();
    let session = session_manager
        .create_session(working_dir)
        .expect("session should be created");

    let mut agent = AgentInfo::new("general", "General");
    // Use a model id that exists in the built-in price table so cost is non-zero.
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "gpt-4o".to_string(),
    });

    processor
        .process_message(
            &session.id,
            "hello",
            &agent,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("process_message should succeed");

    let mut found = None;
    while let Ok(ev) = rx.try_recv() {
        if let Event::RunCostSummary {
            session_id,
            model_id,
            input_tokens,
            output_tokens,
            total_cost_usd,
            duration_ms,
        } = ev
        {
            found = Some((
                session_id,
                model_id,
                input_tokens,
                output_tokens,
                total_cost_usd,
                duration_ms,
            ));
        }
    }

    let (sid, mid, in_tok, out_tok, cost, dur) =
        found.expect("RunCostSummary event should be published");
    assert_eq!(sid, session.id);
    assert_eq!(mid, "gpt-4o");
    assert_eq!(in_tok, 100);
    assert_eq!(out_tok, 50);
    assert!(dur > 0, "duration_ms should be positive");

    let expected = builtin_prices()
        .get("gpt-4o")
        .map(|(input_per_1m, output_per_1m)| {
            (100.0 * input_per_1m / 1_000_000.0) + (50.0 * output_per_1m / 1_000_000.0)
        })
        .unwrap_or(0.0);
    assert!(
        (cost - expected).abs() < 1e-9,
        "cost should match built-in price table: got {cost}, expected {expected}"
    );

    // FR-018: the summary should also be persisted in the storage layer so
    // an explicit `--include-cost` export can retrieve it. The persist is
    // done via `spawn_blocking`, so poll briefly for the row to land.
    let storage = session_manager.storage().clone();
    let mut persisted = Vec::new();
    for _ in 0..50 {
        persisted = storage
            .list_run_cost_summaries(&session.id)
            .unwrap_or_default();
        if !persisted.is_empty() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(persisted.len(), 1, "run-cost summary should be persisted");
    assert_eq!(persisted[0].model_id, "gpt-4o");
    assert_eq!(persisted[0].input_tokens, 100);
    assert_eq!(persisted[0].output_tokens, 50);
    assert!(
        (persisted[0].total_cost_usd - expected).abs() < 1e-9,
        "persisted cost should match: got {}, expected {expected}",
        persisted[0].total_cost_usd
    );
}
