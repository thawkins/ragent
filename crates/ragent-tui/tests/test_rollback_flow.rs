//! Integration tests for the post-termination rollback flow (spec
//! `agentloop`, task T-013 / FR-020):
//!
//! - a published [`Event::LoopChangeSummary`] arms a one-key rollback
//!   offer on the `App` (pending state + status-line hint);
//! - accepting the offer (`Enter`) restores the workspace from the
//!   pre-loop snapshot and drops the pending capture;
//! - declining the offer (`Esc`) keeps every change the loop made and
//!   discards the pending capture so no later rollback can resurrect it;
//! - the offer is not armed for events from a different session.

use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use futures::stream;

use ragent_agent::agent::AgentInfo;
use ragent_agent::event::{Event, EventBus};
use ragent_agent::llm::{ChatRequest, LlmClient, LlmFinishReason, StreamEvent};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::provider::{ModelInfo, Provider, ProviderRegistry};
use ragent_agent::session::SessionManager;
use ragent_agent::session::loop_capture::LoopCapture;
use ragent_agent::storage::Storage;
use ragent_agent::tool;
use ragent_config::{Capabilities, Cost};
use ragent_storage::snapshot::{restore_snapshot, take_snapshot};
use ragent_types::ThinkingConfig;

use ragent_tui::App;
use ragent_tui::app::{ConfiguredProvider, ProviderSource};

#[path = "support/mod.rs"]
mod support;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// A summary event as the processor publishes it after a terminated loop.
fn change_summary(session_id: &str) -> Event {
    Event::LoopChangeSummary {
        session_id: session_id.to_string(),
        status: "completed".to_string(),
        iterations: 3,
        files_modified: 1,
        files_created: 1,
        files_deleted: 0,
        diffstat: "+6 -2 lines across 2 files".to_string(),
        files: vec!["existing.txt".to_string(), "created.txt".to_string()],
    }
}

#[derive(Clone)]
struct ScriptedProvider {
    shared_captured: Arc<std::sync::Mutex<Vec<ChatRequest>>>,
}

struct ScriptedClient {
    captured: Arc<std::sync::Mutex<Vec<ChatRequest>>>,
}

#[async_trait::async_trait]
impl LlmClient for ScriptedClient {
    async fn chat(
        &self,
        request: ChatRequest,
    ) -> Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>> {
        self.captured.lock().expect("captured lock").push(request);
        let events: Vec<StreamEvent> = vec![
            StreamEvent::TextDelta {
                text: "acknowledged".to_string(),
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
            thinking_config: Some(ThinkingConfig::default()),
        }]
    }

    fn as_any_static(&self) -> &(dyn std::any::Any + 'static) {
        self
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &std::collections::HashMap<String, serde_json::Value>,
    ) -> Result<Box<dyn LlmClient>> {
        Ok(Box::new(ScriptedClient {
            captured: Arc::clone(&self.shared_captured),
        }))
    }
}

/// App wired to a scripted provider so the rollback handler's spawned task
/// reaches a real `SessionProcessor::rollback_loop`.
fn make_scripted_app() -> App {
    let captured = Arc::new(std::sync::Mutex::new(Vec::new()));
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let mut provider_registry = ProviderRegistry::new();
    provider_registry.register(Box::new(ScriptedProvider {
        shared_captured: Arc::clone(&captured),
    }));
    let provider_registry = Arc::new(provider_registry);
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(parking_lot::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(ragent_agent::session::processor::SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        agent_manager: std::sync::OnceLock::new(),
        bg_service: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        active_spec: tokio::sync::RwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: parking_lot::RwLock::new(None),
        cached_tool_names: parking_lot::RwLock::new(None),
        cached_tool_definition_bytes: parking_lot::RwLock::new(None),
        llm_client_cache: parking_lot::RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: std::sync::Arc::new(parking_lot::RwLock::new(
            std::collections::HashMap::new(),
        )),
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
            std::collections::HashMap::new(),
        )),
        auto_approve: false,
        system_prompt_cache: parking_lot::RwLock::new(None),
        skill_body_cache: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        telemetry: std::sync::Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        activity_log: std::sync::OnceLock::new(),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
    });
    let agent_info =
        ragent_agent::agent::resolve_agent("general", &Default::default()).expect("resolve agent");
    let mut app = App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        std::sync::Arc::unwrap_or_clone(agent_info),
        false,
        std::path::PathBuf::new(),
    );
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama".to_string(),
        name: "Ollama".to_string(),
        source: ProviderSource::AutoDiscovered,
    });
    app.selected_model = Some("ollama/qwen3:latest".to_string());
    app
}

/// A workspace with a mutated file plus a real pre-loop snapshot capture
/// armed on the processor (mirrors what T-012 leaves pending after
/// termination).
async fn arm_capture(app: &App) -> (tempfile::TempDir, std::path::PathBuf) {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("existing.txt");
    std::fs::write(&target, "original").expect("write original");
    let snapshot =
        take_snapshot("sess-rollback", "msg-1", std::slice::from_ref(&target)).expect("snapshot");
    std::fs::write(&target, "mutated by the loop").expect("mutate");

    let capture = LoopCapture {
        snapshot: Some(snapshot),
        git: None,
    };
    app.session_processor
        .active_loop_captures
        .write()
        .await
        .insert("sess-rollback".to_string(), capture);
    (dir, target)
}

/// FR-020: a change summary arms the rollback offer and hints at the keys.
#[test]
fn test_rollback_offer_arms_on_change_summary() {
    let mut app = make_scripted_app();
    app.session_id = Some("sess-rollback".to_string());
    app.handle_event(change_summary("sess-rollback"));

    let offer = app.pending_rollback.as_ref().expect("offer armed");
    assert_eq!(offer.session_id, "sess-rollback");
    assert_eq!(offer.status, "completed");
    assert_eq!(offer.iterations, 3);
    assert_eq!(offer.diffstat, "+6 -2 lines across 2 files");
    assert_eq!(offer.files.len(), 2);
    assert!(
        app.status.contains("roll back"),
        "status line hints at the rollback keys: {}",
        app.status
    );
}

/// FR-020: the offer is not armed for a different session's summary.
#[test]
fn test_rollback_offer_ignores_other_sessions() {
    let mut app = make_scripted_app();
    app.session_id = Some("sess-rollback".to_string());
    app.handle_event(change_summary("sess-other"));
    assert!(
        app.pending_rollback.is_none(),
        "a foreign session's summary must not arm the offer"
    );
}

/// FR-020: accepting the offer restores the pre-loop snapshot.
#[tokio::test]
async fn test_rollback_accept_restores_snapshot() -> Result<()> {
    let mut app = make_scripted_app();
    let (_dir_guard, target) = arm_capture(&app).await;
    app.session_id = Some("sess-rollback".to_string());
    app.handle_event(change_summary("sess-rollback"));
    assert!(app.pending_rollback.is_some());

    // Route through the App handler so the returned InputAction is
    // dispatched exactly as the runtime does.
    app.handle_key_event(key(KeyCode::Enter));

    // The spawned task restores the captured contents and drops the capture.
    for _ in 0..200 {
        if app
            .session_processor
            .active_loop_captures
            .read()
            .await
            .is_empty()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        app.session_processor
            .active_loop_captures
            .read()
            .await
            .is_empty(),
        "the pending capture is dropped after the restore"
    );
    let content = std::fs::read_to_string(&target).expect("read restored");
    assert_eq!(
        content, "original",
        "FR-020: the pre-loop contents are back"
    );
    Ok(())
}

/// FR-020: declining the offer keeps the changes and discards the capture.
#[tokio::test]
async fn test_rollback_decline_keeps_changes() -> Result<()> {
    let mut app = make_scripted_app();
    let (_dir_guard, target) = arm_capture(&app).await;
    app.session_id = Some("sess-rollback".to_string());
    app.handle_event(change_summary("sess-rollback"));

    app.handle_key_event(key(KeyCode::Esc));

    for _ in 0..200 {
        if app
            .session_processor
            .active_loop_captures
            .read()
            .await
            .is_empty()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(
        app.session_processor
            .active_loop_captures
            .read()
            .await
            .is_empty(),
        "the pending capture is discarded after a decline"
    );
    let content = std::fs::read_to_string(&target).expect("read");
    assert_eq!(
        content, "mutated by the loop",
        "FR-020: declining keeps the loop's changes"
    );
    let transcript = app
        .messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default();
    assert!(
        transcript.contains("declined"),
        "the user is told the changes are kept: {transcript}"
    );
    Ok(())
}

/// A termination event as the processor publishes it (T-008).
fn terminated(session_id: &str, status: &str, iterations: u64) -> Event {
    Event::LoopTerminated {
        session_id: session_id.to_string(),
        status: status.to_string(),
        iterations,
        verification: if status == "completed" {
            Some("check.py: ok".to_string())
        } else {
            None
        },
        reason: if status == "completed" {
            None
        } else {
            Some("test reason".to_string())
        },
    }
}

/// Last transcript message text (empty when no messages).
fn last_transcript(app: &App) -> String {
    app.messages
        .last()
        .map(|m| m.text_content())
        .unwrap_or_default()
}

/// T-025 (FR-019): each termination status renders a distinct banner with the
/// icon, phrase, and iteration count; verification/reason detail is appended.
#[test]
fn test_loop_terminated_banner_is_status_aware() {
    let cases = [
        ("completed", "✓", "Goal loop completed", 2, "2 iterations"),
        ("error", "✗", "Goal loop failed", 1, "1 iteration"),
        (
            "budget_exhausted",
            "⏳",
            "budget exhausted",
            5,
            "5 iterations",
        ),
        ("interrupted", "⏹", "interrupted", 3, "3 iterations"),
    ];
    for (status, icon, phrase, iterations, iterations_label) in cases {
        let mut app = make_scripted_app();
        app.session_id = Some("sess-rollback".to_string());
        app.handle_event(terminated("sess-rollback", status, iterations));
        let transcript = last_transcript(&app);
        assert!(
            transcript.contains(icon) && transcript.contains(phrase),
            "status {status}: banner phrase missing: {transcript}"
        );
        assert!(
            transcript.contains(iterations_label),
            "status {status}: iteration count missing: {transcript}"
        );
    }

    // completed carries the verification outcome; error carries the reason.
    let mut app = make_scripted_app();
    app.session_id = Some("sess-rollback".to_string());
    app.handle_event(terminated("sess-rollback", "completed", 2));
    assert!(
        last_transcript(&app).contains("check.py: ok"),
        "verification outcome appended for completed"
    );

    let mut app = make_scripted_app();
    app.session_id = Some("sess-rollback".to_string());
    app.handle_event(terminated("sess-rollback", "interrupted", 3));
    let transcript = last_transcript(&app);
    assert!(
        transcript.contains("test reason"),
        "failure reason appended for non-success: {transcript}"
    );

    // A foreign session's termination is not rendered.
    let mut app = make_scripted_app();
    app.session_id = Some("sess-rollback".to_string());
    app.handle_event(terminated("sess-other", "error", 1));
    assert!(
        !last_transcript(&app).contains("Goal loop"),
        "foreign session termination must not render"
    );
}

/// T-025 (FR-019): the change summary renders as a diffstat line in the
/// message window together with the rollback offer prompt (FR-020).
#[test]
fn test_change_summary_renders_diffstat_and_rollback_offer() {
    let mut app = make_scripted_app();
    app.session_id = Some("sess-rollback".to_string());
    app.handle_event(change_summary("sess-rollback"));

    let transcript = last_transcript(&app);
    assert!(
        transcript.contains("Loop changes"),
        "diffstat line rendered: {transcript}"
    );
    assert!(
        transcript.contains("+6 -2 lines across 2 files"),
        "diffstat included: {transcript}"
    );
    assert!(
        transcript.contains("1 modified, 1 created, 0 deleted"),
        "file counts included: {transcript}"
    );
    assert!(
        transcript.contains("Roll back"),
        "rollback offer prompt rendered: {transcript}"
    );
    assert!(
        transcript.contains("Enter: roll back") && transcript.contains("Esc: keep changes"),
        "offer names both keys: {transcript}"
    );
    // The status-line hint from T-013 still holds.
    assert!(app.status.contains("roll back"));
}

/// The snapshot plumbing round-trips through the storage restore helper.
#[test]
fn test_snapshot_restore_roundtrip_is_idempotent_shape() -> Result<()> {
    let dir = tempfile::tempdir().expect("tempdir");
    let target = dir.path().join("file.txt");
    std::fs::write(&target, "v1").expect("write");
    let snapshot = take_snapshot("sess-x", "msg-x", std::slice::from_ref(&target))?;
    std::fs::write(&target, "v2")?;
    restore_snapshot(&snapshot)?;
    assert_eq!(std::fs::read_to_string(&target)?, "v1");
    Ok(())
}

/// Type-shape guards for imports used only by test assertions.
#[allow(dead_code)]
fn type_guards() {
    let _ = AgentInfo::new("general", "General");
    let flag: Arc<AtomicBool> = Arc::new(AtomicBool::new(false));
    assert!(!flag.load(std::sync::atomic::Ordering::Relaxed));
}
