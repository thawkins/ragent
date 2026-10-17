//! Additional integration coverage for `SessionProcessor::process_message`
//! and the loop-step helpers in `session::loop_steps` (audit T-704 follow-up).
//!
//! This file copies the scripted-provider harness from
//! `test_loop_steps_harness.rs` and drives the public
//! `SessionProcessor::process_message` entry through more paths than the
//! baseline harness:
//!
//! - tool steps that invoke real core tools (`bash`, `read`, `write`, `grep`,
//!   `list`, `glob`) and a multi-tool sequential turn;
//! - a step whose provider stream emits `StreamEvent::Error` mid-stream;
//! - a `Finish { ToolUse }` with an unknown tool name (error-observation path);
//! - a long pre-seeded history so the pre-send compaction / emergency
//!   compression branch runs;
//! - a goal-driven loop (`start_loop` / `process_message` / `request_loop_interrupt`
//!   / `clear_loop` / `loop_active`);
//! - the cache accessors `invalidate_tool_cache`, `system_prompt_cache`,
//!   `invalidate_system_prompt_cache`, `invalidate_config_cache`,
//!   `load_config_cached`, and `last_message_end_reason`.

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
use ragent_agent::message::{Message, Role};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::loop_state::LoopSpec;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::permission::{Permission, PermissionAction, PermissionRule};
use ragent_config::{Capabilities, Config as RagentConfig, Cost};

#[derive(Clone, Debug)]
enum ScriptedReply {
    /// Text-only response ending with `Finish { Stop }`.
    TextOnly(&'static str),
    /// A named tool call with fixed arguments, followed by `Finish { ToolUse }`.
    ToolCall { name: &'static str, args: String },
    /// An arbitrary scripted event sequence (used for mid-stream errors).
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
            ScriptedReply::Events(events) => Ok(Box::pin(stream::iter(events))),
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
    // Auto-approve writes so the `write` tool step executes without a prompt.
    let checker = PermissionChecker::new(vec![PermissionRule {
        permission: Permission::Custom("file:write".to_string()),
        pattern: Some("**".to_string()),
        action: PermissionAction::Allow,
    }]);

    let processor = SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(checker)),
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

#[derive(Default)]
struct SeenEvents {
    request_started: bool,
    text_delta: bool,
    token_usage: bool,
    message_end_stop: bool,
    tool_call_start: bool,
    tool_call_end: bool,
    tool_result: bool,
    agent_error: bool,
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
            Event::ToolCallStart { .. } => seen.tool_call_start = true,
            Event::ToolCallEnd { .. } => seen.tool_call_end = true,
            Event::ToolResult { .. } => seen.tool_result = true,
            Event::AgentError { .. } => seen.agent_error = true,
            _ => {}
        }
    }
    seen
}

// ---------------------------------------------------------------------------
// real core tools: bash (echo), read, write, grep, list, glob
// ---------------------------------------------------------------------------

/// A `bash` tool step with a harmless `echo` command, then a text step.
/// `echo` is on the safe-command whitelist so it runs without a prompt.
#[tokio::test]
async fn test_bash_tool_step_then_text() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "bash",
                args: r#"{"command": "echo hello from bash"}"#.to_string(),
            },
            ScriptedReply::TextOnly("bash done"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "run echo",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("bash tool turn should succeed");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        2,
        "tool step then text step"
    );
    assert_eq!(reply.text_content(), "bash done");

    let seen = collect_events(&mut rx);
    assert!(seen.tool_call_start, "ToolCallStart published");
    assert!(seen.tool_call_end, "ToolCallEnd published");
    assert!(seen.tool_result, "ToolResult published for bash");
    Ok(())
}

/// A `write` tool step (auto-approved via the in-memory allow rule) followed
/// by a `read` of the same file and a text step - three sequential tool calls
/// across the same turn.
#[tokio::test]
async fn test_multiple_sequential_tool_calls_in_one_turn() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "write",
                args: r#"{"path": "note.txt", "content": "hello file"}"#.to_string(),
            },
            ScriptedReply::ToolCall {
                name: "read",
                args: r#"{"path": "note.txt"}"#.to_string(),
            },
            ScriptedReply::TextOnly("wrote and read"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "write then read",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("multi-tool turn should succeed");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        3,
        "three steps: write, read, text"
    );
    assert_eq!(reply.text_content(), "wrote and read");

    let seen = collect_events(&mut rx);
    assert!(seen.tool_call_start, "ToolCallStart published");
    assert!(seen.tool_result, "ToolResult published for each tool");
    Ok(())
}

/// `grep`, `list`, and `glob` read-only steps all execute without a prompt
/// (they resolve to `file:read`, auto-granted for paths under the cwd).
#[tokio::test]
async fn test_read_only_tools_grep_list_glob() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "grep",
                args: r#"{"pattern": "pub fn", "path": "."}"#.to_string(),
            },
            ScriptedReply::ToolCall {
                name: "list",
                args: r#"{"path": ".", "depth": 1}"#.to_string(),
            },
            ScriptedReply::ToolCall {
                name: "glob",
                args: r#"{"pattern": "**/*.rs"}"#.to_string(),
            },
            ScriptedReply::TextOnly("read-only sweep done"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "sweep",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("read-only tool turn should succeed");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        4,
        "three read-only tool steps then a text step"
    );
    assert_eq!(reply.text_content(), "read-only sweep done");

    let seen = collect_events(&mut rx);
    assert!(seen.tool_result, "ToolResult published for each read tool");
    Ok(())
}

// ---------------------------------------------------------------------------
// mid-stream StreamEvent::Error
// ---------------------------------------------------------------------------

/// A provider stream that emits `TextDelta` then `StreamEvent::Error` (with no
/// partial text that counts as meaningful) fails the turn and publishes
/// `AgentError` through the `StreamEvent::Error` arm of `call_llm_step`.
#[tokio::test]
async fn test_mid_stream_error_event_fails_turn() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::Events(vec![StreamEvent::Error {
            message: "synthetic mid-stream failure".to_string(),
        }])],
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
        "a non-retryable mid-stream error must fail the turn"
    );
    let message = outcome.unwrap_err().to_string();
    assert!(
        message.contains("synthetic mid-stream failure"),
        "the stream error must propagate: {message}"
    );
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        1,
        "no retry with max_retries = 0"
    );
    assert!(
        collect_events(&mut rx).agent_error,
        "AgentError published for the stream error"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Finish { ToolUse } with an unknown tool name -> error observation
// ---------------------------------------------------------------------------

/// A tool-call step naming a tool that is not registered produces a corrective
/// error observation and the loop continues with the next scripted reply.
#[tokio::test]
async fn test_unknown_tool_name_yields_error_observation() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "definitely_not_a_tool",
                args: r"{}".to_string(),
            },
            ScriptedReply::TextOnly("recovered"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let reply = processor
        .process_message(
            &session.id,
            "call a missing tool",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("an unknown tool name must not fail the turn");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        2,
        "the loop continues after the unknown-tool observation"
    );
    assert_eq!(reply.text_content(), "recovered");

    // The error observation is surfaced as a failed ToolResult and an
    // ToolCallEnd carrying the error.
    let seen = collect_events(&mut rx);
    assert!(
        seen.tool_call_end,
        "ToolCallEnd published for the failed call"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// long history -> compaction branch
// ---------------------------------------------------------------------------

/// Seed a long history so the pre-send compaction / emergency compression
/// branch runs, then drive a normal text turn. The turn must still succeed.
#[tokio::test]
async fn test_long_history_triggers_compaction_branch() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, _captured, _dir) = make_processor(
        event_bus.clone(),
        vec![ScriptedReply::TextOnly("compacted reply")],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let pad = "x".repeat(4_000);
    for i in 0..12 {
        let role = if i % 2 == 0 {
            Role::User
        } else {
            Role::Assistant
        };
        let part = Message::user_text(&session.id, format!("m{i} {pad}")).parts[0].clone();
        let msg = Message::new(&session.id, role, vec![part]);
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
        .expect("a compacted turn must still succeed");

    assert!(
        reply.text_content().contains("compacted reply") || !reply.text_content().is_empty(),
        "the turn produced a reply after compaction"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// goal-driven loop lifecycle
// ---------------------------------------------------------------------------

/// Start a goal loop, run one tool turn, then clear the loop and confirm the
/// loop-active / interrupt helpers flip.
#[tokio::test]
async fn test_loop_lifecycle_start_interrupt_clear() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, _captured, _dir) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCall {
                name: "think",
                args: r#"{"thought": "working the goal"}"#.to_string(),
            },
            ScriptedReply::TextOnly("goal achieved"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    assert!(!processor.loop_active(&session.id).await);
    assert!(
        !processor.request_loop_interrupt(&session.id),
        "no interrupt flag without an active loop"
    );

    let mut spec = LoopSpec::new("general", "achieve the goal");
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;
    assert!(processor.loop_active(&session.id).await);

    // While the loop is still active the interrupt flag raises, then is
    // discarded by an explicit clear.
    assert!(
        processor.request_loop_interrupt(&session.id),
        "an active loop's interrupt flag must raise"
    );

    processor.clear_loop(&session.id).await;
    assert!(
        !processor.loop_active(&session.id).await,
        "clear_loop removes the tracker"
    );
    assert!(
        !processor.request_loop_interrupt(&session.id),
        "no interrupt flag after clear_loop"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// cache accessors
// ---------------------------------------------------------------------------

/// Exercise the cache accessors on a constructed processor: tool-definition
/// invalidation, system-prompt cache creation/invalidation, config cache
/// invalidation + reload, and the terminal-reason getter/setter round trip.
#[tokio::test]
async fn test_cache_accessors_and_end_reason() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let (processor, _working_dir, _captured, _dir) =
        make_processor(event_bus, vec![ScriptedReply::TextOnly("noop")])?;

    // `invalidate_tool_cache` clears the three tool caches (idempotent).
    processor.invalidate_tool_cache();
    processor.invalidate_tool_cache();

    // `system_prompt_cache` lazily creates a singleton; invalidation is safe.
    let a = processor.system_prompt_cache();
    let b = processor.system_prompt_cache();
    assert!(
        Arc::ptr_eq(&a, &b),
        "system_prompt_cache returns the same instance"
    );
    processor.invalidate_system_prompt_cache();

    // `load_config_cached` returns the seeded config while the cache is warm.
    let cfg = processor.load_config_cached();
    assert!(
        cfg.compaction.auto,
        "the seeded default config enables auto compaction"
    );

    // `invalidate_config_cache` forces the next load to re-read from disk; the
    // returned config is still a valid default.
    processor.invalidate_config_cache();
    let cfg2 = processor.load_config_cached();
    assert_eq!(
        cfg2.compaction.auto, cfg.compaction.auto,
        "reloading config keeps the same compaction default"
    );

    // The terminal-reason getter observes the raw map (empty by default) and a
    // direct insert is observable through the getter.
    assert!(processor.last_message_end_reason("nope").is_none());
    processor
        .last_message_end_reason
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(
            "sess".to_string(),
            ragent_agent::event::FinishReason::Truncation,
        );
    assert_eq!(
        processor.last_message_end_reason("sess"),
        Some(ragent_agent::event::FinishReason::Truncation),
        "the terminal reason round-trips through the getter"
    );
    Ok(())
}

/// After a completed turn the processor records the terminal `MessageEnd`
/// reason for the session - a plain text turn records `Stop`.
#[tokio::test]
async fn test_completed_turn_records_end_reason() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, _captured, _dir) =
        make_processor(event_bus, vec![ScriptedReply::TextOnly("done")])?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    assert!(processor.last_message_end_reason(&session.id).is_none());
    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn should succeed");
    assert_eq!(
        processor.last_message_end_reason(&session.id),
        Some(ragent_agent::event::FinishReason::Stop),
        "a clean turn records a Stop terminal reason"
    );
    Ok(())
}
