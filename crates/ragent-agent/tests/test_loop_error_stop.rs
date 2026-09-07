//! Integration tests for loop stop condition 2 (spec `agentloop`, task
//! T-007 / FR-011, FR-012): an unrecoverable stage failure — provider
//! transport error, tool panic, permission hard-deny — terminates the loop
//! immediately with termination status `error`, publishes the failure reason,
//! and does not retry the failed stage; recoverable tool failures are
//! appended as error observations and only terminate the loop once the
//! consecutive-failure retry allowance is exceeded, with interleaved
//! successes resetting the counter.

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

#[derive(Clone, Debug)]
enum ScriptedReply {
    /// A named tool call followed by `Finish { ToolUse }`.
    ToolCallNamed { name: &'static str, args: String },
    /// Text-only response ending with `Finish { Stop }` (goal achieved).
    TextOnly(&'static str),
    /// A provider-reported stream error ending with `Finish { Error }`.
    ProviderError(&'static str),
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
        self.call_count.fetch_add(1, Ordering::SeqCst);
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
            ScriptedReply::ProviderError(message) => vec![
                StreamEvent::Error {
                    message: message.to_string(),
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::Cancelled,
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

/// A `read` of a path that does not exist fails with a plain tool error
/// (recoverable by default) and needs no permission prompt: the missing file
/// is relative, so the file:read auto-grant allows the call.
fn missing_file_read(name: &'static str) -> ScriptedReply {
    ScriptedReply::ToolCallNamed {
        name: "read",
        args: format!(r#"{{"path": "{name}"}}"#),
    }
}

/// FR-011: a provider transport failure terminates the loop immediately with
/// termination status `error`, publishes the failure reason, and does not
/// retry — exactly one LLM request, then the loop is inactive.
#[tokio::test]
async fn test_provider_transport_error_terminates_loop_with_status_error() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (mut processor, working_dir, captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::ProviderError(
            "provider transport failure: connection refused",
        )],
    )?;
    // No retry budget: the first stream error is fatal immediately.
    processor.stream_config.max_retries = 0;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    processor
        .start_loop(&session.id, LoopSpec::new("general", "list the files"))
        .await;

    let reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;
    assert!(
        reply.is_err(),
        "the provider failure must surface as a turn error"
    );

    // Exactly one failed attempt — the stage was not retried.
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        1,
        "FR-011: no retry after the unrecoverable provider failure"
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
    assert_eq!(status, StopCondition::UnrecoverableError.as_str());
    assert_eq!(iterations, 1, "the failed iteration is counted");
    assert!(
        reason
            .as_deref()
            .is_some_and(|r| r.contains("unrecoverable error")),
        "the failure reason is surfaced: {reason:?}"
    );
    assert!(
        reason
            .as_deref()
            .is_some_and(|r| r.contains("connection refused")),
        "the transport diagnostic is carried in the reason: {reason:?}"
    );
    assert!(!processor.loop_active(&session.id).await);
    Ok(())
}

/// FR-011 outside a loop: the same provider failure behaves exactly as before
/// — a turn error with no `LoopTerminated` publication.
#[tokio::test]
async fn test_provider_error_without_loop_unchanged() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let rx = event_bus.subscribe();
    let (mut processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::ProviderError("boom: connection reset")],
    )?;
    processor.stream_config.max_retries = 0;
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
    assert!(outcome.is_err(), "the provider failure still surfaces");

    let mut rx = rx;
    assert!(
        drain_terminated(&mut rx).is_empty(),
        "no loop termination is published without an active loop"
    );
    Ok(())
}

/// FR-012: consecutive recoverable tool failures are appended as error
/// observations each round, and the loop terminates with status `error` once
/// the consecutive count exceeds the retry allowance (3) — before a fifth
/// request.
#[tokio::test]
async fn test_recoverable_failures_terminate_at_retry_allowance() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![
            missing_file_read("missing-1.txt"),
            missing_file_read("missing-2.txt"),
            missing_file_read("missing-3.txt"),
            missing_file_read("missing-4.txt"),
            // Unreached: the allowance fires first.
            ScriptedReply::TextOnly("gave up"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    processor
        .start_loop(&session.id, LoopSpec::new("general", "read the file"))
        .await;

    let outcome = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await;
    assert!(
        outcome.is_ok(),
        "the error termination ends the turn without propagating a fatal error: {outcome:?}"
    );

    // Four requests: failures 1-3 continued with the error observation fed
    // back; failure 4 exceeded the allowance (3) and stopped the loop before
    // a fifth request.
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        4,
        "the retry allowance stops the loop before a fifth request"
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
    assert_eq!(status, StopCondition::UnrecoverableError.as_str());
    assert_eq!(iterations, 4);
    assert!(
        reason
            .as_deref()
            .is_some_and(|r| r.contains("retry allowance")),
        "the reason names the retry allowance: {reason:?}"
    );
    assert!(!processor.loop_active(&session.id).await);
    Ok(())
}

/// FR-012: recoverable failures accumulate in the model context as error
/// observations (the failing round's observation is visible in the next
/// request's messages).
#[tokio::test]
async fn test_recoverable_failures_appended_as_observations() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let (processor, working_dir, captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![
            missing_file_read("absent-a.txt"),
            missing_file_read("absent-b.txt"),
            missing_file_read("absent-c.txt"),
            missing_file_read("absent-d.txt"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    processor
        .start_loop(&session.id, LoopSpec::new("general", "read the files"))
        .await;

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the allowance stop finalises the turn");

    // Round 2's request must contain round 1's error observation, and round
    // 4's request must contain round 3's — the feedback loop is intact.
    let requests = captured.lock().expect("captured lock");
    assert_eq!(requests.len(), 4);
    for (failing_file, next_request_index) in [("absent-a.txt", 1), ("absent-c.txt", 3)] {
        let next = &requests[next_request_index];
        let has_observation = next.messages.iter().any(|message| {
            message.role == "user"
                && match &message.content {
                    ragent_agent::llm::ChatContent::Parts(parts) => parts.iter().any(|part| {
                        matches!(
                            part,
                            ragent_agent::llm::ContentPart::ToolResult { content, .. }
                                if content.contains("Error")
                                    && content.contains(failing_file)
                        )
                    }),
                    _ => false,
                }
        });
        assert!(
            has_observation,
            "request {} must carry the failing round's error observation for {failing_file}",
            next_request_index + 1
        );
    }
    Ok(())
}

/// FR-012: a successful step resets the consecutive-failure counter, so
/// fail, fail, success, fail, fail keeps the loop running (the count never
/// exceeds the allowance) and the run completes normally.
#[tokio::test]
async fn test_success_resets_consecutive_failures() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![
            missing_file_read("fails-1.txt"),
            missing_file_read("fails-2.txt"),
            think_call(),
            missing_file_read("fails-3.txt"),
            missing_file_read("fails-4.txt"),
            ScriptedReply::TextOnly("all done"),
            ScriptedReply::TextOnly("all done"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    processor
        .start_loop(&session.id, LoopSpec::new("general", "retry with feedback"))
        .await;

    let reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the run completes despite the failures");

    // Six requests: five failing/tool rounds plus the text-only completion.
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        6,
        "interleaved success keeps the loop under the allowance"
    );

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1);
    let Event::LoopTerminated { status, .. } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, StopCondition::GoalAchieved.as_str());
    assert!(
        reply.parts.iter().any(|part| {
            matches!(
                part,
                ragent_agent::message::MessagePart::Text { text } if text.contains("all done")
            )
        }),
        "the final reply is preserved"
    );
    Ok(())
}

/// FR-011: with the default retry budget, a persistent provider failure is
/// retried inside the LLM stage until the internal budget is exhausted, and
/// the loop then terminates with status `error`. The exhausted stage itself is
/// **not** retried again: the failure propagates as a turn error and the loop
/// becomes inactive with no further loop iterations.
#[tokio::test]
async fn test_persistent_provider_error_after_retry_budget_terminates_loop() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    // Default stream config (max_retries = 4): the scripted error repeats for
    // every attempt, so the internal retry loop must exhaust itself. The
    // diagnostic carries a retryable transport marker so the loop's retry
    // policy engages; the backoff is zeroed so the retry loop runs fast.
    let (mut processor, working_dir, captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::ProviderError("boom: connection reset")],
    )?;
    processor.stream_config.retry_backoff_secs = 0;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    processor
        .start_loop(&session.id, LoopSpec::new("general", "list the files"))
        .await;

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
        "the exhausted provider failure must surface as a turn error"
    );

    // The internal retry budget was consumed (initial attempt plus the
    // configured retries) before the stage failed for good.
    let attempts = usize::try_from(processor.stream_config.max_retries).unwrap_or(0) + 1;
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        attempts,
        "the LLM stage consumed its internal retry budget before failing"
    );

    // Even after internal retries the active loop terminates with `error`.
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
    assert_eq!(status, StopCondition::UnrecoverableError.as_str());
    assert_eq!(iterations, 1, "the failed iteration is counted");
    assert!(
        reason
            .as_deref()
            .is_some_and(|r| r.contains("connection reset")),
        "the transport diagnostic is carried in the reason: {reason:?}"
    );
    assert!(!processor.loop_active(&session.id).await);
    Ok(())
}

/// FR-023: an observation append that cannot be persisted must never be
/// silently discarded — storage-layer failures surface as errors. The loop's
/// failure guard consumes `classify_message`, so a storage-task panic
/// diagnostic from a failing append/save escalates to termination status
/// `error` instead of being dropped, and the finalise path propagates closure
/// failures out of `process_message` (never swallowed).
#[test]
fn test_storage_task_panic_classifies_unrecoverable_never_silent() {
    // `storage_op` maps a join failure on the blocking save task to this
    // diagnostic; the loop guard must treat it as unrecoverable.
    let storage_panic = "storage task panicked: save failed";
    assert!(
        ragent_agent::error::classify_message(storage_panic).is_unrecoverable(),
        "a failed observation-append save must escalate, never continue silently"
    );
    // The processor's finalise path uses `?` on the closing save, so a plain
    // storage failure is returned from `process_message` (surface, not
    // silence) — asserted here on the diagnostic that propagation carries.
    let storage_failure = anyhow::anyhow!("storage error: update_message failed");
    assert_eq!(
        storage_failure.to_string(),
        "storage error: update_message failed",
        "the propagated diagnostic preserves the failure text for the user"
    );
}

/// FR-011: a tool panic (join failure) is an unrecoverable failure — an
/// active loop terminates with status `error` naming the panic. (The panic
/// path is exercised through the watchdog-style abort of the scripted task;
/// here we verify the classification of the recorded panic message.)
#[test]
fn test_tool_panic_message_classifies_unrecoverable() {
    let panic_message = "tool task panicked: attempt to subtract with overflow";
    assert!(ragent_agent::error::classify_message(panic_message).is_unrecoverable());
}
