//! The terminal-outcome constructors (spec `uniformtools` FR-012, FR-014,
//! FR-021).
//!
//! Every way a tool call can stop without a tool result - a pre-dispatch
//! refusal, a guard's denial, and a task that panicked or was aborted - produces
//! the same nine-field outcome tuple and publishes the `ToolCallEnd` that closes
//! the call out. Before T-014 each of those paths assembled its own copy of that
//! tuple in the loop body, and the copies disagreed: the refusal published
//! `Error: <sentence>` while the others published the bare sentence, the status
//! was decided three times, and a join failure closed the call out twice or not
//! at all.
//!
//! These cases pin what a caller can observe, through the public path that
//! produces the outcomes - `SessionProcessor::process_message` - so the
//! constructors are exercised as the loop uses them:
//!
//! * a refused call is recorded as an error, and the text the model reads is the
//!   refusal's own sentence behind the one `Error: ` marker (FR-014, FR-021);
//! * a call stopped by a guard produces the identical record shape, so the two
//!   cannot be told apart in history by their status alone;
//! * each stopped call publishes exactly one `ToolCallEnd`, with its own call
//!   id, so no spinner is left spinning and no sibling result is dropped.
//!
//! The pipeline's *ordering* is pinned separately in `test_unified_dispatch.rs`;
//! these cases are about the shape of a terminal outcome, not about when a stage
//! runs.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::{Event, EventBus};
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::message::{Message, MessagePart, ToolCallStatus};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::permission::PermissionRule;
use ragent_config::{Capabilities, Config as RagentConfig, Cost, StreamConfig};

const PROVIDER_ID: &str = "ollama";
const MODEL_ID: &str = "qwen3:latest";

// ---------------------------------------------------------------------------
// Scripted provider / client
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct ScriptedProvider {
    replies: Arc<Mutex<Vec<(String, String)>>>,
    call_count: Arc<AtomicU32>,
}

struct ScriptedClient {
    replies: Arc<Mutex<Vec<(String, String)>>>,
    call_count: Arc<AtomicU32>,
}

#[async_trait::async_trait]
impl LlmClient for ScriptedClient {
    async fn chat(
        &self,
        _request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        // The scripted replies are consumed in order; once only one remains it
        // repeats, which the plain-text branch below makes harmless.
        let reply = {
            let mut replies = self.replies.lock().expect("replies lock");
            if replies.len() > 1 {
                replies.remove(0)
            } else {
                replies
                    .last()
                    .cloned()
                    .unwrap_or_else(|| ("think".to_string(), "{\"thought\":\"x\"}".to_string()))
            }
        };
        // The first reply is the scripted tool call; every later reply is plain
        // text. A refused or denied call produces no tool result, so a client
        // that kept replaying the scripted call would make the turn loop for
        // ever - the call would never be answered. The call id therefore carries
        // the ordinal, so the record and the events agree on which call they
        // describe.
        let ordinal = self.call_count.load(Ordering::SeqCst);
        let text_reply = ordinal > 1;
        let call_id = format!("call_{ordinal}");
        if text_reply {
            let events = vec![
                StreamEvent::TextDelta {
                    text: "done".to_string(),
                },
                StreamEvent::Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::Stop,
                },
            ];
            return Ok(Box::pin(stream::iter(events)));
        }
        let events = vec![
            StreamEvent::ToolCallStart {
                id: call_id.clone(),
                name: reply.0,
            },
            StreamEvent::ToolCallDelta {
                id: call_id.clone(),
                args_json: reply.1,
            },
            StreamEvent::ToolCallEnd { id: call_id },
            StreamEvent::Usage {
                input_tokens: 10,
                output_tokens: 5,
            },
            StreamEvent::Finish {
                reason: LlmFinishReason::ToolUse,
            },
        ];
        Ok(Box::pin(stream::iter(events)))
    }
}

#[async_trait::async_trait]
impl Provider for ScriptedProvider {
    fn id(&self) -> &'static str {
        PROVIDER_ID
    }

    fn name(&self) -> &'static str {
        "Scripted Mock Ollama"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: MODEL_ID.to_string(),
            provider_id: PROVIDER_ID.to_string(),
            name: "Qwen3".to_string(),
            cost: Cost {
                input: 0.0,
                output: 0.0,
            },
            capabilities: Capabilities {
                vision: false,
                tool_use: true,
                thinking_levels: vec![],
                reasoning: false,
                streaming: true,
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
        }))
    }
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Harness {
    processor: SessionProcessor,
    working_dir: std::path::PathBuf,
    _dir: tempfile::TempDir,
}

fn make_processor(
    event_bus: Arc<EventBus>,
    replies: Vec<(String, String)>,
    auto_approve: bool,
) -> Result<Harness> {
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        replies: Arc::new(Mutex::new(replies)),
        call_count: Arc::new(AtomicU32::new(0)),
    }));

    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let config: RagentConfig = serde_json::from_str("{}").expect("valid config JSON");

    let processor = SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(Vec::<
            PermissionRule,
        >::new(
        )))),
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        tool_repeat_guard: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        mcp_client: std::sync::OnceLock::new(),
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
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
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
        auto_approve,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(HashMap::new()),
        pending_verdicts: ragent_agent::session::pending::PendingVerdicts::new(),
    };
    let tmp = tempfile::tempdir().expect("tempdir");
    Ok(Harness {
        processor,
        working_dir: tmp.path().to_path_buf(),
        _dir: tmp,
    })
}

fn make_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: PROVIDER_ID.to_string(),
        model_id: MODEL_ID.to_string(),
    });
    agent
}

/// The recorded tool-call parts of a reply, in order.
fn tool_calls(reply: &Message) -> Vec<(String, String, ToolCallStatus, Option<String>)> {
    reply
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::ToolCall {
                tool,
                call_id,
                state,
            } => Some((
                tool.clone(),
                call_id.clone(),
                state.status.clone(),
                state.error.clone(),
            )),
            _ => None,
        })
        .collect()
}

/// The `(call_id, error)` of every `ToolCallEnd` on the bus.
fn tool_call_ends(events: &[Event]) -> Vec<(String, Option<String>)> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ToolCallEnd { call_id, error, .. } => Some((call_id.clone(), error.clone())),
            _ => None,
        })
        .collect()
}

/// The text the model is shown for each tool result in the reply's request.
fn tool_result_texts(events: &[Event]) -> Vec<String> {
    events
        .iter()
        .filter_map(|event| match event {
            Event::ToolResult { content, .. } => Some(content.clone()),
            _ => None,
        })
        .collect()
}

fn drain(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Event> {
    let mut events = Vec::new();
    while let Ok(event) = rx.try_recv() {
        events.push(event);
    }
    events
}

/// Runs one tool call through `process_message` and returns the reply and every
/// event the bus carried.
async fn run_one(name: &str, args: &str, auto_approve: bool) -> Result<(Message, Vec<Event>)> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let harness = make_processor(
        event_bus,
        vec![(name.to_string(), args.to_string())],
        auto_approve,
    )?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");
    let reply = harness
        .processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await?;
    let events = drain(&mut rx);
    Ok((reply, events))
}

/// How long the scripted checkpoint prompt may hang before the safe default
/// (denial) resolves it.
///
/// `start_loop` resolves a spec's `None` to the `loop.checkpoint_timeout_secs`
/// config default (120s), so the spec sets a short value explicitly: the case is
/// about the shape of the denial, not about waiting out a production timeout.
const CHECKPOINT_TIMEOUT_SECS: u32 = 1;

/// Run one turn inside a checkpoint-enabled loop whose first call is destructive
/// and nobody answers the prompt, and return the reply and every event.
///
/// The timeout resolves the prompt as a denial, which is the guard-denial path
/// this file pins. `auto_approve` is `false` on purpose: auto-approval answers
/// prompts but never satisfies a destructive-action checkpoint (FR-024).
async fn denied_turn() -> Result<(Message, Vec<Event>)> {
    use ragent_agent::session::loop_state::LoopSpec;

    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let harness = make_processor(
        event_bus,
        vec![("rm".to_string(), r#"{"path": "victim.txt"}"#.to_string())],
        false,
    )?;
    std::fs::write(harness.working_dir.join("victim.txt"), "keep me").expect("seed file");
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "delete the file");
    spec.max_steps = Some(1);
    spec.checkpoint_timeout_secs = Some(CHECKPOINT_TIMEOUT_SECS);
    harness.processor.start_loop(&session.id, spec).await;
    let reply = harness
        .processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await?;
    assert!(
        harness.working_dir.join("victim.txt").exists(),
        "FR-015: the denied destructive call never executed"
    );
    let events = drain(&mut rx);
    Ok((reply, events))
}

// ---------------------------------------------------------------------------
// FR-021 / FR-014: a pre-dispatch refusal
// ---------------------------------------------------------------------------

/// An unknown tool name is recorded as an error, its message is the refusal's
/// own sentence behind the one `Error: ` marker, and exactly one `ToolCallEnd`
/// closes the call out with its real id.
#[tokio::test]
async fn an_unknown_tool_refusal_is_a_terminal_error_outcome() -> Result<()> {
    let (reply, events) = run_one("definitely_not_a_tool", "{}", true).await?;

    let calls = tool_calls(&reply);
    assert_eq!(calls.len(), 1, "the refused call is still recorded");
    let (tool, call_id, status, error) = &calls[0];
    assert_eq!(
        tool, "definitely_not_a_tool",
        "the record names what was sent"
    );
    assert_eq!(
        *status,
        ToolCallStatus::Error,
        "a refused call is recorded as an error (FR-014)"
    );
    let message = error.clone().unwrap_or_default();
    assert!(
        message.contains("Unknown tool: 'definitely_not_a_tool'"),
        "the failure text is the refusal's own sentence (FR-021): {message}"
    );
    assert!(
        message.contains("not executed"),
        "the refusal says nothing ran: {message}"
    );

    let ends = tool_call_ends(&events);
    assert_eq!(
        ends.len(),
        1,
        "a terminal outcome publishes exactly one ToolCallEnd: {ends:?}"
    );
    assert_eq!(&ends[0].0, call_id, "the event carries the call's own id");
    assert_eq!(
        ends[0].1.as_deref(),
        Some(message.as_str()),
        "the event's error and the record's error are the same text, so the UI \
         and the model cannot be told different stories"
    );

    // A refused call produced no tool output, so there is nothing for a
    // `ToolResult` to report: the sentence reaches the model through the
    // recorded call's error field, which the assertion above already pins to the
    // `ToolCallEnd` text. The refusal must not manufacture a result event
    // carrying an empty payload, which is what a client would then render as a
    // successful call with no output.
    let texts = tool_result_texts(&events);
    assert!(
        texts.is_empty(),
        "a refused call publishes no tool result, so its outcome cannot be \
         mistaken for a completed one: {texts:?}"
    );
    Ok(())
}

/// A call the schema already refuses produces the same terminal shape: an error
/// status, the refusal's sentence, and exactly one `ToolCallEnd`.
#[tokio::test]
async fn a_schema_refusal_is_a_terminal_error_outcome() -> Result<()> {
    let (reply, events) = run_one("move_file", r#"{"source": "a.txt"}"#, true).await?;

    let calls = tool_calls(&reply);
    assert_eq!(calls.len(), 1);
    let (_, call_id, status, error) = &calls[0];
    assert_eq!(*status, ToolCallStatus::Error);
    let message = error.clone().unwrap_or_default();
    assert!(
        message.contains("'destination'"),
        "the schema refusal names the parameter (FR-009): {message}"
    );

    let ends = tool_call_ends(&events);
    assert_eq!(ends.len(), 1, "exactly one ToolCallEnd: {ends:?}");
    assert_eq!(&ends[0].0, call_id);
    Ok(())
}

/// A guard denial and a pre-dispatch refusal differ in their sentence, but not
/// in their record shape: both are `Error`, both carry a message, both publish
/// one `ToolCallEnd`. A caller therefore cannot mistake a refusal for a success,
/// and cannot rely on the stage that stopped the call.
///
/// The denial scripted here is a **loop destructive-action checkpoint**: a
/// primary run reaches one by starting a checkpoint-enabled loop whose first
/// call is `rm` with nobody to answer the prompt, so the timeout denies it. The
/// obvious alternative - a sub-agent run calling `AskUserQuestion` - is not a
/// guard denial at all: that path answers the question, and the harness would
/// block forever waiting for the reply.
#[tokio::test]
async fn every_stopped_call_has_the_same_record_shape() -> Result<()> {
    let (refused, refused_events) = run_one("definitely_not_a_tool", "{}", true).await?;
    let (denied, denied_events) = denied_turn().await?;

    let refused_call = tool_calls(&refused).remove(0);
    let denied_call = tool_calls(&denied).remove(0);

    assert_eq!(refused_call.2, ToolCallStatus::Error, "refusal is an error");
    assert_eq!(denied_call.2, ToolCallStatus::Error, "denial is an error");
    assert!(
        refused_call.3.is_some() && denied_call.3.is_some(),
        "each stopped call carries its reason: {refused_call:?} / {denied_call:?}"
    );
    assert!(
        refused_call.3 != denied_call.3,
        "the two reasons are deliberately different sentences"
    );
    assert_eq!(
        tool_call_ends(&refused_events).len(),
        1,
        "the refusal closes the call out once"
    );
    assert_eq!(
        tool_call_ends(&denied_events).len(),
        1,
        "the denial closes the call out once"
    );
    Ok(())
}
