//! Regression test for sub-agent premature-termination (the "narration
//! without findings" truncation pattern).
//!
//! When a sub-agent that was actively calling tools produces a SHORT text-only
//! response (narration like "Now let me check …") without a tool call, the
//! agent loop used to treat that as the final answer — silently accepting a
//! fragment as the deliverable. The fix injects a one-shot "summary nudge"
//! that asks the model to produce its complete findings report, then
//! continues the loop. The next text-only response (the actual findings)
//! terminates the loop normally.
//!
//! This test uses a mock LLM client that returns different responses based
//! on the call number:
//! - Call 1: a `think` tool call + `Finish(ToolUse)` — step 1 does tool work
//! - Call 2: a short text-only narration + `Finish(Stop)` — triggers the nudge
//! - Call 3: a long text findings report + `Finish(Stop)` — the deliverable

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, AgentMode, ModelRef};
use ragent_agent::event::EventBus;
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::{SessionManager, processor::SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::{Capabilities, Cost};
use ragent_types::{ThinkingConfig, ThinkingLevel};

/// Mock client that returns different event sequences based on the call
/// number, simulating the premature-termination pattern.
struct NarrationMockClient {
    call_count: Arc<AtomicUsize>,
    captured_requests: Arc<Mutex<Vec<ChatRequest>>>,
}

#[async_trait::async_trait]
impl LlmClient for NarrationMockClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        self.captured_requests
            .lock()
            .expect("captured requests lock")
            .push(request);
        let call = self.call_count.fetch_add(1, Ordering::Relaxed);
        let events = match call {
            // Call 0: a `think` tool call — step 1 does real tool work.
            0 => vec![
                StreamEvent::ToolCallStart {
                    id: "toolu_narr_001".to_string(),
                    name: "think".to_string(),
                },
                StreamEvent::ToolCallDelta {
                    id: "toolu_narr_001".to_string(),
                    args_json: r#"{"thought":"analyzing codebase"}"#.to_string(),
                },
                StreamEvent::ToolCallEnd {
                    id: "toolu_narr_001".to_string(),
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::ToolUse,
                },
            ],
            // Call 1: short text-only narration — triggers the summary nudge.
            1 => vec![
                StreamEvent::TextDelta {
                    text: "Now let me check the remaining spots: next_task_id \
                           usage and member lookup patterns."
                        .to_string(),
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::Stop,
                },
            ],
            // Call 2 (after nudge): the actual findings report — the deliverable.
            _ => vec![
                StreamEvent::TextDelta {
                    text: "## Findings\n\n### H1. Blocking I/O on async path\n\
                           File: src/foo.rs:42\nIssue: std::fs call in async fn\n\
                           Fix: use spawn_blocking\n\n### H2. Unnecessary clone\n\
                           File: src/bar.rs:88\nIssue: clones Vec every iteration\n\
                           Fix: borrow instead"
                        .to_string(),
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::Stop,
                },
            ],
        };
        Ok(Box::pin(stream::iter(events)))
    }
}

#[derive(Clone)]
struct NarrationMockProvider {
    call_count: Arc<AtomicUsize>,
    captured_requests: Arc<Mutex<Vec<ChatRequest>>>,
}

#[async_trait::async_trait]
impl Provider for NarrationMockProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn name(&self) -> &'static str {
        "Narration Mock"
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
        Ok(Box::new(NarrationMockClient {
            call_count: Arc::clone(&self.call_count),
            captured_requests: Arc::clone(&self.captured_requests),
        }))
    }
}

/// Build a SessionProcessor wired with the narration mock provider.
fn make_processor(
    event_bus: Arc<EventBus>,
) -> (
    SessionProcessor,
    std::path::PathBuf,
    Arc<AtomicUsize>,
    Arc<Mutex<Vec<ChatRequest>>>,
) {
    let call_count = Arc::new(AtomicUsize::new(0));
    let captured_requests = Arc::new(Mutex::new(Vec::new()));
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(NarrationMockProvider {
        call_count: Arc::clone(&call_count),
        captured_requests: Arc::clone(&captured_requests),
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
        // Auto-approve so the `think` tool call in step 1 executes without a
        // permission prompt (which would hang in a test with no TUI).
        auto_approve: true,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
    };

    let tmp = tempfile::tempdir().expect("tempdir");
    (
        processor,
        tmp.path().to_path_buf(),
        call_count,
        captured_requests,
    )
}

/// Sub-agent that produces narration without findings should be nudged to
/// produce its report. The final output must contain the findings, not just
/// the narration fragment.
#[tokio::test]
async fn test_subagent_narration_nudge_produces_findings() {
    let event_bus = Arc::new(EventBus::new(64));
    let (processor, working_dir, call_count, _captured) = make_processor(event_bus.clone());
    let session_manager = processor.session_manager.clone();
    let session = session_manager
        .create_session(working_dir)
        .expect("session should be created");

    let mut agent = AgentInfo::new("explore", "Exploration agent");
    agent.mode = AgentMode::Subagent;
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });

    let reply = processor
        .process_message(
            &session.id,
            "Review the code for optimization opportunities.",
            &agent,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("process_message should succeed");

    let output = reply.text_content();

    // The model was called 3 times: tool call → narration → findings.
    assert_eq!(
        call_count.load(Ordering::Relaxed),
        3,
        "expected 3 LLM calls (tool call, narration, findings), got {}: \
         output was: {output}",
        call_count.load(Ordering::Relaxed)
    );

    // The findings report must be present in the final output.
    assert!(
        output.contains("## Findings"),
        "final output should contain the findings report, got: {output}"
    );
    assert!(
        output.contains("H1. Blocking I/O"),
        "final output should contain finding H1, got: {output}"
    );

    // The narration is deliberately removed from the final saved message so
    // the deliverable contains only the actual findings report.
    assert!(
        !output.contains("Now let me check"),
        "final output should NOT contain the premature narration fragment, got: {output}"
    );
}

/// A primary (non-subagent) agent that produces a short text-only response
/// after tool work should NOT be nudged — the nudge is subagent-only.
#[tokio::test]
async fn test_primary_agent_short_response_not_nudged() {
    let event_bus = Arc::new(EventBus::new(64));
    let (processor, working_dir, call_count, _captured) = make_processor(event_bus.clone());
    let session_manager = processor.session_manager.clone();
    let session = session_manager
        .create_session(working_dir)
        .expect("session should be created");

    let mut agent = AgentInfo::new("general", "General");
    // Primary mode — the nudge must NOT fire.
    agent.mode = AgentMode::Primary;
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });

    let reply = processor
        .process_message(
            &session.id,
            "Review the code for optimization opportunities.",
            &agent,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("process_message should succeed");

    let output = reply.text_content();

    // Primary agent: only 2 calls (tool call → short text). No nudge, no
    // third call for findings.
    assert_eq!(
        call_count.load(Ordering::Relaxed),
        2,
        "primary agent should not be nudged; expected 2 LLM calls, got {}: \
         output was: {output}",
        call_count.load(Ordering::Relaxed)
    );

    // Output should be just the narration, no findings.
    assert!(
        output.contains("Now let me check"),
        "output should contain the narration"
    );
    assert!(
        !output.contains("## Findings"),
        "primary agent should not have findings (no nudge fired)"
    );
}
