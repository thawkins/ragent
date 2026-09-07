#![allow(clippy::assert_is_empty)]
//! Integration tests for per-loop tool-set and scope restriction (spec
//! `agentloop`, task T-009 / FR-008, FR-009, FR-021, FR-022):
//!
//! - a call outside the loop's tool set returns a denial observation
//!   (tool out of scope) and is not executed (FR-009),
//! - a write to a read-only-constrained path returns a denial observation
//!   and does not modify the file (FR-021),
//! - a file operation outside the configured scope boundaries returns a
//!   denial observation (FR-022),
//! - in-tool-set, in-scope calls execute normally.
//!
//! The mock provider scripts one named tool call followed by a text-only
//! reply, so the loop exercises exactly one restriction-relevant action per
//! scenario. Denial observations surface as tool errors in the conversation
//! and must not terminate the loop by themselves.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::AtomicU32;
use std::sync::atomic::Ordering;

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::AgentInfo;
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
use ragent_agent::session::loop_state::LoopSpec;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::Capabilities;
use ragent_config::Cost;
use ragent_types::ThinkingConfig;
use ragent_types::ThinkingLevel;

// --- Unit-level restriction tests (LoopSpec::deny_reason) -------------------

#[test]
fn test_deny_reason_out_of_tool_set_is_tool_out_of_scope() {
    let mut spec = LoopSpec::new("coder", "goal");
    spec.tool_set = vec!["read".to_string(), "grep".to_string()];
    let reason = spec
        .deny_reason("bash", &serde_json::json!({"command": "ls -la"}))
        .expect("bash is outside the tool set");
    assert!(reason.contains("tool out of scope"), "reason: {reason}");
    assert!(reason.contains("bash"), "reason names the tool: {reason}");
    assert!(
        reason.contains("outside the loop's tool set"),
        "reason explains the tool set: {reason}"
    );
}

#[test]
fn test_deny_reason_mandatory_safety_tools_stay_allowed() {
    let mut spec = LoopSpec::new("coder", "goal");
    spec.tool_set = vec!["read".to_string()];
    // FR-008: mandatory safety tools (think, memory) are always allowed even
    // under a restricted tool set.
    assert!(
        spec.deny_reason("think", &serde_json::json!({"thought": "x"}))
            .is_none()
    );
    assert!(
        spec.deny_reason("memory_store", &serde_json::json!({"content": "x"}))
            .is_none()
    );
    // Other tools remain denied.
    assert!(spec.deny_reason("list", &serde_json::json!({})).is_some());
}

#[test]
fn test_deny_reason_no_tool_set_allows_every_tool() {
    let spec = LoopSpec::new("coder", "goal");
    assert!(
        spec.deny_reason("bash", &serde_json::json!({"command": "cargo test"}))
            .is_none()
    );
    assert!(
        spec.deny_reason("write", &serde_json::json!({"path": "x.md"}))
            .is_none()
    );
}

#[test]
fn test_deny_reason_write_to_read_only_path() {
    let mut spec = LoopSpec::new("coder", "goal");
    spec.read_only = vec!["tests/**".to_string()];
    let reason = spec
        .deny_reason(
            "write",
            &serde_json::json!({"path": "tests/test_foo.rs", "content": "boom"}),
        )
        .expect("write to a read-only path is denied");
    assert!(reason.contains("read-only"), "reason: {reason}");
    assert!(
        reason.contains("constraint violation"),
        "reason labels the violation: {reason}"
    );
    // Read-only checks never deny reads.
    assert!(
        spec.deny_reason("read", &serde_json::json!({"path": "tests/test_foo.rs"}))
            .is_none()
    );
}

#[test]
fn test_deny_reason_edit_legacy_alias_and_multi_edit() {
    let mut spec = LoopSpec::new("coder", "goal");
    spec.read_only = vec!["docs/**".to_string()];
    // Canonical name.
    let canonical = spec.deny_reason(
        "multi_edit",
        &serde_json::json!({"edits": [{"file_path": "docs/guide.md", "old_string": "a", "new_string": "b"}]}),
    );
    assert!(canonical.is_some(), "multi_edit to read-only path denied");
    // Legacy alias registered under a different tool name.
    let legacy = spec.deny_reason(
        "multiedit",
        &serde_json::json!({"edits": [{"path": "docs/guide.md", "old_str": "a", "new_str": "b"}]}),
    );
    assert!(legacy.is_some(), "multiedit to read-only path denied");
}

#[test]
fn test_deny_reason_move_source_and_destination_checked() {
    let mut spec = LoopSpec::new("coder", "goal");
    spec.read_only = vec!["tests/**".to_string()];
    let reason = spec
        .deny_reason(
            "move_file",
            &serde_json::json!({"source": "src/a.rs", "destination": "tests/a.rs"}),
        )
        .expect("move INTO a read-only path is denied");
    assert!(
        reason.contains("tests/a.rs"),
        "reason names the target: {reason}"
    );
}

#[test]
fn test_deny_reason_scope_violation_on_file_operation() {
    let mut spec = LoopSpec::new("coder", "goal");
    spec.scope = vec!["src/**".to_string()];
    let reason = spec
        .deny_reason(
            "write",
            &serde_json::json!({"path": "target/out.txt", "content": "x"}),
        )
        .expect("write outside the scope is denied");
    assert!(reason.contains("scope violation"), "reason: {reason}");
    assert!(
        reason.contains("outside the loop's scope"),
        "reason explains the scope: {reason}"
    );
    // In-scope path passes.
    assert!(
        spec.deny_reason("write", &serde_json::json!({"path": "src/main.rs"}))
            .is_none()
    );
}

#[test]
fn test_deny_reason_scope_applies_to_read_operations_too() {
    let mut spec = LoopSpec::new("coder", "goal");
    spec.scope = vec!["src/**".to_string()];
    // FR-022 restricts loop file *access*, not only writes.
    let reason = spec
        .deny_reason("read", &serde_json::json!({"path": "/etc/passwd"}))
        .expect("out-of-scope read denied");
    assert!(reason.contains("scope violation"), "reason: {reason}");
}

#[test]
fn test_deny_reason_bash_sub_command_write_to_read_only_path() {
    let mut spec = LoopSpec::new("coder", "goal");
    spec.read_only = vec!["tests/**".to_string()];
    let reason = spec
        .deny_reason(
            "bash",
            &serde_json::json!({"command": "cargo test && rm tests/test_foo.rs"}),
        )
        .expect("bash rm of a read-only path is denied");
    assert!(reason.contains("read-only"), "reason: {reason}");
    // Reading commands are fine.
    assert!(
        spec.deny_reason(
            "bash",
            &serde_json::json!({"command": "ls tests/ && cargo test"})
        )
        .is_none()
    );
}

#[test]
fn test_deny_reason_bash_out_of_scope_path_token() {
    let mut spec = LoopSpec::new("coder", "goal");
    spec.scope = vec!["src/**".to_string()];
    let reason = spec
        .deny_reason("bash", &serde_json::json!({"command": "cat /etc/hostname"}))
        .expect("bash touching an out-of-scope path is denied");
    assert!(reason.contains("scope violation"), "reason: {reason}");
}

// --- Integration: the permission layer denies and observes ------------------

/// The mock returns one of these per `chat` call, in order; the last entry
/// repeats for any further calls.
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
        }))
    }
}

type MakeProcessorResult = (
    SessionProcessor,
    std::path::PathBuf,
    Arc<Mutex<Vec<ChatRequest>>>,
);

/// Build a processor whose provider replays `replies`.
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

/// FR-009: with `tool_set: ["read"]`, a scripted `bash` call is denied with a
/// "tool out of scope" observation, never executed, and the loop continues to
/// the text-only reply which completes the run normally.
#[tokio::test]
async fn test_out_of_tool_set_call_returns_denial_observation() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCallNamed {
                name: "bash",
                args: r#"{"command": "echo pwned > pwned.txt"}"#.to_string(),
            },
            ScriptedReply::TextOnly("gave up"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut spec = LoopSpec::new("general", "run a shell command");
    spec.tool_set = vec!["read".to_string()];
    processor.start_loop(&session.id, spec).await;

    let reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("process_message should finalise cleanly");

    // Exactly two requests: denial observation appended, loop continued, then
    // the text-only reply ended the run (completed).
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        2,
        "the denial observation must be appended and the loop must continue"
    );

    // The denial observation reached the model context / the tool part.
    let texts = tool_result_texts(&reply);
    assert!(
        texts.iter().any(|t| t.contains("tool out of scope")),
        "expected a tool-out-of-scope denial observation, got: {texts:?}"
    );

    // The denied tool never executed.
    assert!(
        !session.directory.join("pwned.txt").exists(),
        "the denied bash call must not execute"
    );

    // The loop terminated normally on the following text-only reply.
    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "loop terminated exactly once");
    let Event::LoopTerminated { status, .. } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, "completed");
    Ok(())
}

/// FR-021: with `read_only: ["tests/**"]`, a scripted `write` to a protected
/// path returns a read-only constraint observation and does not modify the
/// file. The loop continues and completes on the following text-only reply.
#[tokio::test]
async fn test_write_to_read_only_path_denied_with_observation() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCallNamed {
                name: "write",
                args: r#"{"path": "tests/protected.rs", "content": "boom"}"#.to_string(),
            },
            ScriptedReply::TextOnly("done instead"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    // The protected file pre-exists with known content.
    std::fs::create_dir_all(working_dir.join("tests")).expect("mkdir tests");
    std::fs::write(working_dir.join("tests/protected.rs"), "original").expect("seed file");

    let mut spec = LoopSpec::new("general", "fix the failing test");
    spec.read_only = vec!["tests/**".to_string()];
    processor.start_loop(&session.id, spec).await;

    let reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("process_message should finalise cleanly");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        2,
        "denial observation appended, then the loop continued to completion"
    );

    let texts = tool_result_texts(&reply);
    assert!(
        texts.iter().any(|t| t.contains("read-only")),
        "expected a read-only constraint observation, got: {texts:?}"
    );

    // The protected file is untouched.
    assert_eq!(
        std::fs::read_to_string(working_dir.join("tests/protected.rs")).expect("reread"),
        "original",
        "FR-021: the read-only file must not be modified"
    );

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "loop terminated exactly once");
    let Event::LoopTerminated { status, .. } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, "completed");
    Ok(())
}

/// FR-022: with `scope: ["src/**"]`, a scripted `read` outside the scope
/// returns a scope-violation observation. The loop continues and completes.
#[tokio::test]
async fn test_out_of_scope_file_operation_denied_with_observation() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCallNamed {
                name: "read",
                args: r#"{"path": "/etc/hostname"}"#.to_string(),
            },
            ScriptedReply::TextOnly("understood"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut spec = LoopSpec::new("general", "inspect the source");
    spec.scope = vec!["src/**".to_string()];
    processor.start_loop(&session.id, spec).await;

    let reply = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("process_message should finalise cleanly");

    assert_eq!(
        captured.lock().expect("captured lock").len(),
        2,
        "denial observation appended, then the loop continued to completion"
    );

    let texts = tool_result_texts(&reply);
    assert!(
        texts
            .iter()
            .any(|t| t.contains("scope violation") && t.contains("outside the loop's scope")),
        "expected a scope-violation observation, got: {texts:?}"
    );

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "loop terminated exactly once");
    let Event::LoopTerminated { status, .. } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, "completed");
    Ok(())
}

/// FR-008: with `tool_set: ["read"]`, the wire tool definitions offered to
/// the loop are restricted to the set plus the mandatory safety tools —
/// `bash` is not among them while `think` (always allowed) is.
#[tokio::test]
async fn test_tool_set_restricts_loop_tool_surface() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let (processor, working_dir, captured) =
        make_processor(event_bus.clone(), vec![ScriptedReply::TextOnly("ack")])?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut spec = LoopSpec::new("general", "list the source files");
    spec.tool_set = vec!["read".to_string()];
    processor.start_loop(&session.id, spec).await;

    let mut agent = make_agent();
    agent.mode = ragent_agent::agent::AgentMode::Subagent;
    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should finalise cleanly");

    let requests = captured.lock().expect("captured lock");
    let first = requests.first().expect("at least one request");
    let names: Vec<&str> = first.tools.iter().map(|tool| tool.name.as_str()).collect();
    assert!(
        !names.contains(&"bash"),
        "the loop's tool surface must exclude out-of-set tools, got: {names:?}"
    );
    assert!(
        names.contains(&"read"),
        "the configured tool set is included, got: {names:?}"
    );
    assert!(
        names.contains(&"think"),
        "mandatory safety tools stay available (FR-008), got: {names:?}"
    );
    Ok(())
}

/// Restriction denials are observations, not terminations: the tracker
/// survives a denied call and the loop's stop condition still fires on the
/// following text-only response (no extra LoopTerminated, no budget loss).
#[tokio::test]
async fn test_denial_observation_does_not_terminate_loop() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCallNamed {
                name: "write",
                args: r#"{"path": "tests/locked.rs", "content": "x"}"#.to_string(),
            },
            ScriptedReply::ToolCallNamed {
                name: "write",
                args: r#"{"path": "tests/locked.rs", "content": "y"}"#.to_string(),
            },
            ScriptedReply::TextOnly("done"),
            ScriptedReply::TextOnly("done"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    let mut spec = LoopSpec::new("general", "make the test pass");
    spec.read_only = vec!["tests/**".to_string()];
    processor.start_loop(&session.id, spec).await;

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("process_message should finalise cleanly");

    // Three requests: two denied observations, then the text-only completion.
    assert_eq!(
        captured.lock().expect("captured lock").len(),
        3,
        "denials are observations; the loop keeps iterating until the goal"
    );

    let terminated = drain_terminated(&mut rx);
    assert_eq!(terminated.len(), 1, "terminated exactly once");
    let Event::LoopTerminated {
        status, iterations, ..
    } = terminated.into_iter().next().expect("one event")
    else {
        unreachable!("filtered to LoopTerminated");
    };
    assert_eq!(status, "completed");
    assert_eq!(iterations, 3, "every iteration counted");
    Ok(())
}

/// FR-008 in the other direction: with no tool-set configured the loop's
/// requests offer the full registry (loop restrictions stay out of the way).
#[tokio::test]
async fn test_no_tool_set_offers_full_tool_surface() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let (processor, working_dir, captured) =
        make_processor(event_bus.clone(), vec![ScriptedReply::TextOnly("ack")])?;
    let session = processor
        .session_manager
        .create_session(working_dir)
        .expect("session created");

    processor
        .start_loop(&session.id, LoopSpec::new("general", "explore"))
        .await;

    let mut agent = make_agent();
    agent.mode = ragent_agent::agent::AgentMode::Subagent;
    processor
        .process_message(&session.id, "go", &agent, Arc::new(AtomicBool::new(false)))
        .await
        .expect("process_message should finalise cleanly");

    let requests = captured.lock().expect("captured lock");
    let first = requests.first().expect("at least one request");
    let names: Vec<&str> = first.tools.iter().map(|tool| tool.name.as_str()).collect();
    assert!(
        names.contains(&"bash"),
        "without a tool set every tool is offered, got: {names:?}"
    );
    Ok(())
}

/// Denials must apply even under auto-approve (FR-024): the loop restriction
/// is structural, not a prompt decision.
#[tokio::test]
async fn test_denial_applies_under_auto_approve() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(64));
    let (mut processor, working_dir, _captured) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCallNamed {
                name: "write",
                args: r#"{"path": "tests/blocked.rs", "content": "x"}"#.to_string(),
            },
            ScriptedReply::TextOnly("done"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    std::fs::create_dir_all(working_dir.join("tests")).expect("mkdir tests");

    let mut spec = LoopSpec::new("general", "write the test");
    spec.read_only = vec!["tests/**".to_string()];
    processor.start_loop(&session.id, spec).await;

    // Auto-approve mode: the permission layer would allow everything.
    processor.auto_approve = true;

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
        texts.iter().any(|t| t.contains("read-only")),
        "the denial must apply in auto-approve mode (FR-024), got: {texts:?}"
    );
    assert!(
        !working_dir.join("tests/blocked.rs").exists(),
        "the denied write must not execute even under auto-approve"
    );
    Ok(())
}
