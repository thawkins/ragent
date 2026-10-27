//! End-to-end coverage for the one ordered dispatch pipeline (spec
//! `uniformtools` FR-012, FR-021; T-014).
//!
//! FR-012 requires every tool call - sequential or concurrent - to run through
//! one pipeline whose stages are `decode -> schema -> unknown-tool -> ... ->
//! permission -> backend`, and FR-021 requires an unregistered name to execute
//! nothing while naming the closest registered name when one is close enough.
//!
//! These cases pin what a caller can observe:
//!
//! * an unknown name never executes anything, is recorded as an error, and its
//!   corrective result offers the closest registered name;
//! * an unknown name with no near neighbour says so without inventing a
//!   suggestion;
//! * the pipeline's stages are ordered, not merely present: a call refused by
//!   the schema never reaches the pre-tool hooks or the permission prompt;
//! * the same scripted step produces the same sequence of outcomes whether the
//!   session runs the calls sequentially or concurrently - so the two paths
//!   cannot drift apart;
//! * the agent loop itself duplicates no stage: it calls the one stage body, and
//!   that body names each stage exactly once, in order (source guards).
//!
//! The stage *ordering* lives in `src/session/dispatch.rs`, which the loop now
//! calls once per call for both arms: the sequential arm awaits the pipeline's
//! future and the concurrent arm collects it into `join_all`, so neither can run
//! a stage the other does not. The source guards below read that module and the
//! loop's single call site; the behavioural cases pin what a caller observes.
//! See `specs/uniformtools/T-014-DISPATCH-PIPELINE.md`.
//!
//! Every case uses offline tools (`think`, `calculator`) or a tool that refuses
//! before executing, so nothing here touches the network or writes to the
//! repository.

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
use ragent_agent::message::{Message, MessagePart, ToolCallStatus};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::permission::{Permission, PermissionAction, PermissionRule};
use ragent_config::{Capabilities, Config as RagentConfig, Cost, StreamConfig};

const PROVIDER_ID: &str = "ollama";
const MODEL_ID: &str = "qwen3:latest";

// ---------------------------------------------------------------------------
// Scripted provider / client
// ---------------------------------------------------------------------------

/// One scripted turn: either a text reply, or one-or-more tool calls.
#[derive(Clone, Debug)]
enum ScriptedReply {
    /// Text-only response ending with `Finish { Stop }`.
    Text(String),
    /// One named tool call followed by `Finish { ToolUse }`.
    ToolCall { name: String, args: String },
    /// Several tool calls in a single streamed step, then `Finish { ToolUse }`.
    Tools(Vec<(String, String)>),
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

fn tool_call_events(id: &str, name: &str, args: &str) -> Vec<StreamEvent> {
    vec![
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
        let mut events: Vec<StreamEvent> = match &reply {
            ScriptedReply::Text(text) => vec![StreamEvent::TextDelta { text: text.clone() }],
            ScriptedReply::ToolCall { name, args } => tool_call_events("call_1", name, args),
            ScriptedReply::Tools(calls) => {
                let mut events = Vec::new();
                for (i, (name, args)) in calls.iter().enumerate() {
                    events.extend(tool_call_events(&format!("call_{i}"), name, args));
                }
                events
            }
        };
        events.push(StreamEvent::Usage {
            input_tokens: 10,
            output_tokens: 5,
        });
        let finish = match &reply {
            ScriptedReply::Text(_) => LlmFinishReason::Stop,
            ScriptedReply::ToolCall { .. } | ScriptedReply::Tools(_) => LlmFinishReason::ToolUse,
        };
        events.push(StreamEvent::Finish { reason: finish });
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

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Harness {
    processor: SessionProcessor,
    working_dir: std::path::PathBuf,
    captured: Arc<Mutex<Vec<ChatRequest>>>,
    _dir: tempfile::TempDir,
}

/// Builds a processor with the given scripted replies and config JSON.
///
/// `rules` grant the permission categories the cases need; `auto_approve`
/// controls whether an `Ask` verdict would prompt (it is `false` in every case
/// that asserts a prompt was *not* raised, so a missing prompt is evidence that
/// the pipeline never reached the permission stage).
fn make_processor(
    event_bus: Arc<EventBus>,
    replies: Vec<ScriptedReply>,
    rules: Vec<PermissionRule>,
    auto_approve: bool,
    config_json: &str,
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
    let config: RagentConfig = serde_json::from_str(config_json).expect("valid config JSON");

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
        stream_config: StreamConfig {
            max_retries: 0,
            retry_backoff_secs: 0,
            ..Default::default()
        },
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
    Ok(Harness {
        processor,
        working_dir: tmp.path().to_path_buf(),
        captured,
        _dir: tmp,
    })
}

fn make_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: PROVIDER_ID.to_string(),
        model_id: MODEL_ID.to_string(),
    });
    agent
}

/// The recorded status and error text of the first tool-call part.
fn first_tool_call_state(reply: &Message) -> Option<(String, ToolCallStatus, Option<String>)> {
    reply.parts.iter().find_map(|part| match part {
        MessagePart::ToolCall { tool, state, .. } => {
            Some((tool.clone(), state.status.clone(), state.error.clone()))
        }
        _ => None,
    })
}

/// Drains the bus and reports whether any permission prompt was published.
fn saw_permission_prompt(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> bool {
    count_permission_requests(rx) > 0
}

/// Drains the bus and counts the permission prompts published on it.
fn count_permission_requests(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> usize {
    let mut count = 0;
    while let Ok(event) = rx.try_recv() {
        if matches!(event, Event::PermissionRequested { .. }) {
            count += 1;
        }
    }
    count
}

// ---------------------------------------------------------------------------
// FR-021: an unknown tool name never executes, and offers the closest name
// ---------------------------------------------------------------------------

/// A hallucinated name close to a registered one executes nothing, is recorded
/// as an error, and its corrective result offers the registered spelling.
#[tokio::test]
async fn test_unknown_tool_is_refused_with_a_did_you_mean_hint() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let harness = make_processor(
        event_bus,
        vec![
            ScriptedReply::ToolCall {
                name: "movefile".to_string(),
                args: r#"{"source": "a.txt", "destination": "b.txt"}"#.to_string(),
            },
            ScriptedReply::Text("recovered".to_string()),
        ],
        Vec::new(),
        false,
        "{}",
    )?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");

    let reply = harness
        .processor
        .process_message(
            &session.id,
            "rename it",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("an unknown tool name must not fail the turn");

    assert_eq!(
        reply.text_content(),
        "recovered",
        "the refusal is an observation the model can act on"
    );
    let (tool, status, error) =
        first_tool_call_state(&reply).expect("the refused call is still recorded");
    assert_eq!(tool, "movefile", "the record names what the model sent");
    assert_eq!(
        status,
        ToolCallStatus::Error,
        "a refused call is recorded as an error (FR-014)"
    );
    let message = error.unwrap_or_default();
    assert!(
        message.contains("Unknown tool: 'movefile'"),
        "the refusal names the unknown tool: {message}"
    );
    assert!(
        message.contains("Did you mean 'move_file'?"),
        "the refusal offers the closest registered name: {message}"
    );
    assert!(
        message.contains("not executed"),
        "the refusal says nothing ran: {message}"
    );

    // Nothing ran: neither the rename nor any other tool.
    assert!(
        !harness.working_dir.join("b.txt").exists(),
        "the unknown tool must not execute"
    );
    assert_eq!(
        harness.captured.lock().expect("captured lock").len(),
        2,
        "one tool step and one text step: the loop continued"
    );
    Ok(())
}

/// A name with no near neighbour is refused without inventing a suggestion.
#[tokio::test]
async fn test_unknown_tool_with_no_near_neighbour_offers_no_suggestion() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let harness = make_processor(
        event_bus,
        vec![
            ScriptedReply::ToolCall {
                name: "zwxyz".to_string(),
                args: "{}".to_string(),
            },
            ScriptedReply::Text("recovered".to_string()),
        ],
        Vec::new(),
        false,
        "{}",
    )?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");

    let reply = harness
        .processor
        .process_message(
            &session.id,
            "do something odd",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn continues");

    let (_, status, error) = first_tool_call_state(&reply).expect("the call is recorded");
    assert_eq!(status, ToolCallStatus::Error);
    let message = error.unwrap_or_default();
    assert!(
        message.contains("Unknown tool: 'zwxyz'"),
        "the refusal names the tool: {message}"
    );
    assert!(
        !message.contains("Did you mean"),
        "nothing is close enough, so no suggestion is invented: {message}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// FR-012: the stages are ordered, not merely present
// ---------------------------------------------------------------------------

/// A call that violates the tool's schema is refused *before* the pre-tool hooks
/// and *before* the permission prompt: the hook's marker file is never written
/// and no `PermissionRequested` is published, even though the call's category
/// would have prompted.
#[tokio::test]
async fn test_schema_violation_stops_before_hooks_and_permissions() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let marker_dir = tempfile::tempdir().expect("marker dir");
    let marker = marker_dir.path().join("hook-ran");
    // A `pre_tool_use` hook that records that it ran, for every tool.
    let config = serde_json::json!({
        "hooks": [{
            "trigger": "pre_tool_use",
            "command": format!("touch {}", marker.display()),
            "timeout_secs": 5,
        }]
    })
    .to_string();
    let harness = make_processor(
        event_bus,
        vec![
            // `move_file` requires both `source` and `destination`; the schema
            // refusal must stop the call before either guard below runs.
            ScriptedReply::ToolCall {
                name: "move_file".to_string(),
                args: r#"{"source": "a.txt"}"#.to_string(),
            },
            ScriptedReply::Text("recovered".to_string()),
        ],
        // No rules and no auto-approve: any `Ask` would raise a prompt.
        Vec::new(),
        false,
        &config,
    )?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");

    let reply = harness
        .processor
        .process_message(
            &session.id,
            "rename it",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn continues");

    let (_, status, error) = first_tool_call_state(&reply).expect("the call is recorded");
    assert_eq!(status, ToolCallStatus::Error);
    let message = error.unwrap_or_default();
    assert!(
        message.contains("tool 'move_file'") && message.contains("'destination'"),
        "the schema refusal names the tool and the offending parameter: {message}"
    );

    assert!(
        !marker.exists(),
        "the PreToolUse hook must not run for a call the schema already refused"
    );
    assert!(
        !saw_permission_prompt(&mut rx),
        "the permission stage must not be reached for a call the schema already refused"
    );
    Ok(())
}

/// An unknown name is refused before the permission prompt too, even when the
/// session has no rules and is not auto-approving.
#[tokio::test]
async fn test_unknown_tool_stops_before_the_permission_prompt() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let harness = make_processor(
        event_bus,
        vec![
            ScriptedReply::ToolCall {
                name: "reed".to_string(),
                args: r#"{"path": "notes.txt"}"#.to_string(),
            },
            ScriptedReply::Text("recovered".to_string()),
        ],
        Vec::new(),
        false,
        "{}",
    )?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");

    harness
        .processor
        .process_message(
            &session.id,
            "read it",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn continues");

    assert!(
        !saw_permission_prompt(&mut rx),
        "an unknown tool must be refused before the user is asked to approve it"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// FR-012: sequential and concurrent execution reach the same outcomes
// ---------------------------------------------------------------------------

/// Runs one scripted step of three calls - one that runs, one that violates its
/// schema, one that names no registered tool - and returns the per-call
/// `(tool, status, error)` sequence.
async fn run_mixed_step(parallel: bool) -> Result<Vec<(String, ToolCallStatus, Option<String>)>> {
    let event_bus = Arc::new(EventBus::new(4096));
    let config = serde_json::json!({
        "experimental": { "parallel_tool_calls": parallel }
    })
    .to_string();
    let harness = make_processor(
        event_bus,
        vec![
            ScriptedReply::Tools(vec![
                (
                    "calculator".to_string(),
                    r#"{"expression": "1+1"}"#.to_string(),
                ),
                (
                    "move_file".to_string(),
                    r#"{"source": "a.txt"}"#.to_string(),
                ),
                ("definitely_not_a_tool".to_string(), "{}".to_string()),
            ]),
            ScriptedReply::Text("done".to_string()),
        ],
        // `calculator` declares category `none`; nothing needs a prompt.
        Vec::new(),
        true,
        &config,
    )?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");

    let reply = harness
        .processor
        .process_message(
            &session.id,
            "run three things",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the step runs to completion in both modes");

    let mut outcomes: Vec<(String, ToolCallStatus, Option<String>)> = reply
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::ToolCall { tool, state, .. } => {
                Some((tool.clone(), state.status.clone(), state.error.clone()))
            }
            _ => None,
        })
        .collect();
    // The pipeline preserves provider ordering in both modes; sort by name only
    // so a scheduling difference cannot mask a stage difference.
    outcomes.sort_by(|a, b| a.0.cmp(&b.0));
    Ok(outcomes)
}

/// The same step produces the same per-call outcomes sequentially and
/// concurrently: one success, one schema refusal, one unknown-tool refusal, and
/// the refusals carry the same sentences in both modes.
#[tokio::test]
async fn test_sequential_and_parallel_paths_produce_the_same_outcomes() -> Result<()> {
    let sequential = run_mixed_step(false).await?;
    let parallel = run_mixed_step(true).await?;

    assert_eq!(
        sequential.len(),
        3,
        "all three calls are recorded in the sequential mode: {sequential:?}"
    );
    assert_eq!(
        parallel.len(),
        3,
        "all three calls are recorded in the concurrent mode: {parallel:?}"
    );

    let statuses = |outcomes: &[(String, ToolCallStatus, Option<String>)]| {
        outcomes
            .iter()
            .map(|(tool, status, _)| (tool.clone(), status.clone()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        statuses(&sequential),
        statuses(&parallel),
        "the two execution modes must reach the same verdict for every call"
    );
    assert_eq!(
        sequential[0].1,
        ToolCallStatus::Completed,
        "calculator ran: {:?}",
        sequential[0]
    );
    assert_eq!(
        sequential[1].1,
        ToolCallStatus::Error,
        "move_file was refused by the schema: {:?}",
        sequential[1]
    );
    assert_eq!(
        sequential[2].1,
        ToolCallStatus::Error,
        "the unknown tool was refused: {:?}",
        sequential[2]
    );

    let refusal = |outcomes: &[(String, ToolCallStatus, Option<String>)], tool: &str| {
        outcomes
            .iter()
            .find(|(name, ..)| name == tool)
            .and_then(|(.., error)| error.clone())
            .unwrap_or_default()
    };
    for tool in ["move_file", "definitely_not_a_tool"] {
        assert_eq!(
            refusal(&sequential, tool),
            refusal(&parallel, tool),
            "the {tool} refusal sentence must be identical in both modes"
        );
    }
    assert!(
        refusal(&parallel, "definitely_not_a_tool").contains("Unknown tool"),
        "the unknown-tool refusal survives the concurrent path"
    );
    Ok(())
}

/// The outcomes come back in the provider's order in *both* modes, including a
/// refusal at the head of the step.
///
/// `test_sequential_and_parallel_paths_produce_the_same_outcomes` compares the
/// two modes after sorting by tool name, which is the right comparison for
/// "same verdict" - but it deliberately cannot see an ordering difference. The
/// concurrent arm used to answer a refused call in the prepare loop, where its
/// record was appended while its siblings were still running, so a step whose
/// first call was refused produced two different histories depending on
/// `parallel_tool_calls`. This case asserts the order directly, unsorted.
///
/// The step is built so that "appended where it was decided" and "appended when
/// it completed" both differ from the expected order: the refused call is first
/// and the two tools that run are slow (`bash_reset` takes the session's shell),
/// so the refusals' records would come out last rather than first. The case is
/// a source guard *as well as* a behavioural one, because the arms are
/// distinguishable only in timing: it asserts that the concurrent arm feeds the
/// step's slots into one drain - a single `join_all` over every call of the step
/// - and that a refused call pushes a settled slot instead of calling the record
/// handler inline.
#[tokio::test]
async fn test_outcomes_keep_the_provider_order_in_both_modes() -> Result<()> {
    /// A tool that takes long enough for a completion-order append to differ
    /// from the provider's order: it waits on the session's persistent shell.
    const SLOW_TOOL: &str = "bash_reset";
    for parallel in [false, true] {
        let event_bus = Arc::new(EventBus::new(4096));
        let config = serde_json::json!({
            "experimental": { "parallel_tool_calls": parallel }
        })
        .to_string();
        // Deliberately neither alphabetical nor in completion order: a call
        // refused by its schema, then two slow ones that run.
        let harness = make_processor(
            event_bus,
            vec![
                ScriptedReply::Tools(vec![
                    (
                        "move_file".to_string(),
                        r#"{"source": "a.txt"}"#.to_string(),
                    ),
                    (SLOW_TOOL.to_string(), "{}".to_string()),
                    (SLOW_TOOL.to_string(), "{}".to_string()),
                ]),
                ScriptedReply::Text("done".to_string()),
            ],
            Vec::new(),
            true,
            &config,
        )?;
        let session = harness
            .processor
            .session_manager
            .create_session(harness.working_dir.clone())
            .expect("session created");

        let reply = harness
            .processor
            .process_message(
                &session.id,
                "run three things in order",
                &make_agent(),
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .expect("the step runs to completion in both modes");

        let recorded: Vec<(String, ToolCallStatus)> = reply
            .parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::ToolCall { tool, state, .. } => {
                    Some((tool.clone(), state.status.clone()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            recorded.len(),
            3,
            "every call of the step is recorded (parallel_tool_calls = {parallel}): \
             {recorded:?}"
        );
        assert_eq!(
            recorded[0],
            ("move_file".to_string(), ToolCallStatus::Error),
            "the refused call is recorded first, where the provider listed it \
             (parallel_tool_calls = {parallel}): {recorded:?}"
        );
        assert_eq!(
            recorded[1].0, SLOW_TOOL,
            "the second call keeps its position (parallel_tool_calls = \
             {parallel}): {recorded:?}"
        );
        assert_eq!(
            recorded[2].0, SLOW_TOOL,
            "the third call keeps its position (parallel_tool_calls = \
             {parallel}): {recorded:?}"
        );
    }
    Ok(())
}

/// The concurrent arm applies the step's outcomes in the provider's order, and
/// it does so by draining one list of slots rather than by handling each result
/// where it was produced (FR-012).
///
/// This is the half of the ordering rule a behavioural case cannot pin from the
/// outside: with three fast calls the two shapes happen to agree, so only the
/// source shows that the refused call occupies its own position in the list the
/// drain walks.
#[test]
fn parallel_arm_drains_one_ordered_list_of_slots() {
    let loop_source = read_workspace_file("crates/ragent-agent/src/session/processor.rs");
    // The step is a list in the provider's order, holding either a future (a
    // call that runs) or an outcome already decided (a refused call).
    assert!(
        loop_source.contains("type PendingStepCall =")
            && loop_source.contains("Result<StepOutcomeSlot, Box<ToolStepResult>>"),
        "the concurrent arm must keep one slot per call of the step, in the \
         provider's order (FR-012)"
    );
    // A refused call pushes its settled outcome into that list...
    assert!(
        loop_source.contains("futures.push(Err(Box::new(("),
        "a refused call must occupy its position in the step's slot list rather \
         than append its record where it was decided (FR-012)"
    );
    // ...and the drain walks the whole list once, after every call of the step
    // has been prepared, so no record is written ahead of its neighbours.
    assert!(
        loop_source.contains("join_all(futures.into_iter().map(|slot|"),
        "the concurrent arm must drain the step's slots in one ordered pass \
         (FR-012)"
    );
    assert!(
        loop_source.contains("for (result, stalled_tool, panic_msg, _order) in results"),
        "the drained outcomes must be applied in the list's order (FR-012)"
    );
}

// ---------------------------------------------------------------------------
// Source guards: one loop call site, one pipeline, stages in order
// ---------------------------------------------------------------------------

fn workspace_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(std::path::Path::parent)
        .expect("crates/<name> sits two levels below the workspace root")
        .to_path_buf()
}

fn read_workspace_file(relative: &str) -> String {
    let path = workspace_root().join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The agent loop calls the pipeline exactly once and names no stage of its own,
/// so the sequential and concurrent shapes provably share every stage (FR-012).
#[test]
fn agent_loop_calls_the_one_stage_body_and_duplicates_no_refusal() {
    let loop_source = read_workspace_file("crates/ragent-agent/src/session/processor.rs");
    // Stages 1-3 (decode, resolve, validate) have exactly one implementation,
    // in the pipeline module, and the loop reaches it through one call rather
    // than inlining a second. One definition, and one call site in the loop.
    let pipeline_source = read_workspace_file("crates/ragent-agent/src/session/dispatch.rs");
    let definitions = pipeline_source.matches("fn resolve_call(").count();
    assert_eq!(
        definitions, 1,
        "the decode/resolve/validate stages must have exactly one definition \
         (FR-012); found {definitions}"
    );
    assert_eq!(
        loop_source.matches("fn resolve_call(").count(),
        0,
        "the agent loop must not carry its own copy of the decode/resolve/\
         validate stages (FR-012)"
    );
    let calls = loop_source
        .lines()
        .filter(|line| line.contains("resolve_tool_call("))
        .count();
    assert_eq!(
        calls, 1,
        "the agent loop must reach the decode/resolve/validate stages through \
         exactly one call to `resolve_tool_call` (FR-012); found {calls}"
    );
    // Every stage after those three has one owner too: the loop calls the
    // pipeline once per call and states no stage of its own.
    let pipeline_calls = loop_source
        .lines()
        .filter(|line| line.contains("ToolDispatchPipeline::run("))
        .count();
    assert_eq!(
        pipeline_calls, 1,
        "the agent loop must run every call through exactly one pipeline \
         entry point (FR-012); found {pipeline_calls}"
    );
    // The three ways a call stops without a tool result are built by the
    // pipeline's constructors, not re-assembled in the loop.
    for constructor in ["fn refusal(", "fn denial(", "fn synthesised("] {
        let pipeline_uses = pipeline_source.matches(constructor).count();
        assert!(
            pipeline_uses > 0,
            "the dispatch pipeline must build its `{constructor}` outcomes \
             through its own constructor (FR-012, FR-014)"
        );
    }
    assert!(
        loop_source.contains("ToolDispatchPipeline::refusal(")
            || loop_source.contains("ToolDispatchPipeline::synthesised("),
        "the agent loop answers a refused or failed call with the pipeline's \
         constructor rather than an inline tuple (FR-012)"
    );
    for duplicate in [
        "fn denied_tool_call(",
        "fn unknown_tool_error(",
        "synthetic_error_result",
    ] {
        assert!(
            !loop_source.contains(duplicate),
            "the agent loop must not carry its own `{duplicate}`; the one \
             pipeline shapes every terminal outcome (FR-012)"
        );
    }
}

/// The pipeline's guards run in the documented order. A reordering is a
/// behaviour change - for example running hooks before the loop restriction
/// would execute a subprocess for a call the loop already refused - so the order
/// is asserted rather than left to review.
#[test]
fn pipeline_stages_run_in_the_documented_order() {
    let source = read_workspace_file("crates/ragent-agent/src/session/dispatch.rs");
    // The guards live in `run`; the first three stages are computed by
    // `resolve_call` before it and are asserted separately below.
    let body_start = source
        .find("pub(crate) async fn run(")
        .expect("the pipeline entry point");
    let body_end = source
        .find("pub(crate) fn denial(")
        .expect("the refusal constructor that follows `run`");
    let body = &source[body_start..body_end];

    let stages = [
        ("loop restriction", "deny_reason("),
        ("plan gate", "plan_gate_denial("),
        ("interactive block", "is_interactive_tool("),
        ("pre-tool hooks", "run_pre_tool_use_hooks_decoded("),
        ("repeat guard", "check_tool_repeat_guard("),
        ("security analyzer", "analyze_tool_action("),
        ("permission and backend", "dispatch_tool_with_permissions("),
        ("post-tool hooks", "run_post_tool_use_hooks_decoded("),
    ];
    let mut last: Option<(&str, usize)> = None;
    for (label, needle) in stages {
        let position = body.find(needle).unwrap_or_else(|| {
            panic!("the dispatch pipeline no longer calls `{needle}` ({label})")
        });
        if let Some((previous_label, previous_position)) = last {
            assert!(
                position > previous_position,
                "the dispatch pipeline must run `{previous_label}` before `{label}` \
                 (FR-012); found them at {previous_position} and {position}"
            );
        }
        last = Some((label, position));
    }
}

/// The unknown-tool refusal sits between the decode and the schema check, so an
/// unregistered name is refused without ever being validated against a schema
/// that does not exist - and it happens before any guard.
#[test]
fn unknown_tool_refusal_precedes_every_guard() {
    let source = read_workspace_file("crates/ragent-agent/src/session/dispatch.rs");
    // The first three stages: decode, resolve (the unknown-tool refusal), and
    // the schema validation.
    let stages_start = source
        .find("pub fn resolve_call(")
        .expect("the first-three-stages entry point");
    let stages_end = source
        .find("pub fn suggest_tool_name(")
        .expect("the similarity helper that follows `resolve_call`");
    let stages = &source[stages_start..stages_end];

    let decode = stages
        .find("decode_tool_arguments(")
        .expect("the decode call");
    let unknown = stages
        .find("ToolInputError::unknown_tool(")
        .expect("the unknown-tool refusal");
    let schema = stages
        .find("ToolInputError::schema_violation(")
        .expect("the schema refusal");
    assert!(
        decode < unknown,
        "the arguments are decoded before the tool is resolved"
    );
    assert!(
        unknown < schema,
        "the unknown-tool refusal precedes the schema check it would invalidate \
         (FR-021)"
    );
    assert!(
        !source.contains("unknown_tool_error("),
        "the refusal is built by the shared ToolInputError constructor, not a \
         second ad-hoc error (FR-008, FR-009, FR-021)"
    );

    // And the loop computes those stages before it can reach a guard: the call
    // to `resolve_call` sits ahead of the pipeline entry point that runs the
    // guards, and the loop no longer names a guard of its own.
    let loop_source = read_workspace_file("crates/ragent-agent/src/session/processor.rs");
    let resolve_position = loop_source
        .find("resolve_tool_call(")
        .expect("the loop resolves a call before dispatching it");
    let pipeline_position = loop_source
        .find("ToolDispatchPipeline::run(")
        .expect("the loop runs the one pipeline");
    assert!(
        resolve_position < pipeline_position,
        "the loop must decode/resolve/validate a call before the pipeline's \
         guards run (FR-012, FR-021)"
    );
    assert!(
        !loop_source.contains("crate::session::permissions::check_tool_repeat_guard(")
            && !loop_source.contains("check_tool_repeat_guard("),
        "the repeat guard must run in the pipeline, not in the loop body \
         (FR-012)"
    );
}

// ---------------------------------------------------------------------------
// A tool call whose spelling differs only by letter case still reaches the tool
// ---------------------------------------------------------------------------

/// `calculator` named `CALCULATOR`, with its argument key spelled `EXPRESSION`.
///
/// A provider that capitalises a name or a parameter used to have the call
/// refused as an unknown tool, and the argument key used to be refused as
/// missing. Both now resolve onto the registered spelling, and the record keeps
/// the canonical name so later stages key on one vocabulary.
#[tokio::test]
async fn test_tool_name_and_argument_key_case_variants_still_dispatch() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let harness = make_processor(
        event_bus,
        vec![
            ScriptedReply::ToolCall {
                name: "CALCULATOR".to_string(),
                args: r#"{"EXPRESSION": "6*7"}"#.to_string(),
            },
            ScriptedReply::Text("done".to_string()),
        ],
        Vec::new(),
        true,
        "{}",
    )?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");

    let reply = harness
        .processor
        .process_message(
            &session.id,
            "compute",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("a case-variant tool name must not fail the turn");

    let (tool, status, error) = first_tool_call_state(&reply).expect("the call is recorded");
    assert_eq!(status, ToolCallStatus::Completed, "the tool ran: {error:?}");
    assert_eq!(
        tool, "calculator",
        "the record is written with the registered spelling, so the permission \
         layer and the history agree with the registry"
    );
    // Both the name and the argument key were folded onto the registered
    // spelling by the ONE pipeline the call ran through: not by a second loop
    // stage, and not by the tool body guessing.
    assert!(
        error.is_none(),
        "a folded name and key reach the tool with nothing to report: {error:?}"
    );
    let result_text = reply
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::ToolCall { state, .. } => state.output.clone(),
            _ => None,
        })
        .map(|output| match output {
            serde_json::Value::String(text) => text,
            other => other.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        result_text.contains("42"),
        "the folded argument key reached the tool body: {result_text}"
    );
    Ok(())
}

/// A registered name used verbatim still resolves, and the canonical rewrite is
/// inert for it - the exact lookup is tried first and is never re-spelled.
#[tokio::test]
async fn test_an_exact_tool_name_is_not_rewritten() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let harness = make_processor(
        event_bus,
        vec![
            ScriptedReply::ToolCall {
                name: "calculator".to_string(),
                args: r#"{"expression": "1+1"}"#.to_string(),
            },
            ScriptedReply::Text("done".to_string()),
        ],
        Vec::new(),
        true,
        "{}",
    )?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");

    let reply = harness
        .processor
        .process_message(
            &session.id,
            "compute",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the turn runs");

    let (tool, status, _) = first_tool_call_state(&reply).expect("the call is recorded");
    assert_eq!(tool, "calculator");
    assert_eq!(status, ToolCallStatus::Completed);
    Ok(())
}

/// A name that matches nothing even after folding is still refused, so the
/// case-insensitive fallback cannot turn a hallucinated name into a call.
#[tokio::test]
async fn test_an_unmatched_name_is_still_refused_after_folding() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let harness = make_processor(
        event_bus,
        vec![
            ScriptedReply::ToolCall {
                name: "ZWXYZ".to_string(),
                args: "{}".to_string(),
            },
            ScriptedReply::Text("recovered".to_string()),
        ],
        Vec::new(),
        true,
        "{}",
    )?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness.working_dir.clone())
        .expect("session created");

    let reply = harness
        .processor
        .process_message(
            &session.id,
            "do something odd",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("an unknown tool name must not fail the turn");

    let (tool, status, error) = first_tool_call_state(&reply).expect("the call is recorded");
    assert_eq!(tool, "ZWXYZ", "the record names what the model sent");
    assert_eq!(status, ToolCallStatus::Error);
    let message = error.unwrap_or_default();
    assert!(
        message.contains("Unknown tool: 'ZWXYZ'"),
        "the refusal still names the spelling the model sent: {message}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// FR-012: the permission stage is inside the pipeline, in both modes
// ---------------------------------------------------------------------------

/// The permission stage is a stage of the one pipeline, so the same call raises
/// the same `PermissionRequested` and the same verdict in both modes.
///
/// `test_sequential_and_parallel_paths_produce_the_same_outcomes` compares the
/// *recorded outcomes* of two modes. That leaves one gap: a mode could reach the
/// identical final state through a different route - for instance one arm
/// dispatching straight to a backend while the other consulted the permission
/// checker - and the outcomes would still match. This case closes the gap by
/// observing the *events*: exactly one prompt per call, in both modes.
///
/// Each prompt is answered as soon as it is published, so the case costs one
/// permission round trip per call rather than one 120-second timeout per call.
#[tokio::test]
async fn test_the_permission_stage_runs_once_per_call_in_both_modes() -> Result<()> {
    for parallel in [false, true] {
        let event_bus = Arc::new(EventBus::new(4096));
        let mut rx = event_bus.subscribe();
        let config = serde_json::json!({
            "experimental": { "parallel_tool_calls": parallel }
        })
        .to_string();
        // An `Ask` verdict prompts, and a prompt nobody answers is held for the
        // full 120-second permission timeout - four prompts (two modes, two
        // calls each) would stall this case for eight minutes. Answer each
        // prompt as it is published; the count below is unaffected because the
        // counter reads the request events, not the replies.
        let answer_bus = event_bus.clone();
        let responder = tokio::spawn(async move {
            let mut replies_rx = answer_bus.subscribe();
            loop {
                match replies_rx.recv().await {
                    Ok(Event::PermissionRequested {
                        session_id,
                        request_id,
                        ..
                    }) => answer_bus.publish(Event::PermissionReplied {
                        session_id,
                        request_id,
                        allowed: false,
                        decision: ragent_agent::permission::PermissionDecision::Once,
                    }),
                    Ok(_) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(_) => break,
                }
            }
        });
        let harness = make_processor(
            event_bus,
            vec![
                ScriptedReply::Tools(vec![
                    (
                        "move_file".to_string(),
                        r#"{"source": "a.txt", "destination": "b.txt"}"#.to_string(),
                    ),
                    (
                        "move_file".to_string(),
                        r#"{"source": "c.txt", "destination": "d.txt"}"#.to_string(),
                    ),
                ]),
                ScriptedReply::Text("done".to_string()),
            ],
            // An `Ask` verdict for the write category `move_file` declares,
            // with `auto_approve: false`, holds each call for a prompt: the
            // prompt's presence is evidence the permission stage ran, and its
            // absence would be evidence a mode skipped it.
            vec![PermissionRule {
                permission: Permission::Custom("file:write".to_string()),
                pattern: Some("**".to_string()),
                action: PermissionAction::Ask,
            }],
            false,
            &config,
        )?;
        let session = harness
            .processor
            .session_manager
            .create_session(harness.working_dir.clone())
            .expect("session created");

        let reply = harness
            .processor
            .process_message(
                &session.id,
                "move two files",
                &make_agent(),
                Arc::new(AtomicBool::new(false)),
            )
            .await
            .expect("the step runs to completion in both modes");
        responder.abort();

        let prompts = count_permission_requests(&mut rx);
        let states: Vec<_> = reply
            .parts
            .iter()
            .filter_map(|part| match part {
                MessagePart::ToolCall { tool, state, .. } => {
                    Some((tool.clone(), state.status.clone()))
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            states,
            vec![
                ("move_file".to_string(), ToolCallStatus::Error),
                ("move_file".to_string(), ToolCallStatus::Error),
            ],
            "both calls are held by the permission stage and refused while \
             unattended (parallel_tool_calls = {parallel})"
        );
        assert_eq!(
            prompts, 2,
            "the permission stage ran exactly once per call whether the step \
             ran sequentially or concurrently (parallel_tool_calls = {parallel})"
        );
    }
    Ok(())
}
