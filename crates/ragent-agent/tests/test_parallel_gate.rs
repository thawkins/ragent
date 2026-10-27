//! FR-010 (uniformtools T-015): sequential execution when the provider cannot
//! parallelise.
//!
//! While the active provider profile declares `supports_parallel == false`, the
//! system executes the step's tool calls sequentially even when
//! `experimental.parallel_tool_calls` is true, and logs one `tracing::info!`
//! naming the provider and the reason.
//!
//! The gate is a *demotion* of the user's opt-in, not a change to any call: the
//! sequential arm is the same ordered pipeline with the same recorded order
//! (FR-012), so a demoted step produces the history it would have produced
//! anyway. These cases pin the three things a caller can observe:
//!
//! * a parallelising provider keeps the concurrent arm (the gate never enables
//!   concurrency the user did not ask for, and never removes it either);
//! * a provider whose profile says `false` runs the same scripted step
//!   sequentially and reaches the same per-call verdicts, in the same order;
//! * the demotion is announced exactly once per demoted step - captured from
//!   the real `tracing::info!` through a scoped subscriber - and not at all when
//!   the provider permits parallel calls or the user never opted in.
//!
//! The provider ids used here are real rows of the wire profile table whose
//! `supports_parallel` value this file asserts first, so a row flipped in the
//! table fails here by name rather than silently changing what is being tested.
//! A final source guard pins the gate's *position* in the agent loop - after the
//! config read, before the first call is prepared - which is the part a
//! behavioural case cannot see. See `specs/uniformtools/T-015-PARALLEL-GATE.md`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::{Event, EventBus};
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::message::{Message, MessagePart};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::wire::profile_table::profile_for;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::permission::{Permission, PermissionAction, PermissionRule};
use ragent_config::{Capabilities, Config as RagentConfig, Cost, StreamConfig};
use tracing_subscriber::layer::SubscriberExt as _;

/// The provider id the probe registers itself under.
///
/// It is deliberately **not** a row of the wire profile table: with every
/// declared family permitting parallel calls, an id that declares no contract is
/// the only input the gate's demoting arm can be reached through without a test
/// editing the profile table to suit itself. FR-002's no-catch-all rule is what
/// makes that input real - a provider with no declared profile is not treated as
/// parallel-capable - and `is_none_or(|profile| !profile.supports_parallel)` is
/// the one expression that covers both it and a `false` row.
const SEQUENTIAL_PROVIDER: &str = "parallel-gate-probe";
/// The provider id whose *declared* row this file uses for the "no demotion"
/// case. `ollama` speaks the OpenAI chat family and, being a local server, needs
/// no credential - so that case never depends on an API key being present.
const PARALLEL_PROVIDER: &str = "ollama";
const SEQUENTIAL_MODEL: &str = "parallel-gate-probe-model";
const PARALLEL_MODEL: &str = "parallel-gate-probe-parallel-model";

// ---------------------------------------------------------------------------
// Scripted provider / client
// ---------------------------------------------------------------------------

/// One scripted turn: one-or-more tool calls, or a closing text reply.
#[derive(Clone, Debug)]
enum ScriptedReply {
    Text(String),
    Tools(Vec<(String, String)>),
}

struct ScriptedProvider {
    id: &'static str,
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
    captured: Arc<Mutex<Vec<ChatRequest>>>,
}

struct ScriptedClient {
    replies: Arc<Mutex<Vec<ScriptedReply>>>,
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
            ScriptedReply::Tools(_) => LlmFinishReason::ToolUse,
        };
        events.push(StreamEvent::Finish { reason: finish });
        Ok(Box::pin(stream::iter(events)))
    }
}

#[async_trait::async_trait]
impl Provider for ScriptedProvider {
    fn id(&self) -> &'static str {
        self.id
    }

    fn name(&self) -> &'static str {
        "Scripted Parallel-Gate Provider"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        vec![ModelInfo {
            id: match self.id {
                PARALLEL_PROVIDER => PARALLEL_MODEL,
                _ => SEQUENTIAL_MODEL,
            }
            .to_string(),
            provider_id: self.id.to_string(),
            name: "Probe".to_string(),
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
            captured: Arc::clone(&self.captured),
        }))
    }
}

// ---------------------------------------------------------------------------
// The demotion notice: a scoped `tracing` subscriber over the real log call
// ---------------------------------------------------------------------------

/// Receives every event the gate's `tracing::info!` emits while it is installed,
/// formatted as one line of `field=value` pairs.
struct CaptureLayer {
    events: Arc<Mutex<Vec<String>>>,
}

impl<S: tracing::Subscriber> tracing_subscriber::Layer<S> for CaptureLayer {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        _ctx: tracing_subscriber::layer::Context<'_, S>,
    ) {
        let mut visitor = FieldVisitor { out: String::new() };
        event.record(&mut visitor);
        if let Ok(mut guard) = self.events.lock() {
            guard.push(visitor.out);
        }
    }
}

/// Renders an event's fields, message included, into one line.
struct FieldVisitor {
    out: String,
}

impl tracing::field::Visit for FieldVisitor {
    fn record_str(&mut self, field: &tracing::field::Field, value: &str) {
        self.out.push_str(&format!(" {}={value}", field.name()));
    }

    fn record_debug(&mut self, field: &tracing::field::Field, value: &dyn std::fmt::Debug) {
        self.out.push_str(&format!(" {}={value:?}", field.name()));
    }
}

/// Runs `body` with a scoped subscriber installed and returns both the body's
/// value and the lines every event emitted meanwhile produced.
///
/// `tracing::subscriber::with_default` cannot carry an `.await` (the guard is
/// not `Send`), so the subscriber is installed as the thread default and dropped
/// after the future resolves. Every line a case asserts on is the *gate's* own
/// `tracing::info!`, filtered by its message, so unrelated events are harmless.
async fn with_event_capture<F, T>(body: F) -> (T, Vec<String>)
where
    F: std::future::Future<Output = T>,
{
    let events: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let subscriber = tracing_subscriber::registry().with(CaptureLayer {
        events: Arc::clone(&events),
    });
    let guard = tracing::subscriber::set_default(subscriber);
    let value = body.await;
    drop(guard);
    let lines = events.lock().map(|guard| guard.clone()).unwrap_or_default();
    (value, lines)
}

// ---------------------------------------------------------------------------
// Harness
// ---------------------------------------------------------------------------

struct Harness {
    processor: SessionProcessor,
    working_dir: PathBuf,
    _dir: tempfile::TempDir,
}

fn make_processor(
    event_bus: Arc<EventBus>,
    provider_id: &'static str,
    tools: Vec<(String, String)>,
    parallel: bool,
) -> Result<Harness> {
    // One `Ask` rule for the family the probe step's tool reports, so every call
    // reaches the permission stage and is held there. With no rule at all the
    // verdict is `Ask` too, but an explicit rule makes the prompt certain rather
    // than incidental: the stage is then reached for every call in both arms,
    // which is the observable the cases read.
    let rules = vec![PermissionRule {
        permission: Permission::Custom(PROMPTED_PERMISSION.to_string()),
        pattern: Some("**".to_string()),
        action: PermissionAction::Ask,
    }];
    let captured = Arc::new(Mutex::new(Vec::new()));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        id: provider_id,
        replies: Arc::new(Mutex::new(vec![
            ScriptedReply::Tools(tools),
            ScriptedReply::Text("done".to_string()),
        ])),
        captured: Arc::clone(&captured),
    }));

    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    // The loop resolves the provider's credential before it builds a client, so
    // the probe provider needs one. It is stored in this case's in-memory database
    // rather than in an environment variable: no process-global state is touched,
    // so no sibling test can observe it and none of AGENTS.md's env-isolation
    // rules apply. Nothing is ever sent anywhere - the scripted client answers.
    storage
        .set_provider_auth(provider_id, "parallel-gate-probe-key")
        .expect("provider credential stored");
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    let config_json = serde_json::json!({
        "experimental": { "parallel_tool_calls": parallel }
    })
    .to_string();
    let config: RagentConfig = serde_json::from_str(&config_json).expect("valid config JSON");

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
    };
    let tmp = tempfile::tempdir().expect("tempdir");
    Ok(Harness {
        processor,
        working_dir: tmp.path().to_path_buf(),
        _dir: tmp,
    })
}

fn make_agent(provider_id: &str) -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General");
    agent.model = Some(ModelRef {
        provider_id: provider_id.to_string(),
        model_id: match provider_id {
            PARALLEL_PROVIDER => PARALLEL_MODEL,
            _ => SEQUENTIAL_MODEL,
        }
        .to_string(),
    });
    agent
}

/// The permission family the probe step's tool reports, and the family the
/// harness files its `Ask` rule under.
///
/// `read_file` reports `file:read`. A rule keyed on `file:read` is *not* the one
/// that decides its verdict: `Permission::from("file:read")` normalizes to the
/// `Read` variant, and on that path the layer's within-the-project auto-grant
/// answers `Allow` before any prompt. `bash` is the family whose rule is
/// consulted as a rule, so a probe call carrying `bash` on an empty command is
/// held at the permission stage - which is the observable these cases read.
const PROMPTED_PERMISSION: &str = "bash";

/// The step both demotion cases dispatch: two `Bash` calls carrying `true`.
///
/// `sleep 0.05` is off the safe-command whitelist, so the permission layer holds
/// it and prompts, and it does nothing when it runs. The responder answers
/// `deny`, so the call is refused at the permission stage - which makes that
/// stage the observable: it belongs to the one pipeline, it is reached only after
/// the call has been prepared, and it publishes a prompt per call, so the order
/// of prompts against call ends shows which arm ran the step.
///
/// The command is deliberately SLOW: the two arms differ only in whether the
/// step's calls are awaited one at a time, and a sub-millisecond call would let
/// either arm produce the same event order by accident. Fifty milliseconds is
/// long enough that a concurrently dispatched pair provably overlaps and a
/// sequential pair provably does not.
///
/// Nothing is written: the shell runs in the case's own temporary directory.
fn two_prompted_calls(dir: &std::path::Path) -> Vec<(String, String)> {
    let _ = dir;
    let args = serde_json::json!({ "command": "sleep 0.05" }).to_string();
    vec![
        ("Bash".to_string(), args.clone()),
        ("Bash".to_string(), args),
    ]
}

/// The per-call outcomes of the tool phase, in the order they were recorded.
fn recorded_calls(reply: &Message) -> Vec<String> {
    reply
        .parts
        .iter()
        .filter_map(|part| match part {
            MessagePart::ToolCall { tool, state, .. } => Some(format!("{tool}:{:?}", state.status)),
            _ => None,
        })
        .collect()
}

/// The dispatch events of the tool step, in publication order.
///
/// Both arms call the one pipeline (FR-012), so a call's recorded outcome is
/// identical either way and only the *route* differs. These are the events that
/// show the route: the permission stage publishes a prompt per call, and the
/// close of a call is published by whichever arm awaited it.
#[derive(Clone, Debug, PartialEq, Eq)]
enum StepEvent {
    /// A permission prompt for a call - published by the pipeline's permission
    /// stage, which both arms run.
    Prompt,
    /// A tool call closed with an error.
    EndError(String),
    /// A tool call closed cleanly.
    EndOk(String),
}

/// Spawns a responder that answers every permission prompt as it is published,
/// so a case costs one round trip per call instead of one two-minute prompt
/// timeout per call. `allow: false` refuses, which is the verdict these cases
/// assert on.
fn spawn_permission_responder(bus: &Arc<EventBus>, allow: bool) -> tokio::task::JoinHandle<()> {
    let bus = Arc::clone(bus);
    tokio::spawn(async move {
        let mut rx = bus.subscribe();
        loop {
            match rx.recv().await {
                Ok(Event::PermissionRequested {
                    session_id,
                    request_id,
                    ..
                }) => bus.publish(Event::PermissionReplied {
                    session_id,
                    request_id,
                    allowed: allow,
                    decision: ragent_agent::permission::PermissionDecision::Once,
                }),
                Ok(_) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    })
}

// ---------------------------------------------------------------------------
// The rows this file relies on
// ---------------------------------------------------------------------------

/// The profiles the gate reads say what this file assumes, so a flipped row
/// fails here by name instead of quietly changing the meaning of every case
/// below.
#[test]
fn the_rows_used_here_declare_the_parallelism_they_are_named_for() {
    let parallel = profile_for(PARALLEL_PROVIDER)
        .unwrap_or_else(|| panic!("{PARALLEL_PROVIDER} must have a declared wire profile"));
    assert!(
        parallel.supports_parallel,
        "{PARALLEL_PROVIDER} must declare supports_parallel == true for the \
         'demotion not applied' cases to mean anything (FR-010)"
    );

    // The demoting arm is reached through a provider the wire table does not
    // name: with every family permitting parallel calls, an id with no declared
    // contract is the input the `false` row shares a branch with (FR-002's
    // no-catch-all rule).
    assert!(
        profile_for(SEQUENTIAL_PROVIDER).is_none(),
        "{SEQUENTIAL_PROVIDER} must have no row in the wire table for the \
         demotion cases to reach the gate's demoting arm; if it gains one, \
         point this constant at another id"
    );

    // Every declared family is what the gate can observe; if a family ever
    // declared `false`, the gate's sequential arm would be reachable through a
    // real provider and the task doc's reasoning would need revisiting.
    for (label, profile) in ragent_agent::provider::wire::profile_table::FAMILIES {
        assert!(
            profile.supports_parallel,
            "the {label} family declares supports_parallel == false; the T-015 \
             task doc must be updated to show a real provider exercising the \
             demotion (FR-010)"
        );
    }
}

// ---------------------------------------------------------------------------
// Behaviour: the demotion, and its absence
// ---------------------------------------------------------------------------

/// Runs one turn of the probe with the given `parallel_tool_calls` value and
/// returns the reply, the demotion lines the run logged, and the tool step's
/// dispatch events in publication order.
async fn run_probe(
    provider_id: &'static str,
    parallel: bool,
) -> Result<(Message, Vec<String>, Vec<StepEvent>)> {
    Box::pin(run_probe_inner(provider_id, parallel)).await
}

/// The body of [`run_probe`], boxed by it: a whole agent turn's future is far
/// past the size clippy's `large_futures` is willing to keep on the stack.
async fn run_probe_inner(
    provider_id: &'static str,
    parallel: bool,
) -> Result<(Message, Vec<String>, Vec<StepEvent>)> {
    // A directory of this case's own: the probe step names a path inside it,
    // which the harness turns into the session's working directory, so nothing
    // here reads or writes the repository.
    let harness_dir = tempfile::tempdir().expect("harness dir");
    let event_bus = Arc::new(EventBus::new(4096));

    // The prompt's answer must be published *while* the turn runs, so the
    // responder is spawned before it; a held call would otherwise cost the full
    // permission timeout.
    let responder = spawn_permission_responder(&event_bus, false);

    // Subscribing before the turn is what makes the log complete: a broadcast
    // receiver only sees events published after it subscribed.
    let collector = {
        let bus = Arc::clone(&event_bus);
        tokio::spawn(async move {
            let mut rx = bus.subscribe();
            let mut log = Vec::new();
            loop {
                match tokio::time::timeout(std::time::Duration::from_millis(300), rx.recv()).await {
                    Ok(Ok(Event::PermissionRequested { .. })) => log.push(StepEvent::Prompt),
                    Ok(Ok(Event::ToolCallEnd { call_id, error, .. })) => {
                        if error.is_some() {
                            log.push(StepEvent::EndError(call_id));
                        } else {
                            log.push(StepEvent::EndOk(call_id));
                        }
                    }
                    Ok(Ok(_)) => continue,
                    Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
                    Ok(Err(_)) | Err(_) => break,
                }
            }
            log
        })
    };

    let tools = two_prompted_calls(harness_dir.path());
    let harness = make_processor(Arc::clone(&event_bus), provider_id, tools, parallel)?;
    let session = harness
        .processor
        .session_manager
        .create_session(harness_dir.path().to_path_buf())
        .expect("session created");
    // The session's directory is the case's temp dir, so the harness's own
    // `working_dir` is deliberately unused here; the temp dir is left to the
    // process rather than kept alive, because nothing after the turn reads it.
    let _ = &harness.working_dir;
    std::mem::forget(harness_dir);

    // The turn is boxed for the same reason this helper is: a whole agent
    // session's future does not belong on the stack.
    let (result, logs) = with_event_capture(Box::pin(harness.processor.process_message(
        &session.id,
        "run the step",
        &make_agent(provider_id),
        Arc::new(AtomicBool::new(false)),
    )))
    .await;
    let reply = result.expect("the turn runs to completion");
    responder.abort();
    let events = collector.await.unwrap_or_default();
    Ok((reply, logs, events))
}

/// A provider that permits parallel calls keeps the concurrent arm: the same
/// `experimental.parallel_tool_calls` opt-in still dispatches concurrently, and
/// no demotion is logged.
#[tokio::test]
async fn test_a_parallelising_provider_keeps_the_concurrent_arm() -> Result<()> {
    let (reply, logs, events) = run_probe(PARALLEL_PROVIDER, true).await?;

    assert_eq!(
        recorded_calls(&reply),
        vec!["Bash:Error".to_string(), "Bash:Error".to_string()],
        "both calls reach the permission stage and are held"
    );
    assert!(
        !logs
            .iter()
            .any(|line| line.contains("demoted to sequential")),
        "a provider that permits parallel calls must not be demoted (FR-010): {logs:?}"
    );

    // The concurrent arm prepares the whole step before it dispatches, and
    // drains the slots afterwards, so the step's prompts all arrive ahead of
    // its ends.
    assert_eq!(
        events,
        vec![
            StepEvent::Prompt,
            StepEvent::Prompt,
            StepEvent::EndError("call_0".to_string()),
            StepEvent::EndError("call_1".to_string()),
        ],
        "the concurrent arm prompts for every call of the step, then closes them \
         in the drain (FR-010): the gate removed nothing"
    );
    Ok(())
}

/// A provider with no declared parallel support runs the same step
/// sequentially, reaches the same verdicts in the same order, and announces the
/// demotion exactly once.
#[tokio::test]
async fn test_a_non_parallelising_provider_is_demoted_once_and_keeps_the_history() -> Result<()> {
    // The reference run: the same step with the user's opt-in off.
    let (sequential_reply, sequential_logs, sequential_events) =
        run_probe(SEQUENTIAL_PROVIDER, false).await?;

    // The run under test: the opt-in is on, and the provider cannot carry it.
    let (reply, logs, events) = run_probe(SEQUENTIAL_PROVIDER, true).await?;

    assert_eq!(
        recorded_calls(&reply),
        recorded_calls(&sequential_reply),
        "a demoted step produces the same history as the same step run \
         sequentially (FR-012), so the gate changes concurrency and nothing else"
    );
    assert_eq!(
        recorded_calls(&reply).len(),
        2,
        "every call of the demoted step is still recorded: {:?}",
        recorded_calls(&reply)
    );

    // The demoted step ran through the sequential arm, which is the only
    // observable difference: it walks the step and closes each call before
    // starting the next, so every prompt is followed by that call's end. The
    // concurrent arm answers the prompts first and closes the calls afterwards.
    assert_eq!(
        events,
        vec![
            StepEvent::Prompt,
            StepEvent::EndError("call_0".to_string()),
            StepEvent::Prompt,
            StepEvent::EndError("call_1".to_string()),
        ],
        "a demoted step runs the sequential arm: prompt, answer, next call \
         (FR-010)"
    );
    assert_eq!(
        events, sequential_events,
        "and it is indistinguishable from the same step with the opt-in off, \
         which is what 'forced sequential' has to mean (FR-010)"
    );

    // One notice per demoted step, naming the provider and the reason.
    let notices: Vec<&String> = logs
        .iter()
        .filter(|line| line.contains("demoted to sequential"))
        .collect();
    assert_eq!(
        notices.len(),
        1,
        "the demotion is logged exactly once per demoted step, not once per \
         call (FR-010): {logs:?}"
    );
    let notice = notices[0];
    assert!(
        notice.contains(&format!("provider={SEQUENTIAL_PROVIDER}")),
        "the notice names the provider (FR-010): {notice}"
    );
    assert!(
        notice.contains("reason=") && notice.contains("supports_parallel"),
        "the notice states the reason (FR-010): {notice}"
    );

    // Nothing is logged when the user never opted in: the gate demotes an
    // opt-in, it does not narrate a step that was already sequential.
    assert!(
        !sequential_logs
            .iter()
            .any(|line| line.contains("demoted to sequential")),
        "a step that was never parallel has nothing to demote (FR-010): \
         {sequential_logs:?}"
    );
    Ok(())
}

// ---------------------------------------------------------------------------
// Source guards: where the gate sits, and what it may not do
// ---------------------------------------------------------------------------

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("crates/<name> sits two levels below the workspace root")
        .to_path_buf()
}

fn read_workspace_file(relative: &str) -> String {
    let path = workspace_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|e| panic!("cannot read {}: {}", path.display(), e))
}

/// The gate reads the config flag, then the profile, then forces sequential -
/// in the agent loop, between the config read and the first call prepared.
#[test]
fn the_gate_sits_between_the_config_read_and_the_first_call() {
    let source = read_workspace_file("crates/ragent-agent/src/session/processor.rs");

    let config_pos = source
        .find("let parallel_tool_calls = turn.session_config.experimental.parallel_tool_calls;")
        .expect("the loop must read the user's parallel opt-in");
    let profile_pos = source
        .find("crate::provider::wire::profile_table::profile_for(")
        .expect("the gate must resolve the active provider's wire profile (FR-010)");
    let demote_pos = source
        .find("parallel_tool_calls = false;")
        .expect("a provider that cannot parallelise must force sequential (FR-010)");
    let first_call_pos = source
        .find("let mut futures: Vec<PendingStepCall> = Vec::new();")
        .expect("the step's slot list must exist");
    assert!(
        config_pos < profile_pos && profile_pos < demote_pos && demote_pos < first_call_pos,
        "order must be: read experimental.parallel_tool_calls, resolve the \
         profile, force sequential, then prepare the step's calls (FR-010)"
    );

    // The rule is `is_none_or(|p| !p.supports_parallel)` - one expression that
    // covers both a profile that says `false` and an id with no declared
    // contract, so no provider silently escapes the gate by being absent.
    let gate_block = &source[profile_pos..demote_pos];
    assert!(
        gate_block.contains("!profile.supports_parallel"),
        "the gate must demote when the profile says supports_parallel == false \
         (FR-010): {gate_block}"
    );

    // The notice is one `tracing::info!` inside the gate, naming the provider
    // and the reason.
    let log_block = &source[demote_pos..first_call_pos];
    assert_eq!(
        log_block.matches("tracing::info!(").count(),
        1,
        "the demotion must log exactly one tracing::info! (FR-010): {log_block}"
    );
    assert!(
        log_block.contains("provider = %turn.model_ref.provider_id"),
        "the notice must name the provider (FR-010): {log_block}"
    );
    assert!(
        log_block.contains("reason ="),
        "the notice must state the reason (FR-010): {log_block}"
    );

    // The gate only ever clears the flag: no assignment in the block turns it
    // back on, so a provider that permits parallel calls is untouched and
    // concurrency is never enabled here (the user's opt-in is the only switch).
    assert!(
        !log_block.contains("parallel_tool_calls = true"),
        "the gate may only demote; it must never enable concurrency (FR-010)"
    );
}

/// FR-015's "one declaration, every consumer": the family profiles state
/// `supports_parallel` once, and the agent loop is the field's only writer.
#[test]
fn every_family_profile_states_supports_parallel_and_the_loop_is_its_reader() {
    let profile_table =
        read_workspace_file("crates/ragent-llm/src/providers/wire/profile_table.rs");
    let families = profile_table
        .matches(": ToolWireProfile = ToolWireProfile {")
        .count();
    let declarations = profile_table.matches("supports_parallel:").count();
    assert_eq!(
        declarations, families,
        "every family const must state supports_parallel: found {declarations} \
         declarations across {families} profile consts (FR-002, FR-010)"
    );

    // The matrix renders the field (FR-016), so a `false` row is visible
    // before a session runs.
    let matrix = read_workspace_file("crates/ragent-llm/src/providers/wire/matrix.rs");
    assert!(
        matrix.contains("profile.supports_parallel"),
        "the wire matrix must render the supports_parallel column (FR-016)"
    );

    // The field's doc states the contract the gate implements.
    let wire_mod = read_workspace_file("crates/ragent-llm/src/providers/wire/mod.rs");
    assert!(
        wire_mod.contains("`false` forces sequential execution"),
        "ToolWireProfile::supports_parallel must document the false arm (FR-010)"
    );
}
