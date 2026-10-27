//! Additional integration coverage for `SessionProcessor::process_message` /
//! `process_user_message` and the extracted loop steps in
//! `session::loop_steps` (coverage campaign: `processor.rs` + `loop_steps.rs`).
//!
//! This file copies the scripted-provider harness from
//! `test_loop_steps_harness.rs` (an external `tests/` crate cannot call the
//! `pub(crate)` loop-step methods, so every path is driven through the public
//! `process_message` / `process_user_message` entry points and the public
//! `run_init_exchange`) and widens it to reach branches the existing
//! `test_processor_loop_paths.rs` and `test_loop_steps_extra.rs` files do not:
//!
//! - the display-only AGENTS.md init acknowledgement (and its second-turn skip);
//! - the public `run_init_exchange` path (persisted ack);
//! - reasoning-only stream events and the `ReasoningDelta` publish;
//! - `RateLimit` / `QuotaUpdate`, `ToolsSent`, `ToolCallArgs`, `ToolCallBatch`;
//! - `FinishReason::Length` (truncation notice) and `FinishReason::ContentFilter`
//!   and a bare `Finish { ToolUse }` with no tool calls;
//! - the successful pre-send compaction path (with a `compaction.model` notice)
//! - the sub-agent filtered tool surface (no interactive tools on the wire);
//! - a goal loop with a restricted tool set on the wire;
//! - XMl- dialect and bare-JSON-array text tool-call recovery (plus the
//!   mixed-array rejection);
//! - the schema-violation corrective error and the unknown-tool "did you mean"
//!   suggestion;
//! - parallel tool calls;
//! - the tool-call args accumulation cap.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, AgentMode, ModelRef};
use ragent_agent::event::{Event, EventBus, FinishReason};
use ragent_agent::llm::{ChatContent, ChatRequest, ContentPart, LlmClient, StreamEvent};
use ragent_agent::message::{Message, MessagePart, Role, ToolCallStatus};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::loop_state::LoopSpec;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::permission::PermissionRule;
use ragent_config::{Capabilities, Config as RagentConfig, Cost, StreamConfig};

const MODEL_ID: &str = "qwen3:latest";
const PROVIDER_ID: &str = "ollama";

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
    context_window: usize,
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
                    reason: FinishReason::Stop,
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
                    reason: FinishReason::ToolUse,
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
                    reason: FinishReason::ToolUse,
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
                vision: true,
                tool_use: true,
                thinking_levels: vec![],
                reasoning: false,
                streaming: true,
            },
            context_window: self.context_window,
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

fn default_config() -> RagentConfig {
    serde_json::from_str("{}").expect("valid default config JSON")
}

fn make_processor(
    event_bus: Arc<EventBus>,
    replies: Vec<ScriptedReply>,
    rules: Vec<PermissionRule>,
    auto_approve: bool,
    stream_config: StreamConfig,
    context_window: usize,
    config: RagentConfig,
) -> Result<Harness> {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        replies: Arc::new(Mutex::new(replies)),
        call_count: Arc::new(AtomicU32::new(0)),
        captured: Arc::clone(&captured),
        context_window,
    }));

    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));

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

/// Default harness: no rules, interactive (auto-approve off), no retries, the
/// standard 128k context window, and the default config.
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
        128_000,
        default_config(),
    )
}

fn make_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: PROVIDER_ID.to_string(),
        model_id: MODEL_ID.to_string(),
    });
    agent
}

fn make_subagent() -> AgentInfo {
    let mut agent = make_agent();
    agent.mode = AgentMode::Subagent;
    agent
}

fn long_text(len: usize) -> String {
    "y".repeat(len)
}

// ---------------------------------------------------------------------------
// Event observation
// ---------------------------------------------------------------------------

#[derive(Default)]
struct SeenEvents {
    request_started: bool,
    text_delta: bool,
    reasoning_delta: bool,
    token_usage: bool,
    message_end_stop: bool,
    message_end_length: bool,
    message_end_content_filter: bool,
    message_end_tool_use: bool,
    message_end_cancelled: bool,
    tool_call_start: bool,
    tool_call_end: bool,
    tool_call_args: bool,
    tool_call_batch: bool,
    tool_result: bool,
    tools_sent: bool,
    agent_error: bool,
    model_response: bool,
    run_cost_summary: bool,
    agent_notice: bool,
    quota_update: bool,
    compression_started: bool,
    compression_finished: bool,
    loop_status: Option<String>,
}

fn collect_events(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> SeenEvents {
    let mut seen = SeenEvents::default();
    while let Ok(event) = rx.try_recv() {
        match event {
            Event::RequestStarted { .. } => seen.request_started = true,
            Event::TextDelta { .. } => seen.text_delta = true,
            Event::ReasoningDelta { .. } => seen.reasoning_delta = true,
            Event::TokenUsage { .. } => seen.token_usage = true,
            Event::MessageEnd { reason, .. } => match reason {
                FinishReason::Stop => seen.message_end_stop = true,
                FinishReason::Length => seen.message_end_length = true,
                FinishReason::ContentFilter => seen.message_end_content_filter = true,
                FinishReason::ToolUse => seen.message_end_tool_use = true,
                FinishReason::Cancelled => seen.message_end_cancelled = true,
                FinishReason::Truncation => {}
            },
            Event::ToolCallStart { .. } => seen.tool_call_start = true,
            Event::ToolCallEnd { .. } => seen.tool_call_end = true,
            Event::ToolCallArgs { .. } => seen.tool_call_args = true,
            Event::ToolCallBatch { .. } => seen.tool_call_batch = true,
            Event::ToolResult { .. } => seen.tool_result = true,
            Event::ToolsSent { .. } => seen.tools_sent = true,
            Event::AgentError { .. } => seen.agent_error = true,
            Event::ModelResponse { .. } => seen.model_response = true,
            Event::RunCostSummary { .. } => seen.run_cost_summary = true,
            Event::AgentNotice { .. } => seen.agent_notice = true,
            Event::QuotaUpdate { .. } => seen.quota_update = true,
            Event::CompressionStarted { .. } => seen.compression_started = true,
            Event::CompressionFinished { .. } => seen.compression_finished = true,
            Event::LoopTerminated { status, .. } => seen.loop_status = Some(status),
            _ => {}
        }
    }
    seen
}

fn captured_count(captured: &Arc<Mutex<Vec<ChatRequest>>>) -> usize {
    captured.lock().expect("captured lock").len()
}

fn captured_requests(captured: &Arc<Mutex<Vec<ChatRequest>>>) -> Vec<ChatRequest> {
    captured.lock().expect("captured lock").clone()
}

/// The `is_error` flags of the tool results carried by the `index`-th request's
/// history, in order.
///
/// FR-014 is about what the *provider* receives, so the flag is read off the
/// wire form (`ContentPart::ToolResult`) rather than off the rendered message.
fn refusal_results(captured: &Arc<Mutex<Vec<ChatRequest>>>, index: usize) -> Vec<bool> {
    let requests = captured.lock().expect("captured lock");
    let Some(request) = requests.get(index) else {
        return Vec::new();
    };
    request
        .messages
        .iter()
        .filter_map(|m| match &m.content {
            ChatContent::Parts(parts) => Some(parts),
            ChatContent::Text(_) => None,
        })
        .flatten()
        .filter_map(|p| match p {
            ContentPart::ToolResult { is_error, .. } => Some(*is_error),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Event-shape coverage: reasoning, rate limit, tools sent, tool-call args/batch
// ---------------------------------------------------------------------------

/// A reasoning-only stream publishes `ReasoningDelta` and records a `Reasoning`
/// part; the final reply carries no text.
#[tokio::test]
async fn test_reasoning_only_step_records_reasoning_part() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Events(vec![
            StreamEvent::ReasoningStart,
            StreamEvent::ReasoningDelta {
                text: "pondering".to_string(),
            },
            StreamEvent::ReasoningEnd,
            StreamEvent::Usage {
                input_tokens: 10,
                output_tokens: 5,
            },
            StreamEvent::Finish {
                reason: FinishReason::Stop,
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
            "think it through",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("reasoning-only turn succeeds");
    assert_eq!(captured_count(&captured), 1);
    assert_eq!(reply.text_content(), "");
    assert!(
        reply
            .parts
            .iter()
            .any(|p| matches!(p, MessagePart::Reasoning { .. })),
        "the reasoning buffer is recorded as a Reasoning part"
    );

    let seen = collect_events(&mut rx);
    assert!(seen.reasoning_delta, "ReasoningDelta must be published");
    assert!(seen.message_end_stop);
    Ok(())
}

/// A `RateLimit` event surfaces a `QuotaUpdate` carrying the request percentage.
#[tokio::test]
async fn test_rate_limit_event_publishes_quota_update() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Events(vec![
            StreamEvent::TextDelta {
                text: "ok".to_string(),
            },
            StreamEvent::RateLimit {
                requests_used_pct: Some(42.0),
                tokens_used_pct: None,
            },
            StreamEvent::Usage {
                input_tokens: 1,
                output_tokens: 1,
            },
            StreamEvent::Finish {
                reason: FinishReason::Stop,
            },
        ])],
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
        .expect("rate-limit turn succeeds");

    assert!(
        collect_events(&mut rx).quota_update,
        "a QuotaUpdate is published for the rate-limit percentage"
    );
    Ok(())
}

/// A tool step publishes `ToolsSent` (step 1), `ToolCallArgs`, and a final
/// `ToolCallBatch` summarising the step.
#[tokio::test]
async fn test_tool_step_publishes_tools_sent_args_and_batch() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "calculator".to_string(),
                args: r#"{"expression": "6 * 7"}"#.to_string(),
            },
            ScriptedReply::Text("done".to_string()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    processor
        .process_message(
            &session.id,
            "compute",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("tool turn succeeds");

    assert_eq!(captured_count(&captured), 2);
    let seen = collect_events(&mut rx);
    assert!(seen.tools_sent, "ToolsSent published on the first step");
    assert!(seen.tool_call_args, "ToolCallArgs published per call");
    assert!(seen.tool_call_batch, "ToolCallBatch published per step");
    Ok(())
}

// ---------------------------------------------------------------------------
// finish_reason variants
// ---------------------------------------------------------------------------

/// `Finish { Length }` on an interactive turn publishes the truncation notice
/// and records `Length` as the terminal reason.
#[tokio::test]
async fn test_finish_length_records_reason_and_publishes_notice() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Events(vec![
            StreamEvent::TextDelta {
                text: "a long but cut-off answer".to_string(),
            },
            StreamEvent::Usage {
                input_tokens: 10,
                output_tokens: 5,
            },
            StreamEvent::Finish {
                reason: FinishReason::Length,
            },
        ])],
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
        .expect("length turn succeeds");

    assert_eq!(
        processor.last_message_end_reason(&session.id),
        Some(FinishReason::Length)
    );
    let seen = collect_events(&mut rx);
    assert!(seen.message_end_length, "MessageEnd carries Length");
    assert!(
        seen.agent_notice,
        "an interactive turn is warned about the truncation"
    );
    Ok(())
}

/// `Finish { ContentFilter }` records `ContentFilter` as the terminal reason and
/// does NOT emit the truncation notice.
#[tokio::test]
async fn test_finish_content_filter_records_reason() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Events(vec![
            StreamEvent::Usage {
                input_tokens: 10,
                output_tokens: 5,
            },
            StreamEvent::Finish {
                reason: FinishReason::ContentFilter,
            },
        ])],
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
        .expect("content-filter turn succeeds");

    assert_eq!(
        processor.last_message_end_reason(&session.id),
        Some(FinishReason::ContentFilter)
    );
    assert!(collect_events(&mut rx).message_end_content_filter);
    Ok(())
}

/// A bare `Finish { ToolUse }` with no tool calls and no text ends the turn and
/// records `Truncation` as the terminal reason: spec `uniformtools` FR-022 - a
/// tool-use finish with zero complete calls is a lost tool call, not a clean
/// tool-use turn, so the reply is not presented as a completed answer.
#[tokio::test]
async fn test_finish_tool_use_without_calls_records_truncation() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Events(vec![
            StreamEvent::Usage {
                input_tokens: 10,
                output_tokens: 5,
            },
            StreamEvent::Finish {
                reason: FinishReason::ToolUse,
            },
        ])],
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
        .expect("tool-use-without-call turn succeeds");
    assert_eq!(captured_count(&captured), 1, "no second request");
    assert_eq!(
        processor.last_message_end_reason(&session.id),
        Some(FinishReason::Truncation)
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// AGENTS.md init acknowledgement
// ---------------------------------------------------------------------------

/// With an AGENTS.md present and no prior assistant turn, the display-only init
/// acknowledgement issues an extra request before the main turn; a second turn
/// skips it because an assistant message now exists.
#[tokio::test]
async fn test_init_acknowledgement_runs_then_skips() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::Text("acknowledged".to_string()),
            ScriptedReply::Text("first reply".to_string()),
            ScriptedReply::Text("second reply".to_string()),
        ],
    )?;
    std::fs::write(working_dir.join("AGENTS.md"), "# Guidelines\n\nBe nice.\n")
        .expect("write AGENTS.md");
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let first = processor
        .process_message(
            &session.id,
            "turn one",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("first turn succeeds");
    assert_eq!(first.text_content(), "first reply");
    assert_eq!(
        captured_count(&captured),
        2,
        "init ack + main request on the first turn"
    );

    let second = processor
        .process_message(
            &session.id,
            "turn two",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("second turn succeeds");
    assert_eq!(second.text_content(), "second reply");
    assert_eq!(
        captured_count(&captured),
        3,
        "the init ack is skipped once an assistant turn exists"
    );
    Ok(())
}

/// The public `run_init_exchange` resolves the model, streams the ack, and
/// persists the user trigger plus the assistant acknowledgement.
#[tokio::test]
async fn test_run_init_exchange_persists_ack() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Text("hello".to_string())],
    )?;
    std::fs::write(working_dir.join("AGENTS.md"), "# Guidelines\n").expect("write AGENTS.md");
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    processor
        .run_init_exchange(&session.id, &make_agent(), Arc::new(AtomicBool::new(false)))
        .await
        .expect("run_init_exchange succeeds");
    assert_eq!(captured_count(&captured), 1, "one init request");

    let stored = processor
        .session_manager
        .get_messages(&session.id)
        .expect("messages");
    assert!(
        stored.iter().any(|m| m.text_content() == "hello"),
        "the ack is persisted for a well-formed history"
    );
    assert!(collect_events(&mut rx).agent_notice);
    Ok(())
}

// ---------------------------------------------------------------------------
// Sub-agent tool surface
// ---------------------------------------------------------------------------

/// A sub-agent's advertised tool surface excludes interactive tools.
#[tokio::test]
async fn test_subagent_tool_surface_excludes_interactive_tools() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Text(long_text(2500))],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    processor
        .process_message(
            &session.id,
            "sub task",
            &make_subagent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("subagent turn succeeds");

    let requests = captured_requests(&captured);
    assert_eq!(requests.len(), 1);
    assert!(
        !requests[0].tools.is_empty(),
        "tool definitions are still advertised"
    );
    assert!(
        requests[0]
            .tools
            .iter()
            .all(|t| t.name != "AskUserQuestion" && t.name != "question"),
        "interactive tools are removed from the sub-agent wire surface"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Goal loop with a restricted tool set
// ---------------------------------------------------------------------------

/// A goal loop with a configured tool set narrows the wire tool definitions to
/// that set plus the mandatory safety tools.
#[tokio::test]
async fn test_loop_tool_set_restricts_wire_surface() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "think".to_string(),
                args: r#"{"thought": "restricted"}"#.to_string(),
            },
            ScriptedReply::Text("goal met".to_string()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut spec = LoopSpec::new("general", "stay in scope");
    spec.tool_set = vec!["think".to_string()];
    processor.start_loop(&session.id, spec).await;

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("restricted loop turn succeeds");

    let requests = captured_requests(&captured);
    assert!(!requests.is_empty());
    assert!(
        requests[0].tools.iter().any(|t| t.name == "think"),
        "the loop's own tool is offered"
    );
    assert!(
        requests[0].tools.iter().all(|t| t.name != "Bash"),
        "tools outside the loop set are not offered on the wire"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Pre-send compaction
// ---------------------------------------------------------------------------

/// A large history with a `compaction.model` override triggers the pre-send
/// compaction path: the summary request runs, the override notice is published,
/// and the main turn still completes.
#[tokio::test]
async fn test_pre_send_compaction_with_model_override() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let config: RagentConfig = serde_json::from_str(
        r#"{"compaction":{"auto":true,"model":{"provider_id":"ollama","model_id":"qwen3:latest"}}}"#,
    )
    .expect("config");
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::Text("summary of earlier work".to_string()),
            ScriptedReply::Text("main reply".to_string()),
        ],
        Vec::new(),
        false,
        StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
        128_000,
        config,
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    // Seed enough volume to exceed the 70 % compaction threshold.
    let pad = "x".repeat(4_000);
    for i in 0..120 {
        let role = if i % 2 == 0 {
            Role::User
        } else {
            Role::Assistant
        };
        let msg = Message::new(
            &session.id,
            role,
            vec![MessagePart::Text {
                text: format!("m{i} {pad}"),
            }],
        );
        processor
            .session_manager
            .storage()
            .create_message(&msg)
            .expect("seed message");
    }

    let reply = processor
        .process_message(
            &session.id,
            "continue",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("compacted turn succeeds");
    assert_eq!(reply.text_content(), "main reply");
    assert_eq!(
        captured_count(&captured),
        2,
        "one summary request then the main request"
    );

    let stored = processor
        .session_manager
        .get_messages(&session.id)
        .expect("messages");
    assert!(
        stored.iter().any(|m| m.role == Role::Compaction),
        "the synthetic compaction message is persisted"
    );

    let seen = collect_events(&mut rx);
    assert!(seen.compression_started, "CompressionStarted published");
    assert!(
        seen.agent_notice,
        "the compaction.model override notice is published"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Text-format tool-call recovery
// ---------------------------------------------------------------------------

/// The XML function-calls dialect is recovered into a real tool call.
#[tokio::test]
async fn test_xml_toolcall_recovery() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::Text(
                "<function_calls>\n<function=calculator>\n\
                 <parameter=expression>1+1</parameter>\n</function>\n</function_calls>"
                    .to_string(),
            ),
            ScriptedReply::Text("after xml".to_string()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "recover xml",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("xml recovery turn succeeds");
    assert_eq!(
        captured_count(&captured),
        2,
        "the recovered call runs a step"
    );
    assert!(reply.text_content().contains("after xml"));
    assert!(collect_events(&mut rx).tool_result);
    Ok(())
}

/// A bare JSON array of tool-call objects is recovered into two calls.
#[tokio::test]
async fn test_bare_json_array_toolcall_recovery() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::Text(
                r#"[{"name":"think","arguments":{"thought":"a"}},{"name":"calculator","arguments":{"expression":"2+2"}}]"#
                    .to_string(),
            ),
            ScriptedReply::Text("after json".to_string()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "recover json",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("json recovery turn succeeds");
    assert_eq!(captured_count(&captured), 2);
    assert!(reply.text_content().contains("after json"));
    Ok(())
}

/// A mixed JSON array (one entry is not a tool call) is treated as prose, not a
/// partial invocation, so the turn ends with the text.
#[tokio::test]
async fn test_mixed_json_array_not_recovered() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Text(
            r#"[{"name":"think","arguments":{"thought":"x"}},{"nope":1}]"#.to_string(),
        )],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "mixed array",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("mixed-array turn succeeds");
    assert_eq!(captured_count(&captured), 1, "no recovery, no extra step");
    assert!(
        reply.text_content().contains("nope"),
        "the mixed array is kept as prose"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Schema violation + unknown-tool suggestion
// ---------------------------------------------------------------------------

/// A tool call missing a required argument receives a corrective schema error
/// and the loop continues.
#[tokio::test]
async fn test_schema_violation_recovered() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "read".to_string(),
                args: "{}".to_string(),
            },
            ScriptedReply::Text("recovered".to_string()),
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
        .expect("schema-violation turn does not fail");
    assert_eq!(captured_count(&captured), 2);
    assert_eq!(reply.text_content(), "recovered");
    assert!(collect_events(&mut rx).tool_call_end);
    Ok(())
}

/// An unknown tool name whose nearest neighbour exists yields a "did you mean"
/// suggestion and the loop continues.
#[tokio::test]
async fn test_unknown_tool_suggestion_recovered() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "reed".to_string(),
                args: r#"{"path": "x"}"#.to_string(),
            },
            ScriptedReply::Text("recovered from typo".to_string()),
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
        .expect("unknown-tool turn does not fail");
    assert_eq!(captured_count(&captured), 2);
    assert!(reply.text_content().contains("recovered from typo"));
    assert!(collect_events(&mut rx).tool_call_end);
    Ok(())
}

// ---------------------------------------------------------------------------
// Parallel tool calls
// ---------------------------------------------------------------------------

/// Two tool calls in one step with parallel execution enabled both run and a
/// single `ToolCallBatch` is published.
#[tokio::test]
async fn test_parallel_tool_calls_batch() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let config: RagentConfig =
        serde_json::from_str(r#"{"experimental":{"parallel_tool_calls":true}}"#).expect("config");
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::Tools(vec![
                ("think".to_string(), r#"{"thought": "a"}"#.to_string()),
                (
                    "calculator".to_string(),
                    r#"{"expression": "1+1"}"#.to_string(),
                ),
            ]),
            ScriptedReply::Text("parallel done".to_string()),
        ],
        Vec::new(),
        false,
        StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
        128_000,
        config,
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "parallel",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("parallel-call turn succeeds");
    assert_eq!(captured_count(&captured), 2);
    assert_eq!(reply.text_content(), "parallel done");
    let seen = collect_events(&mut rx);
    assert!(seen.tool_call_batch, "a single batch is published");
    assert!(seen.tool_result);
    Ok(())
}

// ---------------------------------------------------------------------------
// Tool-call args accumulation cap
// ---------------------------------------------------------------------------

/// A tool-call delta past the 1 MiB accumulation cap is truncated, the args
/// parse fails, and the model receives a corrective error before recovering.
#[tokio::test]
async fn test_tool_call_args_cap_truncates_and_recovers() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::Events(vec![
                StreamEvent::ToolCallStart {
                    id: "c1".to_string(),
                    name: "read".to_string(),
                },
                StreamEvent::ToolCallDelta {
                    id: "c1".to_string(),
                    // Just over the 1 MiB (1_048_576 byte) cap so the guard trips.
                    args_json: "x".repeat(1_048_600),
                },
                StreamEvent::ToolCallEnd {
                    id: "c1".to_string(),
                },
                StreamEvent::Usage {
                    input_tokens: 1,
                    output_tokens: 1,
                },
                StreamEvent::Finish {
                    reason: FinishReason::ToolUse,
                },
            ]),
            ScriptedReply::Text("recovered after cap".to_string()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "oversized args",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("capped-args turn does not fail");
    assert_eq!(captured_count(&captured), 2);
    assert!(reply.text_content().contains("recovered after cap"));
    Ok(())
}

// ---------------------------------------------------------------------------
// Turn-to-turn history cache
// ---------------------------------------------------------------------------

/// Two sequential turns reuse the per-session chat-message cache; both succeed.
#[tokio::test]
async fn test_second_turn_reuses_history_cache() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::Text("first".to_string()),
            ScriptedReply::Text("second".to_string()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let first = processor
        .process_message(
            &session.id,
            "one",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("first turn succeeds");
    let second = processor
        .process_message(
            &session.id,
            "two",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("second turn succeeds");

    assert_eq!(first.text_content(), "first");
    assert_eq!(second.text_content(), "second");
    assert_eq!(captured_count(&captured), 2);
    // The second request carries the accumulated history (user + assistant + user).
    let requests = captured_requests(&captured);
    assert!(
        requests[1].messages.len() >= 3,
        "history grows across turns"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Interim persistence of a tool-call part
// ---------------------------------------------------------------------------

/// A tool step persists the interim assistant row with a completed `ToolCall`
/// part before the final save.
#[tokio::test]
async fn test_tool_step_records_completed_toolcall_part() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, _captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "think".to_string(),
                args: r#"{"thought": "persisted"}"#.to_string(),
            },
            ScriptedReply::Text("finished".to_string()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "persist",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn succeeds");

    assert!(matches!(
        reply.parts.iter().find_map(|p| match p {
            MessagePart::ToolCall { tool, state, .. } if tool == "think" => {
                Some(state.status.clone())
            }
            _ => None,
        }),
        Some(ToolCallStatus::Completed)
    ));
    Ok(())
}

// ---------------------------------------------------------------------------
// FR-007 (uniformtools T-012): the loop reaches a call's arguments through the
// one shared decoder, so a provider quirk no longer looks like a broken call.
// ---------------------------------------------------------------------------

/// Empty and whitespace-only argument text decodes to `{}`, so a parameterless
/// tool runs instead of failing - and it runs through the same gate a real
/// object does, not the corrective-error branch.
#[tokio::test]
async fn test_whitespace_arguments_run_the_tool_with_an_empty_object() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "tool_info".to_string(),
                args: "  \n\t ".to_string(),
            },
            ScriptedReply::Text("listed".to_string()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "list the tools",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("whitespace-argument turn succeeds");

    // The tool ran: `tool_info` takes no parameters and succeeds on `{}`.
    assert_eq!(
        captured_count(&captured),
        2,
        "the tool result is fed back for a second step"
    );
    assert_eq!(reply.text_content(), "listed");
    let state = reply
        .parts
        .iter()
        .find_map(|p| match p {
            MessagePart::ToolCall { tool, state, .. } if tool == "tool_info" => Some(state.clone()),
            _ => None,
        })
        .expect("the tool call is recorded");
    assert_eq!(
        state.status,
        ToolCallStatus::Completed,
        "whitespace arguments must decode to an empty object, not a corrective \
         error (FR-007): {:?}",
        state.error
    );
    assert_eq!(state.input, serde_json::json!({}));
    let seen = collect_events(&mut rx);
    assert!(seen.tool_call_end, "the call closes out");
    assert!(!seen.agent_error, "no corrective error is raised");
    Ok(())
}

/// A double-encoded argument payload (a JSON string holding the object) decodes
/// to that object, so the tool receives the arguments the server meant.
#[tokio::test]
async fn test_encoded_string_arguments_run_the_tool_decoded() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, _captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "think".to_string(),
                // The server put the argument object inside a string field.
                args: serde_json::to_string(r#"{"thought":"decoded"}"#).expect("encode"),
            },
            ScriptedReply::Text("thought about it".to_string()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "think",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("encoded-string turn succeeds");

    let state = reply
        .parts
        .iter()
        .find_map(|p| match p {
            MessagePart::ToolCall { tool, state, .. } if tool == "think" => Some(state.clone()),
            _ => None,
        })
        .expect("the tool call is recorded");
    assert_eq!(
        state.status,
        ToolCallStatus::Completed,
        "an encoded-string payload must decode to its object (FR-007): {:?}",
        state.error
    );
    assert_eq!(
        state.input,
        serde_json::json!({"thought": "decoded"}),
        "the tool received the decoded object, not the wire string"
    );
    Ok(())
}

/// A payload that is valid JSON but not an object is still refused with the
/// corrective error, so accepting more shapes did not make a bare array or a
/// number executable.
#[tokio::test]
async fn test_non_object_arguments_are_still_refused() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "tool_info".to_string(),
                args: "[1, 2]".to_string(),
            },
            ScriptedReply::Text("retried".to_string()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "list the tools",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("non-object turn succeeds");

    assert_eq!(
        captured_count(&captured),
        2,
        "the corrective error goes back to the model for a retry"
    );
    assert_eq!(reply.text_content(), "retried");
    let state = reply
        .parts
        .iter()
        .find_map(|p| match p {
            MessagePart::ToolCall { tool, state, .. } if tool == "tool_info" => Some(state.clone()),
            _ => None,
        })
        .expect("the refused call is recorded");
    assert_eq!(state.status, ToolCallStatus::Error);
    let error = state.error.expect("the correction is recorded");
    assert!(
        error.contains("an array"),
        "the corrective message names what was found (FR-008): {error}"
    );
    assert!(
        error.contains("tool_info"),
        "the corrective message names the tool: {error}"
    );
    assert!(
        error.contains("resend"),
        "the corrective message tells the model how to fix the call (FR-008): {error}"
    );
    assert_eq!(
        refusal_results(&captured, 1),
        vec![true],
        "the refused call's tool result is marked is_error (FR-014), not presented \
         as a completed call"
    );
    Ok(())
}

/// The FR-009 corrective error names the offending parameter and its declared
/// type, and the result it produces carries `is_error` (FR-014) - one refusal
/// value drives the sentence, the status and the flag, so the wire form and the
/// record cannot disagree.
#[tokio::test]
async fn test_schema_violation_names_the_parameter_and_is_marked_on_the_wire() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                // `move_file` requires both `source` and `destination`.
                name: "move_file".to_string(),
                args: r#"{"source": "a.txt"}"#.to_string(),
            },
            ScriptedReply::Text("fixed it".to_string()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "move the file",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("schema-violation turn succeeds");

    assert_eq!(captured_count(&captured), 2, "the correction is fed back");
    assert_eq!(reply.text_content(), "fixed it");
    let state = reply
        .parts
        .iter()
        .find_map(|p| match p {
            MessagePart::ToolCall { tool, state, .. } if tool == "move_file" => Some(state.clone()),
            _ => None,
        })
        .expect("the refused call is recorded");
    assert_eq!(
        state.status,
        ToolCallStatus::Error,
        "a call that never ran is an error"
    );
    let error = state.error.expect("the correction is recorded");
    assert!(
        error.contains("tool 'move_file'"),
        "the sentence names the tool (FR-009): {error}"
    );
    assert!(
        error.contains("'destination'"),
        "the sentence names the offending parameter (FR-009): {error}"
    );
    assert!(
        error.contains("string"),
        "the sentence states the declared type (FR-009): {error}"
    );
    assert!(
        !error.contains("Invalid tool arguments:"),
        "the sentence comes from the shared constructor, not a second wording: {error}"
    );
    assert_eq!(
        refusal_results(&captured, 1),
        vec![true],
        "the refused call's tool result is marked is_error (FR-014)"
    );
    Ok(())
}
