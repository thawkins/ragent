//! Deep-path integration coverage for `SessionProcessor::process_message` /
//! `process_user_message` and the extracted loop steps in
//! `session::loop_steps` (coverage campaign: `processor.rs` + `loop_steps.rs`).
//!
//! This file copies the scripted-provider harness pattern from
//! `test_loop_steps_harness.rs` / `test_processor_more_paths.rs` (an external
//! `tests/` crate cannot call the `pub(crate)` loop-step methods, so every path
//! is driven through the public `process_message` / `process_user_message` /
//! `run_init_exchange` entry points) and targets branches the existing suites
//! (`test_processor_loop_paths.rs`, `test_processor_more_paths.rs`,
//! `test_loop_*`) do not reach:
//!
//! - the `max_steps <= 1` empty tool surface and the `step > max_steps`
//!   "Reached maximum steps" guard;
//! - `prepare_client` provider-not-found inside an active loop;
//! - the AGENTS.md init acknowledgement failing (provider error) and its
//!   several `run_init_exchange` early-return branches;
//! - a blocking `on_session_end` Stop hook feeding guidance back to the model,
//!   bounded by `MAX_STOP_CONTINUATIONS`;
//! - automatic `in_progress` -> `completed` spec task completion after a write
//!   inside the active spec's own directory;
//! - a silent end-of-stream recorded as `Truncation` with the interactive
//!   "response may be incomplete" notice;
//! - emergency overflow compaction triggered by a pre-stream `chat()` error
//!   (the sibling of the already-covered mid-stream overflow path).

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::{Event, EventBus, FinishReason};
use ragent_agent::llm::{ChatRequest, LlmClient, StreamEvent};
use ragent_agent::message::{Message, MessagePart, Role};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::loop_state::LoopSpec;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
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
    /// A single provider-level failure (the `chat()` call returns `Err`).
    Error(String),
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
            ScriptedReply::Error(message) => return Err(anyhow::anyhow!("{message}")),
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

// ---------------------------------------------------------------------------
// Event observation
// ---------------------------------------------------------------------------

#[derive(Default)]
struct SeenEvents {
    message_end_stop: bool,
    tools_sent: bool,
    agent_error: bool,
    agent_notice: bool,
    tool_result: bool,
    tool_call_end: bool,
}

fn collect_events(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> SeenEvents {
    let mut seen = SeenEvents::default();
    while let Ok(event) = rx.try_recv() {
        match event {
            Event::MessageEnd {
                reason: FinishReason::Stop,
                ..
            } => seen.message_end_stop = true,
            Event::ToolsSent { .. } => seen.tools_sent = true,
            Event::AgentError { .. } => seen.agent_error = true,
            Event::AgentNotice { .. } => seen.agent_notice = true,
            Event::ToolResult { .. } => seen.tool_result = true,
            Event::ToolCallEnd { .. } => seen.tool_call_end = true,
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

fn no_retry() -> StreamConfig {
    StreamConfig {
        max_retries: 0,
        retry_backoff_secs: 0,
        ..Default::default()
    }
}

/// Seed a session with a long alternating user/assistant history so compaction
/// has a non-empty head to summarise.
fn seed_long_history(storage: &Storage, session_id: &str) {
    let pad = "x".repeat(4_000);
    for i in 0..10 {
        let role = if i % 2 == 0 {
            Role::User
        } else {
            Role::Assistant
        };
        let part = Message::user_text(session_id, format!("message {i} {pad}")).parts[0].clone();
        let msg = Message::new(session_id, role, vec![part]);
        storage.create_message(&msg).expect("seed message");
    }
}

// ---------------------------------------------------------------------------
// max_steps == 1: empty wire surface + Reached-maximum-steps guard
// ---------------------------------------------------------------------------

/// An agent whose `max_steps` is 1 gets an EMPTY tool surface on the wire (the
/// `max_steps <= 1` branch) and no `ToolsSent` event.
#[tokio::test]
async fn test_max_steps_one_sends_no_tools() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) =
        make_plain(event_bus.clone(), vec![ScriptedReply::Text("short".into())])?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut agent = make_agent();
    agent.max_steps = Some(1);

    let reply = processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("single-step turn succeeds");
    assert_eq!(reply.text_content(), "short");

    let requests = captured_requests(&captured);
    assert_eq!(requests.len(), 1);
    assert!(
        requests[0].tools.is_empty(),
        "max_steps == 1 suppresses the wire tool surface"
    );
    assert!(
        !collect_events(&mut rx).tools_sent,
        "no ToolsSent when the surface is empty"
    );
    Ok(())
}

/// With `max_steps == 1` a tool-call reply dispatches once, then the next
/// iteration trips the `step > max_steps` guard and publishes `AgentError`.
#[tokio::test]
async fn test_max_steps_one_reached_maximum_steps_error() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::ToolCall {
            name: "think".into(),
            args: r#"{"thought": "one"}"#.into(),
        }],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut agent = make_agent();
    agent.max_steps = Some(1);

    let reply = processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("turn returns normally after the step cap");
    // Only one LLM call: the second iteration breaks before its request.
    assert_eq!(captured_count(&captured), 1);
    assert!(
        matches!(
            reply.parts.iter().find_map(|p| match p {
                MessagePart::ToolCall { tool, .. } if tool == "think" => Some(()),
                _ => None,
            }),
            Some(())
        ),
        "the executed tool call is recorded"
    );

    let seen = collect_events(&mut rx);
    assert!(
        seen.agent_error,
        "the step cap publishes an AgentError observation"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// prepare_client: provider not found (inside an active loop -> fatal)
// ---------------------------------------------------------------------------

/// A model that names an unregistered provider fails `prepare_client`; with an
/// active loop the failure terminates it with status `error`.
#[tokio::test]
async fn test_provider_not_found_fails_turn_and_loop() -> Result<()> {
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

    processor
        .start_loop(&session.id, LoopSpec::new("general", "goal"))
        .await;

    let mut agent = make_agent();
    agent.model = Some(ModelRef {
        provider_id: "not-a-provider".to_string(),
        model_id: "whatever".to_string(),
    });

    let outcome = processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await;
    assert!(
        outcome.is_err(),
        "an unknown provider fails the provider stage"
    );
    assert_eq!(
        captured_count(&captured),
        0,
        "no request without a provider"
    );
    assert!(collect_events(&mut rx).agent_error);
    assert!(
        !processor.loop_active(&session.id).await,
        "the fatal provider stage terminates the loop"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// AGENTS.md init acknowledgement
// ---------------------------------------------------------------------------

/// When the init acknowledgement request fails at the provider level, the
/// failure is logged and the main turn still runs.
#[tokio::test]
async fn test_init_ack_provider_error_is_skipped() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::Error("init transport failure".into()),
            ScriptedReply::Text("main reply".into()),
        ],
    )?;
    std::fs::write(working_dir.join("AGENTS.md"), "# Guidelines\n").expect("write AGENTS.md");
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
        .expect("the main turn runs despite the failed init ack");
    assert_eq!(reply.text_content(), "main reply");
    assert_eq!(
        captured_count(&captured),
        2,
        "one failed init call then the main request"
    );
    Ok(())
}

/// `run_init_exchange` short-circuits when no instruction files are discovered.
#[tokio::test]
async fn test_run_init_exchange_no_instruction_files() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Text("unused".into())],
    )?;
    // No AGENTS.md in the session directory.
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    processor
        .run_init_exchange(&session.id, &make_agent(), Arc::new(AtomicBool::new(false)))
        .await
        .expect("early return succeeds");
    assert_eq!(
        captured_count(&captured),
        0,
        "no request without guidelines"
    );
    assert!(
        collect_events(&mut rx).agent_notice,
        "the discovery summary is published"
    );
    Ok(())
}

/// `run_init_exchange` short-circuits when the session already has an assistant
/// turn.
#[tokio::test]
async fn test_run_init_exchange_skips_when_already_done() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Text("unused".into())],
    )?;
    std::fs::write(working_dir.join("AGENTS.md"), "# Guidelines\n").expect("write AGENTS.md");
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");
    // Seed a prior assistant turn so the init exchange is skipped.
    processor
        .session_manager
        .storage()
        .create_message(&Message::new(
            &session.id,
            Role::Assistant,
            vec![MessagePart::Text {
                text: "prior".to_string(),
            }],
        ))
        .expect("seed assistant message");

    processor
        .run_init_exchange(&session.id, &make_agent(), Arc::new(AtomicBool::new(false)))
        .await
        .expect("skip succeeds");
    assert_eq!(captured_count(&captured), 0, "no request when already done");
    Ok(())
}

/// `run_init_exchange` silently no-ops when the agent has no model configured.
#[tokio::test]
async fn test_run_init_exchange_no_model_noop() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Text("unused".into())],
    )?;
    std::fs::write(working_dir.join("AGENTS.md"), "# Guidelines\n").expect("write AGENTS.md");
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut agent = make_agent();
    agent.model = None;
    processor
        .run_init_exchange(&session.id, &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("no-model no-op succeeds");
    assert_eq!(captured_count(&captured), 0);
    Ok(())
}

/// `run_init_exchange` no-ops when the provider is not registered.
#[tokio::test]
async fn test_run_init_exchange_provider_not_found_noop() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Text("unused".into())],
    )?;
    std::fs::write(working_dir.join("AGENTS.md"), "# Guidelines\n").expect("write AGENTS.md");
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut agent = make_agent();
    agent.model = Some(ModelRef {
        provider_id: "missing".to_string(),
        model_id: "x".to_string(),
    });
    processor
        .run_init_exchange(&session.id, &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("provider-not-found no-op succeeds");
    assert_eq!(captured_count(&captured), 0);
    Ok(())
}

// ---------------------------------------------------------------------------
// Blocking Stop hook feeds guidance back
// ---------------------------------------------------------------------------

/// A blocking `on_session_end` Stop hook returns guidance that is fed back to
/// the model as a synthetic user turn; the feedback is bounded by
/// `MAX_STOP_CONTINUATIONS` (3), so a model that always replies with text runs
/// four LLM requests before the turn finally ends.
#[tokio::test]
async fn test_blocking_stop_hook_feeds_guidance_back() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let config: RagentConfig = serde_json::from_value(serde_json::json!({
        "hooks": [{
            "trigger": "on_session_end",
            "command": r#"printf '{"hookSpecificOutput":{"additionalContext":"please fix the issue"}}'"#
        }]
    }))
    .expect("config with a Stop hook");
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::Text("reply".into())],
        false,
        no_retry(),
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
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the hook loop still ends normally");
    // The reply accumulates one text part per continuation step, so it is the
    // scripted text repeated once per LLM request.
    assert!(
        reply.text_content().contains("reply"),
        "the final reply carries the model output: {:?}",
        reply.text_content()
    );

    // The model only ever replies with text; each no-tool response triggers the
    // Stop hook, whose guidance is fed back until the continuation bound stops
    // it. The last entry of the script repeats, so 1 + 3 continuations run.
    assert_eq!(
        captured_count(&captured),
        4,
        "1 initial + 3 bounded Stop-hook continuations"
    );

    let seen = collect_events(&mut rx);
    assert!(
        seen.agent_notice,
        "feeding findings back publishes an AgentNotice"
    );
    assert!(seen.message_end_stop);
    Ok(())
}

// ---------------------------------------------------------------------------
// Automatic spec-task completion after a spec-dir write
// ---------------------------------------------------------------------------

/// A write inside the active spec's own directory reaches the auto-complete
/// branch (the `writes_in_spec_dir` guard passes) and rewrites the spec
/// (`write_spec` normalises the SPEC.md frontmatter). The spec is seeded with
/// no frontmatter, so the rewrite is observable: `SPEC.md` gains a frontmatter
/// block only if the branch ran. The `in_progress` -> `completed` transition is
/// applied to the in-memory task list only, so the PLAN.md task table itself is
/// not rewritten by this path.
#[tokio::test]
async fn test_spec_task_auto_completed_after_spec_dir_write() -> Result<()> {
    use ragent_specs::{SpecIo, SpecManager};

    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "Write".into(),
                args: r#"{"path": "specs/testspec/notes.md", "content": "note"}"#.into(),
            },
            ScriptedReply::Text("wrote a spec note".into()),
        ],
        true,
        no_retry(),
        128_000,
        default_config(),
    )?;

    // Build a spec under `<working_dir>/specs` with one `in_progress` task and
    // deliberately NO frontmatter, so the auto-update's `write_spec` rewrite is
    // observable on disk.
    let specs_root = working_dir.join("specs");
    std::fs::create_dir_all(&specs_root).expect("specs root");
    let id = ragent_specs::spec::SpecId::new("testspec").expect("valid id");
    let spec_md = "# Test Spec\n";
    let plan_md = "# Plan\n\n## Tasks\n\n\
                   | ID | Title | Requirement | Effort | Priority | Status | Dependencies |\n\
                   |----|-------|-------------|--------|----------|--------|--------------|\n\
                   | T-001 | Do X | — | S | High | in_progress | — |\n";
    SpecIo::create_spec_dir(&specs_root, &id, spec_md, plan_md)
        .await
        .expect("create spec dir");
    let spec_path = specs_root.join("testspec").join("SPEC.md");
    assert!(
        !std::fs::read_to_string(&spec_path)
            .expect("read spec")
            .starts_with("---"),
        "the seeded SPEC.md has no frontmatter"
    );

    let manager = Arc::new(SpecManager::new(&specs_root));
    processor
        .spec_manager
        .set(Arc::clone(&manager))
        .map_err(|_| anyhow::anyhow!("spec_manager already set"))?;
    *processor.active_spec.write().await = Some("testspec".to_string());

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "work the spec",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("spec-dir write turn succeeds");
    assert_eq!(reply.text_content(), "wrote a spec note");
    assert_eq!(captured_count(&captured), 2);

    // The auto-complete branch ran `write_spec`, which normalises the
    // frontmatter: SPEC.md now carries a `---\nstatus: ...\n---` block.
    let spec_after = std::fs::read_to_string(&spec_path).expect("read spec after");
    assert!(
        spec_after.starts_with("---\nstatus:"),
        "the auto-complete branch rewrote SPEC.md frontmatter: {spec_after:?}"
    );
    Ok(())
}

/// A write OUTSIDE the active spec directory must NOT reach the auto-complete
/// branch (the `writes_in_spec_dir` guard fails), so the spec's SPEC.md is left
/// untouched - a frontmatter-less SPEC.md stays frontmatter-less.
#[tokio::test]
async fn test_spec_task_not_completed_for_unrelated_write() -> Result<()> {
    use ragent_specs::{SpecIo, SpecManager};

    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, _captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "Write".into(),
                args: r#"{"path": "scratch/notes.md", "content": "note"}"#.into(),
            },
            ScriptedReply::Text("wrote a scratch note".into()),
        ],
        true,
        no_retry(),
        128_000,
        default_config(),
    )?;

    let specs_root = working_dir.join("specs");
    std::fs::create_dir_all(&specs_root).expect("specs root");
    let id = ragent_specs::spec::SpecId::new("testspec").expect("valid id");
    let spec_md = "# Test Spec\n";
    let plan_md = "# Plan\n\n## Tasks\n\n\
                   | ID | Title | Requirement | Effort | Priority | Status | Dependencies |\n\
                   |----|-------|-------------|--------|----------|--------|--------------|\n\
                   | T-001 | Do X | — | S | High | in_progress | — |\n";
    SpecIo::create_spec_dir(&specs_root, &id, spec_md, plan_md)
        .await
        .expect("create spec dir");
    let spec_path = specs_root.join("testspec").join("SPEC.md");

    let manager = Arc::new(SpecManager::new(specs_root.clone()));
    processor
        .spec_manager
        .set(Arc::clone(&manager))
        .map_err(|_| anyhow::anyhow!("spec_manager already set"))?;
    *processor.active_spec.write().await = Some("testspec".to_string());

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    processor
        .process_message(
            &session.id,
            "scratch write",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("scratch-write turn succeeds");

    let spec_after = std::fs::read_to_string(&spec_path).expect("read spec after");
    assert!(
        !spec_after.starts_with("---"),
        "an unrelated write must not trigger the auto-complete rewrite: {spec_after:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Pre-stream provider error: retryable retry succeeds, permanent bails
// ---------------------------------------------------------------------------

/// A transient `chat()` failure (before any stream exists) is retried when the
/// retry budget allows, publishes the "will retry" notice, and the following
/// attempt succeeds.
#[tokio::test]
async fn test_pre_stream_transient_error_retries_and_succeeds() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::Error("connection reset by peer".into()),
            ScriptedReply::Text("recovered".into()),
        ],
        false,
        StreamConfig {
            max_retries: 1,
            retry_backoff_secs: 0,
            ..Default::default()
        },
        128_000,
        default_config(),
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
        .expect("the transient pre-stream error is retried to success");
    assert_eq!(reply.text_content(), "recovered");
    assert_eq!(captured_count(&captured), 2, "the error was retried once");
    assert!(
        collect_events(&mut rx).agent_notice,
        "the retry notice is published"
    );
    Ok(())
}

/// A permanent `chat()` failure (before any stream exists) is surfaced fatally
/// without retrying.
#[tokio::test]
async fn test_pre_stream_permanent_error_no_retry() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::Error(
            "invalid_request_error: model_not_supported".into(),
        )],
        false,
        StreamConfig {
            max_retries: 2,
            retry_backoff_secs: 0,
            ..Default::default()
        },
        128_000,
        default_config(),
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
    assert!(outcome.is_err(), "a permanent error fails the turn");
    assert_eq!(
        captured_count(&captured),
        1,
        "no retry for a permanent pre-stream error"
    );
    assert!(collect_events(&mut rx).agent_error);
    Ok(())
}

// ---------------------------------------------------------------------------
// Tool-repeat guard interactive prompt (primary run)
// ---------------------------------------------------------------------------

/// In an interactive (non-auto-approve) primary run, the sixth identical tool
/// call raises a `PermissionRequested` prompt; a spawned responder denies it, so
/// the guard surfaces a corrective observation and the loop continues.
#[tokio::test]
async fn test_tool_repeat_guard_prompts_and_user_denies() -> Result<()> {
    use ragent_agent::permission::PermissionDecision;

    let event_bus = Arc::new(EventBus::new(4096));
    let same_args = r#"{"thought": "same"}"#;
    // Six identical calls: the first five pass, the sixth prompts (count > 5).
    // After the denial the model sees the corrective observation and replies.
    let mut replies: Vec<ScriptedReply> = (0..6)
        .map(|_| ScriptedReply::ToolCall {
            name: "think".into(),
            args: same_args.into(),
        })
        .collect();
    replies.push(ScriptedReply::Text("guard prompted then finished".into()));

    // Interactive run: auto_approve off, no rules.
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        replies,
        false,
        no_retry(),
        128_000,
        default_config(),
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    // Deny the repeat-guard prompt as soon as it appears.
    let mut resp_rx = event_bus.subscribe();
    let sid = session.id.clone();
    let bus = event_bus.clone();
    tokio::spawn(async move {
        while let Ok(event) = resp_rx.recv().await {
            if let Event::PermissionRequested {
                request_id,
                session_id,
                permission,
                ..
            } = event
                && session_id == sid
                && permission == "tool:repeat"
            {
                bus.publish(Event::PermissionReplied {
                    session_id: sid.clone(),
                    request_id,
                    allowed: false,
                    decision: PermissionDecision::Deny,
                });
                // Keep answering: a denial does not reset the tracker, so a
                // further identical call would prompt again. Answering every
                // prompt keeps the run from stalling on the guard timeout.
            }
        }
    });

    let reply = processor
        .process_message(
            &session.id,
            "loop on the same call",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the repeat-guard turn succeeds");
    assert!(
        reply
            .text_content()
            .contains("guard prompted then finished")
    );
    // Five clean calls + the denied sixth call + the final text step.
    assert_eq!(
        captured_count(&captured),
        7,
        "the loop kept running after the denial"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// A tool that returns an error observation keeps the loop running
// ---------------------------------------------------------------------------

/// A real tool failure (reading a missing file) is recorded as an `Error`
/// tool-call part and its message is fed back as a corrective observation; the
/// loop then continues to the text step and the turn completes normally.
#[tokio::test]
async fn test_tool_error_observation_recorded_and_recovers() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_plain(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "read".into(),
                args: r#"{"path": "does-not-exist.txt"}"#.into(),
            },
            ScriptedReply::Text("recovered from the tool error".into()),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "read a missing file",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("a tool error does not fail the turn");
    assert_eq!(reply.text_content(), "recovered from the tool error");
    assert_eq!(captured_count(&captured), 2);

    // The failed call is recorded as an Error status tool-call part.
    assert!(
        matches!(
            reply.parts.iter().find_map(|p| match p {
                MessagePart::ToolCall { tool, state, .. } if tool == "read" => {
                    Some(state.status.clone())
                }
                _ => None,
            }),
            Some(ragent_agent::message::ToolCallStatus::Error)
        ),
        "the failed tool call is recorded with an Error status"
    );
    let seen = collect_events(&mut rx);
    assert!(seen.tool_result, "the error ToolResult is published");
    assert!(seen.tool_call_end, "the error ToolCallEnd is published");
    Ok(())
}

// ---------------------------------------------------------------------------
// Silent end-of-stream -> Truncation notice for an interactive session
// ---------------------------------------------------------------------------

/// An interactive turn whose stream ends silently (no `Finish`, non-empty text)
/// records the terminal reason as `Truncation` and publishes the visible
/// "response may be incomplete" notice.
#[tokio::test]
async fn test_silent_end_of_stream_truncation_notice() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir) = make_plain(
        event_bus.clone(),
        vec![ScriptedReply::Events(vec![
            StreamEvent::TextDelta {
                text: "a cut-off answer".into(),
            },
            StreamEvent::Usage {
                input_tokens: 10,
                output_tokens: 5,
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
        .expect("silent-end turn succeeds");
    assert_eq!(reply.text_content(), "a cut-off answer");
    assert_eq!(
        processor.last_message_end_reason(&session.id),
        Some(FinishReason::Truncation),
        "a silent end-of-stream is recorded as Truncation"
    );
    assert!(
        collect_events(&mut rx).agent_notice,
        "the truncation notice is published for an interactive session"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Emergency overflow compaction on a pre-stream chat() error
// ---------------------------------------------------------------------------

/// A `chat()` failure carrying a context-overflow message (before any stream is
/// established) triggers emergency overflow compaction and a single retry with
/// the compacted history. This is the pre-stream sibling of the mid-stream
/// overflow path covered by `test_compaction_integration.rs`.
#[tokio::test]
async fn test_pre_stream_overflow_error_compacts_and_retries() -> Result<()> {
    // `compaction.auto = false` so the pre-send path is skipped and the
    // emergency overflow path is exercised in isolation (FR-008).
    let config: RagentConfig =
        serde_json::from_str(r#"{"compaction":{"auto":false,"keep":{"tokens":0.8}}}"#)
            .expect("config");
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            // (1) real attempt: provider fails pre-stream with an overflow.
            ScriptedReply::Error(
                "This model's maximum context length is 4096 tokens. \
                 However, your message resulted in 9001 tokens."
                    .into(),
            ),
            // (2) the emergency summarisation call.
            ScriptedReply::Text("## Summary\n- prior work completed".into()),
            // (3) the retried real attempt (last entry repeats).
            ScriptedReply::Text("recovered after compaction".into()),
        ],
        false,
        StreamConfig {
            max_retries: 1,
            retry_backoff_secs: 0,
            ..Default::default()
        },
        200_000,
        config,
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");
    // Seed a long history so the emergency compaction has a non-empty head.
    seed_long_history(processor.session_manager.storage(), &session.id);

    let reply = processor
        .process_message(
            &session.id,
            "continue",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the overflow recovers and the turn succeeds");
    assert!(
        reply.text_content().contains("recovered after compaction"),
        "the retried request produced the final reply: {:?}",
        reply.text_content()
    );

    // Summarisation + retry: at least two provider calls were made.
    assert!(
        captured_count(&captured) >= 2,
        "expected an emergency summarisation call and a retry"
    );
    let requests = captured_requests(&captured);
    assert!(
        requests.iter().any(|r| r.tools.is_empty()),
        "the emergency summarisation request carries no tools"
    );
    Ok(())
}
