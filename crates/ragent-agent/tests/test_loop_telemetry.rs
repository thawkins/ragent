//! Integration tests for loop telemetry (spec `agentloop`, task T-017 /
//! FR-025): a goal-driven loop run records `record_agent_loop(iterations,
//! duration)` exactly once, the per-iteration tool-call tally is published
//! as the per-session tool-call total on termination, and the per-step
//! event-bus counters (step number, tool-call count) stay visible to
//! subscribers after the run.
//!
//! The mock provider scripts tool-call replies (a `think` call, which needs
//! no permission prompt) followed by a text-only reply so the loop shape is
//! deterministic.

// The tests serialise on a process-global `std::sync::Mutex`, so each of
// them holds the guard across await points. On a single test thread no
// other task contends for the lock, so this cannot deadlock.
#![allow(clippy::await_holding_lock)]

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
use ragent_telemetry::counters;

/// Serialises the tests that read the process-global telemetry snapshot
/// mirror: `counters::current_values()` is shared across the whole test
/// binary, so concurrent tests would otherwise observe each other's final
/// writes (a flaky cross-contamination of `agent_loop_iterations_last`).
static COUNTER_LOCK: Mutex<()> = Mutex::new(());

#[derive(Clone, Debug)]
enum ScriptedReply {
    /// A named tool call followed by `Finish { ToolUse }`.
    ToolCallNamed { name: &'static str, args: String },
    /// Text-only response ending with `Finish { Stop }` (goal achieved).
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
            ScriptedReply::ToolCallNamed { name, args } => vec![
                StreamEvent::ToolCallStart {
                    id: "call_1".to_string(),
                    name: name.to_string(),
                },
                StreamEvent::ToolCallDelta {
                    id: "call_1".to_string(),
                    args_json: args,
                },
                StreamEvent::ToolCallEnd {
                    id: "call_1".to_string(),
                },
                StreamEvent::Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::ToolUse,
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
    let config: RagentConfig =
        serde_json::from_str(r#"{"loop":{"max_steps":25}}"#).expect("valid config JSON");
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

/// The `think` tool needs no permission prompt, so a scripted call executes
/// cleanly against the default registry.
fn think_call() -> ScriptedReply {
    ScriptedReply::ToolCallNamed {
        name: "think",
        args: r#"{"thought": "considering the next step"}"#.to_string(),
    }
}

/// FR-025: `record_agent_loop` is recorded exactly once per run, with the
/// final iteration count and a non-negative duration. The run here spans
/// two iterations: one tool call, then the text-only completion.
#[tokio::test]
async fn test_record_agent_loop_recorded_once_with_final_iterations() -> Result<()> {
    let _counter_guard = COUNTER_LOCK.lock().expect("counter lock");
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![think_call(), ScriptedReply::TextOnly("finished")],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    let mut spec = LoopSpec::new("general", "use a tool then finish");
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;

    let agent = make_agent();
    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should succeed");

    // One loop run, terminated once as `completed` after both iterations.
    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "termination published exactly once");
    let Event::LoopTerminated {
        status, iterations, ..
    } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, StopCondition::GoalAchieved.as_str());
    assert_eq!(iterations, 2, "tool step plus the final text step");
    assert_eq!(captured.lock().expect("captured lock").len(), 2);

    // FR-025: the telemetry record carries the FINAL iteration count and a
    // non-negative duration. The snapshot mirror is updated even with the
    // OTEL provider disabled.
    let values = counters::current_values();
    assert_eq!(
        values.agent_loop_iterations_last, 2,
        "the recorded iteration count is the final one"
    );
    assert!(
        values.agent_loop_duration_last >= 0.0,
        "a duration is always recorded"
    );
    // The exactly-once flag flipped during the run.
    assert!(processor.loop_telemetry_recorded.load(Ordering::SeqCst));
    assert!(!processor.loop_active(&session.id).await);
    Ok(())
}

/// FR-025: per-iteration tool-call counts accumulate across the run and are
/// published as the per-session tool-call total; the per-step event-bus
/// counters (step number and tool calls) remain visible after the run.
#[tokio::test]
async fn test_per_iteration_tool_call_counts_accumulate() -> Result<()> {
    let _counter_guard = COUNTER_LOCK.lock().expect("counter lock");
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        // The trailing TextOnly is duplicated: the scripted client removes
        // consumed entries, so the final reply must stay reachable once the
        // list shrinks (the same scripted-call note as
        // test_loop_restrictions.rs).
        vec![
            think_call(),
            think_call(),
            ScriptedReply::TextOnly("finished"),
            ScriptedReply::TextOnly("finished"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    let mut spec = LoopSpec::new("general", "call a tool twice then finish");
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;

    let agent = make_agent();
    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should succeed");

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "termination published exactly once");
    let Event::LoopTerminated {
        status, iterations, ..
    } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, StopCondition::GoalAchieved.as_str());
    assert_eq!(iterations, 3, "two tool steps plus the final text step");

    // FR-025: the two tool calls (one per iteration) accumulate into the
    // per-session tool-call total recorded with the loop telemetry.
    let values = counters::current_values();
    assert_eq!(
        values.tool_calls_per_session_last, 2,
        "per-iteration tool-call counts are summed into the run total"
    );
    assert_eq!(
        values.agent_loop_iterations_last, 3,
        "the recorded iteration count is the final one"
    );

    // Per-step event-bus counter visibility: the bus still reports the
    // final step number and total tool calls for this session.
    assert_eq!(event_bus.current_step(&session.id), 3);
    assert_eq!(event_bus.current_tool_calls(&session.id), 2);
    Ok(())
}

/// FR-025: an external termination (no run-start instant) records no loop
/// telemetry — the record belongs to in-run termination paths, exactly once
/// per run.
#[tokio::test]
async fn test_external_termination_records_no_loop_telemetry() -> Result<()> {
    let _counter_guard = COUNTER_LOCK.lock().expect("counter lock");
    let event_bus = Arc::new(EventBus::new(64));
    let (processor, working_dir, _captured, _dir_guard) =
        make_processor(event_bus.clone(), vec![ScriptedReply::TextOnly("done")])?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    // Start with a known snapshot baseline: the mirror is process-global, so
    // a concurrent sibling test could otherwise be observed here.
    let baseline = counters::current_values();

    let spec = LoopSpec::new("general", "an externally terminated goal");
    processor.start_loop(&session.id, spec).await;

    let effective = processor
        .terminate_loop(&session.id, StopCondition::HumanIntervention, None, None)
        .await;
    assert_eq!(
        effective,
        Some(StopCondition::HumanIntervention),
        "the loop terminated via the external path"
    );

    // No in-run termination happened, so this loop published no telemetry
    // record: the mirror is unchanged from the baseline (concurrent sibling
    // tests holding the lock cannot have written either).
    let values = counters::current_values();
    assert_eq!(
        values.agent_loop_iterations_last, baseline.agent_loop_iterations_last,
        "external termination does not publish loop telemetry"
    );
    assert!(!processor.loop_telemetry_recorded.load(Ordering::SeqCst));
    Ok(())
}

/// FR-025: starting a second loop run clears the exactly-once flag so the
/// next run publishes its own telemetry record.
#[tokio::test]
async fn test_start_loop_resets_telemetry_recorded_flag() -> Result<()> {
    let _counter_guard = COUNTER_LOCK.lock().expect("counter lock");
    let event_bus = Arc::new(EventBus::new(64));
    let (processor, working_dir, _captured, _dir_guard) =
        make_processor(event_bus.clone(), vec![ScriptedReply::TextOnly("done")])?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    let spec = LoopSpec::new("general", "first run");
    processor.start_loop(&session.id, spec).await;
    processor
        .terminate_loop(&session.id, StopCondition::GoalAchieved, None, None)
        .await;

    let spec = LoopSpec::new("general", "second run");
    processor.start_loop(&session.id, spec).await;
    assert!(
        !processor.loop_telemetry_recorded.load(Ordering::SeqCst),
        "a new run must be able to publish its telemetry record again"
    );
    Ok(())
}
