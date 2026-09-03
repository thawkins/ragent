#![allow(clippy::assert_is_empty)]
//! Regression test for the recurring "wait_agents reports truncated output"
//! complaint.
//!
//! Root cause: when the combined output from several completed background
//! sub-agents exceeds the generic 12 000-char tool-result budget, the
//! head+tail truncation in `session/history.rs::tool_result_content_for_llm`
//! silently drops one agent's report from the middle.  The previously-fixed
//! paths in `tool/wait_agents.rs` (lines 120-131 for already-completed tasks
//! and 169-174 for tasks completing during the wait loop) both return the
//! full `entry.result`; what was missing is a durable way for the model to
//! recover that full report after the generic truncation fires.
//!
//! Fix: `wait_agents` now mirrors every completed agent's full output into
//! the tool's JSON metadata as a `"results"` array
//! (`{task_id, agent, success, output}` per entry) so the data survives in
//! the persisted `ToolCallState` regardless of the context-window cut.
//!
//! This test seeds a pseudo-completed background task directly into the
//! `AgentManager` map (the same state it would reach after
//! `processor.process_message` returns) and drives `wait_agents` against
//! it, asserting the full long output is recoverable from both `content`
//! and `metadata.results[*].output`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use parking_lot::RwLock;
use ragent_agent::event::{Event, EventBus};
use ragent_agent::permission::PermissionChecker;
use ragent_agent::session::SessionManager;
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::storage::Storage;
use ragent_agent::task::{AgentManager, TaskEntry, TaskStatus};
use ragent_agent::tool::wait_agents::WaitAgentsTool;
use ragent_agent::tool::{Tool, ToolContext, ToolRegistry};
use ragent_llm::provider::{ModelInfo, Provider, ProviderRegistry};
use serde_json::json;
use tokio::sync::RwLock as TokioRwLock;

/// Provider whose model resolution panics, used to simulate a sub-agent
/// that dies inside its background task.
#[derive(Clone)]
struct PanicProvider;

#[async_trait::async_trait]
impl Provider for PanicProvider {
    fn id(&self) -> &'static str {
        "panic"
    }

    fn name(&self) -> &'static str {
        "Panic Provider"
    }

    fn default_models(&self) -> Vec<ModelInfo> {
        panic!("simulated sub-agent panic during model resolution");
    }

    async fn create_client(
        &self,
        _api_key: &str,
        _base_url: Option<&str>,
        _options: &std::collections::HashMap<String, serde_json::Value>,
    ) -> anyhow::Result<Box<dyn ragent_agent::llm::LlmClient>> {
        panic!("simulated sub-agent panic during client creation");
    }

    fn as_any_static(&self) -> &dyn std::any::Any {
        self
    }
}

fn test_processor_with_panic_provider() -> Arc<SessionProcessor> {
    let processor = test_processor();
    let mut processor = Arc::try_unwrap(processor)
        .unwrap_or_else(|_| panic!("test processor should have refcount 1"));
    if let Some(registry) = Arc::get_mut(&mut processor.provider_registry) {
        registry.register(Box::new(PanicProvider));
    }
    Arc::new(processor)
}

#[tokio::test]
async fn test_wait_agents_returns_when_background_task_panics() {
    let event_bus = Arc::new(EventBus::new(16));
    let processor = test_processor_with_panic_provider();
    let manager = Arc::new(AgentManager::new(event_bus.clone(), processor, 4, 300));
    let parent_sid = "parent-sess";

    let entry = manager
        .spawn_background(
            parent_sid,
            "explore",
            "do the thing",
            None,
            &PathBuf::from("/tmp"),
        )
        .await
        .expect("spawn should succeed");

    let ctx = make_ctx(parent_sid, event_bus.clone(), Arc::clone(&manager));
    let tool = WaitAgentsTool;

    let output = tokio::time::timeout(
        Duration::from_secs(5),
        tool.execute(json!({"task_ids": [entry.id], "timeout_secs": 60}), &ctx),
    )
    .await
    .expect("wait_agents should return quickly after a panic")
    .expect("wait_agents should succeed");

    assert!(
        output.content.contains("panicked"),
        "wait_agents should surface the panic failure; got: {}",
        output.content
    );
}

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
    })
}

fn make_ctx(
    session_id: &str,
    event_bus: Arc<EventBus>,
    agent_manager: Arc<AgentManager>,
) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        working_dir: PathBuf::from("/tmp"),
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
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        canonical_cache: std::sync::Arc::new(ragent_tools_core::CanonicalPathCache::new()),
    }
}

/// Seed a completed background task directly into the manager map (same state
/// it reaches after `processor.process_message` succeeds).
async fn seed_completed_task(
    manager: &AgentManager,
    parent_session_id: &str,
    task_id: &str,
    agent_name: &str,
    result: &str,
    output_file: Option<PathBuf>,
) {
    let entry = TaskEntry {
        id: task_id.to_string(),
        parent_session_id: parent_session_id.to_string(),
        child_session_id: format!("child-{task_id}"),
        agent_name: agent_name.to_string(),
        task_prompt: "ignored".to_string(),
        background: true,
        status: TaskStatus::Completed,
        result: Some(Arc::from(result)),
        error: None,
        created_at: Utc::now(),
        completed_at: Some(Utc::now()),
        reported: false,
        waiter_count: 0,
        output_file,
        report_status: ragent_agent::task::ReportStatus::default(),
    };
    manager.seed_completed_for_test(entry).await;
}

#[tokio::test]
async fn test_wait_agents_results_meta_contains_full_output() {
    let event_bus = Arc::new(EventBus::new(16));
    let processor = test_processor();
    let manager = Arc::new(AgentManager::new(event_bus.clone(), processor, 4, 300));
    let parent_sid = "parent-sess";
    let task_id = "task-alpha-0000";
    // Deliberately longer than a single chunk of text the agent actually
    // writes — ensures the results[] path carries the whole report, not a
    // truncated preview.
    let long_report = format!(
        "FINDINGS: {}\n\nDetailed report body end.",
        "x".repeat(20_000)
    );
    let report_path = PathBuf::from("/project/log/subagents/task-alpha-0000.md");
    seed_completed_task(
        &manager,
        parent_sid,
        task_id,
        "explore",
        &long_report,
        Some(report_path.clone()),
    )
    .await;

    let ctx = make_ctx(parent_sid, event_bus.clone(), Arc::clone(&manager));
    let tool = WaitAgentsTool;
    let output = tool
        .execute(json!({}), &ctx)
        .await
        .expect("wait_agents should succeed");

    // 1. `content` still carries the full report (previously fixed path).
    assert!(
        output.content.contains(&long_report),
        "content must include the full agent report; got {} chars",
        output.content.len()
    );
    assert!(output.content.contains("1 task(s) completed"));

    // 2. Metadata `results[]` mirrors the same full output so the data
    //    survives the generic 12k head+tail truncation applied later in
    //    `history.rs::tool_result_content_for_llm`.
    let meta = output.metadata.expect("metadata must be present");
    let results = meta
        .get("results")
        .and_then(serde_json::Value::as_array)
        .expect("results array must exist");
    assert_eq!(results.len(), 1, "one completed task → one results entry");
    let entry = &results[0];
    assert_eq!(
        entry.get("task_id").and_then(serde_json::Value::as_str),
        Some(task_id)
    );
    assert_eq!(
        entry.get("agent").and_then(serde_json::Value::as_str),
        Some("explore")
    );
    assert_eq!(
        entry.get("success").and_then(serde_json::Value::as_bool),
        Some(true)
    );
    let output_text = entry
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("results[0].output must be a string");
    assert_eq!(
        output_text, long_report,
        "results[0].output must be the COMPLETE agent report, not a truncated preview"
    );

    // 3. The durable on-disk report path is surfaced in BOTH the tool
    //    content and the metadata results entry so the parent agent can
    //    recover omitted findings with the `read` tool.
    assert_eq!(
        entry.get("output_file").and_then(serde_json::Value::as_str),
        Some(report_path.display().to_string().as_str())
    );
    assert!(
        output.content.contains(&report_path.display().to_string()),
        "content must reference the log/subagents report file"
    );
    assert!(output.content.contains("Full report:"));
}

/// Seed a running background task directly into the manager map.  Used to
/// set up the race/scan regression tests below; the task can later be
/// overwritten with a completed entry.
async fn seed_running_task(
    manager: &AgentManager,
    parent_session_id: &str,
    task_id: &str,
    agent_name: &str,
    task_prompt: &str,
) {
    let entry = TaskEntry {
        id: task_id.to_string(),
        parent_session_id: parent_session_id.to_string(),
        child_session_id: format!("child-{task_id}"),
        agent_name: agent_name.to_string(),
        task_prompt: task_prompt.to_string(),
        background: true,
        status: TaskStatus::Running,
        result: None,
        error: None,
        created_at: Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ragent_agent::task::ReportStatus::default(),
    };
    manager.seed_completed_for_test(entry).await;
}

#[tokio::test]
async fn test_wait_agents_returns_when_task_completes_via_event() {
    let event_bus = Arc::new(EventBus::new(16));
    let processor = test_processor();
    let manager = Arc::new(AgentManager::new(event_bus.clone(), processor, 4, 300));
    let parent_sid = "parent-sess";
    let task_id = "explore-feedface";

    seed_running_task(&manager, parent_sid, task_id, "explore", "do the thing").await;

    let manager2 = Arc::clone(&manager);
    let event_bus2 = Arc::clone(&event_bus);
    let parent_sid2 = parent_sid.to_string();
    let task_id2 = task_id.to_string();
    tokio::spawn(async move {
        tokio::time::sleep(Duration::from_millis(20)).await;
        let child_session_id = format!("child-{task_id2}");
        seed_completed_task(
            &manager2,
            &parent_sid2,
            &task_id2,
            "explore",
            "event result",
            None,
        )
        .await;
        event_bus2.publish(Event::SubagentComplete {
            session_id: parent_sid2,
            task_id: task_id2,
            child_session_id,
            summary: "event result".to_string(),
            success: true,
            duration_ms: 42,
            finish_reason: "complete".to_string(),
        });
    });

    let ctx = make_ctx(parent_sid, event_bus.clone(), Arc::clone(&manager));
    let tool = WaitAgentsTool;

    let output = tokio::time::timeout(
        Duration::from_secs(2),
        tool.execute(json!({"task_ids": [task_id], "timeout_secs": 60}), &ctx),
    )
    .await
    .expect("wait_agents should return when the task completes via event")
    .expect("wait_agents should succeed");

    assert!(
        output.content.contains("event result"),
        "wait_agents must return the completed task's result; got: {}",
        output.content
    );
    assert!(
        output.content.contains("1 task(s) completed"),
        "wait_agents must report one completed task; got: {}",
        output.content
    );
}

#[tokio::test]
async fn test_wait_agents_falls_back_to_scan_when_event_is_missed() {
    let event_bus = Arc::new(EventBus::new(16));
    let processor = test_processor();
    let manager = Arc::new(AgentManager::new(event_bus.clone(), processor, 4, 300));
    let parent_sid = "parent-sess";
    let task_id = "explore-deadbeef";

    seed_running_task(&manager, parent_sid, task_id, "explore", "do the thing").await;

    let manager2 = Arc::clone(&manager);
    let parent_sid2 = parent_sid.to_string();
    let task_id2 = task_id.to_string();
    tokio::spawn(async move {
        // Let wait_agents finish its initial snapshot before marking the task
        // complete, then do *not* publish SubagentComplete.  The periodic scan
        // fallback must detect the completed entry instead.
        tokio::time::sleep(Duration::from_millis(20)).await;
        seed_completed_task(
            &manager2,
            &parent_sid2,
            &task_id2,
            "explore",
            "scanned result",
            None,
        )
        .await;
    });

    let ctx = make_ctx(parent_sid, event_bus.clone(), Arc::clone(&manager));
    let tool = WaitAgentsTool;

    let output = tokio::time::timeout(
        // The tool polls the task map every 5 seconds; give it a generous
        // margin so the test is not flaky on a busy CI runner.
        Duration::from_secs(8),
        tool.execute(json!({"task_ids": [task_id], "timeout_secs": 60}), &ctx),
    )
    .await
    .expect("wait_agents should fall back to scanning the task map")
    .expect("wait_agents should succeed");

    assert!(
        output.content.contains("scanned result"),
        "periodic scan must pick up the completed task; got: {}",
        output.content
    );
    assert!(
        output.content.contains("1 task(s) completed"),
        "wait_agents must report one completed task; got: {}",
        output.content
    );
}

#[tokio::test]
async fn test_wait_agents_race_snapshot_and_completion_uses_scan() {
    let event_bus = Arc::new(EventBus::new(16));
    let processor = test_processor();
    let manager = Arc::new(AgentManager::new(event_bus.clone(), processor, 4, 300));
    let parent_sid = "parent-sess";
    let task_id = "explore-cafebabe";

    seed_running_task(&manager, parent_sid, task_id, "explore", "do the thing").await;

    // Complete the task *synchronously* before invoking wait_agents, but
    // do not publish the SubagentComplete event.  The initial snapshot will
    // see Running, but the post-increment re-scan must notice it is already
    // completed and return without entering the long wait loop.
    seed_completed_task(
        &manager,
        parent_sid,
        task_id,
        "explore",
        "race result",
        None,
    )
    .await;

    let ctx = make_ctx(parent_sid, event_bus.clone(), Arc::clone(&manager));
    let tool = WaitAgentsTool;

    let output = tokio::time::timeout(
        Duration::from_millis(500),
        tool.execute(json!({"task_ids": [task_id], "timeout_secs": 60}), &ctx),
    )
    .await
    .expect("wait_agents should return immediately for an already-completed task")
    .expect("wait_agents should succeed");

    assert!(
        output.content.contains("race result"),
        "post-increment scan must catch the race between snapshot and completion; got: {}",
        output.content
    );
    assert!(
        output.content.contains("1 task(s) completed"),
        "wait_agents must report one completed task; got: {}",
        output.content
    );
}

#[tokio::test]
async fn test_wait_agents_results_meta_no_tasks() {
    let event_bus = Arc::new(EventBus::new(16));
    let processor = test_processor();
    let manager = Arc::new(AgentManager::new(event_bus.clone(), processor, 4, 300));
    let ctx = make_ctx("parent-sess", event_bus, manager);

    let tool = WaitAgentsTool;
    let output = tool
        .execute(json!({}), &ctx)
        .await
        .expect("no running tasks must return early without error");
    assert!(
        output.content.contains("No running background tasks"),
        "expected the 'no tasks' early-return path, got: {}",
        output.content
    );
}
