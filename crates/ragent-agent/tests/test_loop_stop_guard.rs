#![allow(clippy::assert_is_empty)]
//! Integration tests for the loop stop-flag guard (spec `agentloop`, task
//! T-008 / FR-010, FR-017): once the loop's stop flag is set — by goal
//! achievement or a budget breach — no further LLM request is sent, the
//! termination event is published exactly once, and the turn finalises
//! cleanly.
//!
//! The mock provider scripts per-call responses: text-only replies end the
//! loop with `GoalAchieved`, while tool-call replies keep it running so the
//! step/token budget gates can fire before the next request.

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
use ragent_config::{Capabilities, Config as RagentConfig, Cost, LoopConfig};
use ragent_types::{ThinkingConfig, ThinkingLevel};

/// The mock returns one of these per `chat` call, in order; the last entry
/// repeats for any further calls.
#[derive(Clone, Debug)]
enum ScriptedReply {
    /// Text-only response ending with `Finish { Stop }` (goal achieved).
    TextOnly(&'static str),
    /// A single `think` tool call followed by `Finish { ToolUse }`.
    ToolCall,
    /// A `think` tool call with a `Usage` event carrying heavy token counts,
    /// so the loop keeps iterating while the token tally accumulates.
    ToolCallWithUsage,
}

#[derive(Clone)]
struct ScriptedProvider {
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
    call_count: Arc<AtomicU32>,
    /// Shared with the created clients so all requests (across client
    /// instances) land in one capturable list.
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
            ScriptedReply::ToolCall => vec![
                StreamEvent::ToolCallStart {
                    id: "call-1".to_string(),
                    name: "think".to_string(),
                },
                StreamEvent::ToolCallDelta {
                    id: "call-1".to_string(),
                    args_json: r#"{"thought":"working"}"#.to_string(),
                },
                StreamEvent::ToolCallEnd {
                    id: "call-1".to_string(),
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::ToolUse,
                },
            ],
            ScriptedReply::ToolCallWithUsage => vec![
                StreamEvent::ToolCallStart {
                    id: "call-1".to_string(),
                    name: "think".to_string(),
                },
                StreamEvent::ToolCallDelta {
                    id: "call-1".to_string(),
                    args_json: r#"{"thought":"working"}"#.to_string(),
                },
                StreamEvent::ToolCallEnd {
                    id: "call-1".to_string(),
                },
                StreamEvent::Usage {
                    input_tokens: 600,
                    output_tokens: 400,
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
);

/// Build a processor whose provider replays `replies` with the compiled
/// default loop config; the captured request handle is shared so tests can
/// count real conversation requests.
fn make_processor(
    event_bus: Arc<EventBus>,
    replies: Vec<ScriptedReply>,
) -> Result<MakeProcessorResult> {
    make_processor_with_config(event_bus, replies, LoopConfig::default())
}

/// T-003: build a processor with a specific loop config injected into the
/// processor's config cache (empty `file_mtimes` keep the cache hit, so no
/// disk config or env-var override can interfere with the test).
fn make_processor_with_config(
    event_bus: Arc<EventBus>,
    replies: Vec<ScriptedReply>,
    loop_config: LoopConfig,
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
    let loop_json = serde_json::to_string(&loop_config).expect("serialisable loop config");
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
    Ok((processor, tmp.path().to_path_buf(), captured))
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

/// FR-010 / FR-017: a text-only first response terminates the loop with
/// `GoalAchieved` after exactly one LLM request; the stop flag prevents any
/// further stage (no second request), and the event is published once.
#[tokio::test]
async fn test_goal_achieved_stops_loop_after_single_request() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured) = make_processor(event_bus.clone(), Vec::new())?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    processor
        .start_loop(&session.id, LoopSpec::new("general", "answer the question"))
        .await;

    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
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

    // Exactly one conversation request: the loop stopped after the first
    // exchange (no additional stage ran after the stop flag was set).
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        1,
        "the loop must stop after the goal-achieving response"
    );

    // The termination event is published exactly once with the completed
    // status and the recorded iteration count.
    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "LoopTerminated published exactly once");
    let Event::LoopTerminated {
        session_id,
        status,
        iterations,
        verification,
        reason,
    } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(session_id, session.id);
    assert_eq!(status, StopCondition::GoalAchieved.as_str());
    assert_eq!(iterations, 1, "one completed iteration");
    assert_eq!(verification, None);
    assert_eq!(reason, None);
    assert!(!processor.loop_active(&session.id).await);
    Ok(())
}

/// FR-013 / FR-017: with `max_steps: 2` and a mock that always returns tool
/// calls, the third request must never be sent — the budget gate terminates
/// with `budget_exhausted` BEFORE the next LLM request.
#[tokio::test]
async fn test_step_budget_stops_before_next_request() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    // Every call answers with a tool call, so the loop can only end via the
    // budget gate.
    let (processor, working_dir, captured) =
        make_processor(event_bus.clone(), vec![ScriptedReply::ToolCall])?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut spec = LoopSpec::new("general", "keep calling tools forever");
    spec.max_steps = Some(2);
    processor.start_loop(&session.id, spec).await;

    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });

    let reply = processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should finalise cleanly at the budget gate");

    assert!(
        reply.text_content().is_empty() || !reply.text_content().contains("panic"),
        "the turn should finalise via the normal finalise path"
    );

    // Exactly two requests: the third was gated before it could be sent.
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        2,
        "max_steps=2 must stop the loop BEFORE the third request (FR-013)"
    );

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "termination published exactly once");
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
    assert_eq!(iterations, 2);
    assert_eq!(
        reason.as_deref(),
        Some("step budget exhausted (2/2 iterations)")
    );
    assert!(!processor.loop_active(&session.id).await);
    Ok(())
}

/// FR-014 / FR-017: with a small `cost_limit`, the token tally from the first
/// exchange breaches the budget and the second request must never be sent.
/// The mock keeps calling tools (with heavy usage) so the loop stays alive
/// until the token gate fires at the top of the next iteration.
#[tokio::test]
async fn test_token_budget_stops_before_next_request() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    // Every call answers with a tool call + heavy usage (600 in + 400 out).
    let (processor, working_dir, captured) =
        make_processor(event_bus.clone(), vec![ScriptedReply::ToolCallWithUsage])?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut spec = LoopSpec::new("general", "keep responding");
    // The first exchange tallies 1000 tokens, breaching this limit before the
    // second request could be sent.
    spec.cost_limit = Some(500);
    processor.start_loop(&session.id, spec).await;

    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });

    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should finalise cleanly");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        1,
        "the token budget must stop the loop BEFORE the second request (FR-014)"
    );

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "termination published exactly once");
    let Event::LoopTerminated {
        status, iterations, ..
    } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, StopCondition::BudgetExhausted.as_str());
    // The breaching iteration never began (begin_step refuses on breach), so
    // the published count is the completed iterations only.
    assert_eq!(iterations, 1);
    assert!(!processor.loop_active(&session.id).await);
    Ok(())
}
/// T-003 (FR-013): when the loop spec leaves `max_steps` unset, the `loop`
/// config default applies — here `loop.max_steps: 2` from the injected
/// config stops the loop before the third request even though the spec
/// itself carries no budget.
#[tokio::test]
async fn test_config_default_step_budget_stops_loop() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let loop_config = LoopConfig {
        max_steps: 2,
        ..LoopConfig::default()
    };
    let (processor, working_dir, captured) = make_processor_with_config(
        event_bus.clone(),
        vec![ScriptedReply::ToolCall],
        loop_config,
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    // The spec carries no budgets: the config defaults must arm the gates.
    processor
        .start_loop(&session.id, LoopSpec::new("general", "keep calling tools"))
        .await;

    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });

    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should finalise cleanly at the budget gate");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        2,
        "the config loop.max_steps default must stop the loop BEFORE the third request (FR-013)"
    );

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "termination published exactly once");
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
    assert_eq!(iterations, 2);
    assert_eq!(
        reason.as_deref(),
        Some("step budget exhausted (2/2 iterations)")
    );
    Ok(())
}

/// T-003 (FR-014): when the loop spec leaves `cost_limit` unset, the `loop`
/// config cost budget applies — the token tally from the first exchange
/// breaches it and the second request is never sent.
#[tokio::test]
async fn test_config_cost_limit_stops_loop() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let loop_config = LoopConfig {
        cost_limit: Some(500),
        ..LoopConfig::default()
    };
    // Every call answers with a tool call + heavy usage (600 in + 400 out).
    let (processor, working_dir, captured) = make_processor_with_config(
        event_bus.clone(),
        vec![ScriptedReply::ToolCallWithUsage],
        loop_config,
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    processor
        .start_loop(&session.id, LoopSpec::new("general", "keep responding"))
        .await;

    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });

    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should finalise cleanly");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        1,
        "the config loop.cost_limit default must stop the loop BEFORE the second request (FR-014)"
    );

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "termination published exactly once");
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
    assert_eq!(iterations, 1);
    assert_eq!(
        reason.as_deref(),
        Some("token cost budget exhausted (1000 tokens accumulated)")
    );
    Ok(())
}

/// T-003: an explicitly configured spec budget wins over the loop config
/// default — the spec's `max_steps: 4` overrides the injected config value
/// of 2, so the loop runs four iterations before stopping.
#[tokio::test]
async fn test_explicit_spec_budget_wins_over_config_default() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let loop_config = LoopConfig {
        max_steps: 2,
        cost_limit: Some(500),
        ..LoopConfig::default()
    };
    let (processor, working_dir, captured) = make_processor_with_config(
        event_bus.clone(),
        vec![ScriptedReply::ToolCall],
        loop_config,
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut spec = LoopSpec::new("general", "keep calling tools");
    // Explicit spec budget: wins over the config's max_steps = 2. The cost
    // limit stays unset in the spec, so no token gate applies.
    spec.max_steps = Some(4);
    processor.start_loop(&session.id, spec).await;

    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });

    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should finalise cleanly at the budget gate");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        4,
        "the explicit spec budget (4) must override the config default (2)"
    );

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "termination published exactly once");
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
    assert_eq!(iterations, 4);
    assert_eq!(
        reason.as_deref(),
        Some("step budget exhausted (4/4 iterations)")
    );
    Ok(())
}
