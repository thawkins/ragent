//! Integration tests for sub-agent cooperative suspension and resume
//! (spec `agentui` FR-035): `AgentManager::suspend_task` parks the live
//! agent loop at its next inter-step safe point - no further LLM request -
//! until `resume_task` continues it or `cancel_agent` cancels the turn.
//!
//! The parked-loop assertions drive a REAL `spawn_sync` against a scripted
//! provider (same fixture shape as `test_loop_interrupt.rs`), so the
//! manager, the processor-loop gate and the event flow are exercised end to
//! end. Status-transition unit cases (unknown task, wrong-status resume)
//! run against a seeded task map exactly as the `test_tool_task_adapters`
//! registry tests do.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use futures::FutureExt;
use futures::stream;
use ragent_agent::event::{Event, EventBus};
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::task::{AgentManager, TaskResult, TaskStatus};
use ragent_agent::tool;
use ragent_config::{Capabilities, Config as RagentConfig, Cost};
use ragent_types::{ThinkingConfig, ThinkingLevel};

#[path = "task_support/mod.rs"]
mod task_support;
use task_support::{entry_full, manager};

// -- Scripted provider (mirrors `test_loop_interrupt.rs`) --------------------

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
    shared_captured: Arc<Mutex<Vec<ChatRequest>>>,
}

struct ScriptedClient {
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
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
        // Hold the stream briefly before delivering events so the turn is
        // still live when the test task resumes and issues its suspend. The
        // scripted reply here is a tool call, so - even if the suspend lands
        // mid-flight - the next iteration's safe-point gate always parks the
        // loop before the second LLM request.
        std::thread::sleep(std::time::Duration::from_millis(50));
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
                thinking_levels: vec![],
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
            captured: Arc::clone(&self.shared_captured),
        }))
    }
}

// -- Processor / manager builders ---------------------------------------------

/// A scripted processor plus the agent manager bound into its
/// `agent_manager` slot and the captured-request handle.
async fn make_suspension_fixture(
    event_bus: Arc<EventBus>,
    replies: Vec<ScriptedReply>,
) -> (Arc<AgentManager>, Arc<Mutex<Vec<ChatRequest>>>) {
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        replies: Arc::new(Mutex::new(replies)),
        shared_captured: Arc::clone(&captured),
    }));

    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let config: RagentConfig =
        serde_json::from_str(r#"{"loop":{"max_steps":25}}"#).expect("valid config JSON");
    let processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![]))),
        event_bus: Arc::clone(&event_bus),
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
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
            std::collections::HashMap::new(),
        )),
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
        loop_telemetry_recorded: AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(HashMap::new()),
    });
    let mgr = Arc::new(AgentManager::new(
        Arc::clone(&event_bus),
        Arc::clone(&processor),
        4,
        300,
    ));
    assert!(
        processor.agent_manager.set(Arc::clone(&mgr)).is_ok(),
        "agent_manager slot is empty"
    );
    (mgr, captured)
}

/// Count how many times the scripted LLM client was invoked.
fn llm_call_count(captured: &Arc<Mutex<Vec<ChatRequest>>>) -> usize {
    captured.lock().expect("captured lock").len()
}

/// Poll until `done()` returns `Some`, or the deadline passes (30 s).
async fn poll_until<T>(mut done: impl FnMut() -> Option<T>, what: &str) -> T {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        if let Some(value) = done() {
            return value;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

/// Drain the bus until an event matching `matches` arrives (30 s budget).
async fn expect_event(
    rx: &mut tokio::sync::broadcast::Receiver<Event>,
    what: &str,
    mut matches: impl FnMut(&Event) -> bool,
) -> Event {
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        match rx.try_recv() {
            Ok(event) if matches(&event) => return event,
            Ok(_) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Empty) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Lagged(_)) => {}
            Err(tokio::sync::broadcast::error::TryRecvError::Closed) => {
                panic!("event bus closed while waiting for {what}")
            }
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for {what}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}

/// The `think` tool needs no permission prompt, so a scripted call executes
/// cleanly against the default registry (same fixture as `test_loop_interrupt`).
fn think_call() -> ScriptedReply {
    ScriptedReply::ToolCallNamed {
        name: "think",
        args: r#"{"thought": "considering the next step"}"#.to_string(),
    }
}

/// Spawn a live sub-agent turn on a separate task; a multi-threaded runtime
/// is required because the scripted client holds a worker for 50 ms per call.
fn spawn_turn(
    mgr: &Arc<AgentManager>,
    dir: &std::path::Path,
) -> tokio::task::JoinHandle<Result<TaskResult>> {
    let mgr = Arc::clone(mgr);
    let dir = dir.to_path_buf();
    tokio::spawn(async move {
        mgr.spawn_sync("parent-session", "general", "task", None, &dir)
            .await
    })
}

// -- Tests ---------------------------------------------------------------------

/// FR-035: suspending a running sub-agent parks the loop (no further LLM
/// request while parked), publishes `SubagentSuspended`, and `resume_task`
/// restores it - the turn then completes normally with `SubagentResumed`
/// published and the manager suspend flag cleaned up.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn suspend_then_resume_parks_and_continues_the_loop() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(256));
    let mut rx = event_bus.subscribe();
    let (mgr, captured) = make_suspension_fixture(
        event_bus,
        vec![
            think_call(),
            // Unreached after a mid-turn suspend: resume continues here.
            ScriptedReply::TextOnly("done"),
        ],
    )
    .await;
    let dir = tempfile::tempdir().expect("tempdir");

    let handle = spawn_turn(&mgr, dir.path());

    // The task entry is registered before the first LLM call, so the suspend
    // lands inside call 1 (or parks iteration 1 before it). The scripted
    // reply is a tool call, so the loop always reaches the next iteration's
    // gate - the park is race-free by construction.
    let task_id = poll_until(
        || {
            mgr.tasks_snapshot()
                .now_or_never()?
                .into_iter()
                .find(|e| e.status == TaskStatus::Running)
                .map(|e| e.id.clone())
        },
        "running task entry",
    )
    .await;
    mgr.suspend_task(&task_id)
        .await
        .expect("suspend a running task");

    let (entry, child_sid, frozen) = poll_until(
        || {
            mgr.tasks_snapshot()
                .now_or_never()?
                .into_iter()
                .find(|e| e.id == task_id && e.status == TaskStatus::Suspended)
                .map(|e| {
                    let child = e.child_session_id.clone();
                    (e, child, llm_call_count(&captured))
                })
        },
        "entry status to reach Suspended",
    )
    .await;
    let _ = entry;

    // While the flag is set, no further LLM request may be issued: wait past
    // the in-flight stage, snapshot the call count, re-check, assert frozen.
    tokio::time::sleep(std::time::Duration::from_millis(600)).await;
    assert_eq!(
        llm_call_count(&captured),
        frozen,
        "loop kept issuing LLM calls while suspended"
    );

    // The suspend event reached the bus.
    let suspended = expect_event(
        &mut rx,
        "SubagentSuspended",
        |e| matches!(e, Event::SubagentSuspended { task_id: tid, .. } if tid == &task_id),
    )
    .await;
    match &suspended {
        Event::SubagentSuspended {
            child_session_id, ..
        } => assert_eq!(child_session_id, &child_sid),
        _ => unreachable!("expect_event guarantees variant"),
    }

    // Resume: the parked loop continues and the turn completes (the second
    // scripted reply is TextOnly, so exactly one more LLM call is made).
    mgr.resume_task(&task_id)
        .await
        .expect("resume a suspended task");
    let result = tokio::time::timeout(std::time::Duration::from_secs(30), handle)
        .await
        .expect("turn completed after resume")
        .expect("spawn_sync join");
    let result = result.expect("spawn_sync ok");
    assert!(!result.response.is_empty(), "empty post-resume response");
    assert_eq!(result.entry.status, TaskStatus::Completed);
    // The resumed loop must have made progress (at least the final LLM
    // request that produced the response); the exact count depends on the
    // scripted tool's stop-guidance continuation, so only a floor holds.
    assert!(
        llm_call_count(&captured) > frozen,
        "resumed loop issued no further LLM calls"
    );
    let resumed = expect_event(
        &mut rx,
        "SubagentResumed",
        |e| matches!(e, Event::SubagentResumed { task_id: tid, .. } if tid == &task_id),
    )
    .await;
    match &resumed {
        Event::SubagentResumed {
            child_session_id, ..
        } => assert_eq!(child_session_id, &child_sid),
        _ => unreachable!("expect_event guarantees variant"),
    }

    // Cleanup removed the suspend flag with the completion bookkeeping.
    assert!(
        mgr.suspend_flag_for_session(&child_sid).is_none(),
        "suspend flag was not cleaned up after completion"
    );
    Ok(())
}

/// Cancelling a suspended task ends the parked turn immediately: the loop
/// breaks out of the park into the existing cancel branch, no further LLM
/// request is issued, and the suspend flag is removed with the completion
/// bookkeeping.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancel_while_suspended_ends_the_turn() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(256));
    let (mgr, captured) = make_suspension_fixture(
        event_bus,
        vec![think_call(), ScriptedReply::TextOnly("late")],
    )
    .await;
    let dir = tempfile::tempdir().expect("tempdir");

    let handle = spawn_turn(&mgr, dir.path());

    let task_id = poll_until(
        || {
            mgr.tasks_snapshot()
                .now_or_never()?
                .into_iter()
                .find(|e| e.status == TaskStatus::Running)
                .map(|e| e.id.clone())
        },
        "running task entry",
    )
    .await;
    mgr.suspend_task(&task_id)
        .await
        .expect("suspend a running task");
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    let calls_at_cancel = llm_call_count(&captured);
    mgr.cancel_agent(&task_id)
        .await
        .expect("cancel a suspended task");

    // The parked loop observes the set cancel flag within a 200 ms tick and
    // the turn ends promptly (the processor returns the empty cancelled-turn
    // message, so the sync spawn path reports an empty response, not an
    // error).
    let result = tokio::time::timeout(std::time::Duration::from_secs(15), handle)
        .await
        .expect("cancelled turn ended promptly")
        .expect("spawn_sync join");
    let result = result.expect("cancel path returns Ok");
    assert_eq!(
        llm_call_count(&captured),
        calls_at_cancel,
        "cancelled turn issued further LLM calls"
    );
    let child_sid = result.entry.child_session_id;
    assert!(
        mgr.suspend_flag_for_session(&child_sid).is_none(),
        "suspend flag was not cleaned up after cancellation"
    );
    Ok(())
}

/// Unknown task ids are rejected for both directions.
#[tokio::test]
async fn suspend_and_resume_unknown_task_errors() {
    let mgr = manager(4);
    let err = mgr
        .suspend_task("ghost")
        .await
        .expect_err("unknown task suspend");
    assert!(err.to_string().contains("not found"), "err: {err}");
    let err = mgr
        .resume_task("ghost")
        .await
        .expect_err("unknown task resume");
    assert!(err.to_string().contains("not found"), "err: {err}");
}

/// Status transitions: only a Running task may be suspended, only a
/// Suspended task may be resumed, and re-suspending an already-suspended
/// task is an idempotent no-op.
#[tokio::test]
async fn suspend_resume_transition_rules() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full(
        "finished",
        TaskStatus::Completed,
        "p",
        false,
        false,
    ))
    .await;
    mgr.seed_completed_for_test(entry_full(
        "parked",
        TaskStatus::Suspended,
        "p",
        false,
        false,
    ))
    .await;

    let err = mgr
        .suspend_task("finished")
        .await
        .expect_err("suspended a completed task");
    assert!(err.to_string().contains("not running"), "err: {err}");

    mgr.suspend_task("parked")
        .await
        .expect("re-suspend is an idempotent no-op");
    let err = mgr
        .resume_task("finished")
        .await
        .expect_err("resumed a non-suspended task");
    assert!(err.to_string().contains("not suspended"), "err: {err}");

    // Resuming a still-running (never parked) task is an idempotent no-op.
    mgr.seed_completed_for_test(entry_full("live", TaskStatus::Running, "p", false, false))
        .await;
    mgr.resume_task("live")
        .await
        .expect("resume of a running task is a no-op");
}
