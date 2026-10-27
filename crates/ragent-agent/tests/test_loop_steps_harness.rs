//! Fake-provider harness for the agent-loop step logic (audit T-704).
//!
//! `prepare_client`, `call_llm_step`, and the final-save path are inherent
//! `pub(crate)` methods on `SessionProcessor`, so an external `tests/` crate
//! cannot call them directly (and adding an inline `#[cfg(test)]` module is
//! forbidden by the plan's exit criteria). Instead this harness drives the
//! public `SessionProcessor::process_message`, which runs the loop in order:
//! `prepare_client` -> `build_turn_*` -> `call_llm_step` -> final save. A
//! scripted `Provider` + `LlmClient` makes every step deterministic.
//!
//! Covered:
//! - `prepare_client` - the captured `ChatRequest` carries the resolved model,
//!   the turn system prompt, the tool definitions, and the user message;
//! - `call_llm_step` - a text-only response and a tool-then-text two-step run
//!   both accumulate into the final assistant message; usage/`RequestStarted`
//!   events are published;
//! - final save - the returned assistant `Message` is persisted and a terminal
//!   `MessageEnd { Stop }` is published;
//! - the provider-error path surfaces an error and publishes `AgentError`.

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
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::{Capabilities, Config as RagentConfig, Cost};

#[derive(Clone, Debug)]
enum ScriptedReply {
    /// Text-only response ending with `Finish { Stop }`.
    TextOnly(&'static str),
    /// A named tool call with fixed arguments, followed by `Finish { ToolUse }`.
    ToolCall { name: &'static str, args: String },
    /// The provider fails the request outright.
    Error(&'static str),
    /// A tool-use finish with no tool call at all: the model narrated the
    /// invocation as text and reported a tool-use stop (FR-022).
    ToolUseFinishWithoutCall(&'static str),
    /// A tool-use finish where a call opened but never ended (FR-022's
    /// boundary: a started call is a parsed call).
    ToolUseFinishWithStartedOnlyCall,
    /// A signed reasoning block followed by a tool call (FR-011): the provider
    /// streams thinking text, then its signature, then the call the thinking
    /// decided.
    SignedReasoningThenToolCall {
        text: &'static str,
        signature: &'static str,
        name: &'static str,
        args: String,
    },
}

#[derive(Clone)]
struct ScriptedProvider {
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
    call_count: Arc<AtomicU32>,
    captured: Arc<Mutex<Vec<ChatRequest>>>,
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
        self.call_count.fetch_add(1, Ordering::SeqCst);

        // Strict in-order consumption; the last entry repeats once the script
        // is exhausted.
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

        match reply {
            ScriptedReply::Error(message) => Err(anyhow::anyhow!("{message}")),
            ScriptedReply::TextOnly(text) => {
                let events: [StreamEvent; 3] = [
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
                ];
                Ok(Box::pin(stream::iter(events)))
            }
            ScriptedReply::ToolCall { name, args } => {
                let events: [StreamEvent; 5] = [
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
                ];
                Ok(Box::pin(stream::iter(events)))
            }
            ScriptedReply::ToolUseFinishWithoutCall(text) => {
                let events: [StreamEvent; 3] = [
                    StreamEvent::TextDelta {
                        text: text.to_string(),
                    },
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
            ScriptedReply::SignedReasoningThenToolCall {
                text,
                signature,
                name,
                args,
            } => {
                let events: [StreamEvent; 9] = [
                    StreamEvent::ReasoningStart,
                    StreamEvent::ReasoningDelta {
                        text: text.to_string(),
                    },
                    StreamEvent::ReasoningEnd,
                    StreamEvent::ReasoningSignature {
                        signature: signature.to_string(),
                    },
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
                ];
                Ok(Box::pin(stream::iter(events)))
            }
            ScriptedReply::ToolUseFinishWithStartedOnlyCall => {
                // A call that opens, streams half its arguments and is then cut
                // off by the provider's tool-use finish. Repeating it forever
                // would spin the tool phase, so the script closes with a plain
                // text step once the cut-off call has been dispatched.
                if self.call_count.load(Ordering::SeqCst) > 1 {
                    let events: [StreamEvent; 1] = [StreamEvent::Finish {
                        reason: LlmFinishReason::Stop,
                    }];
                    return Ok(Box::pin(stream::iter(events)));
                }
                let events: [StreamEvent; 4] = [
                    StreamEvent::ToolCallStart {
                        id: "call_cut".to_string(),
                        name: "read".to_string(),
                    },
                    StreamEvent::ToolCallDelta {
                        id: "call_cut".to_string(),
                        args_json: "{\"path\": \"no".to_string(),
                    },
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
            captured: Arc::clone(&self.captured),
        }))
    }
}

type Harness = (
    SessionProcessor,
    std::path::PathBuf,
    Arc<Mutex<Vec<ChatRequest>>>,
    tempfile::TempDir,
);

/// Build a processor whose provider replays `replies`. `max_retries` is zeroed
/// so the provider-error path fails on the first attempt without backoff sleeps.
fn make_processor(event_bus: Arc<EventBus>, replies: Vec<ScriptedReply>) -> Result<Harness> {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        replies: Arc::new(Mutex::new(replies)),
        call_count: Arc::new(AtomicU32::new(0)),
        captured: Arc::clone(&captured),
    }));

    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let config: RagentConfig = serde_json::from_str("{}").expect("valid config JSON");
    let stream_config = ragent_agent::StreamConfig {
        max_retries: 0,
        retry_backoff_secs: 0,
        ..Default::default()
    };

    let processor = SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
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
        stream_config,
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
        pending_verdicts: ragent_agent::session::pending::PendingVerdicts::new(),
    };
    let tmp = tempfile::tempdir().expect("tempdir");
    Ok((processor, tmp.path().to_path_buf(), captured, tmp))
}

fn make_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });
    agent
}

/// Drain the broadcast buffer, returning the observed event kinds as a set of
/// booleans keyed by the assertions each test cares about.
#[derive(Default)]
struct SeenEvents {
    request_started: bool,
    text_delta: bool,
    token_usage: bool,
    message_end_stop: bool,
    message_end_truncation: bool,
    tool_call_start: bool,
    tool_call_end: bool,
    agent_error: bool,
    /// One entry per `AgentNotice` message, in arrival order.
    notices: Vec<String>,
}

fn collect_events(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> SeenEvents {
    let mut seen = SeenEvents::default();
    while let Ok(event) = rx.try_recv() {
        match event {
            Event::RequestStarted { .. } => seen.request_started = true,
            Event::TextDelta { .. } => seen.text_delta = true,
            Event::TokenUsage { .. } => seen.token_usage = true,
            Event::MessageEnd {
                reason: LlmFinishReason::Stop,
                ..
            } => seen.message_end_stop = true,
            Event::MessageEnd {
                reason: LlmFinishReason::Truncation,
                ..
            } => seen.message_end_truncation = true,
            Event::ToolCallStart { .. } => seen.tool_call_start = true,
            Event::ToolCallEnd { .. } => seen.tool_call_end = true,
            Event::AgentError { .. } => seen.agent_error = true,
            Event::AgentNotice { message, .. } => seen.notices.push(message),
            _ => {}
        }
    }
    seen
}

// ---------------------------------------------------------------------------
// prepare_client + call_llm_step + final save: single text step
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_single_text_step_prepares_client_calls_llm_and_persists_message() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::TextOnly("hello world")],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "explain the loop",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("process_message should succeed");

    // `prepare_client` produced exactly one request, resolved the model, and
    // attached the turn system prompt, tool definitions, and user message.
    let requests = captured.lock().expect("captured lock").clone();
    assert_eq!(requests.len(), 1, "one LLM request for a single text step");
    let request = &requests[0];
    assert_eq!(request.model, "qwen3:latest");
    assert!(
        request.system.as_ref().is_some_and(|s| !s.is_empty()),
        "prepare_client must build a non-empty turn system prompt"
    );
    assert!(
        !request.tools.is_empty(),
        "the tool registry must be advertised to the model"
    );
    assert_eq!(request.session_id.as_deref(), Some(session.id.as_str()));
    assert!(
        request.messages.iter().any(|m| m.role == "user"),
        "the user message must be part of the turn"
    );

    // The final-save path returns the accumulated assistant message.
    assert_eq!(reply.role, ragent_agent::message::Role::Assistant);
    assert_eq!(reply.text_content(), "hello world");

    // ...and it is persisted in the session store.
    let stored = processor
        .session_manager
        .get_messages(&session.id)
        .expect("messages");
    let assistant = stored
        .iter()
        .find(|m| m.id == reply.id)
        .expect("assistant message persisted");
    assert_eq!(assistant.text_content(), "hello world");

    let seen = collect_events(&mut rx);
    assert!(seen.request_started, "RequestStarted must be published");
    assert!(seen.text_delta, "TextDelta must be published");
    assert!(seen.token_usage, "TokenUsage must be published");
    assert!(
        seen.message_end_stop,
        "a terminal MessageEnd{{Stop}} is expected"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// call_llm_step: tool call step followed by a text step
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_tool_step_then_text_step_runs_two_requests() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "think",
                args: r#"{"thought": "reason about it"}"#.to_string(),
            },
            ScriptedReply::TextOnly("finished after tool"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("multi-step run should succeed");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        2,
        "one request per step: tool call, then text"
    );
    assert_eq!(reply.text_content(), "finished after tool");

    let seen = collect_events(&mut rx);
    assert!(seen.tool_call_start, "ToolCallStart must be published");
    assert!(seen.tool_call_end, "ToolCallEnd must be published");
    assert!(
        seen.message_end_stop,
        "run must end with MessageEnd{{Stop}}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// provider-error path
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_provider_error_surfaces_and_publishes_agent_error() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::Error("provider exploded")],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let outcome = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;
    assert!(
        outcome.is_err(),
        "a provider failure must surface as an error"
    );
    let message = outcome.unwrap_err().to_string();
    assert!(
        message.contains("provider exploded"),
        "the provider error must propagate: {message}"
    );

    // The request was attempted exactly once (retries disabled).
    assert_eq!(captured.lock().expect("captured lock").len(), 1);
    assert!(
        collect_events(&mut rx).agent_error,
        "AgentError must be published on provider failure"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// FR-022: a tool-use finish with no parsed call is a truncation, and the
// text-recovery path still gets the reply
// ---------------------------------------------------------------------------

/// Every clause of FR-022 the harness can observe without a provider.
#[test]
fn test_a_tool_use_finish_without_a_call_is_a_truncation() {
    use ragent_agent::session::processor::SessionProcessor;

    let tool_use = LlmFinishReason::ToolUse;
    assert!(
        SessionProcessor::finish_was_tool_use_without_a_call(Some(&tool_use), false, 0),
        "a tool-use finish with no call at all must be treated as truncation"
    );
    // A call that opened is a parsed call even when it never ended: its
    // truncated argument deltas are reported by the dispatch pipeline.
    assert!(
        !SessionProcessor::finish_was_tool_use_without_a_call(Some(&tool_use), true, 0),
        "a started call counts as parsed"
    );
    // A complete call is the ordinary path, not a truncation.
    assert!(
        !SessionProcessor::finish_was_tool_use_without_a_call(Some(&tool_use), true, 2),
        "a complete call list is the ordinary tool phase"
    );
    // Any other finish reason is left alone.
    for reason in [
        LlmFinishReason::Stop,
        LlmFinishReason::Length,
        LlmFinishReason::ContentFilter,
        LlmFinishReason::Cancelled,
        LlmFinishReason::Truncation,
    ] {
        assert!(
            !SessionProcessor::finish_was_tool_use_without_a_call(Some(&reason), false, 0),
            "{reason:?} must not be reclassified as a lost tool call"
        );
    }
    assert!(
        !SessionProcessor::finish_was_tool_use_without_a_call(None, false, 0),
        "no finish reason leaves the existing silent-cut detection in charge"
    );
}

/// A narrated call on a tool-use finish is still recovered from the reply text,
/// and is no longer reported as a truncation once it executes.
#[tokio::test]
async fn test_a_tool_use_finish_without_a_call_recovers_the_text_call() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let narrated = "Reading the notes now.\n\
                    <toolcall>{\"name\":\"think\",\"arguments\":{\"thought\":\"recovered\"}}</toolcall>";
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolUseFinishWithoutCall(narrated),
            ScriptedReply::TextOnly("recovered call ran"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("process_message should succeed");

    // The recovered call ran, so the turn needed a second request and the
    // model's own narration was blanked out of the history rather than being
    // replayed beside the tool_use part.
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        2,
        "the recovered call must execute and produce a second step"
    );
    // The recovered call's markup is blanked from the text (the span becomes
    // spaces), while the rest of the reply keeps its bytes so the model does
    // not see its own narration duplicated beside the tool_use part.
    let text = reply.text_content();
    assert!(
        text.starts_with("Reading the notes now."),
        "the prose before the call survives: {text:?}"
    );
    assert!(
        text.ends_with("recovered call ran"),
        "the second step's text is appended after the blanked span: {text:?}"
    );
    assert!(
        !text.contains("<toolcall>") && !text.contains("\"name\""),
        "the recovered markup must be blanked out of the saved reply: {text:?}"
    );
    let second = captured.lock().expect("captured lock")[1].clone();
    assert!(
        second.messages.iter().any(|m| matches!(
            &m.content,
            ragent_agent::llm::ChatContent::Parts(parts)
                if parts.iter().any(|p| matches!(
                    p,
                    ragent_agent::llm::ContentPart::ToolUse { name, .. } if name == "think"
                ))
        )),
        "the recovered call must be in the history the model sees"
    );

    let seen = collect_events(&mut rx);
    assert!(
        seen.notices
            .iter()
            .any(|n| n.contains("Recovered 1 tool call(s) from text-format output")),
        "recovery must be announced: {:?}",
        seen.notices
    );
    // A recovered call means the turn did not lose anything, so the recorded
    // finish reason is normalised back to tool use instead of ending the run
    // as truncated.
    assert!(
        seen.message_end_stop,
        "a recovered call must end the run with a clean stop"
    );
    assert!(
        !seen.message_end_truncation,
        "a recovered call must not be reported as a truncation"
    );
    Ok(())
}

/// With no recoverable markup the turn ends on the prose reply, and it is
/// reported as truncated rather than as a clean answer.
#[tokio::test]
async fn test_a_tool_use_finish_without_a_call_and_no_markup_reports_truncation() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::ToolUseFinishWithoutCall(
            "I would read the notes, but I cannot.",
        )],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the turn must still end with the prose reply");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        1,
        "no spurious recovery prompt may be sent"
    );
    let seen = collect_events(&mut rx);
    assert!(
        seen.notices
            .iter()
            .any(|n| n.contains("reported a tool call but none could be parsed")),
        "the lost tool call must be announced: {:?}",
        seen.notices
    );
    assert!(
        seen.message_end_truncation,
        "the lost tool call must leave the run recorded as truncated"
    );
    assert!(
        !seen.message_end_stop,
        "the lost tool call must not be recorded as a clean stop"
    );
    // A step recorded as truncated never gets the "ask the model to continue"
    // kick - that path is for a provider that stopped on its own, and this one
    // already reported a tool use - so no continuation prompt is sent.
    assert!(
        !seen
            .notices
            .iter()
            .any(|n| n.contains("asking the model to continue")),
        "no spurious truncation-continuation prompt may be sent: {:?}",
        seen.notices
    );
    Ok(())
}

/// Boundary: a call that started and then lost its argument stream is a parsed
/// call, so the step is NOT reclassified as a text-recovery truncation.
#[tokio::test]
async fn test_a_started_but_unfinished_call_is_not_reclassified() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::ToolUseFinishWithStartedOnlyCall],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the dispatch pipeline reports the truncated arguments");

    // The step still went to the tool phase: exactly one request, and its
    // captured assistant history carries the started call with the arguments
    // the provider managed to stream.
    let requests = captured.lock().expect("captured lock").clone();
    assert!(
        requests.len() < 5,
        "a started call keeps the turn in the normal tool phase (one dispatch, \
         then the closing text step); a reclassification would arm text \
         recovery and loop until the step budget: {} requests",
        requests.len()
    );
    assert_eq!(
        requests.len(),
        2,
        "a started call keeps the turn in the normal tool phase (one dispatch, \
         then the closing text step); a reclassification would arm text recovery \
         and loop until the step budget"
    );
    // The provider's own call is the one that was dispatched - not a
    // recovered replacement - and its cut-off arguments are reported by the
    // normal pipeline as an invalid-argument tool result the model can correct.
    assert!(
        requests[1].messages.iter().any(|m| matches!(
            &m.content,
            ragent_agent::llm::ChatContent::Parts(parts)
                if parts.iter().any(|p| matches!(
                    p,
                    ragent_agent::llm::ContentPart::ToolResult { tool_use_id, is_error, .. }
                        if tool_use_id == "call_cut" && *is_error
                ))
        )),
        "the started call must be dispatched and its bad arguments refused: {:?}",
        requests[1].messages
    );
    let seen = collect_events(&mut rx);
    assert!(
        !seen
            .notices
            .iter()
            .any(|n| n.contains("reported a tool call but none could be parsed")),
        "a started call must not be reclassified as a lost call: {:?}",
        seen.notices
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// FR-011: the signed reasoning block is echoed within the turn (T-018)
// ---------------------------------------------------------------------------

/// Helper: read the `ContentPart`s of the assistant turn in `request` that
/// carries `call_1`, in the order the encoder will see them.
fn assistant_parts_of(request: &ChatRequest) -> Vec<&ragent_agent::llm::ContentPart> {
    request
        .messages
        .iter()
        .filter(|m| m.role == "assistant")
        .find_map(|m| match &m.content {
            ragent_agent::llm::ChatContent::Parts(parts)
                if parts
                    .iter()
                    .any(|p| matches!(p, ragent_agent::llm::ContentPart::ToolUse { .. })) =>
            {
                Some(parts.iter().collect())
            }
            _ => None,
        })
        .expect("an assistant turn carrying a tool call")
}

/// FR-011: the turn that answers a tool call must send its own signed thinking
/// block back *in the same turn*.
///
/// This is the half a history converter cannot provide. `chat_messages` is what
/// the next request is built from, so a block recorded only in the saved
/// `MessagePart`s would first reach the provider on the following turn - one
/// request too late for the provider that requires it.
#[tokio::test]
async fn test_the_echoed_reasoning_block_precedes_the_tool_call_in_the_same_turn() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus,
        vec![
            ScriptedReply::SignedReasoningThenToolCall {
                text: "weigh the options",
                signature: "EqQBCkYIBxgCKkA",
                name: "think",
                args: r#"{"thought": "reason about it"}"#.to_string(),
            },
            ScriptedReply::TextOnly("finished after tool"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the run should succeed");

    let requests = captured.lock().expect("captured lock").clone();
    assert_eq!(requests.len(), 2, "one request per step");
    let parts = assistant_parts_of(&requests[1]);
    let reasoning_at = parts
        .iter()
        .position(|p| matches!(p, ragent_agent::llm::ContentPart::Reasoning { .. }))
        .expect("the echoing turn must carry the reasoning block (FR-011)");
    let call_at = parts
        .iter()
        .position(|p| matches!(p, ragent_agent::llm::ContentPart::ToolUse { .. }))
        .expect("the turn carries its tool call");
    assert!(
        reasoning_at < call_at,
        "the thinking that decided a call must arrive before the call: {parts:?}"
    );
    // Both halves of the block survive: the text and the provider's signature
    // over it. An echoed block with one half missing is rejected or
    // unverifiable rather than merely unsent.
    match parts[reasoning_at] {
        ragent_agent::llm::ContentPart::Reasoning { text, signature } => {
            assert_eq!(text, "weigh the options");
            assert_eq!(signature.as_deref(), Some("EqQBCkYIBxgCKkA"));
        }
        other => panic!("expected a reasoning part, got {other:?}"),
    }
    Ok(())
}

/// FR-011: the signature also survives into the persisted assistant message, so
/// a resumed session (or a later turn's rebuilt history) replays a signed block
/// rather than an unsigned paraphrase of the thinking.
#[tokio::test]
async fn test_the_persisted_reasoning_part_keeps_its_signature() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, _captured, _dir) = make_processor(
        event_bus,
        vec![
            ScriptedReply::SignedReasoningThenToolCall {
                text: "weigh the options",
                signature: "EqQBCkYIBxgCKkA",
                name: "think",
                args: r#"{"thought": "reason about it"}"#.to_string(),
            },
            ScriptedReply::TextOnly("finished after tool"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the run should succeed");

    let stored = processor
        .session_manager
        .get_messages(&session.id)
        .expect("messages");
    let assistant = stored
        .iter()
        .find(|m| m.id == reply.id)
        .expect("the final assistant message is persisted");
    let reasoning = assistant
        .parts
        .iter()
        .find_map(|p| match p {
            ragent_agent::message::MessagePart::Reasoning { text, signature } => {
                Some((text.clone(), signature.clone()))
            }
            _ => None,
        })
        .expect("the reasoning block is part of the saved message");
    assert_eq!(reasoning.0, "weigh the options");
    assert_eq!(
        reasoning.1.as_deref(),
        Some("EqQBCkYIBxgCKkA"),
        "a saved block without its signature cannot be echoed verifiably"
    );

    // The saved parts keep the echo's order (reasoning before the call), so a
    // resumed session rebuilds the same assistant turn the provider accepted.
    // The message accumulates across steps, so the closing step's answer text
    // follows the call; the leading pair is what this case is about.
    let kind = |p: &ragent_agent::message::MessagePart| match p {
        ragent_agent::message::MessagePart::Reasoning { .. } => "reasoning",
        ragent_agent::message::MessagePart::ToolCall { .. } => "tool_call",
        ragent_agent::message::MessagePart::Text { .. } => "text",
        ragent_agent::message::MessagePart::Image(_) => "image",
    };
    let order: Vec<&str> = assistant.parts.iter().map(kind).collect();
    assert_eq!(
        order.first_chunk::<2>(),
        Some(&["reasoning", "tool_call"]),
        "the reasoning block precedes the call it decided: {order:?}"
    );
    Ok(())
}
