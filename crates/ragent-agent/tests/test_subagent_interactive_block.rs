//! Interactive-tool block in subagent runs (subagent reliability fix):
//!
//! - an `ask_user` call in a Subagent-mode run is denied with a corrective
//!   observation (never executed) and the loop continues,
//! - interactive tools are removed from the wire tool definitions offered to
//!   a Subagent-mode run,
//! - an `ask_user` call in a Primary (interactive) run still executes and
//!   returns the user's reply.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::AgentInfo;
use ragent_agent::agent::AgentMode;
use ragent_agent::agent::ModelRef;
use ragent_agent::event::Event;
use ragent_agent::event::EventBus;
use ragent_agent::llm::ChatRequest;
use ragent_agent::llm::LlmClient;
use ragent_agent::llm::LlmFinishReason;
use ragent_agent::llm::StreamEvent;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::ModelInfo;
use ragent_agent::provider::Provider;
use ragent_agent::provider::ProviderRegistry;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::Capabilities;
use ragent_config::Cost;
use ragent_types::ThinkingConfig;
use ragent_types::ThinkingLevel;

/// The mock returns one of these per `chat` call; the last entry repeats.
#[derive(Clone, Debug)]
enum ScriptedReply {
    /// A named tool call followed by `Finish { ToolUse }`.
    ToolCallNamed { name: &'static str, args: String },
    /// Text-only response ending with `Finish { Stop }`.
    TextOnly(&'static str),
    /// An `ask_user` call with a valid question payload; the responder task
    /// answers it like the TUI would.
    AskUserCall { reply: &'static str },
}

#[derive(Clone)]
struct ScriptedProvider {
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
    call_count: Arc<AtomicU32>,
    shared_captured: Arc<Mutex<Vec<ChatRequest>>>,
    event_bus: Arc<EventBus>,
}

struct ScriptedClient {
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
    call_count: Arc<AtomicU32>,
    captured: Arc<Mutex<Vec<ChatRequest>>>,
    event_bus: Arc<EventBus>,
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
        // For the ask_user scenario: publish the matching QuestionAnswered
        // event right before the stream begins so the tool's unbounded wait
        // resolves without a live TUI.
        if let ScriptedReply::AskUserCall { reply } = &reply {
            self.event_bus.publish(Event::QuestionAnswered {
                session_id: "*".to_string(),
                request_id: "*".to_string(),
                response: (*reply).to_string(),
            });
        }
        let events: Vec<StreamEvent> = match &reply {
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
            ScriptedReply::ToolCallNamed { name, args } => {
                vec![
                    StreamEvent::ToolCallStart {
                        id: "call_1".to_string(),
                        name: (*name).to_string(),
                    },
                    StreamEvent::ToolCallDelta {
                        id: "call_1".to_string(),
                        args_json: args.clone(),
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
                ]
            }
            ScriptedReply::AskUserCall { reply } => {
                vec![
                    StreamEvent::ToolCallStart {
                        id: "call_1".to_string(),
                        name: "ask_user".to_string(),
                    },
                    // Valid JSON question payload so the tool actually
                    // executes and publishes QuestionRequested; the
                    // responder task then answers like the TUI would.
                    StreamEvent::ToolCallDelta {
                        id: "call_1".to_string(),
                        args_json: format!(r#"{{"question": "{reply}"}}"#),
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
                ]
            }
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
            event_bus: Arc::clone(&self.event_bus),
        }))
    }
}

use std::collections::HashMap;

type MakeProcessorResult = (
    SessionProcessor,
    std::path::PathBuf,
    Arc<Mutex<Vec<ChatRequest>>>,
);

/// Build a processor whose provider replays `replies` against `event_bus`.
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
        event_bus: event_bus.clone(),
    }));

    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
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
        cached_config: parking_lot::Mutex::new(None),
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
        active_loop_specs: tokio::sync::RwLock::new(HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    };
    let tmp = tempfile::tempdir().expect("tempdir");
    Ok((processor, tmp.path().to_path_buf(), captured))
}

fn make_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });
    agent
}

/// Collect the denial/error text of every tool-call part in the saved reply.
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

/// A subagent run whose model calls `ask_user` gets a denial observation and
/// the loop continues to completion - the run never blocks on a user that is
/// not attached to the session.
#[tokio::test]
async fn test_ask_user_denied_in_subagent_run() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let (processor, working_dir, captured) = make_processor(
        event_bus,
        vec![
            ScriptedReply::ToolCallNamed {
                name: "ask_user",
                args: r#"{"question": "which path?"}"#.to_string(),
            },
            ScriptedReply::TextOnly("decided on my own"),
        ],
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
            .any(|t| t.contains("interactive tool") && t.contains("not available in subagent runs")),
        "expected an interactive-tool denial observation, got: {texts:?}"
    );
    Ok(())
}

/// A Primary (interactive) run can still call `ask_user` - the block only
/// applies to subagent runs. A spawned responder answers the question the
/// way the TUI would.
#[tokio::test]
async fn test_ask_user_still_works_in_primary_run() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let (processor, working_dir, _captured) = make_processor(
        event_bus,
        vec![
            ScriptedReply::AskUserCall {
                reply: "user says go ahead",
            },
            ScriptedReply::TextOnly("thanks"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    // Answer the next QuestionRequested for this session like the TUI does.
    // Subscribe BEFORE the run starts: the test runtime is single-threaded,
    // so a receiver created inside the spawned task would only come into
    // existence after the tool has already published its question (and
    // broadcast receivers only see events published after subscribing).
    let responder = {
        let bus = processor.event_bus.clone();
        let session_id = session.id.clone();
        let mut rx = bus.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(Event::QuestionRequested {
                        session_id: qsid,
                        request_id,
                        ..
                    }) if qsid == session_id => {
                        bus.publish(Event::QuestionAnswered {
                            session_id: session_id.clone(),
                            request_id,
                            response: "user says go ahead".to_string(),
                        });
                        break;
                    }
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        })
    };

    let reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("process_message should finalise cleanly");
    // Bounded wait: the responder answers the (single) question and exits;
    // a timeout here only guards against a regression that would otherwise
    // hang the whole suite.
    let _ = tokio::time::timeout(std::time::Duration::from_secs(5), responder).await;

    let texts: Vec<String> = reply
        .parts
        .iter()
        .filter_map(|part| match part {
            ragent_agent::message::MessagePart::ToolCall { state, .. } => state
                .output
                .as_ref()
                .and_then(|o| o.get("content"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string),
            _ => None,
        })
        .collect();
    assert!(
        texts.iter().any(|t| t.contains("user says go ahead")),
        "the primary run's ask_user call should return the user reply, got: {texts:?}"
    );
    Ok(())
}

/// Interactive tools are not offered on the wire to a subagent run.
#[tokio::test]
async fn test_interactive_tools_absent_from_subagent_tool_surface() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let (processor, working_dir, captured) =
        make_processor(event_bus, vec![ScriptedReply::TextOnly("ack")])?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut agent = make_agent();
    agent.mode = AgentMode::Subagent;
    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should finalise cleanly");

    let requests = captured.lock().expect("captured lock");
    let first = requests.first().expect("at least one request");
    let names: Vec<&str> = first.tools.iter().map(|tool| tool.name.as_str()).collect();
    assert!(
        !names.contains(&"ask_user"),
        "interactive tools must be absent from the subagent tool surface, got: {names:?}"
    );
    assert!(
        names.contains(&"read"),
        "non-interactive tools stay available, got: {names:?}"
    );
    Ok(())
}
