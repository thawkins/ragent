//! FR-015: a suspended task is not reported as running by `wait_agents`.
//!
//! A parked (suspended) background task is still awaited - only a terminal
//! status settles a wait (FR-030) - but it produces no work while it is parked.
//! A wait that returns with such a task still parked must therefore say so:
//!
//! - the content must report the task as `suspended`, never as "still running";
//! - `metadata.suspended` / `metadata.suspended_count` must name it;
//! - `metadata.still_running` / `metadata.still_running_count` must not include
//!   it, so a caller cannot mistake a parked task for live work;
//! - `metadata.unsettled[*].status` must carry the canonical `suspended` label;
//! - the task must never be reported as completed.
//!
//! The registry is seeded directly (the same state `suspend_task` leaves an
//! entry in) because driving a real background sub-agent to a parked state
//! needs a live provider.

use std::path::PathBuf;
use std::sync::Arc;

use chrono::Utc;
use parking_lot::RwLock;
use ragent_agent::event::EventBus;
use ragent_agent::permission::PermissionChecker;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::task::{AgentManager, TaskEntry, TaskStatus};
use ragent_agent::tool::wait_agents::WaitAgentsTool;
use ragent_agent::tool::{Tool, ToolContext, ToolOutput, ToolRegistry};
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
        connector_session: tokio::sync::RwLock::new(None),
        connector_statuses: tokio::sync::RwLock::new(None),
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
        active_loops: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        active_loop_specs: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        loop_telemetry_recorded: std::sync::atomic::AtomicBool::new(false),
        active_loop_interrupts: parking_lot::RwLock::new(std::collections::HashMap::new()),
        active_loop_captures: tokio::sync::RwLock::new(std::collections::HashMap::new()),
        last_message_end_reason: std::sync::RwLock::new(std::collections::HashMap::new()),
        pending_verdicts: ragent_agent::session::pending::PendingVerdicts::new(),
    })
}

fn make_ctx(
    session_id: &str,
    event_bus: Arc<EventBus>,
    agent_manager: Arc<AgentManager>,
) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        working_dir: PathBuf::from("target/temp"),
        event_bus,
        storage: None,
        agent_manager: Some(agent_manager),
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
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        canonical_cache: std::sync::Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

/// Seed one background task in an arbitrary status straight into the registry.
async fn seed_task(
    manager: &AgentManager,
    parent_session_id: &str,
    task_id: &str,
    status: TaskStatus,
) {
    let entry = TaskEntry {
        id: task_id.to_string(),
        parent_session_id: parent_session_id.to_string(),
        child_session_id: format!("child-{task_id}"),
        agent_name: "explore".to_string(),
        task_prompt: "ignored".to_string(),
        background: true,
        detached: false,
        status,
        result: None,
        error: None,
        created_at: Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ragent_agent::task::ReportStatus::default(),
        time_bound_secs: None,
    };
    manager.seed_completed_for_test(entry).await;
}

/// `metadata.<key>` as an array of strings, or an empty vector when absent.
fn string_array(output: &ToolOutput, key: &str) -> Vec<String> {
    output
        .metadata
        .as_ref()
        .and_then(|m| m.get(key))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// `metadata.<key>` as a number.
fn number(output: &ToolOutput, key: &str) -> u64 {
    output
        .metadata
        .as_ref()
        .and_then(|m| m.get(key))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or_else(|| {
            panic!(
                "metadata.{key} must be a number; got {:#?}",
                output.metadata
            )
        })
}

/// A parked task is reported as `suspended` in every place the wait names it,
/// and never as running.
#[tokio::test]
async fn test_wait_agents_reports_suspended_task_as_suspended() {
    let event_bus = Arc::new(EventBus::new(16));
    let manager = Arc::new(AgentManager::new(
        event_bus.clone(),
        test_processor(),
        4,
        300,
    ));
    let parent_sid = "parent-sess";

    seed_task(
        &manager,
        parent_sid,
        "task-parked-001",
        TaskStatus::Suspended,
    )
    .await;

    let ctx = make_ctx(parent_sid, event_bus.clone(), Arc::clone(&manager));
    let output = WaitAgentsTool
        .execute(
            json!({"task_ids": ["task-parked-001"], "timeout_secs": 1}),
            &ctx,
        )
        .await
        .expect("wait_agents should succeed");

    // The parked task is named as suspended, not as still running.
    assert_eq!(
        number(&output, "suspended_count"),
        1,
        "FR-015: the parked task must be counted in suspended_count; got: {}",
        output.content
    );
    assert_eq!(
        string_array(&output, "suspended"),
        vec!["task-parked-001".to_string()],
        "FR-015: the parked task id must be listed under `suspended`"
    );
    assert_eq!(
        number(&output, "still_running_count"),
        0,
        "FR-015: a parked task must not be counted as still running"
    );
    assert!(
        string_array(&output, "still_running").is_empty(),
        "FR-015: `still_running` must not name a parked task"
    );

    // The content must say `suspended` and must not call the task running.
    assert!(
        output.content.contains("suspended"),
        "FR-015: the content must report the task as suspended; got: {}",
        output.content
    );
    assert!(
        !output.content.contains("still running"),
        "FR-015: the content must not describe a parked task as still running; got: {}",
        output.content
    );
}

/// The per-task entries of the tasks that did not settle carry their real
/// status, so a caller can branch on `suspended` without parsing prose.
#[tokio::test]
async fn test_wait_agents_unsettled_entries_carry_the_suspended_status() {
    let event_bus = Arc::new(EventBus::new(16));
    let manager = Arc::new(AgentManager::new(
        event_bus.clone(),
        test_processor(),
        4,
        300,
    ));
    let parent_sid = "parent-sess";

    seed_task(
        &manager,
        parent_sid,
        "task-parked-002",
        TaskStatus::Suspended,
    )
    .await;

    let ctx = make_ctx(parent_sid, event_bus.clone(), Arc::clone(&manager));
    let output = WaitAgentsTool
        .execute(
            json!({"task_ids": ["task-parked-002"], "timeout_secs": 1}),
            &ctx,
        )
        .await
        .expect("wait_agents should succeed");

    let unsettled = output
        .metadata
        .as_ref()
        .and_then(|m| m.get("unsettled"))
        .and_then(serde_json::Value::as_array)
        .expect("metadata.unsettled must be an array");

    let entry = unsettled
        .iter()
        .find(|e| e.get("task_id").and_then(serde_json::Value::as_str) == Some("task-parked-002"))
        .expect("the parked task must appear in metadata.unsettled");

    assert_eq!(
        entry.get("status").and_then(serde_json::Value::as_str),
        Some(TaskStatus::Suspended.as_str()),
        "FR-015/FR-007: an unsettled entry must carry the canonical status label"
    );

    // A parked task never settled, so it must not appear in the completed set.
    assert_eq!(
        number(&output, "completed_count"),
        0,
        "FR-015: the wait must not claim the parked task completed"
    );
}

/// A running task is still reported as running and is not folded into the
/// suspended set - the distinction is real in both directions.
#[tokio::test]
async fn test_wait_agents_keeps_a_running_task_out_of_the_suspended_set() {
    let event_bus = Arc::new(EventBus::new(16));
    let manager = Arc::new(AgentManager::new(
        event_bus.clone(),
        test_processor(),
        4,
        300,
    ));
    let parent_sid = "parent-sess";

    seed_task(&manager, parent_sid, "task-running-01", TaskStatus::Running).await;

    let ctx = make_ctx(parent_sid, event_bus.clone(), Arc::clone(&manager));
    let output = WaitAgentsTool
        .execute(
            json!({"task_ids": ["task-running-01"], "timeout_secs": 1}),
            &ctx,
        )
        .await
        .expect("wait_agents should succeed");

    assert_eq!(
        number(&output, "still_running_count"),
        1,
        "a running task must still be counted as running; got: {}",
        output.content
    );
    assert_eq!(
        number(&output, "suspended_count"),
        0,
        "a running task must not be counted as suspended"
    );
    assert!(
        output.content.contains("still running"),
        "a genuinely running task keeps the still-running wording; got: {}",
        output.content
    );
    assert!(
        !output.content.contains("suspended"),
        "a running task must not be described as suspended; got: {}",
        output.content
    );
}

/// Both kinds in one wait: each is named under its own state, and the summary
/// tells the caller that the parked one needs a resume or cancel to progress.
#[tokio::test]
async fn test_wait_agents_mixed_running_and_suspended_are_reported_separately() {
    let event_bus = Arc::new(EventBus::new(16));
    let manager = Arc::new(AgentManager::new(
        event_bus.clone(),
        test_processor(),
        4,
        300,
    ));
    let parent_sid = "parent-sess";

    seed_task(&manager, parent_sid, "task-running-02", TaskStatus::Running).await;
    seed_task(
        &manager,
        parent_sid,
        "task-parked-003",
        TaskStatus::Suspended,
    )
    .await;

    let ctx = make_ctx(parent_sid, event_bus.clone(), Arc::clone(&manager));
    let output = WaitAgentsTool
        .execute(
            json!({"task_ids": ["task-running-02", "task-parked-003"], "timeout_secs": 1}),
            &ctx,
        )
        .await
        .expect("wait_agents should succeed");

    assert_eq!(number(&output, "still_running_count"), 1);
    assert_eq!(number(&output, "suspended_count"), 1);
    assert_eq!(
        string_array(&output, "still_running"),
        vec!["task-running-02".to_string()]
    );
    assert_eq!(
        string_array(&output, "suspended"),
        vec!["task-parked-003".to_string()]
    );
    assert!(
        output.content.contains("resumed or cancelled"),
        "FR-015: the summary must say the parked task needs a resume or cancel; got: {}",
        output.content
    );
    assert_eq!(
        number(&output, "completed_count"),
        0,
        "neither task settled, so neither may be reported as completed"
    );
}
