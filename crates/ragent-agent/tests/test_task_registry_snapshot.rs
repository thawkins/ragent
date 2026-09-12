//! Tests for `AgentManager::tasks_snapshot`.
//!
//! The TUI reconciles its event-driven `active_tasks` view against this
//! snapshot after a broadcast-channel `Lagged` burst drops
//! `SubagentStart`/`SubagentComplete` events; the snapshot must return every
//! entry in the map (running and completed) regardless of parent session so
//! the merge can both re-add missing agents and remove stale ghosts.

use std::sync::Arc;

use chrono::Utc;
use parking_lot::RwLock;
use ragent_agent::event::EventBus;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::task::{AgentManager, TaskEntry, TaskStatus};
use ragent_agent::tool::ToolRegistry;
use ragent_llm::provider::ProviderRegistry;
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
        skill_registry_cache: parking_lot::Mutex::new(None),
        active_loops: TokioRwLock::new(std::collections::HashMap::new()),
        active_loop_specs: TokioRwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: TokioRwLock::new(std::collections::HashMap::new()),
    })
}

/// Build a task entry with an arbitrary status (used to seed the registry).
fn entry(id: &str, status: TaskStatus) -> TaskEntry {
    TaskEntry {
        id: id.to_string(),
        parent_session_id: "parent-sess".to_string(),
        child_session_id: format!("child-{id}"),
        agent_name: "explore".to_string(),
        task_prompt: "x".to_string(),
        background: true,
        status,
        result: None,
        error: None,
        created_at: Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ragent_agent::task::ReportStatus::default(),
    }
}

#[tokio::test]
async fn test_tasks_snapshot_returns_every_entry_regardless_of_status() {
    let event_bus = Arc::new(EventBus::new(16));
    let manager = Arc::new(AgentManager::new(event_bus, test_processor(), 4, 300));

    // `seed_completed_for_test` inserts any entry directly into the map;
    // the name only reflects its original purpose.
    manager
        .seed_completed_for_test(entry("t-running", TaskStatus::Running))
        .await;
    manager
        .seed_completed_for_test(entry("t-completed", TaskStatus::Completed))
        .await;
    manager
        .seed_completed_for_test(entry("t-failed", TaskStatus::Failed))
        .await;
    manager
        .seed_completed_for_test(entry("t-cancelled", TaskStatus::Cancelled))
        .await;

    let snapshot = manager.tasks_snapshot().await;
    let ids: Vec<String> = snapshot.iter().map(|e| e.id.clone()).collect();
    assert_eq!(ids.len(), 4, "all entries returned, got: {ids:?}");
    assert!(ids.contains(&"t-running".to_string()));
    assert!(ids.contains(&"t-completed".to_string()));
    assert!(ids.contains(&"t-failed".to_string()));
    assert!(ids.contains(&"t-cancelled".to_string()));

    // Statuses round-trip so the reconciler can distinguish live agents
    // from ghosts of dropped completions.
    let by_id: std::collections::HashMap<String, TaskStatus> =
        snapshot.into_iter().map(|e| (e.id, e.status)).collect();
    assert_eq!(by_id.get("t-running"), Some(&TaskStatus::Running));
    assert_eq!(by_id.get("t-completed"), Some(&TaskStatus::Completed));
}

#[tokio::test]
async fn test_tasks_snapshot_empty_when_no_tasks() {
    let event_bus = Arc::new(EventBus::new(16));
    let manager = Arc::new(AgentManager::new(event_bus, test_processor(), 4, 300));
    let snapshot = manager.tasks_snapshot().await;
    assert!(
        snapshot.is_empty(),
        "expected empty snapshot, got {:?}",
        snapshot.iter().map(|e| e.id.clone()).collect::<Vec<_>>()
    );
}
