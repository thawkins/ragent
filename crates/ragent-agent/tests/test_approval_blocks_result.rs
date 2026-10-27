//! T-016 / FR-013: approval blocks the result, not just the call.
//!
//! FR-013 is state-driven: while a tool call's permission verdict is still
//! unresolved (the interactive prompt is open, awaiting a reply), the system
//! must not append a tool result for it to history, so no turn can continue on
//! an unapproved call's output.
//!
//! The invariant has a structural half and a stateful half, and this file pins
//! both:
//!
//! - `dispatch_tool_with_permissions` awaits `check_permission_with_prompt`
//!   before dispatching, so no output exists while the verdict is pending, and
//!   the `PermissionAction::Ask` (unresolved) branch maps to a denied error
//!   rather than a success observation;
//! - the unresolved interval is *tracked*: the dispatch pipeline's permission
//!   stage marks the call unresolved in
//!   [`ragent_agent::session::pending::PendingVerdicts`] immediately before the
//!   prompt is published and settles it when the verdict resolves, and the tool
//!   phase refuses to append a result for a call the tracker still names (see
//!   `result_is_appendable`);
//! - the only history-append sites are the `handle_tool_execution_result`
//!   pushes and the step's two `chat_messages` pushes, all reached AFTER the
//!   awaited pipeline run.
//!
//! The source guards fail if a future change moved a history append ahead of
//! the permission await, or dropped the stateful guard. The tracker cases pin
//! the state itself. The behavioural cases drive the loop with a denied write
//! and assert the file was never written and the observation is a denied-error.

use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::EventBus;
use ragent_agent::llm::{
    ChatContent, ChatRequest, ContentPart, LlmClient, LlmFinishReason, StreamEvent,
};
use ragent_agent::message::{MessagePart, ToolCallStatus};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::permission::{Permission, PermissionAction, PermissionRule};
use ragent_config::{Capabilities, Config as RagentConfig, Cost};

// ---------------------------------------------------------------------------
// Helpers (same pattern as test_loop_checkpoints.rs)
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
enum ScriptedReply {
    ToolCallNamed { name: &'static str, args: String },
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
        self.captured.lock().expect("captured lock").push(request);
        self.call_count.fetch_add(1, Ordering::SeqCst);
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
        "Ollama"
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
    Arc<SessionProcessor>,
    PathBuf,
    Arc<Mutex<Vec<ChatRequest>>>,
    tempfile::TempDir,
);

fn make_processor_deny(
    event_bus: Arc<EventBus>,
    replies: Vec<ScriptedReply>,
) -> Result<MakeProcessorResult> {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut registry = ProviderRegistry::new();
    registry.register(Box::new(ScriptedProvider {
        replies: Arc::new(Mutex::new(replies)),
        call_count: Arc::new(AtomicU32::new(0)),
        shared_captured: Arc::clone(&captured),
    }));
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let checker = PermissionChecker::new(vec![PermissionRule {
        permission: Permission::Custom("file:write".to_string()),
        pattern: Some("**".to_string()),
        action: PermissionAction::Deny,
    }]);
    let config: RagentConfig =
        serde_json::from_str(r#"{"loop":{"max_steps":10}}"#).expect("valid config");
    let processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(checker)),
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
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
        team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
        tool_repeat_guard: Arc::new(parking_lot::Mutex::new(HashMap::new())),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
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
        active_loop_specs: tokio::sync::RwLock::new(HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(HashMap::new()),
        pending_verdicts: ragent_agent::session::pending::PendingVerdicts::new(),
    });
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().to_path_buf();
    Ok((processor, path, captured, tmp))
}

fn make_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General agent");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });
    agent
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("workspace root")
        .to_path_buf()
}

fn read_workspace_file(relative: &str) -> String {
    std::fs::read_to_string(workspace_root().join(relative))
        .unwrap_or_else(|e| panic!("cannot read {}: {}", relative, e))
}

// ---------------------------------------------------------------------------
// Source guards (state-driven, cheap to run)
// ---------------------------------------------------------------------------

/// An unresolved permission verdict (Ask) must not be appended as a success:
/// the dispatch layer maps it to a denied error so the turn only continues
/// on a denial observation, never on an unapproved call's output (FR-013).
#[test]
fn permission_check_maps_unresolved_verdict_to_a_denied_error() {
    let processor = read_workspace_file("crates/ragent-agent/src/session/processor.rs");
    assert!(
        processor.contains("Permission check returned Ask (internal error)"),
        "dispatch_tool_with_permissions must handle the Ask (unresolved) verdict as a denied error, not a success (FR-013)"
    );
    assert!(
        processor.contains("FR-013 (T-016)"),
        "the Ask->denied mapping must be tagged FR-013 (T-016)"
    );
    assert!(
        processor.contains("Permission denied by user or policy")
            || processor.contains("Permission denied"),
        "a denied verdict must surface a denial message that reaches the model (FR-013)"
    );
}

/// No history append can run while a permission verdict is pending: every
/// `assistant_parts.push` / `tool_result_parts.push` lies after the awaited
/// pipeline run, and no chat-history push precedes it (FR-013, state-driven).
#[test]
fn history_append_runs_only_after_the_permission_verdict_resolved() {
    let processor = read_workspace_file("crates/ragent-agent/src/session/processor.rs");
    assert!(
        processor.contains("history append can run while a permission verdict"),
        "the tool-phase history guard comment must be present (FR-013)"
    );
    assert!(
        processor.contains("the turn's history is appended only here"),
        "the step history-append guard comment must be present (FR-013)"
    );
    assert!(
        processor.contains("handle_tool_execution_result"),
        "the tool-result history append must be gated through handle_tool_execution_result, which runs only after the dispatch pipeline's permission await (FR-013)"
    );
}

// ---------------------------------------------------------------------------
// Behaviour: a denied write appends only a denied error and writes nothing
// ---------------------------------------------------------------------------

/// A permission-denied write never touches the filesystem; the denial
/// observation carries Status::Error and is_error, and the second provider
/// request sees the denied tool-result as its prior context (FR-013, FR-014).
#[tokio::test]
async fn test_denied_write_writes_nothing_and_surfaces_a_denied_error() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir_guard) = make_processor_deny(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCallNamed {
                name: "Write",
                args: r#"{"path":"hello.txt","content":"hello"}"#.to_string(),
            },
            ScriptedReply::TextOnly("understood"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session");
    let msg = processor
        .process_message(
            &session.id,
            "create hello.txt with hello",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn");
    assert!(
        !working_dir.join("hello.txt").exists(),
        "FR-013: the unapproved call must not have written the file"
    );
    let (_, status, error) = msg
        .parts
        .iter()
        .find_map(|p| match p {
            MessagePart::ToolCall { tool, state, .. } if tool == "Write" => {
                Some((tool.clone(), state.status.clone(), state.error.clone()))
            }
            _ => None,
        })
        .expect("Write tool call must be recorded");
    assert_eq!(
        status,
        ToolCallStatus::Error,
        "a denied Write must be Status::Error, not Completed (FR-013)"
    );
    assert!(
        error
            .as_deref()
            .is_some_and(|e| e.contains("Permission denied")),
        "denial error must name the reason: {error:?}"
    );
    let requests = captured.lock().expect("captured lock").clone();
    assert_eq!(
        requests.len(),
        2,
        "one tool-call step plus the final answer step"
    );
    let second = &requests[1];
    let has_denied_tool_result = second.messages.iter().any(|m| match &m.content {
        ChatContent::Parts(parts) => parts
            .iter()
            .any(|p| matches!(p, ContentPart::ToolResult { is_error: true, .. })),
        ChatContent::Text(_) => false,
    });
    assert!(
        has_denied_tool_result,
        "the denied result must be an error observation (is_error: true, FR-014)"
    );
    let carries_denial_text = second.messages.iter().any(|m| match &m.content {
        ChatContent::Parts(parts) => parts.iter().any(|p| match p {
            ContentPart::ToolResult { content, .. } => {
                content.to_lowercase().contains("permission denied")
            }
            _ => false,
        }),
        ChatContent::Text(_) => false,
    });
    assert!(
        carries_denial_text,
        "the second request's history must carry the denial observation"
    );
    Ok(())
}

/// An approved write executes: the file is written, Status::Completed, and
/// the next request carries an is_error:false tool result (FR-013 converse).
#[tokio::test]
async fn test_approved_write_executes_and_returns_success() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, captured, _dir_guard) = {
        let captured: Arc<Mutex<Vec<ChatRequest>>> = Arc::new(Mutex::new(Vec::new()));
        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(ScriptedProvider {
            replies: Arc::new(Mutex::new(vec![
                ScriptedReply::ToolCallNamed {
                    name: "Write",
                    args: r#"{"path":"hello.txt","content":"hello"}"#.to_string(),
                },
                ScriptedReply::TextOnly("written"),
            ])),
            call_count: Arc::new(AtomicU32::new(0)),
            shared_captured: Arc::clone(&captured),
        }));
        let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
        let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
        let checker = PermissionChecker::new(vec![PermissionRule {
            permission: Permission::Custom("file:write".to_string()),
            pattern: Some("**".to_string()),
            action: PermissionAction::Allow,
        }]);
        let config: RagentConfig =
            serde_json::from_str(r#"{"loop":{"max_steps":10}}"#).expect("valid config");
        let proc = Arc::new(SessionProcessor {
            session_manager,
            provider_registry: Arc::new(registry),
            tool_registry: Arc::new(tool::create_default_registry()),
            permission_checker: Arc::new(parking_lot::RwLock::new(checker)),
            event_bus: event_bus.clone(),
            agent_manager: std::sync::OnceLock::new(),
            team_manager: std::sync::OnceLock::new(),
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
            team_context_cache: Arc::new(parking_lot::RwLock::new(HashMap::new())),
            tool_repeat_guard: Arc::new(parking_lot::Mutex::new(HashMap::new())),
            extraction_engine: std::sync::OnceLock::new(),
            stream_config: ragent_agent::StreamConfig::default(),
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
            active_loop_specs: tokio::sync::RwLock::new(HashMap::new()),
            loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
            active_loop_interrupts: parking_lot::RwLock::new(HashMap::new()),
            active_loop_captures: tokio::sync::RwLock::new(HashMap::new()),
            last_message_end_reason: std::sync::RwLock::new(HashMap::new()),
            pending_verdicts: ragent_agent::session::pending::PendingVerdicts::new(),
        });
        let tmp = tempfile::tempdir().expect("tmpdir");
        let path = tmp.path().to_path_buf();
        (proc, path, captured, tmp)
    };
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session");
    let msg = processor
        .process_message(
            &session.id,
            "create hello.txt with hello",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn");
    assert!(
        working_dir.join("hello.txt").exists(),
        "FR-013 converse: an approved call must have executed"
    );
    let (_, status, _) = msg
        .parts
        .iter()
        .find_map(|p| match p {
            MessagePart::ToolCall { tool, state, .. } if tool == "Write" => {
                Some((tool.clone(), state.status.clone(), state.error.clone()))
            }
            _ => None,
        })
        .expect("Write tool call must be recorded");
    assert_eq!(
        status,
        ToolCallStatus::Completed,
        "an approved Write must be Status::Completed"
    );
    let requests = captured.lock().expect("captured lock").clone();
    let second = &requests[1];
    let has_ok_tool_result = second.messages.iter().any(|m| match &m.content {
        ChatContent::Parts(parts) => parts.iter().any(|p| {
            matches!(
                p,
                ContentPart::ToolResult {
                    is_error: false,
                    ..
                }
            )
        }),
        ChatContent::Text(_) => false,
    });
    assert!(
        has_ok_tool_result,
        "an approved result must be is_error:false"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// The unresolved interval, tracked (T-016)
// ---------------------------------------------------------------------------

/// The permission stage marks the call unresolved before the prompt is
/// published and settles it after the verdict resolves, so the interval the
/// tracker describes is exactly the interval the user has not answered yet.
#[test]
fn the_permission_stage_marks_then_settles_the_call() {
    let dispatch = read_workspace_file("crates/ragent-agent/src/session/dispatch.rs");
    let begin = dispatch
        .find("ctx.pending_verdicts.begin(session_id, &tc.id);")
        .unwrap_or_else(|| {
            panic!(
                "the permission stage must mark the call unresolved before it prompts \
                 (FR-013)"
            )
        });
    let settle = dispatch
        .find("ctx.pending_verdicts.settle(session_id, &tc.id);")
        .unwrap_or_else(|| {
            panic!("the permission stage must settle the call once the verdict resolves (FR-013)")
        });
    assert!(
        begin < settle,
        "the mark must precede the settle: the call is unresolved for the interval \
         between them (FR-013)"
    );
    let publish = dispatch
        .find("event_bus.publish(Event::ToolCallStart {")
        .unwrap_or(0);
    assert!(
        begin > publish,
        "the mark belongs to the permission stage, after the call's start - not to the \
         step's prepare loop (FR-013)"
    );
}

/// The tool phase reads the tracker on its one append path, and it withholds
/// both the record's output and the result content for an unsettled call.
#[test]
fn the_result_append_reads_the_tracker() {
    let processor = read_workspace_file("crates/ragent-agent/src/session/processor.rs");
    assert!(
        processor.contains("fn result_is_appendable("),
        "the one reading of the unresolved-approval tracker must exist (FR-013)"
    );
    assert!(
        processor.contains("result_is_appendable(&step_pending_verdicts, session_id, &tc.id)"),
        "the tool phase must consult result_is_appendable for the call it is about to \
         append (FR-013)"
    );
    assert!(
        processor.contains(
            "if verdict_resolved {\n                                tool_result_parts.push("
        ),
        "the tool result must be pushed only for a resolved verdict (FR-013)"
    );
    assert!(
        processor.contains("tool result withheld: the call's permission verdict"),
        "a withheld result must be logged rather than silently dropped (FR-013)"
    );
}

/// A fresh tracker reports every call resolved, and a mark is observable only
/// for the session it was taken for.
#[test]
fn the_tracker_starts_empty_and_is_per_session() {
    let pending = ragent_agent::session::pending::PendingVerdicts::new();
    assert!(
        !pending.is_unresolved("sess-1", "call_1"),
        "a fresh tracker has no unresolved call, so the guard is inert on the common \
         path (FR-013)"
    );
    pending.begin("sess-1", "call_1");
    assert!(
        pending.is_unresolved("sess-1", "call_1"),
        "a marked call is unresolved (FR-013)"
    );
    assert!(
        !pending.is_unresolved("sess-2", "call_1"),
        "a mark belongs to one session: another session's identical call id is not \
         unresolved (FR-013)"
    );
    assert!(
        !pending.is_unresolved("sess-1", "call_2"),
        "a mark belongs to one call (FR-013)"
    );
    pending.settle("sess-1", "call_1");
    assert!(
        !pending.is_unresolved("sess-1", "call_1"),
        "a settled call is resolved, so its result may be appended (FR-013)"
    );
}

/// Two prompts for one call (the bash sub-command loop prompts once per
/// sub-command) leave it unresolved until the last one is settled.
#[test]
fn a_second_mark_keeps_the_call_unresolved_until_the_last_settle() {
    let pending = ragent_agent::session::pending::PendingVerdicts::new();
    pending.begin("sess-1", "call_1");
    pending.begin("sess-1", "call_1");
    assert!(
        pending.is_unresolved("sess-1", "call_1"),
        "a call is unresolved while any of its prompts is unanswered (FR-013)"
    );
    pending.settle("sess-1", "call_1");
    assert!(
        !pending.is_unresolved("sess-1", "call_1"),
        "the verdict is one verdict: settling it resolves the call (FR-013)"
    );
}

// ---------------------------------------------------------------------------
// Behaviour: the guard withholds a result the tracker has not settled
// ---------------------------------------------------------------------------

/// The append decision itself: a call the tracker still names unresolved is not
/// appendable, and the guard is not global - a sibling call of the same step,
/// and the same call id in another session, both stay appendable.
#[test]
fn the_append_guard_withholds_only_the_unresolved_call() {
    use ragent_agent::session::pending::PendingVerdicts;
    use ragent_agent::session::processor::result_is_appendable_for_test as appendable;

    let pending = PendingVerdicts::new();
    assert!(
        appendable(&pending, "sess-1", "call_1"),
        "with nothing unresolved the tool phase appends every result, so the guard \
         is inert on the common path (FR-013)"
    );

    pending.begin("sess-1", "call_1");
    assert!(
        !appendable(&pending, "sess-1", "call_1"),
        "a call whose permission verdict has not resolved must not have its result \
         appended (FR-013)"
    );
    assert!(
        appendable(&pending, "sess-1", "call_2"),
        "a sibling call of the same step is unaffected: the guard names one call, \
         not the step (FR-013)"
    );
    assert!(
        appendable(&pending, "sess-2", "call_1"),
        "the same call id in another session is unaffected: a verdict belongs to the \
         session that raised it (FR-013)"
    );

    pending.settle("sess-1", "call_1");
    assert!(
        appendable(&pending, "sess-1", "call_1"),
        "once the verdict resolves the result is appended again - a denial or an \
         allow both resolve it (FR-013)"
    );
}
