//! Integration tests for destructive-action checkpoints (spec `agentloop`,
//! task T-010 / FR-015, FR-024):
//!
//! - with checkpoints enabled (the loop default), a destructive tool call
//!   (`rm`) forces a human-approval checkpoint prompt even when an allow
//!   rule would otherwise auto-approve it;
//! - an unanswered checkpoint prompt (timeout) behaves as denial, with a
//!   "checkpoint denied" observation explaining the safe default;
//! - disabling checkpoints restores the plain permission behaviour (the
//!   allow rule auto-approves and the call executes);
//! - auto-approve mode (`--yes`) is still subject to the checkpoint (the
//!   destructive call is denied), while plain auto-approved calls execute;
//! - a checkpoint prompt answered via `PermissionReplied` resolves it;
//! - a checkpoint prompt answered with a denial keeps the file and surfaces
//!   the checkpoint explanation as an observation while the loop continues
//!   (task T-022, TC-014 deny path);
//! - a hard-deny rule blocks the destructive call under auto-approve
//!   (autopilot) mode without prompting (task T-022, TC-019 / FR-024).
//!
//! The `Esc` mid-loop interrupt path (task T-022, TC-015) is covered by
//! `test_loop_interrupt.rs`.

use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::{AtomicU32, Ordering};

use anyhow::Result;
use futures::stream;
use ragent_agent::agent::{AgentInfo, ModelRef};
use ragent_agent::event::{Event, EventBus};
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::loop_state::{LoopSpec, StopCondition};
use ragent_agent::session::processor::{CachedConfig, SessionProcessor};
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::permission::{Permission, PermissionAction, PermissionRule};
use ragent_config::{Capabilities, Config as RagentConfig, Cost};

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
        self.call_count.fetch_add(1, Ordering::SeqCst);
        // Strict in-order consumption: each call serves the next scripted
        // reply; once the script is exhausted the final entry repeats for any
        // further calls.
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
            captured: Arc::clone(&self.shared_captured),
        }))
    }
}

type MakeProcessorResult = (
    Arc<SessionProcessor>,
    std::path::PathBuf,
    Arc<Mutex<Vec<ChatRequest>>>,
    // Held by the caller so the session directory stays alive on disk.
    tempfile::TempDir,
);

/// Build a processor whose provider replays `replies` with a permission
/// checker that auto-allows file operations (so the destructive call would
/// execute without the checkpoint) and the loop config injected into the
/// config cache.
fn make_processor(
    event_bus: Arc<EventBus>,
    replies: Vec<ScriptedReply>,
    auto_approve: bool,
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
    let checker = PermissionChecker::new(vec![PermissionRule {
        permission: Permission::Custom("file:write".to_string()),
        pattern: Some("**".to_string()),
        action: PermissionAction::Allow,
    }]);
    let config: RagentConfig =
        serde_json::from_str(r#"{"loop":{"max_steps":25,"checkpoint_timeout_secs":100}}"#)
            .expect("valid config JSON");
    let processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(provider_registry),
        tool_registry: Arc::new(tool::create_default_registry()),
        permission_checker: Arc::new(parking_lot::RwLock::new(checker)),
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
        auto_approve,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(HashMap::new()),
    });
    let tmp = tempfile::tempdir().expect("tempdir");
    let path = tmp.path().to_path_buf();
    Ok((processor, path, captured, tmp))
}

/// A minimal agent driving the scripted provider.
fn make_agent() -> AgentInfo {
    let mut agent = AgentInfo::new("general", "General agent");
    agent.model = Some(ModelRef {
        provider_id: "ollama".to_string(),
        model_id: "qwen3:latest".to_string(),
    });
    agent
}

/// Drain every buffered event from the bus, keeping them all so multiple
/// assertions can filter the same snapshot.
fn drain_all(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Event> {
    let mut found = Vec::new();
    while let Ok(event) = rx.try_recv() {
        found.push(event);
    }
    found
}

/// Drain every event matching `pred` from the bus.
fn drain_events<F>(rx: &mut tokio::sync::broadcast::Receiver<Event>, pred: F) -> Vec<Event>
where
    F: Fn(&Event) -> bool,
{
    drain_all(rx)
        .into_iter()
        .filter(|event| pred(event))
        .collect()
}

/// The scripted destructive call: `rm` a file that exists.
fn rm_call() -> ScriptedReply {
    ScriptedReply::ToolCallNamed {
        name: "rm",
        args: r#"{"path": "victim.txt"}"#.to_string(),
    }
}

/// The scripted think call: a non-destructive tool that auto-executes.
fn think_call() -> ScriptedReply {
    ScriptedReply::ToolCallNamed {
        name: "think",
        args: r#"{"thought": "considering"}"#.to_string(),
    }
}

/// Filter an event snapshot for [`Event::ToolCallEnd`] entries.
fn tool_ends(events: &[Event]) -> Vec<Event> {
    events
        .iter()
        .filter(|event| matches!(event, Event::ToolCallEnd { .. }))
        .cloned()
        .collect()
}

/// FR-015: with checkpoints enabled (the default), a destructive call (`rm`)
/// forces the checkpoint prompt even though an allow rule would auto-approve
/// it; with nobody answering, the timeout behaves as denial and the
/// observation carries the checkpoint explanation. The file survives.
#[tokio::test]
async fn test_checkpoint_denies_destructive_call_under_allow_rule() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    // Subscribe BEFORE the turn so the checkpoint prompt is captured.
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![rm_call(), ScriptedReply::TextOnly("done")],
        false,
    )?;
    std::fs::write(working_dir.join("victim.txt"), "keep me").expect("seed file");

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "delete the file");
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");

    // The turn is over: snapshot the bus ONCE so both assertions see the
    // same events (a second drain would find an empty buffer).
    let events = drain_all(&mut rx);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::PermissionRequested { .. }))
            .count(),
        1,
        "the destructive call raised exactly one checkpoint prompt"
    );

    // The tool result is the checkpoint-denial observation.
    let ends = tool_ends(&events);
    assert_eq!(ends.len(), 1);
    if let Event::ToolCallEnd {
        error: Some(err), ..
    } = ends.into_iter().next().expect("one end")
    {
        assert!(
            err.starts_with("checkpoint denied: destructive action 'rm'"),
            "the observation explains the checkpoint denial: {err}"
        );
        assert!(
            err.contains("deletion, config writes, dependency installation"),
            "the observation names the checkpoint-protected categories: {err}"
        );
    }

    // The file was NOT deleted: the timeout defaulted to the safe choice.
    assert!(
        working_dir.join("victim.txt").exists(),
        "FR-015: the destructive action never executed"
    );
    // Only the rm step and the final answer reached the provider.
    assert_eq!(captured.lock().expect("captured lock").len(), 2);
    Ok(())
}

/// FR-015: with checkpoints disabled in the loop spec, the allow rule
/// auto-approves the destructive call and it executes — checkpoints only
/// apply while the loop has them enabled.
#[tokio::test]
async fn test_checkpoints_disabled_restores_allow_rule_behaviour() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![rm_call(), ScriptedReply::TextOnly("done")],
        false,
    )?;
    std::fs::write(working_dir.join("victim.txt"), "delete me").expect("seed file");

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "delete the file");
    spec.checkpoints = false;
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");

    assert!(
        !working_dir.join("victim.txt").exists(),
        "with checkpoints off the allow rule auto-approves the deletion"
    );
    let requests = drain_events(&mut rx, |event| {
        matches!(event, Event::PermissionRequested { .. })
    });
    assert!(
        requests.is_empty(),
        "no checkpoint prompt is raised when checkpoints are disabled"
    );
    Ok(())
}

/// FR-024: in auto-approve mode (autopilot/YOLO, `--yes`), the forced
/// destructive-action checkpoint still applies — the destructive call is
/// checkpointed and denied, while a non-destructive call auto-approves.
#[tokio::test]
async fn test_auto_approve_still_enforces_checkpoint() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![rm_call(), think_call(), ScriptedReply::TextOnly("done")],
        true,
    )?;
    std::fs::write(working_dir.join("victim.txt"), "keep me").expect("seed file");

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "delete the file then think");
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");

    assert!(
        working_dir.join("victim.txt").exists(),
        "FR-024: auto-approval answers prompts but the destructive-action \
         checkpoint is still enforced"
    );
    let requests = drain_events(&mut rx, |event| {
        matches!(event, Event::PermissionRequested { .. })
    });
    assert_eq!(
        requests.len(),
        1,
        "exactly the destructive call was checkpointed in auto-approve mode"
    );
    Ok(())
}

/// FR-024 (control): with checkpoints disabled, plain auto-approve mode
/// executes the destructive call without prompting.
#[tokio::test]
async fn test_auto_approve_without_checkpoints_executes() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![rm_call(), ScriptedReply::TextOnly("done")],
        true,
    )?;
    std::fs::write(working_dir.join("victim.txt"), "delete me").expect("seed file");

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "delete the file");
    spec.checkpoints = false;
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");

    assert!(
        !working_dir.join("victim.txt").exists(),
        "auto-approve without checkpoints executes the destructive call"
    );
    let requests = drain_events(&mut rx, |event| {
        matches!(event, Event::PermissionRequested { .. })
    });
    assert!(requests.is_empty());
    Ok(())
}

/// FR-015: a checkpoint prompt answered via `PermissionReplied` resolves it
/// — an approval lets the destructive call execute.
#[tokio::test]
async fn test_checkpoint_answered_with_permission_replied() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![rm_call(), ScriptedReply::TextOnly("done")],
        false,
    )?;
    std::fs::write(working_dir.join("victim.txt"), "delete me").expect("seed file");

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "delete the file");
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;

    // Answer the checkpoint prompt as soon as it is published.
    let answer_bus = event_bus.clone();
    let responder = tokio::spawn(async move {
        let mut rx = answer_bus.subscribe();
        loop {
            match rx.recv().await {
                Ok(Event::PermissionRequested {
                    session_id,
                    request_id,
                    ..
                }) => {
                    answer_bus.publish(Event::PermissionReplied {
                        session_id,
                        request_id,
                        allowed: true,
                        decision: ragent_agent::permission::PermissionDecision::Once,
                    });
                }
                Ok(_) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");
    responder.abort();

    assert!(
        !working_dir.join("victim.txt").exists(),
        "the approved checkpoint let the destructive call execute"
    );
    Ok(())
}

/// FR-015 (control): a non-destructive call in a checkpointed loop takes the
/// normal permission path — no checkpoint prompt is raised and the allow
/// rule auto-approves it.
#[tokio::test]
async fn test_non_destructive_call_not_checkpointed() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![think_call(), ScriptedReply::TextOnly("done")],
        false,
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "think");
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");

    let requests = drain_events(&mut rx, |event| {
        matches!(event, Event::PermissionRequested { .. })
    });
    assert!(
        requests.is_empty(),
        "non-destructive calls never raise the checkpoint prompt"
    );
    Ok(())
}

/// FR-015 (unit-level, helper): the checkpoint denial observation names the
/// tool and resource, and the destructive classifier covers the documented
/// categories (delete, config write, dependency install, destructive git).
#[tokio::test]
async fn test_checkpoint_reason_and_classifier() -> Result<()> {
    let reason = ragent_agent::session::permissions::checkpoint_reason_for_test("rm", "victim.txt");
    assert!(reason.contains("checkpoint denied"));
    assert!(reason.contains("rm"));
    assert!(reason.contains("victim.txt"));

    let destructive = |name: &str, args: &str| {
        let input: serde_json::Value = serde_json::from_str(args).expect("json");
        ragent_agent::session::permissions::is_destructive_tool_for_test(name, &input)
    };
    // Deletion.
    assert!(destructive("rm", r#"{"path":"a"}"#));
    // Config write.
    assert!(destructive("config_write", r#"{"path":"ragent.json"}"#));
    // Dependency installation.
    assert!(destructive("cargo_install", r#"{"crate":"serde"}"#));
    assert!(destructive("npm_install", r#"{"package":"left-pad"}"#));
    // Destructive git variants (force push / hard reset) are marked, while
    // read-only git workflows are not.
    assert!(destructive("git_push", r#"{"force":true}"#));
    assert!(!destructive("git_push", r#"{"force":false}"#));
    assert!(destructive("git_reset", r#"{"mode":"hard"}"#));
    assert!(!destructive("git_reset", r#"{"mode":"soft"}"#));
    // Bash sub-commands are scanned for destructive prefixes.
    assert!(destructive(
        "bash",
        r#"{"command":"git push --force origin main"}"#
    ));
    assert!(destructive("bash", r#"{"command":"rm -rf build"}"#));
    assert!(!destructive(
        "bash",
        r#"{"command":"cargo test --workspace"}"#
    ));
    assert!(!destructive("write", r#"{"path":"notes.md"}"#));
    Ok(())
}

/// FR-015 (TC-014 deny path): a checkpoint prompt answered with a denial
/// keeps the file and surfaces the checkpoint explanation as an observation,
/// while the loop itself continues and terminates `completed` — a user denial
/// is an observation, not an unrecoverable stop condition.
#[tokio::test]
async fn test_checkpoint_prompt_denied_by_user_keeps_file_and_continues() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![rm_call(), ScriptedReply::TextOnly("done")],
        false,
    )?;
    std::fs::write(working_dir.join("victim.txt"), "keep me").expect("seed file");

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "delete the file");
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;

    // Deny the checkpoint prompt as soon as it is published.
    let answer_bus = event_bus.clone();
    let responder = tokio::spawn(async move {
        let mut rx = answer_bus.subscribe();
        loop {
            match rx.recv().await {
                Ok(Event::PermissionRequested {
                    session_id,
                    request_id,
                    ..
                }) => {
                    answer_bus.publish(Event::PermissionReplied {
                        session_id,
                        request_id,
                        allowed: false,
                        decision: ragent_agent::permission::PermissionDecision::Once,
                    });
                }
                Ok(_) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(_) => break,
            }
        }
    });

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the denied checkpoint ends the turn normally");
    responder.abort();

    // Snapshot the bus ONCE: every assertion filters this same snapshot (a
    // second drain would find an empty buffer).
    let events = drain_all(&mut rx);

    // The file survived: the denial answered the checkpoint.
    assert!(
        working_dir.join("victim.txt").exists(),
        "the user denial keeps the victim file"
    );

    // The destructive call raised exactly one checkpoint prompt.
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::PermissionRequested { .. }))
            .count(),
        1,
        "the destructive call raised exactly one checkpoint prompt"
    );

    // The tool result is the checkpoint-denial observation.
    let ends = tool_ends(&events);
    assert_eq!(ends.len(), 1);
    if let Event::ToolCallEnd {
        error: Some(err), ..
    } = ends.into_iter().next().expect("one end")
    {
        assert!(
            err.starts_with("checkpoint denied: destructive action 'rm'"),
            "the observation explains the answered denial: {err}"
        );
    }

    // The denial was an observation, not a fatal error: the loop continued
    // and terminated `completed` after the follow-up answer.
    let terminated: Vec<&Event> = events
        .iter()
        .filter(|event| matches!(event, Event::LoopTerminated { .. }))
        .collect();
    assert_eq!(terminated.len(), 1, "the loop terminated exactly once");
    if let Event::LoopTerminated {
        status, iterations, ..
    } = terminated.into_iter().next().cloned().expect("one event")
    {
        assert_eq!(status, StopCondition::GoalAchieved.as_str());
        assert_eq!(iterations, 2, "the denied step plus the final answer");
    }
    // Only the rm step and the final answer reached the provider.
    assert_eq!(captured.lock().expect("captured lock").len(), 2);
    Ok(())
}

/// FR-024 (TC-019): a hard-deny rule is never overridden by auto-approval —
/// in a checkpointed loop launched with autopilot (`--yes`), the policy deny
/// blocks the destructive call without prompting, the file survives, and the
/// denial is surfaced as an observation while the loop completes afterwards.
#[tokio::test]
async fn test_hard_deny_rule_enforced_under_auto_approve() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![rm_call(), ScriptedReply::TextOnly("done")],
        true,
    )?;
    std::fs::write(working_dir.join("victim.txt"), "keep me").expect("seed file");

    // Policy hard-deny: the ruleset blocks file writes outright.
    *processor.permission_checker.write() = PermissionChecker::new(vec![PermissionRule {
        permission: Permission::Custom("file:write".to_string()),
        pattern: Some("**".to_string()),
        action: PermissionAction::Deny,
    }]);

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "delete the file");
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;

    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("the policy denial ends the turn normally");

    // FR-024: the hard-deny rule blocked the destructive call.
    assert!(
        working_dir.join("victim.txt").exists(),
        "the hard-deny rule blocks the destructive action"
    );

    let events = drain_all(&mut rx);

    // An explicit policy deny resolves without any interactive prompt —
    // auto-approval never overrides the deny verdict.
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, Event::PermissionRequested { .. }))
            .count(),
        0,
        "an explicit policy deny resolves without prompting"
    );

    // The denial is surfaced as an observation for the model.
    let ends = tool_ends(&events);
    assert_eq!(ends.len(), 1);
    if let Event::ToolCallEnd {
        error: Some(err), ..
    } = ends.into_iter().next().expect("one end")
    {
        assert!(
            err.starts_with("checkpoint denied: destructive action 'rm'"),
            "the denial is surfaced as an observation: {err}"
        );
    }
    Ok(())
}
