//! Integration coverage for `SessionProcessor::process_message` /
//! `process_user_message` and the extracted loop steps in
//! `session::loop_steps` (coverage campaign, processor.rs + loop_steps.rs).
//!
//! This file copies the scripted-provider harness pattern from
//! `test_loop_steps_harness.rs` (an external `tests/` crate cannot call the
//! `pub(crate)` loop-step methods, so every path is driven through the public
//! `process_message` / `process_user_message` entry points) and widens it into
//! a configurable harness so it can exercise more branches than the baseline
//! and `test_loop_steps_extra.rs` files:
//!
//! - dozens of real core tools across a single turn (`think`, `calculator`,
//!   `get_env`, `ragent_info`, `os_info`, `file_info`, `Write`, `Edit`,
//!   `multiedit`, `append_to_file`, `copy_file`, `move_file`, `diff`, `read`,
//!   `rm`, `Grep`, `list`, `Glob`);
//! - a multipart user message carrying an image part (async history path);
//! - malformed tool-call JSON (corrective error observation);
//! - two tool calls in one streamed step;
//! - retryable stream error -> retry with a cleared buffer; a mid-stream
//!   error with meaningful partial output kept without retrying; a permanent
//!   provider error surfaced fatally; a silent `Finish { Stop }` with no text;
//! - the pre-set cancel flag; the tool-repeat guard (auto-approve denial);
//!   permission-denied write recovery;
//! - `EnterPlanMode` (agent switch), `ExitPlanMode` (agent restore),
//!   `agent_complete` - each breaking the loop;
//! - `AskUserQuestion` answered by a spawned responder task;
//! - text-format tool-call recovery from bare JSON;
//! - goal-loop lifecycle: budget exhaustion, goal achieved, a passing and a
//!   failing verification gate;
//! - sub-agent paths: interactive-tool denial, the premature-termination
//!   narration nudge, and the silent-truncation continuation;
//! - a missing-model provider-stage failure.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, AgentMode, ModelRef};
use ragent_agent::event::{Event, EventBus};
use ragent_agent::llm::{
    ChatContent, ChatRequest, ContentPart, LlmClient, LlmFinishReason, StreamEvent,
};
use ragent_agent::message::{ImageData, Message, MessagePart, Role};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::loop_state::LoopSpec;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::permission::{Permission, PermissionAction, PermissionRule};
use ragent_config::{Capabilities, Config as RagentConfig, Cost, StreamConfig};

// ---------------------------------------------------------------------------
// Scripted provider / client
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum ScriptedReply {
    /// Text-only response ending with `Finish { Stop }`.
    Text(String),
    /// One named tool call followed by `Finish { ToolUse }`.
    ToolCall { name: String, args: String },
    /// Several tool calls in a single streamed step, then `Finish { ToolUse }`.
    Tools(Vec<(String, String)>),
    /// An arbitrary scripted event sequence.
    Events(Vec<StreamEvent>),
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

fn tool_call_events(id: &str, name: &str, args: &str) -> [StreamEvent; 3] {
    [
        StreamEvent::ToolCallStart {
            id: id.to_string(),
            name: name.to_string(),
        },
        StreamEvent::ToolCallDelta {
            id: id.to_string(),
            args_json: args.to_string(),
        },
        StreamEvent::ToolCallEnd { id: id.to_string() },
    ]
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

        // Strict in-order consumption; the last entry repeats once exhausted.
        let reply = {
            let mut replies = self.replies.lock().expect("replies lock");
            if replies.len() > 1 {
                replies.remove(0)
            } else {
                replies
                    .last()
                    .cloned()
                    .unwrap_or(ScriptedReply::Text("done".to_string()))
            }
        };

        let events: Vec<StreamEvent> = match reply {
            ScriptedReply::Text(text) => vec![
                StreamEvent::TextDelta { text },
                StreamEvent::Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                },
                StreamEvent::Finish {
                    reason: LlmFinishReason::Stop,
                },
            ],
            ScriptedReply::ToolCall { name, args } => {
                let mut events: Vec<StreamEvent> = tool_call_events("call_1", &name, &args)
                    .into_iter()
                    .collect();
                events.push(StreamEvent::Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                });
                events.push(StreamEvent::Finish {
                    reason: LlmFinishReason::ToolUse,
                });
                events
            }
            ScriptedReply::Tools(calls) => {
                let mut events: Vec<StreamEvent> = Vec::new();
                for (i, (name, args)) in calls.iter().enumerate() {
                    let id = format!("call_{i}");
                    events.extend(tool_call_events(&id, name, args));
                }
                events.push(StreamEvent::Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                });
                events.push(StreamEvent::Finish {
                    reason: LlmFinishReason::ToolUse,
                });
                events
            }
            ScriptedReply::Events(events) => events,
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
                vision: true,
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

/// Build a processor with caller-supplied permission rules, auto-approve flag,
/// and stream configuration. `max_retries`/backoff default to zero unless the
/// caller overrides them.
fn make_processor(
    event_bus: Arc<EventBus>,
    replies: Vec<ScriptedReply>,
    rules: Vec<PermissionRule>,
    auto_approve: bool,
    stream_config: StreamConfig,
) -> Result<Harness> {
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

    let processor = SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(rules))),
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
    Ok((processor, tmp.path().to_path_buf(), captured, tmp))
}

/// Default harness: no rules, interactive (auto-approve off), no retries.
fn make_plain(event_bus: Arc<EventBus>, replies: Vec<ScriptedReply>) -> Result<Harness> {
    make_processor(
        event_bus,
        replies,
        Vec::new(),
        false,
        StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
    )
}

fn allow_write_rule() -> PermissionRule {
    PermissionRule {
        permission: Permission::Custom("file:write".to_string()),
        pattern: Some("**".to_string()),
        action: PermissionAction::Allow,
    }
}

fn deny_write_rule() -> PermissionRule {
    PermissionRule {
        permission: Permission::Custom("file:write".to_string()),
        pattern: Some("**".to_string()),
        action: PermissionAction::Deny,
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

fn make_subagent() -> AgentInfo {
    let mut agent = make_agent();
    agent.mode = AgentMode::Subagent;
    agent
}

// ---------------------------------------------------------------------------
// Event observation
// ---------------------------------------------------------------------------

#[derive(Default)]
struct SeenEvents {
    request_started: bool,
    text_delta: bool,
    token_usage: bool,
    message_end_stop: bool,
    message_end_cancelled: bool,
    tool_call_start: bool,
    tool_call_end: bool,
    tool_result: bool,
    agent_error: bool,
    model_response: bool,
    run_cost_summary: bool,
    agent_notice: bool,
    task_completed: bool,
    question_requested: bool,
    loop_status: Option<String>,
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
                reason: LlmFinishReason::Cancelled,
                ..
            } => seen.message_end_cancelled = true,
            Event::ToolCallStart { .. } => seen.tool_call_start = true,
            Event::ToolCallEnd { .. } => seen.tool_call_end = true,
            Event::ToolResult { .. } => seen.tool_result = true,
            Event::AgentError { .. } => seen.agent_error = true,
            Event::ModelResponse { .. } => seen.model_response = true,
            Event::RunCostSummary { .. } => seen.run_cost_summary = true,
            Event::AgentNotice { .. } => seen.agent_notice = true,
            Event::TaskCompleted { .. } => seen.task_completed = true,
            Event::QuestionRequested { .. } => seen.question_requested = true,
            Event::LoopTerminated { status, .. } => seen.loop_status = Some(status),
            _ => {}
        }
    }
    seen
}

fn captured_count(captured: &Arc<Mutex<Vec<ChatRequest>>>) -> usize {
    captured.lock().expect("captured lock").len()
}

// ---------------------------------------------------------------------------
// Baseline text turn + cost/end-reason accounting
// ---------------------------------------------------------------------------

/// A plain text turn persists the assistant message, records the terminal
/// reason, and publishes the cost/usage/model-response events.
#[tokio::test]
async fn test_text_only_stop_records_cost_and_end_reason() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) =
        make_plain(event_bus.clone(), vec![ScriptedReply::Text("hello".into())])?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    assert!(processor.last_message_end_reason(&session.id).is_none());
    let reply = processor
        .process_message(
            &session.id,
            "hi",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("text turn succeeds");
    assert_eq!(reply.role, Role::Assistant);
    assert_eq!(reply.text_content(), "hello");
    assert_eq!(captured_count(&captured), 1);

    let stored = processor
        .session_manager
        .get_messages(&session.id)
        .expect("messages");
    assert!(stored.iter().any(|m| m.id == reply.id));

    assert_eq!(
        processor.last_message_end_reason(&session.id),
        Some(ragent_agent::event::FinishReason::Stop)
    );

    let seen = collect_events(&mut rx);
    assert!(seen.request_started);
    assert!(seen.text_delta);
    assert!(seen.token_usage);
    assert!(seen.model_response);
    assert!(
        seen.run_cost_summary,
        "RunCostSummary published on teardown"
    );
    assert!(seen.message_end_stop);
    Ok(())
}

// ---------------------------------------------------------------------------
// Multipart user message (image part)
// ---------------------------------------------------------------------------

/// `process_user_message` accepts a multipart message; an image part drives the
/// async history conversion that base64-encodes the file into the request.
#[tokio::test]
async fn test_multipart_image_user_message() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) =
        make_plain(event_bus.clone(), vec![ScriptedReply::Text("seen".into())])?;

    let img_path = working_dir.join("pic.png");
    std::fs::write(&img_path, b"\x89PNG\r\n\x1a\n not-a-real-png").expect("write png");

    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let parts = vec![
        MessagePart::Text {
            text: "describe".into(),
        },
        MessagePart::Image(Box::new(ImageData {
            mime_type: "image/png".into(),
            path: img_path.clone(),
        })),
    ];
    let user_msg = Message::new(&session.id, Role::User, parts);

    let reply = processor
        .process_user_message(
            &session.id,
            user_msg,
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("multipart turn succeeds");
    assert_eq!(reply.text_content(), "seen");

    let reqs = captured.lock().expect("captured lock").clone();
    assert_eq!(reqs.len(), 1);
    let has_image = reqs[0].messages.iter().any(|m| {
        match &m.content {
        ChatContent::Parts(parts) => parts.iter().any(|p| {
            matches!(p, ContentPart::ImageUrl { url } if url.starts_with("data:image/png;base64,"))
        }),
        ChatContent::Text(_) => false,
    }
    });
    assert!(
        has_image,
        "the image part is base64-encoded into the request"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Many real core tools across one turn
// ---------------------------------------------------------------------------

/// A single turn that walks a long chain of read-only/leaf tools. Every step
/// executes (no permission prompt) because the tools are either side-effect
/// free or read files under the working directory with relative paths.
#[tokio::test]
async fn test_many_real_tools_in_one_turn() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "think".into(),
                args: r#"{"thought": "planning"}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "calculator".into(),
                args: r#"{"expression": "2 + 2 * 3"}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "ragent_info".into(),
                args: r#"{"format": "json"}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "os_info".into(),
                args: r#"{"format": "json", "probe": false}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "file_info".into(),
                args: r#"{"path": "."}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "get_env".into(),
                args: r#"{"name": "PATH"}"#.into(),
            },
            ScriptedReply::Text("all done".into()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "run the tools",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("multi-tool turn succeeds");
    assert_eq!(
        captured_count(&captured),
        7,
        "six tool steps then a text step"
    );
    assert_eq!(reply.text_content(), "all done");

    let seen = collect_events(&mut rx);
    assert!(seen.tool_call_start);
    assert!(seen.tool_result);
    Ok(())
}

// ---------------------------------------------------------------------------
// Malformed args / two calls in one step
// ---------------------------------------------------------------------------

/// A tool call whose `args_json` is not valid JSON is short-circuited with a
/// corrective error observation and the loop continues.
#[tokio::test]
async fn test_malformed_json_args_recovered() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "read".into(),
                args: "{not valid json".into(),
            },
            ScriptedReply::Text("recovered".into()),
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
        .expect("malformed-args turn does not fail");
    assert_eq!(captured_count(&captured), 2);
    assert_eq!(reply.text_content(), "recovered");
    // The corrective error observation is fed back to the model via the
    // assistant message's tool-call part (no per-call ToolCallEnd event is
    // published on this synthetic short-circuit path).
    assert!(collect_events(&mut rx).message_end_stop);
    Ok(())
}

/// Two tool calls emitted in a single streamed step are both dispatched.
#[tokio::test]
async fn test_two_tool_calls_in_one_step() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::Tools(vec![
                ("think".to_string(), r#"{"thought": "a"}"#.to_string()),
                (
                    "calculator".to_string(),
                    r#"{"expression": "1+1"}"#.to_string(),
                ),
            ]),
            ScriptedReply::Text("both ran".into()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "two calls",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("two-call step succeeds");
    assert_eq!(
        captured_count(&captured),
        2,
        "one tool step + one text step"
    );
    assert_eq!(reply.text_content(), "both ran");
    assert!(collect_events(&mut rx).tool_result);
    Ok(())
}

// ---------------------------------------------------------------------------
// Stream error retry / fatal / silent
// ---------------------------------------------------------------------------

/// A retryable stream error with no partial output triggers a retry attempt;
/// the second attempt's response completes the turn.
#[tokio::test]
async fn test_retry_after_empty_stream_error() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::Events(vec![StreamEvent::Error {
                message: "connection reset by peer".into(),
            }]),
            ScriptedReply::Text("recovered after retry".into()),
        ],
        Vec::new(),
        false,
        StreamConfig {
            max_retries: 1,
            retry_backoff_secs: 0,
            ..Default::default()
        },
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
        .expect("retry recovers the turn");
    assert_eq!(captured_count(&captured), 2, "the error was retried once");
    assert_eq!(reply.text_content(), "recovered after retry");
    assert!(collect_events(&mut rx).agent_notice);
    Ok(())
}

/// A retryable stream error that arrives after meaningful partial output is
/// kept without retrying (the partial text becomes the reply).
#[tokio::test]
async fn test_partial_output_error_kept() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Events(vec![
            StreamEvent::TextDelta {
                text: "partial answer".into(),
            },
            StreamEvent::Error {
                message: "connection reset by peer".into(),
            },
        ])],
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
        .expect("partial output turn succeeds");
    assert_eq!(captured_count(&captured), 1, "no retry when output is kept");
    assert!(reply.text_content().contains("partial answer"));
    Ok(())
}

/// A permanent provider error is surfaced fatally without retrying.
#[tokio::test]
async fn test_fatal_permanent_error_no_retry() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::Events(vec![StreamEvent::Error {
            message: "invalid_request_error: model_not_supported".into(),
        }])],
        Vec::new(),
        false,
        StreamConfig {
            max_retries: 2,
            retry_backoff_secs: 0,
            ..Default::default()
        },
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
        "a permanent provider error fails the turn"
    );
    assert_eq!(
        captured_count(&captured),
        1,
        "no retry for a permanent error"
    );
    assert!(collect_events(&mut rx).agent_error);
    Ok(())
}

/// A silent `Finish { Stop }` with no text produces an empty assistant reply.
#[tokio::test]
async fn test_empty_response_stop() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir) =
        make_plain(event_bus.clone(), vec![ScriptedReply::Text(String::new())])?;
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
        .expect("empty turn succeeds");
    assert_eq!(reply.text_content(), "");
    assert!(collect_events(&mut rx).message_end_stop);
    Ok(())
}

// ---------------------------------------------------------------------------
// Cancellation
// ---------------------------------------------------------------------------

/// A cancel flag already raised aborts the loop before any LLM request and
/// ends the turn with `Cancelled`.
#[tokio::test]
async fn test_cancel_flag_pre_set() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) =
        make_plain(event_bus.clone(), vec![ScriptedReply::Text("never".into())])?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let cancel = Arc::new(AtomicBool::new(true));
    let reply = processor
        .process_message(&session.id, "go", &make_agent(), cancel)
        .await
        .expect("a cancelled turn returns normally");
    assert_eq!(reply.text_content(), "");
    assert_eq!(
        captured_count(&captured),
        0,
        "no request when cancelled first"
    );

    let seen = collect_events(&mut rx);
    assert!(seen.message_end_cancelled);
    assert!(seen.run_cost_summary);
    Ok(())
}

// ---------------------------------------------------------------------------
// Tool-repeat guard
// ---------------------------------------------------------------------------

/// Six identical tool calls: the first five pass, the sixth is auto-denied by
/// the repeat guard (auto-approve run), then a final text step ends the turn.
#[tokio::test]
async fn test_tool_repeat_guard_auto_denies() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let same_args = r#"{"thought": "same"}"#;
    let mut replies: Vec<ScriptedReply> = (0..7)
        .map(|_| ScriptedReply::ToolCall {
            name: "think".into(),
            args: same_args.into(),
        })
        .collect();
    replies.push(ScriptedReply::Text("guard done".into()));

    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        replies,
        Vec::new(),
        true,
        StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "loop forever",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("repeat-guard turn succeeds");
    assert!(
        captured_count(&captured) >= 8,
        "the loop kept running after denial"
    );
    assert!(reply.text_content().contains("guard done"));

    let seen = collect_events(&mut rx);
    assert!(
        seen.tool_call_end,
        "the denied call is closed out with an error"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Loop-breaking tool metadata
// ---------------------------------------------------------------------------

/// `EnterPlanMode` reports `agent_switch` metadata, which breaks the tool loop.
/// Auto-approve keeps the `plan` permission from prompting (the test must not
/// block on an interactive permission dialog).
#[tokio::test]
async fn test_plan_enter_switch_breaks_loop() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::ToolCall {
            name: "EnterPlanMode".into(),
            args: r#"{"task": "design something"}"#.into(),
        }],
        Vec::new(),
        true,
        StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let _reply = processor
        .process_message(
            &session.id,
            "plan",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("EnterPlanMode turn succeeds");
    assert_eq!(
        captured_count(&captured),
        1,
        "the loop breaks after a switch"
    );
    Ok(())
}

/// `ExitPlanMode` reports `agent_restore` metadata, which also breaks the loop.
#[tokio::test]
async fn test_plan_exit_restore_breaks_loop() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::ToolCall {
            name: "ExitPlanMode".into(),
            args: r#"{"summary": "here is the plan"}"#.into(),
        }],
        Vec::new(),
        true,
        StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let _reply = processor
        .process_message(
            &session.id,
            "finish plan",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("ExitPlanMode turn succeeds");
    assert_eq!(captured_count(&captured), 1);
    Ok(())
}

/// `agent_complete` reports `agent_complete` metadata, which breaks the loop
/// (the completed tool call is recorded in the final message).
#[tokio::test]
async fn test_agent_complete_breaks_loop() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::ToolCall {
            name: "agent_complete".into(),
            args: r#"{"summary": "all done"}"#.into(),
        }],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let final_message = processor
        .process_message(
            &session.id,
            "finish",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("agent_complete turn succeeds");
    assert_eq!(
        captured_count(&captured),
        1,
        "the loop breaks on completion"
    );
    // The final message carries the `agent_complete` tool call recorded as a
    // completed `ToolCall` part (`agent_complete_requested` broke the loop).
    assert!(matches!(
        final_message.parts.iter().find_map(|p| match p {
            MessagePart::ToolCall { tool, state, .. } if tool == "agent_complete" => {
                Some(state.status.clone())
            }
            _ => None,
        }),
        Some(ragent_agent::message::ToolCallStatus::Completed)
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// Permission paths
// ---------------------------------------------------------------------------

/// Auto-approve mode lets a `Write` step execute (the `Some(true)` allow path),
/// then a `read` verifies the file.
#[tokio::test]
async fn test_auto_approve_write_then_read() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "Write".into(),
                args: r#"{"path": "auto.txt", "content": "auto-approved"}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "read".into(),
                args: r#"{"path": "auto.txt"}"#.into(),
            },
            ScriptedReply::Text("wrote".into()),
        ],
        Vec::new(),
        true,
        StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "write a file",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("auto-approve turn succeeds");
    assert_eq!(captured_count(&captured), 3);
    assert_eq!(reply.text_content(), "wrote");
    assert!(working_dir.join("auto.txt").is_file());
    Ok(())
}

/// A long chain of file tools under an allow rule: write, edit, multiedit,
/// append, copy, move, file_info, diff, read, rm.
#[tokio::test]
async fn test_edit_chain_of_file_tools() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "Write".into(),
                args: r#"{"path": "a.txt", "content": "hello world\n"}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "Edit".into(),
                args: r#"{"file_path": "a.txt", "old_string": "hello", "new_string": "hi"}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "multiedit".into(),
                args: r#"{"edits": [{"file_path": "a.txt", "old_string": "hi", "new_string": "hey"}]}"#
                    .into(),
            },
            ScriptedReply::ToolCall {
                name: "append_to_file".into(),
                args: r#"{"path": "a.txt", "content": "more\n"}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "copy_file".into(),
                args: r#"{"source": "a.txt", "destination": "b.txt"}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "move_file".into(),
                args: r#"{"source": "b.txt", "destination": "c.txt"}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "file_info".into(),
                args: r#"{"path": "c.txt"}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "diff".into(),
                args: r#"{"text_a": "one", "text_b": "two"}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "read".into(),
                args: r#"{"path": "c.txt"}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "rm".into(),
                args: r#"{"path": "c.txt"}"#.into(),
            },
            ScriptedReply::Text("chain complete".into()),
        ],
        vec![allow_write_rule()],
        false,
        StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "chain",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("file-tool chain succeeds");
    assert_eq!(captured_count(&captured), 11, "ten tool steps then text");
    assert_eq!(reply.text_content(), "chain complete");
    assert!(!working_dir.join("c.txt").exists(), "rm removed the file");
    assert!(collect_events(&mut rx).tool_result);
    Ok(())
}

/// A `file:write` deny rule blocks the write with a permission error that the
/// loop surfaces as a recoverable observation before continuing to text.
#[tokio::test]
async fn test_permission_denied_write_recovers() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "Write".into(),
                args: r#"{"path": "blocked.txt", "content": "x"}"#.into(),
            },
            ScriptedReply::Text("recovered from denial".into()),
        ],
        vec![deny_write_rule()],
        false,
        StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "Write",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("denied write does not fail the turn");
    assert_eq!(captured_count(&captured), 2);
    assert_eq!(reply.text_content(), "recovered from denial");
    assert!(!working_dir.join("blocked.txt").exists());
    assert!(collect_events(&mut rx).tool_call_end);
    Ok(())
}

// ---------------------------------------------------------------------------
// ask_user answered by a responder
// ---------------------------------------------------------------------------

/// `AskUserQuestion` blocks until a `QuestionAnswered` arrives; a spawned responder
/// answers the request so the tool returns the response and the loop advances.
#[tokio::test]
async fn test_ask_user_question_answered() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "AskUserQuestion".into(),
                args: r#"{"question": "pick one"}"#.into(),
            },
            ScriptedReply::Text("answered".into()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    // Subscribe synchronously so the responder cannot miss the request.
    let mut resp_rx = event_bus.subscribe();
    let sid = session.id.clone();
    let bus = event_bus.clone();
    tokio::spawn(async move {
        while let Ok(event) = resp_rx.recv().await {
            if let Event::QuestionRequested {
                request_id,
                session_id,
                ..
            } = event
                && session_id == sid
            {
                bus.publish(Event::QuestionAnswered {
                    session_id: sid.clone(),
                    request_id,
                    response: "42".into(),
                });
                break;
            }
        }
    });

    let reply = processor
        .process_message(
            &session.id,
            "ask",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("ask_user turn succeeds");
    assert_eq!(reply.text_content(), "answered");
    assert_eq!(captured_count(&captured), 2);
    Ok(())
}

// ---------------------------------------------------------------------------
// Text-format tool-call recovery
// ---------------------------------------------------------------------------

/// A response that is a bare JSON tool-call object is recovered into a real
/// tool call and executed instead of being treated as prose.
#[tokio::test]
async fn test_text_format_tool_call_recovery() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::Text(
                r#"{"name": "think", "arguments": {"thought": "from text"}}"#.into(),
            ),
            ScriptedReply::Text("after recovery".into()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "recover",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("text-recovery turn succeeds");
    assert_eq!(
        captured_count(&captured),
        2,
        "the recovered call runs a step"
    );
    // The recovered JSON narration remains as a text part, so the final reply
    // carries both the (blanked/kept) narration and the follow-up text.
    assert!(reply.text_content().contains("after recovery"));
    assert!(collect_events(&mut rx).tool_result);
    Ok(())
}

// ---------------------------------------------------------------------------
// Goal-loop lifecycle
// ---------------------------------------------------------------------------

/// A loop with `max_steps = 1` terminates as `budget_exhausted` after the
/// first step.
#[tokio::test]
async fn test_loop_budget_exhausted() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::ToolCall {
            name: "think".into(),
            args: r#"{"thought": "step"}"#.into(),
        }],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut spec = LoopSpec::new("general", "bounded goal");
    spec.max_steps = Some(1);
    processor.start_loop(&session.id, spec).await;
    assert!(processor.loop_active(&session.id).await);

    let _reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("budget-loop turn succeeds");

    let seen = collect_events(&mut rx);
    assert_eq!(seen.loop_status.as_deref(), Some("budget_exhausted"));
    assert_eq!(
        captured_count(&captured),
        1,
        "stopped before a second request"
    );
    Ok(())
}

/// A goal loop with no verification command completes when the model returns a
/// text-only response (`completed`).
#[tokio::test]
async fn test_loop_goal_achieved() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Text("goal met".into())],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let spec = LoopSpec::new("general", "reach the goal");
    processor.start_loop(&session.id, spec).await;

    let _reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("goal-loop turn succeeds");

    assert_eq!(
        collect_events(&mut rx).loop_status.as_deref(),
        Some("completed")
    );
    Ok(())
}

/// A passing verification command terminates the loop as `completed`.
#[tokio::test]
async fn test_loop_verification_pass() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Text("verified".into())],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut spec = LoopSpec::new("general", "verify the goal");
    spec.verify_cmd = Some("true".to_string());
    processor.start_loop(&session.id, spec).await;

    let _reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("verification turn succeeds");

    assert_eq!(
        collect_events(&mut rx).loop_status.as_deref(),
        Some("completed")
    );
    Ok(())
}

/// A failing verification command with no steps remaining terminates the loop
/// as `budget_exhausted` after the retry budget is spent.
#[tokio::test]
async fn test_loop_verification_fail_budget() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) =
        make_plain(event_bus.clone(), vec![ScriptedReply::Text("x".into())])?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut spec = LoopSpec::new("general", "never verified");
    spec.verify_cmd = Some("false".to_string());
    spec.max_steps = Some(2);
    processor.start_loop(&session.id, spec).await;

    let _reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("failing-verification turn succeeds");

    let seen = collect_events(&mut rx);
    assert_eq!(seen.loop_status.as_deref(), Some("budget_exhausted"));
    assert!(seen.agent_notice, "the failure observation is surfaced");
    assert_eq!(
        captured_count(&captured),
        2,
        "two attempts before budget stop"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Sub-agent paths
// ---------------------------------------------------------------------------

/// In a sub-agent run an interactive tool (`AskUserQuestion`) is denied with a
/// corrective observation rather than blocking.
#[tokio::test]
async fn test_subagent_interactive_tool_denied() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let long = format!("subagent recovered{}", "y".repeat(2500));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "think".into(),
                args: r#"{"thought": "a"}"#.into(),
            },
            ScriptedReply::ToolCall {
                name: "AskUserQuestion".into(),
                args: r#"{"question": "should never block"}"#.into(),
            },
            ScriptedReply::Text(long.clone()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "sub task",
            &make_subagent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("interactive denial is recoverable");
    assert_eq!(
        captured_count(&captured),
        3,
        "the loop continued past the denial"
    );
    assert!(reply.text_content().contains("subagent recovered"));
    assert!(collect_events(&mut rx).tool_call_end);
    Ok(())
}

/// A sub-agent that ends a tool turn with a short text-only response is nudged
/// to produce its findings; a later long response terminates the turn.
#[tokio::test]
async fn test_subagent_narration_nudge() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let long = "z".repeat(2500);
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "think".into(),
                args: r#"{"thought": "working"}"#.into(),
            },
            ScriptedReply::Text("Now let me check...".into()),
            ScriptedReply::Text(long.clone()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "sub task",
            &make_subagent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("subagent narration-nudge turn succeeds");
    assert!(
        captured_count(&captured) >= 3,
        "the nudge forced a further step"
    );
    assert_eq!(reply.text_content(), long);
    assert!(collect_events(&mut rx).agent_notice);
    Ok(())
}

/// A sub-agent whose stream ends silently (no `Finish`) with non-empty text is
/// asked to continue; the continuation response completes the turn.
#[tokio::test]
async fn test_subagent_silent_truncation_continuation() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::Events(vec![
                StreamEvent::TextDelta {
                    text: "cut off mid".into(),
                },
                StreamEvent::Usage {
                    input_tokens: 10,
                    output_tokens: 5,
                },
            ]),
            ScriptedReply::Text("-sentence continued".into()),
        ],
        Vec::new(),
        false,
        StreamConfig {
            max_retries: 2,
            retry_backoff_secs: 0,
            ..Default::default()
        },
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "sub task",
            &make_subagent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("truncation-continuation turn succeeds");
    assert_eq!(
        captured_count(&captured),
        2,
        "a continuation request was issued"
    );
    assert!(reply.text_content().contains("continued"));
    assert!(collect_events(&mut rx).agent_notice);
    Ok(())
}

// ---------------------------------------------------------------------------
// Provider-stage failure
// ---------------------------------------------------------------------------

/// An agent without a configured model fails `prepare_client` and surfaces an
/// `AgentError` without any LLM request.
#[tokio::test]
async fn test_missing_model_fails_turn() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Text("unused".into())],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut agent = AgentInfo::new("general", "General");
    agent.model = None;

    let outcome = processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await;
    assert!(outcome.is_err(), "a missing model fails the provider stage");
    assert_eq!(captured_count(&captured), 0, "no request without a model");
    assert!(collect_events(&mut rx).agent_error);
    Ok(())
}
