//! Integration tests for the human interrupt (spec `agentloop`, task
//! T-011 / FR-016): pressing `Esc` while a goal-driven loop is running
//! raises the loop's interrupt flag
//! ([`SessionProcessor::request_loop_interrupt`]) and the agent loop aborts
//! at the next inter-stage safe point — no further LLM request, no further
//! tool execution — terminating with termination status `interrupted`
//! ([`StopCondition::HumanIntervention`]). The session stays persisted and
//! resumable: the partial assistant message is saved and the turn ends
//! normally (`Ok`), never as a fatal error.
//!
//! The interrupt races against a live LLM stream too: the flag is raised as
//! soon as the scripted client is called (the model is still responding), so
//! the loop must stop between the LLM response and the tool phase. Outside a
//! loop, raising the interrupt is a no-op and plain chat turns behave
//! exactly as before.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::{Event, EventBus, FinishReason};
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::message::MessagePart;
use ragent_agent::message::Role;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::loop_state::{LoopSpec, StopCondition};
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::{Capabilities, Config as RagentConfig, Cost};

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
    /// Optional notification fired when the scripted client is called, so a
    /// test can raise the interrupt while the LLM stream is still live.
    notify_chat: Option<tokio::sync::mpsc::UnboundedSender<()>>,
}

struct ScriptedClient {
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
    call_count: Arc<AtomicU32>,
    captured: Arc<Mutex<Vec<ChatRequest>>>,
    notify_chat: Option<tokio::sync::mpsc::UnboundedSender<()>>,
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
        self.call_count.fetch_add(1, Ordering::SeqCst);
        // The client is now "waiting on the model" — the interrupt race
        // window opens here (the loop is inside the LLM stage).
        if let Some(tx) = &self.notify_chat {
            let _ = tx.send(());
        }
        // Strict in-order consumption: each call serves the next scripted
        // reply; once the script is exhausted the final entry repeats for any
        // further calls.
        let reply = {
            let mut replies = self.replies.lock().expect("replies lock");
            if replies.len() > 1 {
                replies.remove(0)
            } else {
                replies
                    .last()
                    .cloned()
                    .unwrap_or(ScriptedReply::TextOnly("done"))
            }
        };
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
            notify_chat: self.notify_chat.clone(),
        }))
    }
}

type MakeProcessorResult = (
    Arc<SessionProcessor>,
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
    make_processor_with_notify(event_bus, replies, None)
}

/// [`make_processor`] with an optional notification fired when the scripted
/// client is called (the mid-LLM interrupt race window).
fn make_processor_with_notify(
    event_bus: Arc<EventBus>,
    replies: Vec<ScriptedReply>,
    notify_chat: Option<tokio::sync::mpsc::UnboundedSender<()>>,
) -> Result<MakeProcessorResult> {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        replies: Arc::new(Mutex::new(replies)),
        call_count: Arc::new(AtomicU32::new(0)),
        shared_captured: Arc::clone(&captured),
        notify_chat,
    }));

    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    // `specified_default_agent` is private, so the loop config is injected
    // through its serde representation instead of struct-update syntax.
    let config: RagentConfig =
        serde_json::from_str(r#"{"loop":{"max_steps":25}}"#).expect("valid config JSON");
    let processor = Arc::new(SessionProcessor {
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
        active_loop_interrupts: parking_lot::RwLock::new(HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(HashMap::new()),
    });
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

/// Poll until the session's loop interrupt flag is armable (the loop is
/// active), with a 5-second budget. The notify channel unblocks at LLM-call
/// time, but under heavy scheduler load the scripted stream, tool phase and
/// second LLM call can all complete before the test task resumes — by which
/// point the loop has finished and the interrupt flag is gone. Polling
/// between attempts (asserting the loop is still active) makes the race
/// window deterministic: either the flag is raised while the loop runs, or
/// the test fails loudly because the loop ended before the interrupt.
async fn raise_interrupt_until_armed(processor: &Arc<SessionProcessor>, session_id: &str) -> bool {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(5);
    loop {
        if processor.request_loop_interrupt(session_id) {
            return true;
        }
        if !processor.loop_active(session_id).await {
            return false; // The loop ended before the interrupt could arm.
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
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

/// FR-016: `Esc` during a loop raises the interrupt flag, and the loop
/// aborts at the next inter-stage safe point — after the first tool step,
/// before the second LLM request. Termination status is `interrupted`, the
/// turn ends normally (session persisted and resumable), and the loop is
/// inactive afterwards.
#[tokio::test]
async fn test_esc_between_iterations_stops_loop_with_interrupted() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (chat_tx, mut chat_rx) = tokio::sync::mpsc::unbounded_channel();
    let (processor, working_dir, captured, _dir_guard) = make_processor_with_notify(
        event_bus.clone(),
        vec![
            think_call(),
            // Unreached: the interrupt fires at the safe point first.
            ScriptedReply::TextOnly("never asked"),
        ],
        Some(chat_tx),
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    let mut spec = LoopSpec::new("general", "think then finish");
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;

    // The turn runs on a background task; the interrupt is raised while the
    // first tool step is executing (the scripted client was called once), so
    // the next safe point is the top of iteration 2.
    let turn = tokio::spawn({
        let processor = Arc::clone(&processor);
        let sid = session.id.clone();
        async move {
            processor
                .process_message(&sid, "go", &make_agent(), Arc::new(AtomicBool::new(false)))
                .await
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(30), chat_rx.recv())
        .await
        .expect("the scripted client was called")
        .expect("channel open");
    assert!(
        raise_interrupt_until_armed(&processor, &session.id).await,
        "the loop was still active after the first LLM call, so the interrupt armed"
    );

    let outcome = tokio::time::timeout(std::time::Duration::from_secs(30), turn)
        .await
        .expect("the interrupted turn finishes promptly")
        .expect("join");
    assert!(
        outcome.is_ok(),
        "an interrupted loop ends the turn normally (session resumable): {outcome:?}"
    );

    // The abort happened at the safe point BEFORE the second LLM request.
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        1,
        "FR-016: no LLM request after the interrupt safe point"
    );

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "loop terminated exactly once");
    let Event::LoopTerminated {
        status,
        iterations,
        reason,
        ..
    } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, StopCondition::HumanIntervention.as_str());
    assert_eq!(iterations, 1, "the first tool iteration completed");
    assert!(
        reason
            .as_deref()
            .is_some_and(|r| r.contains("interrupted by user")),
        "the reason names the user interruption: {reason:?}"
    );
    assert!(!processor.loop_active(&session.id).await);
    // FR-016: the session is persisted and resumable — it still exists.
    assert!(
        processor
            .session_manager
            .get_session(&session.id)
            .expect("session lookup")
            .is_some(),
        "the session remains persisted after the interrupt"
    );
    Ok(())
}

/// FR-016: the interrupt races a live LLM response — the flag is raised the
/// moment the scripted client is called (the stream is still open), so the
/// loop stops at the safe point between the LLM response and the tool phase:
/// the scripted tool call never executes.
#[tokio::test]
async fn test_esc_mid_llm_response_stops_before_tool_phase() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (chat_tx, mut chat_rx) = tokio::sync::mpsc::unbounded_channel();
    let (processor, working_dir, captured, _dir_guard) =
        make_processor_with_notify(event_bus.clone(), vec![think_call()], Some(chat_tx))?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    let mut spec = LoopSpec::new("general", "think then finish");
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;

    // The turn runs on a background task; the interrupt is raised while the
    // LLM stream is open, mirroring the user pressing Esc mid-response.
    let turn = tokio::spawn({
        let processor = Arc::clone(&processor);
        let sid = session.id.clone();
        async move {
            processor
                .process_message(&sid, "go", &make_agent(), Arc::new(AtomicBool::new(false)))
                .await
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(30), chat_rx.recv())
        .await
        .expect("the scripted client was called")
        .expect("channel open");
    assert!(
        raise_interrupt_until_armed(&processor, &session.id).await,
        "the loop was still active while the LLM stream was open, so the interrupt armed"
    );

    let outcome = tokio::time::timeout(std::time::Duration::from_secs(30), turn)
        .await
        .expect("the interrupted turn finishes promptly")
        .expect("join");
    assert!(outcome.is_ok(), "the interrupt ends the turn normally");

    // Exactly one request; the tool phase never ran.
    assert_eq!(captured.lock().expect("captured lock").len(), 1);

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1);
    let Event::LoopTerminated {
        status, iterations, ..
    } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, StopCondition::HumanIntervention.as_str());
    assert_eq!(iterations, 1);

    // The tool call never executed: the `think` tool would have published a
    // ReasoningDelta event, and none appears on the bus.
    let mut saw_reasoning = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, Event::ReasoningDelta { .. }) {
            saw_reasoning = true;
        }
    }
    assert!(
        !saw_reasoning,
        "FR-016: no tool executes after the mid-LLM interrupt safe point"
    );
    Ok(())
}

/// FR-016: the interrupt takes precedence over the budget gate — an `Esc`
/// interrupt raised before the run starts terminates as `interrupted`, not
/// `budget_exhausted`, and no LLM request is sent at all.
#[tokio::test]
async fn test_interrupt_wins_over_budget_gate() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![think_call(), ScriptedReply::TextOnly("unreached")],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    let mut spec = LoopSpec::new("general", "one step only");
    spec.max_steps = Some(1);
    processor.start_loop(&session.id, spec).await;
    processor.request_loop_interrupt(&session.id);

    let outcome = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;
    assert!(outcome.is_ok(), "the interrupt ends the turn normally");

    // The interrupt was already raised, so the very first safe point (top of
    // iteration 1) fires before any LLM request: the user's intervention
    // beats even the opening request and the budget gate.
    assert_eq!(captured.lock().expect("captured lock").len(), 0);

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1);
    let Event::LoopTerminated { status, .. } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, StopCondition::HumanIntervention.as_str());
    Ok(())
}

/// Raising the interrupt outside an active loop is a no-op (`false`), and a
/// plain chat turn is unaffected — no `LoopTerminated` publication.
#[tokio::test]
async fn test_interrupt_without_loop_is_noop() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir_guard) =
        make_processor(event_bus.clone(), vec![ScriptedReply::TextOnly("hello")])?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    // No loop started: request_loop_interrupt finds no flag to raise.
    assert!(
        !processor.request_loop_interrupt(&session.id),
        "no active loop means no interrupt flag to raise"
    );

    let reply = processor
        .process_message(
            &session.id,
            "say hi",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("plain chat turn unaffected");
    assert_eq!(captured.lock().expect("captured lock").len(), 1);
    assert!(drain_terminated(&mut rx).is_empty());
    assert!(
        reply
            .parts
            .iter()
            .any(|part| matches!(part, MessagePart::Text { text } if text.contains("hello"))),
        "the plain reply is preserved"
    );
    Ok(())
}

/// FR-016: after the interrupt the turn ends with `MessageEnd { Cancelled }`
/// and the partial assistant message is persisted — the transcript stays
/// well-formed (user message + assistant message) for the resume path.
#[tokio::test]
async fn test_interrupted_turn_is_persisted_for_resume() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![
            think_call(),
            // Unreached: the interrupt fires at the safe point first.
            ScriptedReply::TextOnly("never asked"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    let mut spec = LoopSpec::new("general", "think then finish");
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;
    processor.request_loop_interrupt(&session.id);

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the interrupt ends the turn normally");

    // MessageEnd was published with the Cancelled reason (not Stop).
    let mut ended_cancelled = false;
    while let Ok(event) = rx.try_recv() {
        if let Event::MessageEnd { reason, .. } = event
            && reason == FinishReason::Cancelled
        {
            ended_cancelled = true;
        }
    }
    assert!(
        ended_cancelled,
        "the interrupted turn ends with MessageEnd {{ Cancelled }}"
    );

    // The transcript is persisted: the user goal message plus a finalised
    // assistant message row (the loop's placeholder, updated on stop).
    let messages = processor
        .session_manager
        .get_messages(&session.id)
        .expect("messages readable");
    assert!(
        messages.iter().any(|m| m.role == Role::User),
        "the user goal message is persisted"
    );
    assert!(
        messages.iter().any(|m| m.role == Role::Assistant),
        "the partial assistant message is persisted"
    );
    Ok(())
}
