//! Integration tests for the loop verification gate (spec `agentloop`,
//! task T-005 / FR-007, FR-010): when a loop carries a verification command,
//! the model's first no-tool-call response triggers the command automatically.
//! Success completes the run, failure with steps remaining feeds the failure
//! output back as an observation, and failure without steps remaining ends
//! the run as a budget exhaustion.
//!
//! The mock provider scripts text-only replies so the gate is the only thing
//! that decides when the loop stops.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::{Event, EventBus};
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::loop_state::{LoopSpec, StopCondition};
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::{Capabilities, Config as RagentConfig, Cost};
use ragent_types::ThinkingConfig;

/// Text-only replies terminate the model side; the verification gate then
/// decides the loop's fate.
#[derive(Clone, Debug)]
enum ScriptedReply {
    TextOnly(&'static str),
}

#[derive(Clone)]
struct ScriptedProvider {
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
    call_count: Arc<AtomicU32>,
    shared_captured: Arc<Mutex<Vec<ChatRequest>>>,
}

struct ScriptedClient {
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
    call_count: Arc<AtomicU32>,
    captured: Arc<Mutex<Vec<ChatRequest>>>,
}

#[async_trait::async_trait]
impl LlmClient for ScriptedClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        self.captured
            .lock()
            .expect("captured requests lock")
            .push(request);
        let call = self.call_count.fetch_add(1, Ordering::SeqCst);
        let mut replies = self.replies.lock().expect("replies lock");
        let reply = if replies.is_empty() {
            ScriptedReply::TextOnly("done")
        } else {
            let index = usize::try_from(call).unwrap_or(usize::MAX);
            if index < replies.len() - 1 {
                replies.remove(0)
            } else {
                replies
                    .last()
                    .cloned()
                    .unwrap_or(ScriptedReply::TextOnly("done"))
            }
        };
        drop(replies);
        let events: Vec<StreamEvent> = match reply {
            ScriptedReply::TextOnly(text) => vec![
                StreamEvent::TextDelta {
                    text: text.to_string(),
                },
                StreamEvent::Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::Stop,
                },
            ],
        };
        Ok(Box::pin(stream::iter(events)))
    }
}

#[async_trait::async_trait]
impl Provider for ScriptedProvider {
    fn id(&self) -> &'static str {
        "ollama"
    }

    fn name(&self) -> &'static str {
        "Scripted Mock Ollama"
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
                reasoning: false,
                streaming: true,
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
            },
            context_window: 128_000,
            max_output: Some(8_192),
            request_multiplier: None,
            thinking_config: Some(ThinkingConfig::default()),
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
        Ok(Box::new(ScriptedClient {
            replies: Arc::clone(&self.replies),
            call_count: Arc::clone(&self.call_count),
            captured: Arc::clone(&self.shared_captured),
        }))
    }
}

type MakeProcessorResult = (
    SessionProcessor,
    std::path::PathBuf,
    Arc<Mutex<Vec<ChatRequest>>>,
    // Held by the caller so the session directory stays alive on disk.
    tempfile::TempDir,
);

/// Build a processor whose provider replays `replies` with the default loop
/// config injected into the processor's config cache (empty `file_mtimes`
/// keep the cache hit, so no disk config or env-var override interferes).
fn make_processor(
    event_bus: Arc<EventBus>,
    replies: Vec<ScriptedReply>,
) -> Result<MakeProcessorResult> {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        replies: Arc::new(Mutex::new(replies)),
        call_count: Arc::new(AtomicU32::new(0)),
        shared_captured: Arc::clone(&captured),
    }));

    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    // `specified_default_agent` is private, so the loop config is injected
    // through its serde representation instead of struct-update syntax.
    let loop_json = serde_json::to_string(&LoopConfig::default()).expect("serialisable");
    let config: RagentConfig =
        serde_json::from_str(&format!(r#"{{"loop":{loop_json}}}"#)).expect("valid config JSON");
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
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    };
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().to_path_buf();
    Ok((processor, path, captured, tmp))
}

/// Drain every `LoopTerminated` event from the bus.
fn drain_terminated(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Event> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if matches!(event, Event::LoopTerminated { .. }) {
            events.push(event);
        }
    }
    events
}

fn make_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });
    agent
}

use ragent_config::LoopConfig;

/// FR-007 / FR-010 (pass path): a no-tool-call response with a verification
/// command that succeeds terminates with `completed` and the verification
/// label in the event.
#[tokio::test]
async fn test_verification_pass_completes_loop() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir_guard) =
        make_processor(event_bus.clone(), vec![ScriptedReply::TextOnly("finished")])?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    let mut spec = LoopSpec::new("general", "make it work");
    spec.verify_cmd = Some("true".to_string());
    processor.start_loop(&session.id, spec).await;

    let agent = make_agent();
    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should succeed");

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "termination published exactly once");
    let Event::LoopTerminated {
        status,
        iterations,
        verification,
        ..
    } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, StopCondition::GoalAchieved.as_str());
    assert_eq!(iterations, 1);
    assert_eq!(
        verification.as_deref(),
        Some("verification command passed"),
        "the verification outcome label is published"
    );
    assert!(!processor.loop_active(&session.id).await);
    Ok(())
}

/// FR-007 (fail with steps remaining): the first failing gate appends the
/// failure output as an observation and continues; the next no-tool-call
/// response re-runs the gate, which now passes.
#[tokio::test]
async fn test_verification_failure_appends_observation_and_continues() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::TextOnly("still working"),
            ScriptedReply::TextOnly("done"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    // Stateful command: the first run fails, any later run passes.
    let marker = working_dir.join("gate-marker");
    let verify_cmd = format!(
        "if [ -f {} ]; then exit 0; else touch {}; exit 7; fi",
        marker.display(),
        marker.display()
    );
    let mut spec = LoopSpec::new("general", "make the gate pass");
    spec.verify_cmd = Some(verify_cmd.clone());
    processor.start_loop(&session.id, spec).await;

    let agent = make_agent();
    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should succeed");

    // Two requests: the failed gate fed the observation back into the
    // context and the loop continued rather than terminating.
    let has_observation = {
        let requests = captured.lock().expect("captured lock");
        assert_eq!(requests.len(), 2, "failure with steps remaining continues");
        let second_messages = &requests[1].messages;
        second_messages.iter().any(|message| {
            message.role == "user"
                && match &message.content {
                    ragent_agent::llm::ChatContent::Text(text) => {
                        text.contains("verification command failed") && text.contains("exit code 7")
                    }
                    _ => false,
                }
        })
    };
    assert!(
        has_observation,
        "the second request must contain the verification-failure observation"
    );

    // The loop eventually completed via the re-run gate.
    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "termination published exactly once");
    let Event::LoopTerminated {
        status,
        iterations,
        verification,
        ..
    } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, StopCondition::GoalAchieved.as_str());
    assert_eq!(iterations, 2, "two completed iterations");
    assert_eq!(verification.as_deref(), Some("verification command passed"));
    assert!(!processor.loop_active(&session.id).await);
    Ok(())
}

/// FR-007 (fail without steps): when the step budget is already breached,
/// a failed gate cannot retry, so the run terminates `budget_exhausted`.
#[tokio::test]
async fn test_verification_failure_without_steps_ends_budget_exhausted() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::TextOnly("claim done")],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    let mut spec = LoopSpec::new("general", "impossible goal");
    spec.verify_cmd = Some("exit 7".to_string());
    spec.max_steps = Some(1);
    processor.start_loop(&session.id, spec).await;

    let agent = make_agent();
    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should finalise cleanly");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        1,
        "no second request may follow the exhausted budget"
    );
    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1);
    let Event::LoopTerminated {
        status,
        verification,
        reason,
        ..
    } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, StopCondition::BudgetExhausted.as_str());
    assert!(
        reason
            .as_deref()
            .is_some_and(|r| r.contains("no steps remain")),
        "reason explains the exhausted verification: {reason:?}"
    );
    assert!(
        verification
            .as_deref()
            .is_some_and(|v| v.contains("exit code 7")),
        "the failure output is surfaced: {verification:?}"
    );
    assert!(!processor.loop_active(&session.id).await);
    Ok(())
}

/// FR-010 regression (T-004 path): with no verification command configured,
/// a no-tool-call response completes the loop directly after one request.
#[tokio::test]
async fn test_no_verification_command_completes_directly() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir_guard) =
        make_processor(event_bus.clone(), vec![ScriptedReply::TextOnly("answer")])?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    processor
        .start_loop(&session.id, LoopSpec::new("general", "answer the question"))
        .await;

    let agent = make_agent();
    processor
        .process_message(
            &session.id,
            "hello",
            &agent,
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("process_message should succeed");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        1,
        "no verification command: the first response completes the loop"
    );
    let terminated = drain_terminated(&mut rx);
    let Event::LoopTerminated {
        status,
        verification,
        ..
    } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, StopCondition::GoalAchieved.as_str());
    assert_eq!(verification, None, "no verification ran");
    Ok(())
}

/// FR-007: a stateful gate that keeps failing while steps remain produces
/// one observation per failing no-tool-call response; the loop continues
/// until the budget gate stops it.
#[tokio::test]
async fn test_verification_failures_accumulate_until_budget_stops() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::TextOnly("try 1"),
            ScriptedReply::TextOnly("try 2"),
            ScriptedReply::TextOnly("try 3"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    // Always-failing gate.
    let mut spec = LoopSpec::new("general", "never passes");
    spec.verify_cmd = Some("echo failing-output; exit 7".to_string());
    spec.max_steps = Some(3);
    processor.start_loop(&session.id, spec).await;

    let agent = make_agent();
    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should finalise cleanly");

    // Iteration 1 responds text-only, the gate fails (steps remain), the
    // observation is appended; iteration 2 the same; iteration 3: after the
    // request the step budget is exhausted, so the failing gate ends the
    // run as `budget_exhausted` without a fourth request.
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        3,
        "the budget gate stops the loop before a fourth request"
    );
    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1);
    let Event::LoopTerminated {
        status,
        iterations,
        reason,
        ..
    } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, StopCondition::BudgetExhausted.as_str());
    assert_eq!(iterations, 3);
    assert!(
        reason
            .as_deref()
            .is_some_and(|r| r.contains("no steps remain")),
        "reason names the exhausted verification: {reason:?}"
    );
    assert!(!processor.loop_active(&session.id).await);
    Ok(())
}
