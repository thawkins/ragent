//! Integration tests for the structured-goal loop prompt (spec `agentloop`,
//! task T-016 / FR-006): a goal-driven loop turn composes the structured
//! goal — success state, verification command, scope boundaries, read-only
//! constraints, tool set, and budget knobs — into the system prompt, while a
//! plain chat turn renders none of it.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::EventBus;
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::loop_state::LoopSpec;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::{Config as RagentConfig, Cost};
use ragent_types::ThinkingLevel;

#[derive(Clone)]
struct CapturingProvider {
    captured: Arc<Mutex<Vec<ChatRequest>>>,
    call_count: Arc<AtomicU32>,
}

struct CapturingClient {
    captured: Arc<Mutex<Vec<ChatRequest>>>,
    call_count: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl LlmClient for CapturingClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        self.captured
            .lock()
            .expect("captured requests lock")
            .push(request);
        self.call_count.fetch_add(1, Ordering::SeqCst);
        let events: Vec<StreamEvent> = vec![
            StreamEvent::TextDelta {
                text: "ack".to_string(),
            },
            StreamEvent::Usage {
                input_tokens: 10,
                output_tokens: 5,
            },
            StreamEvent::Finish {
                reason: LlmFinishReason::Stop,
            },
        ];
        Ok(Box::pin(stream::iter(events)))
    }
}

#[async_trait::async_trait]
impl Provider for CapturingProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn name(&self) -> &'static str {
        "Capturing Mock Ollama"
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
            capabilities: ragent_config::Capabilities {
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![ThinkingLevel::Auto, ThinkingLevel::Off],
            },
            context_window: 128_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: None,
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
        Ok(Box::new(CapturingClient {
            captured: Arc::clone(&self.captured),
            call_count: Arc::clone(&self.call_count),
        }))
    }
}

fn make_processor() -> anyhow::Result<(
    Arc<SessionProcessor>,
    std::path::PathBuf,
    Arc<Mutex<Vec<ChatRequest>>>,
    tempfile::TempDir,
)> {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(CapturingProvider {
        captured: Arc::clone(&captured),
        call_count: Arc::new(AtomicU32::new(0)),
    }));
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, Arc::new(EventBus::new(4096))));
    let config: RagentConfig =
        serde_json::from_str(r#"{"loop":{"max_steps":25}}"#).expect("valid config JSON");
    let processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus: Arc::new(EventBus::new(4096)),
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(HashMap::new()),
        cached_config: parking_lot::Mutex::new(Some(CachedConfig {
            config: Arc::new(config),
            file_mtimes: Vec::new(),
            env_overrides_present: false,
        })),
        team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
            std::collections::HashMap::new(),
        )),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(HashMap::new()),
    });
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().to_path_buf();
    Ok((processor, path, captured, tmp))
}

fn make_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General agent");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });
    agent
}

/// The system prompt of the first captured LLM request.
fn first_system(captured: &Arc<Mutex<Vec<ChatRequest>>>) -> String {
    let requests = captured.lock().expect("captured lock");
    requests
        .first()
        .and_then(|request| request.system.as_ref().map(|system| system.to_string()))
        .unwrap_or_default()
}

/// FR-006: the goal section reaches the loop's system prompt with every
/// structured component.
#[tokio::test]
async fn test_goal_loop_composes_structured_goal_into_system_prompt() -> Result<()> {
    let (processor, working_dir, captured, _dir_guard) = make_processor()?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("coder", "all tests pass");
    spec.verify_cmd = Some("cargo test".to_string());
    spec.scope = vec!["src/**".to_string(), "tests/**".to_string()];
    spec.read_only = vec!["tests/**".to_string()];
    spec.tool_set = vec!["bash".to_string(), "read".to_string()];
    spec.max_steps = Some(12);
    spec.cost_limit = Some(40_000);
    processor.start_loop(&session.id, spec).await;

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");

    let system = first_system(&captured);
    assert!(system.contains("## Goal Loop"), "missing goal section");
    assert!(system.contains("`coder`"), "agent preset named: {system}");
    assert!(
        system.contains("all tests pass"),
        "success state rendered: {system}"
    );
    assert!(
        system.contains("cargo test"),
        "verification command rendered: {system}"
    );
    assert!(
        system.contains("src/**") && system.contains("tests/**"),
        "scope boundaries rendered: {system}"
    );
    assert!(
        system.contains("read-only"),
        "read-only constraints rendered: {system}"
    );
    assert!(
        system.contains("`bash`") || system.contains("bash"),
        "tool set rendered: {system}"
    );
    assert!(
        system.contains("12") && system.contains("40"),
        "budget knobs rendered: {system}"
    );
    Ok(())
}

/// FR-006: a minimal spec still renders the success state and the resolved
/// config-default budget.
#[tokio::test]
async fn test_minimal_spec_renders_success_state_and_default_budget() -> Result<()> {
    let (processor, working_dir, captured, _dir_guard) = make_processor()?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    processor
        .start_loop(&session.id, LoopSpec::new("general", "write the file"))
        .await;

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");

    let system = first_system(&captured);
    assert!(
        system.contains("## Goal Loop") && system.contains("write the file"),
        "minimal spec success state rendered: {system}"
    );
    assert!(
        system.contains("25"),
        "config-default step budget resolved into the prompt: {system}"
    );
    Ok(())
}

/// FR-006: a plain chat turn (no active loop) renders none of the goal
/// section.
#[tokio::test]
async fn test_plain_chat_turn_has_no_goal_section() -> Result<()> {
    let (processor, working_dir, captured, _dir_guard) = make_processor()?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    processor
        .process_message(
            &session.id,
            "just chat",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");

    let system = first_system(&captured);
    assert!(
        !system.contains("## Goal Loop"),
        "no goal section on plain turns: {system}"
    );
    Ok(())
}
