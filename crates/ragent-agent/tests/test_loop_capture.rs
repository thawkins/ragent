//! Integration tests for the pre-loop workspace capture and the post-loop
//! change summary (spec `agentloop`, task T-012 / FR-018, FR-019):
//!
//! - a goal-driven loop captures a workspace snapshot BEFORE its first
//!   write action executes, so the snapshot records the workspace exactly
//!   as the loop found it;
//! - a loop that never writes never captures (read-only runs stay cheap);
//! - a workspace outside a git repository warns that rollback will be
//!   snapshot-only before the loop starts (FR-018);
//! - on termination the change summary (modified/created/deleted counts +
//!   diffstat) is published as [`Event::LoopChangeSummary`] and matches the
//!   induced file changes (FR-019);
//! - the rollback path (`SessionProcessor::rollback_loop`, FR-020 plumbing
//!   for T-013) restores the captured contents, including files the loop
//!   deleted (task T-023);
//! - declining the rollback offer (the TUI handler runs
//!   `clear_loop_captures`) keeps every change the loop made and drops the
//!   pending capture so no later rollback can resurrect it (FR-020).

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
use ragent_agent::session::loop_capture::LoopCapture;
use ragent_agent::session::loop_state::LoopSpec;
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
/// checker that auto-allows file writes (so scripted write tools execute)
/// and the default loop config injected into the config cache.
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
    let checker = PermissionChecker::new(vec![PermissionRule {
        permission: Permission::Custom("file:write".to_string()),
        pattern: Some("**".to_string()),
        action: PermissionAction::Allow,
    }]);
    let config: RagentConfig =
        serde_json::from_str(r#"{"loop":{"max_steps":25}}"#).expect("valid config JSON");
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

/// Drain every event matching `pred` from the bus.
fn drain_events<F>(rx: &mut tokio::sync::broadcast::Receiver<Event>, pred: F) -> Vec<Event>
where
    F: Fn(&Event) -> bool,
{
    let mut found = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if pred(&event) {
            found.push(event);
        }
    }
    found
}

/// Drain the single [`Event::LoopChangeSummary`] from the bus.
fn drain_change_summary(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Event> {
    drain_events(rx, |event| matches!(event, Event::LoopChangeSummary { .. }))
}

/// FR-018: the workspace snapshot is captured BEFORE the first write action
/// — the pre-existing file's original content is inside the capture, and the
/// loop's own write (which created the file) is not.
#[tokio::test]
async fn test_pre_loop_snapshot_captured_before_first_write() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCallNamed {
                name: "write",
                args: r#"{"path": "loop_written.txt", "content": "written by the loop"}"#
                    .to_string(),
            },
            ScriptedReply::TextOnly("done"),
        ],
    )?;
    std::fs::write(working_dir.join("pre_existing.txt"), "original").expect("seed file");

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "write the file");
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

    let capture = processor
        .active_loop_captures
        .read()
        .await
        .get(&session.id)
        .cloned()
        .expect("the loop armed a pre-loop capture");
    let LoopCapture { snapshot, git } = &capture;
    let snapshot = snapshot
        .as_ref()
        .expect("a write happened so a snapshot exists");
    let pre_existing = snapshot
        .files
        .get(&working_dir.join("pre_existing.txt"))
        .expect("the pre-existing file was captured before the write");
    assert_eq!(pre_existing, b"original", "pre-write contents are captured");
    assert!(
        !snapshot
            .files
            .contains_key(&working_dir.join("loop_written.txt")),
        "the loop's own write is NOT in the pre-loop capture"
    );
    // A plain tempdir is not a git repository: the state records `None` but
    // the loop still ran (the warning is asserted separately).
    assert!(git.is_none());
    let _ = drain_change_summary(&mut rx);
    Ok(())
}

/// FR-018: a loop that never performs a write action never captures — the
/// capture entry exists with no snapshot, so no summary is published.
#[tokio::test]
async fn test_read_only_loop_captures_nothing() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCallNamed {
                name: "think",
                args: r#"{"thought": "considering"}"#.to_string(),
            },
            ScriptedReply::TextOnly("answer"),
        ],
    )?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "think then answer");
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
    assert_eq!(captured.lock().expect("captured lock").len(), 2);

    let capture = processor
        .active_loop_captures
        .read()
        .await
        .get(&session.id)
        .cloned()
        .expect("the loop recorded its capture entry");
    assert!(
        capture.snapshot.is_none(),
        "FR-018: no write action means no workspace snapshot"
    );
    // The empty summary is still published (all counts zero) so consumers
    // see a paired summary for every terminated loop with a capture entry.
    let summaries = drain_change_summary(&mut rx);
    assert_eq!(summaries.len(), 1, "one change summary for the loop");
    if let Event::LoopChangeSummary {
        files_modified,
        files_created,
        files_deleted,
        ..
    } = summaries.into_iter().next().expect("one summary")
    {
        assert_eq!(files_modified, 0);
        assert_eq!(files_created, 0);
        assert_eq!(files_deleted, 0);
    }
    Ok(())
}

/// FR-018: a workspace outside a git repository warns that rollback will be
/// snapshot-only BEFORE the loop starts.
#[tokio::test]
async fn test_non_git_workspace_warns_before_start() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir_guard) =
        make_processor(event_bus.clone(), vec![ScriptedReply::TextOnly("done")])?;
    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");

    processor
        .start_loop(&session.id, LoopSpec::new("general", "goal"))
        .await;

    let notices = drain_events(&mut rx, |event| matches!(event, Event::AgentNotice { .. }));
    assert!(
        notices.iter().any(|event| match event {
            Event::AgentNotice { message, .. } => {
                message.contains("not inside a git repository") && message.contains("snapshot-only")
            }
            _ => false,
        }),
        "FR-018: the snapshot-only warning is published before the loop \
         writes anything: {notices:?}"
    );
    Ok(())
}

/// FR-019: the change summary counts match the induced file changes — the
/// loop modifies one pre-existing file and creates another.
#[tokio::test]
async fn test_change_summary_counts_match_induced_changes() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCallNamed {
                name: "write",
                args:
                    r#"{"path": "existing.txt", "content": "modified contents\nnow two lines\n"}"#
                        .to_string(),
            },
            ScriptedReply::ToolCallNamed {
                name: "write",
                args: r#"{"path": "brand_new.txt", "content": "created by the loop\n"}"#
                    .to_string(),
            },
            ScriptedReply::TextOnly("done"),
        ],
    )?;
    std::fs::write(working_dir.join("existing.txt"), "original line\n").expect("seed file");

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "update the files");
    spec.max_steps = Some(5);
    processor.start_loop(&session.id, spec).await;

    let outcome = processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");
    assert!(
        !outcome.parts.is_empty(),
        "the turn produced a final assistant message"
    );

    let summaries = drain_change_summary(&mut rx);
    assert_eq!(summaries.len(), 1, "exactly one change summary");
    let Event::LoopChangeSummary {
        status,
        iterations,
        files_modified,
        files_created,
        files_deleted,
        diffstat,
        files,
        ..
    } = summaries.into_iter().next().expect("one summary")
    else {
        unreachable!("filtered to LoopChangeSummary");
    };
    assert_eq!(status, "completed");
    assert_eq!(iterations, 3, "two write steps plus the final answer");
    assert_eq!(files_modified, 1, "existing.txt was modified");
    assert_eq!(files_created, 1, "brand_new.txt was created");
    assert_eq!(files_deleted, 0);
    assert!(
        diffstat.starts_with('+') && diffstat.contains("lines across 2 files"),
        "the diffstat aggregates the changed files: {diffstat:?}"
    );
    assert_eq!(
        files,
        vec!["brand_new.txt".to_string(), "existing.txt".to_string()],
        "affected files are sorted relative paths"
    );
    Ok(())
}

/// FR-020 plumbing (T-013): after termination the capture survives so the
/// rollback offer can restore the pre-loop contents; a second rollback is a
/// no-op and the loop's created file stays (outside the snapshot's file set).
#[tokio::test]
async fn test_rollback_restores_pre_loop_contents() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCallNamed {
                name: "write",
                args: r#"{"path": "existing.txt", "content": "mutated by the loop"}"#.to_string(),
            },
            ScriptedReply::TextOnly("done"),
        ],
    )?;
    std::fs::write(working_dir.join("existing.txt"), "original").expect("seed file");

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "mutate the file");
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

    let mutated = std::fs::read_to_string(working_dir.join("existing.txt")).expect("read");
    assert_eq!(mutated, "mutated by the loop", "the loop's write landed");

    // The capture survives termination (the rollback offer is pending).
    assert!(
        processor
            .active_loop_captures
            .read()
            .await
            .contains_key(&session.id),
        "the capture survives termination until rollback or clear"
    );

    let restored = processor
        .rollback_loop(&session.id)
        .await
        .expect("rollback");
    assert!(restored, "the pending capture was restored");
    let content = std::fs::read_to_string(working_dir.join("existing.txt")).expect("read");
    assert_eq!(
        content, "original",
        "FR-020: the pre-loop contents are back"
    );

    // A second rollback finds no pending capture.
    let again = processor
        .rollback_loop(&session.id)
        .await
        .expect("rollback");
    assert!(!again, "no capture remains after the first rollback");

    // Declining the offer (explicit clear) also drops the capture.
    let mut spec2 = LoopSpec::new("general", "mutate again");
    spec2.max_steps = Some(5);
    processor.start_loop(&session.id, spec2).await;
    processor
        .process_message(
            &session.id,
            "go",
            &make_agent(),
            Arc::new(AtomicBool::new(false)),
        )
        .await
        .expect("turn completes");
    processor.clear_loop(&session.id).await;
    let declined = processor
        .rollback_loop(&session.id)
        .await
        .expect("rollback");
    assert!(!declined, "clear_loop discarded the pending capture");
    Ok(())
}

/// FR-019 (T-023, deleted-file lane): the change summary counts a file the
/// loop deleted — 0 modified, 0 created, 1 deleted — and names it among the
/// affected files; the diffstat charges the deleted file's removed lines.
/// FR-020: the subsequent rollback restores the deleted file's contents.
#[tokio::test]
async fn test_change_summary_counts_deleted_files() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let mut rx = event_bus.subscribe();
    let (processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCallNamed {
                name: "rm",
                args: r#"{"path": "doomed.txt"}"#.to_string(),
            },
            ScriptedReply::TextOnly("done"),
        ],
    )?;
    std::fs::write(working_dir.join("doomed.txt"), "line one\nline two\n").expect("seed file");

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    // Checkpoints are off so the scripted rm executes directly under the
    // allow rule; the first-write capture arming is independent of them.
    let mut spec = LoopSpec::new("general", "remove the file");
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
        !working_dir.join("doomed.txt").exists(),
        "the loop's rm landed"
    );

    // FR-018: the first write action (the rm) armed a pre-loop snapshot that
    // still records the doomed file's pre-write contents.
    let capture = processor
        .active_loop_captures
        .read()
        .await
        .get(&session.id)
        .cloned()
        .expect("the loop armed a pre-loop capture");
    let snapshot = capture
        .snapshot
        .as_ref()
        .expect("a write happened so a snapshot exists");
    assert_eq!(
        snapshot
            .files
            .get(&working_dir.join("doomed.txt"))
            .map(Vec::as_slice),
        Some(b"line one\nline two\n".as_slice()),
        "the deleted file's pre-write contents are captured"
    );

    // FR-019: the summary counts match the induced change — exactly one
    // deletion, no modifications or creations, and the removed lines
    // contribute to the diffstat.
    let summaries = drain_change_summary(&mut rx);
    assert_eq!(summaries.len(), 1, "exactly one change summary");
    let Event::LoopChangeSummary {
        status,
        files_modified,
        files_created,
        files_deleted,
        diffstat,
        files,
        ..
    } = summaries.into_iter().next().expect("one summary")
    else {
        unreachable!("filtered to LoopChangeSummary");
    };
    assert_eq!(status, "completed");
    assert_eq!(files_modified, 0);
    assert_eq!(files_created, 0);
    assert_eq!(files_deleted, 1, "doomed.txt was deleted");
    assert_eq!(files, vec!["doomed.txt".to_string()]);
    assert_eq!(
        diffstat, "+0 -2 lines across 1 file",
        "the deleted file's removed lines charge the diffstat"
    );

    // FR-020: rollback restores the deleted file from the capture.
    let restored = processor
        .rollback_loop(&session.id)
        .await
        .expect("rollback");
    assert!(restored, "the pending capture was restored");
    assert_eq!(
        std::fs::read_to_string(working_dir.join("doomed.txt")).expect("read restored"),
        "line one\nline two\n",
        "FR-020: the deleted-file restore lane is covered"
    );
    Ok(())
}

/// FR-020 (T-023, decline lane): declining the rollback offer — what the TUI
/// handler does by running `clear_loop_captures` — keeps every change the
/// loop made on disk and discards the pending capture so a later rollback
/// cannot resurrect it.
#[tokio::test]
async fn test_rollback_decline_keeps_loop_changes() -> Result<()> {
    let event_bus = Arc::new(EventBus::new(4096));
    let (processor, working_dir, _captured, _dir_guard) = make_processor(
        event_bus.clone(),
        vec![
            ScriptedReply::ToolCallNamed {
                name: "write",
                args: r#"{"path": "existing.txt", "content": "mutated by the loop"}"#.to_string(),
            },
            ScriptedReply::TextOnly("done"),
        ],
    )?;
    std::fs::write(working_dir.join("existing.txt"), "original").expect("seed file");

    let session = processor
        .session_manager
        .create_session(working_dir.clone())
        .expect("session created");
    let mut spec = LoopSpec::new("general", "mutate the file");
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

    // Decline the offer exactly as the TUI handler does.
    processor.clear_loop_captures().await;

    // FR-020: declining keeps the changes — the mutated content survives.
    assert_eq!(
        std::fs::read_to_string(working_dir.join("existing.txt")).expect("read"),
        "mutated by the loop",
        "FR-020: declining keeps the loop's changes"
    );

    // The discarded capture cannot resurrect: rollback is a no-op.
    let again = processor
        .rollback_loop(&session.id)
        .await
        .expect("rollback");
    assert!(!again, "no capture entry remains after the decline");
    Ok(())
}
