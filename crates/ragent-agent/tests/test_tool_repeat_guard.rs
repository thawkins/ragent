//! Integration tests for the tool-repeat guard (FR-044):
//!
//! - five consecutive identical tool calls pass through untouched;
//! - the sixth identical call raises a `PermissionRequested` prompt in an
//!   interactive primary run; an "allow" reply lets the run continue and
//!   resets the tracker;
//! - a "deny" reply surfaces a corrective observation to the model;
//! - an unanswered prompt (timeout) counts as denial;
//! - a different-args call in between resets the consecutive counter;
//! - subagent and auto-approve (`--yes`) runs fail closed: the sixth call is
//!   denied with a corrective observation and no prompt is raised.
//!
//! The harness mirrors `test_subagent_interactive_block.rs`: a scripted
//! provider replays tool calls, and `PermissionReplied` events are answered
//! by a spawned responder task.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, AgentMode, ModelRef};
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
    /// A named tool call with fixed arguments, followed by `Finish
    /// { ToolUse }`.
    ToolCall { name: &'static str, args: String },
    /// Text-only response ending with `Finish { Stop }`.
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
        self.call_count.fetch_add(1, Ordering::SeqCst);
        // Strict in-order consumption: each call serves the next scripted
        // reply; once the script is exhausted the final entry repeats.
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
            ScriptedReply::ToolCall { name, args } => vec![
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
            captured: Arc::clone(&self.shared_captured),
        }))
    }
}

type MakeProcessorResult = (
    SessionProcessor,
    std::path::PathBuf,
    Arc<Mutex<Vec<ChatRequest>>>,
    tempfile::TempDir,
);

/// Build a processor whose provider replays `replies`. The `think` tool needs
/// no permission prompt, so every scripted call passes the permission layer.
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
    let config: RagentConfig = serde_json::from_str("{}").expect("valid config JSON");
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

/// A scripted `think` call with constant arguments: five in a row stay under
/// the repeat limit; the sixth crosses it.
fn think_call() -> ScriptedReply {
    ScriptedReply::ToolCall {
        name: "think",
        args: r#"{"thought": "same step"}"#.to_string(),
    }
}

fn think_call_variant() -> ScriptedReply {
    ScriptedReply::ToolCall {
        name: "think",
        args: r#"{"thought": "a different step"}"#.to_string(),
    }
}

fn tool_result_texts(reply: &ragent_agent::message::Message) -> Vec<String> {
    reply
        .parts
        .iter()
        .filter_map(|part| match part {
            ragent_agent::message::MessagePart::ToolCall { state, .. } => state.error.clone(),
            _ => None,
        })
        .collect()
}

/// Count `PermissionRequested` events sitting in the broadcast buffer.
fn count_permission_prompts(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> usize {
    let mut count = 0;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, Event::PermissionRequested { .. }) {
            count += 1;
        }
    }
    count
}

/// Drain the buffer and report whether any `tool:repeat` prompt appeared.
fn saw_repeat_prompt(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> bool {
    let mut saw = false;
    while let Ok(event) = rx.try_recv() {
        if matches!(
            event,
            Event::PermissionRequested { permission, .. } if permission == "tool:repeat"
        ) {
            saw = true;
        }
    }
    saw
}

/// Five identical calls execute fully (no prompt, no denial): the run makes
/// one request per call and every observation is a successful result, and no
/// `PermissionRequested` event is ever published.
#[tokio::test]
async fn test_five_identical_calls_pass_without_prompt() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let repeats = std::iter::repeat_n(think_call(), 5).collect::<Vec<_>>();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        [repeats, vec![ScriptedReply::TextOnly("done")]].concat(),
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
        .expect("process_message should succeed");

    let request_count = captured.lock().expect("captured lock").len();
    assert_eq!(
        request_count, 6,
        "five tool calls + one text-only completion expected"
    );

    let prompt_count = count_permission_prompts(&mut rx);
    assert_eq!(prompt_count, 0, "no permission prompt under the limit");
    Ok(())
}

/// The sixth identical call raises a `tool:repeat` permission prompt; an
/// "allow" reply resets the tracker and the run continues to completion.
#[tokio::test]
async fn test_sixth_identical_call_prompts_and_allow_continues() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let repeats = std::iter::repeat_n(think_call(), 6).collect::<Vec<_>>();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        [repeats, vec![ScriptedReply::TextOnly("done")]].concat(),
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    // Answer the repeat prompt as soon as it is published.
    let answer_bus = event_bus.clone();
    let responder = tokio::spawn(async move {
        let mut rx = answer_bus.subscribe();
        loop {
            match rx.recv().await {
                Ok(Event::PermissionRequested {
                    session_id,
                    request_id,
                    permission,
                    ..
                }) if permission == "tool:repeat" => {
                    answer_bus.publish(Event::PermissionReplied {
                        session_id,
                        request_id,
                        allowed: true,
                        decision: ragent_agent::permission::PermissionDecision::Once,
                    });
                }
                Ok(_) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");
    responder.abort();

    // 6 tool-call requests + 1 text-only completion. The 6th call was
    // allowed through, so the model saw a normal result and continued.
    let request_count = captured.lock().expect("captured lock").len();
    assert_eq!(
        request_count, 7,
        "all six calls executed and the run completed"
    );

    let saw = saw_repeat_prompt(&mut rx);
    assert!(saw, "a tool:repeat prompt must be raised");
    Ok(())
}

/// A different-args call resets the consecutive counter: three identical,
/// one different, then three more identical - still no prompt.
#[tokio::test]
async fn test_different_args_reset_counter() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            think_call(),
            think_call(),
            think_call(),
            think_call_variant(),
            think_call(),
            think_call(),
            think_call(),
            ScriptedReply::TextOnly("done"),
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
        .expect("process_message should succeed");

    let request_count = captured.lock().expect("captured lock").len();
    assert_eq!(
        request_count, 8,
        "script is consumed in order and the final text-only reply repeats"
    );

    let prompt_count = count_permission_prompts(&mut rx);
    assert_eq!(prompt_count, 0, "a different call resets the counter");
    Ok(())
}

/// A user denial on the sixth identical call surfaces a corrective
/// observation to the model; the run continues and completes.
#[tokio::test]
async fn test_user_deny_surfaces_corrective_observation() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let repeats = std::iter::repeat_n(think_call(), 6).collect::<Vec<_>>();
    let (processor, working_dir, _captured, _dir) = make_processor(
        event_bus.clone(),
        [repeats, vec![ScriptedReply::TextOnly("done")]].concat(),
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let answer_bus = event_bus.clone();
    let responder = tokio::spawn(async move {
        let mut rx = answer_bus.subscribe();
        loop {
            match rx.recv().await {
                Ok(Event::PermissionRequested {
                    session_id,
                    request_id,
                    permission,
                    ..
                }) if permission == "tool:repeat" => {
                    answer_bus.publish(Event::PermissionReplied {
                        session_id,
                        request_id,
                        allowed: false,
                        decision: ragent_agent::permission::PermissionDecision::Once,
                    });
                }
                Ok(_) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });

    let reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");
    responder.abort();

    let texts = tool_result_texts(&reply);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("identical arguments") && t.contains("user declined")),
        "expected a corrective denial observation, got: {texts:?}"
    );
    Ok(())
}

/// An unanswered repeat prompt (timeout) counts as denial and the
/// observation explains the timeout.
#[tokio::test]
async fn test_unanswered_prompt_times_out_as_denial() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let repeats = std::iter::repeat_n(think_call(), 6).collect::<Vec<_>>();
    let (processor, working_dir, _captured, _dir) = make_processor(
        event_bus,
        [repeats, vec![ScriptedReply::TextOnly("done")]].concat(),
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    // Shrink the guard timeout from the 120 s default so the denial
    // observation arrives within the test budget. The guard reads the
    // config's `loop.checkpoint_timeout_secs` value, so a two-second loop
    // config (injected into the config cache) drives the prompt deadline.

    // Rebuild with a two-second timeout: overwrite the cached config so the
    // guard's `checkpoint_timeout_secs` lookup resolves to 2 s instead of
    // the 120 s default.
    let timeout_config: RagentConfig =
        serde_json::from_str(r#"{"loop":{"checkpoint_timeout_secs":2}}"#)
            .expect("valid timeout config JSON");
    *processor.cached_config.lock() = Some(CachedConfig {
        config: Arc::new(timeout_config),
        file_mtimes: Vec::new(),
        env_overrides_present: false,
    });

    let reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");

    let texts = tool_result_texts(&reply);
    assert!(
        texts.iter().any(|t| t.contains("identical arguments")),
        "expected a timeout-denial observation, got: {texts:?}"
    );
    Ok(())
}

/// A subagent run never prompts: the sixth identical call is denied with a
/// corrective observation and the loop continues.
#[tokio::test]
async fn test_subagent_run_denies_without_prompting() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let repeats = std::iter::repeat_n(think_call(), 6).collect::<Vec<_>>();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        [repeats, vec![ScriptedReply::TextOnly("done")]].concat(),
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut agent = make_agent();
    agent.mode = AgentMode::Subagent;

    let reply = processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should finalise cleanly");

    let request_count = captured.lock().expect("captured lock").len();
    assert!(
        request_count >= 2,
        "denial observation appended and the loop continued, got {request_count} requests"
    );

    let texts = tool_result_texts(&reply);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("identical arguments") && t.contains("auto-denied")),
        "expected a subagent auto-denial observation, got: {texts:?}"
    );

    let prompt_count = count_permission_prompts(&mut rx);
    assert_eq!(prompt_count, 0, "subagent runs must not prompt");
    Ok(())
}

/// Auto-approve (`--yes`) runs also fail closed: no prompt, corrective
/// observation instead.
#[tokio::test]
async fn test_auto_approve_run_denies_without_prompting() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let repeats = std::iter::repeat_n(think_call(), 6).collect::<Vec<_>>();
    let (mut processor, working_dir, _captured, _dir) = make_processor(
        event_bus.clone(),
        [repeats, vec![ScriptedReply::TextOnly("done")]].concat(),
    )?;
    processor.auto_approve = true;
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
        .expect("process_message should finalise cleanly");

    let texts = tool_result_texts(&reply);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("identical arguments") && t.contains("auto-denied")),
        "expected an auto-approve denial observation, got: {texts:?}"
    );

    let prompt_count = count_permission_prompts(&mut rx);
    assert_eq!(prompt_count, 0, "auto-approve runs must not prompt");
    Ok(())
}
