//! Regression tests for the detached (`/spawn`) sub-agent path and the
//! completion-bookkeeping gaps that made `/spawn general … write ANTIPAT.md`
//! appear to do nothing.
//!
//! Covered regressions (live-observed):
//!
//! 1. **`output_file` was never populated** — the task layer never wrote
//!    `log/subagents/<task-id>.md`, so the durable recovery path documented
//!    in `wait_agents`/`list_agents` did not exist on disk.
//! 2. **`finish_reason` was hard-coded `"stop"`** — a run cut by the
//!    provider's silent end-of-stream still claimed a healthy finish in the
//!    Agents panel and the log line.
//! 3. **Detached visibility semantics** — a detached task must be invisible
//!    to the delegation surface (`list_agents`, `running_background_count`,
//!    `wait_agents` default wait-set, `drain_completed` injection) while
//!    still being reaped and still present in `tasks_snapshot` (the Agents
//!    panel reconcile source).

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use parking_lot::RwLock;
use ragent_agent::event::EventBus;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::task::{AgentManager, ReportStatus, TaskEntry, TaskStatus, persist_task_output};
use ragent_agent::tool::wait_agents::WaitAgentsTool;
use ragent_agent::tool::{Tool, ToolContext, ToolRegistry};
use ragent_llm::provider::ProviderRegistry;
use serde_json::json;
use tokio::sync::RwLock as TokioRwLock;

fn test_processor() -> Arc<SessionProcessor> {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let event_bus = Arc::new(EventBus::new(16));
    let session_manager = Arc::new(SessionManager::new(storage, event_bus.clone()));
    Arc::new(SessionProcessor {
        session_manager,
        provider_registry: Arc::new(ProviderRegistry::new()),
        tool_registry: Arc::new(ToolRegistry::new()),
        permission_checker: Arc::new(RwLock::new(PermissionChecker::new(vec![]))),
        event_bus,
        agent_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
        mcp_client: std::sync::OnceLock::new(),
        code_index: std::sync::OnceLock::new(),
        active_spec: TokioRwLock::new(None),
        spec_manager: std::sync::OnceLock::new(),
        cached_tool_definitions: RwLock::new(None),
        cached_tool_names: RwLock::new(None),
        cached_tool_definition_bytes: RwLock::new(None),
        llm_client_cache: RwLock::new(std::collections::HashMap::new()),
        cached_config: parking_lot::Mutex::new(None),
        team_context_cache: Arc::new(RwLock::new(std::collections::HashMap::new())),
        tool_repeat_guard: std::sync::Arc::new(parking_lot::Mutex::new(
            std::collections::HashMap::new(),
        )),
        extraction_engine: std::sync::OnceLock::new(),
        stream_config: ragent_agent::StreamConfig::default(),
        auto_approve: false,
        system_prompt_cache: RwLock::new(None),
        skill_body_cache: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        telemetry: Arc::new(ragent_agent::telemetry::TelemetrySubsystem::disabled()),
        bg_service: std::sync::OnceLock::new(),
        activity_log: std::sync::OnceLock::new(),
        activity_log_tx: tokio::sync::Mutex::new(None),
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: TokioRwLock::new(std::collections::HashMap::new()),
        active_loop_specs: TokioRwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: TokioRwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
    })
}

fn make_ctx(session_id: &str, event_bus: Arc<EventBus>, manager: Arc<AgentManager>) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus,
        storage: None,
        agent_manager: Some(manager),
        active_model: None,
        provider_registry: None,
        team_context: None,
        team_manager: None,
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        allowed_roots: Vec::new(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        tool_registry: ToolContext::default_tool_registry(),
        canonical_cache: std::sync::Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

/// Build a completed background task entry for a given parent session.
fn completed_entry(parent_sid: &str, task_id: &str, detached: bool, result: &str) -> TaskEntry {
    TaskEntry {
        id: task_id.to_string(),
        parent_session_id: parent_sid.to_string(),
        child_session_id: format!("child-{task_id}"),
        agent_name: "general".to_string(),
        task_prompt: "write the report".to_string(),
        background: true,
        detached,
        status: TaskStatus::Completed,
        result: Some(Arc::from(result)),
        error: None,
        created_at: Utc::now(),
        completed_at: Some(Utc::now()),
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::default(),
    }
}

// ---------------------------------------------------------------------------
// 1. persist_task_output - the durable report file is written.
// ---------------------------------------------------------------------------

#[test]
fn test_persist_task_output_writes_full_report_file() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = persist_task_output(
        dir.path(),
        "general-abc12345",
        "general",
        "audit prompt",
        42,
        "FULL REPORT BODY",
    )
    .expect("persist should succeed in a writable dir");

    assert_eq!(
        path,
        dir.path().join("log/subagents/general-abc12345.md"),
        "report lands at log/subagents/<task-id>.md under the working dir"
    );
    let body = std::fs::read_to_string(&path).expect("report readable");
    assert!(body.contains("FULL REPORT BODY"));
    assert!(body.contains("task: audit prompt"));
    assert!(body.contains("duration_ms: 42"));
    // No half-written temp file left behind.
    assert!(
        !dir.path()
            .join("log/subagents/.general-abc12345.md.tmp")
            .exists(),
        "temp file must be renamed away"
    );
}

#[test]
fn test_persist_task_output_failure_returns_none_not_panic() {
    // A path that cannot be created (child of an existing *file*) must yield
    // `None`, not an unwrap/panic — the caller then leaves `output_file`
    // unset rather than crashing the completion path.
    let dir = tempfile::tempdir().expect("tempdir");
    let blocker = dir.path().join("not-a-dir");
    std::fs::write(&blocker, b"file").unwrap();
    let bad_root = blocker.join("child"); // create_dir_all under a file fails
    let result = persist_task_output(&bad_root, "t-x", "general", "p", 1, "body");
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// 2. last_message_end_reason - real finish reasons recorded & readable.
// ---------------------------------------------------------------------------

#[test]
fn test_last_message_end_reason_round_trip() {
    let processor = test_processor();
    assert!(processor.last_message_end_reason("sess-x").is_none());

    processor
        .last_message_end_reason
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(
            "sess-x".to_string(),
            ragent_agent::event::FinishReason::Truncation,
        );

    assert_eq!(
        processor.last_message_end_reason("sess-x"),
        Some(ragent_agent::event::FinishReason::Truncation),
        "the task registry reads the real terminal reason instead of \
         publishing a hard-coded \"stop\""
    );
    // Other sessions are unaffected.
    assert!(processor.last_message_end_reason("sess-y").is_none());
}

/// The single classifier the Agents panel and the task registry both use must
/// agree on every `finish_reason` label that can reach them, so a truncated
/// run is never painted as a healthy finish.
#[test]
fn test_report_status_classification_is_consistent() {
    // Provider cut the reply: all spellings mean an incomplete report.
    for label in ["length", "truncation", "content_filter", "truncated"] {
        assert_eq!(
            ReportStatus::from_finish_reason_label(label),
            ReportStatus::Truncated,
            "'{label}' means the provider cut the reply short"
        );
    }
    // Continuation retry recovered the tail.
    assert_eq!(
        ReportStatus::from_finish_reason_label("continued"),
        ReportStatus::Continued
    );
    // Healthy finishes and the empty spawn-placeholder label.
    for label in ["stop", "tool_use", "cancelled", ""] {
        assert_eq!(
            ReportStatus::from_finish_reason_label(label),
            ReportStatus::Complete,
            "'{label}' is not a truncation signature"
        );
    }
}

// ---------------------------------------------------------------------------
// 3. Detached visibility semantics.
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_detached_completed_task_hidden_from_delegation_surface() {
    let event_bus = Arc::new(EventBus::new(16));
    let manager = Arc::new(AgentManager::new(
        event_bus.clone(),
        test_processor(),
        4,
        300,
    ));
    let parent = "parent-sess";

    // One detached task and one normal background task for contrast.
    manager
        .seed_completed_for_test(completed_entry(
            parent,
            "general-detached1",
            true,
            "detached out",
        ))
        .await;
    manager
        .seed_completed_for_test(completed_entry(
            parent,
            "general-normal01",
            false,
            "normal out",
        ))
        .await;

    // (a) list_agents hides detached entries.
    let listed = manager.list_agents(parent).await;
    assert!(
        listed.iter().all(|e| e.id == "general-normal01"),
        "list_agents must exclude detached entries, got {:?}",
        listed.iter().map(|e| e.id.clone()).collect::<Vec<_>>()
    );

    // (b) running_background_count is unaffected by detached entries.
    assert_eq!(manager.running_background_count().await, 0);

    // (c) wait_agents with omit-task_ids only reports the normal task —
    //     the detached one is invisible even to the tool's default wait set.
    let ctx = make_ctx(parent, event_bus, Arc::clone(&manager));
    let out = WaitAgentsTool
        .execute(json!({}), &ctx)
        .await
        .expect("wait_agents");
    assert!(
        out.content.contains("1 task(s) completed"),
        "only the normal task is reported, got: {}",
        out.content
    );
    assert!(
        !out.content.contains("detached out"),
        "detached task body must never leak into wait_agents output, got: {}",
        out.content
    );
}

#[tokio::test]
async fn test_detached_task_reaped_but_present_in_snapshot() {
    let event_bus = Arc::new(EventBus::new(16));
    let manager = Arc::new(AgentManager::new(event_bus, test_processor(), 4, 300));
    let parent = "parent-sess";

    manager
        .seed_completed_for_test(completed_entry(parent, "general-detached1", true, "x"))
        .await;
    // Real spawns trip the P-11 flag; the seeding helper intentionally does
    // not, so arm it here to exercise the same drain path a live run takes.
    manager.set_pending_background_for_test();

    // The Agents-panel reconcile source still sees it (running *and*
    // completed entries) so UI ghost-state cleanup can work.
    let snapshot = manager.tasks_snapshot().await;
    assert!(snapshot.iter().any(|e| e.id == "general-detached1"));

    // drain_completed returns nothing for the parent (no injection)…
    let drained = manager.drain_completed(parent).await;
    assert!(
        drained.is_empty(),
        "detached completions are reaped, never injected"
    );
    // …and the entry is reaped from the task map so the registry does not
    // leak completed detached agents for the process lifetime.
    assert!(
        manager.get_task("general-detached1").await.is_none(),
        "detached entries must be reaped after drain"
    );
    // has_pending_background clears once nothing remains pending.
    assert!(!manager.has_pending_background());
}
