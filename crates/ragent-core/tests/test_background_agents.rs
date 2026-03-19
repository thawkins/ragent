//! Tests for Phase 2 background agent infrastructure:
//! - cancel_task tool
//! - list_tasks tool
//! - ExperimentalFlags config defaults
//! - TaskManager drain_completed
//! - TaskEntry serialization with reported field

use ragent_core::config::{Config, ExperimentalFlags};
use ragent_core::event::EventBus;
use ragent_core::task::{TaskEntry, TaskStatus};
use ragent_core::tool::create_default_registry;
use ragent_core::tool::{Tool, ToolContext};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

// ── Config defaults ────────────────────────────────────────────

#[test]
fn test_experimental_flags_defaults() {
    let flags = ExperimentalFlags::default();
    assert_eq!(flags.max_background_agents, 4);
    assert_eq!(flags.background_agent_timeout, 3600);
    assert!(!flags.open_telemetry);
}

#[test]
fn test_config_experimental_defaults() {
    let config = Config::default();
    assert_eq!(config.experimental.max_background_agents, 4);
    assert_eq!(config.experimental.background_agent_timeout, 3600);
}

#[test]
fn test_experimental_flags_serde_round_trip() {
    let flags = ExperimentalFlags {
        open_telemetry: true,
        max_background_agents: 8,
        background_agent_timeout: 1200,
    };
    let json = serde_json::to_string(&flags).unwrap();
    let parsed: ExperimentalFlags = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.max_background_agents, 8);
    assert_eq!(parsed.background_agent_timeout, 1200);
    assert!(parsed.open_telemetry);
}

#[test]
fn test_experimental_flags_serde_defaults_on_missing_fields() {
    // Only open_telemetry present; new fields should get defaults
    let json = r#"{"open_telemetry": false}"#;
    let flags: ExperimentalFlags = serde_json::from_str(json).unwrap();
    assert_eq!(flags.max_background_agents, 4);
    assert_eq!(flags.background_agent_timeout, 3600);
}

// ── TaskEntry reported field ───────────────────────────────────

#[test]
fn test_task_entry_reported_field_default() {
    let json = r#"{
        "id": "t1",
        "parent_session_id": "p1",
        "child_session_id": "c1",
        "agent_name": "explore",
        "task_prompt": "find auth",
        "background": true,
        "status": "running",
        "result": null,
        "error": null,
        "created_at": "2025-01-01T00:00:00Z",
        "completed_at": null
    }"#;
    let entry: TaskEntry = serde_json::from_str(json).unwrap();
    assert!(
        !entry.reported,
        "reported should default to false when missing"
    );
}

#[test]
fn test_task_entry_reported_round_trip() {
    let entry = TaskEntry {
        id: "t1".to_string(),
        parent_session_id: "p1".to_string(),
        child_session_id: "c1".to_string(),
        agent_name: "explore".to_string(),
        task_prompt: "find auth".to_string(),
        background: true,
        status: TaskStatus::Completed,
        result: Some("Found 3 modules".to_string()),
        error: None,
        created_at: chrono::Utc::now(),
        completed_at: Some(chrono::Utc::now()),
        reported: true,
    };
    let json = serde_json::to_string(&entry).unwrap();
    let parsed: TaskEntry = serde_json::from_str(&json).unwrap();
    assert!(parsed.reported);
}

// ── Tool registration ──────────────────────────────────────────

#[test]
fn test_registry_has_cancel_task() {
    let registry = create_default_registry();
    assert!(registry.get("cancel_task").is_some());
}

#[test]
fn test_registry_has_list_tasks() {
    let registry = create_default_registry();
    assert!(registry.get("list_tasks").is_some());
}

#[test]
fn test_registry_total_tool_count() {
    let registry = create_default_registry();
    assert_eq!(registry.list().len(), 52);
}

// ── cancel_task tool ───────────────────────────────────────────

fn make_ctx() -> ToolContext {
    ToolContext {
        session_id: "test-session".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
    }
}

#[tokio::test]
async fn test_cancel_task_no_task_manager() {
    let tool = ragent_core::tool::cancel_task::CancelTaskTool;
    let ctx = make_ctx();
    let result = tool.execute(json!({"task_id": "abc123"}), &ctx).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("TaskManager has not been initialised")
    );
}

#[tokio::test]
async fn test_cancel_task_missing_task_id() {
    let tool = ragent_core::tool::cancel_task::CancelTaskTool;
    let ctx = make_ctx();
    let result = tool.execute(json!({}), &ctx).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Missing required parameter: task_id")
    );
}

// ── list_tasks tool ────────────────────────────────────────────

#[tokio::test]
async fn test_list_tasks_no_task_manager() {
    let tool = ragent_core::tool::list_tasks::ListTasksTool;
    let ctx = make_ctx();
    let result = tool.execute(json!({}), &ctx).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("TaskManager has not been initialised")
    );
}

#[tokio::test]
async fn test_list_tasks_missing_task_id_detail() {
    let tool = ragent_core::tool::list_tasks::ListTasksTool;
    let ctx = make_ctx();
    let result = tool.execute(json!({"task_id": "nonexistent"}), &ctx).await;
    // Without task_manager, this should fail
    assert!(result.is_err());
}

// ── Tool parameter schemas ─────────────────────────────────────

#[test]
fn test_cancel_task_schema() {
    let tool = ragent_core::tool::cancel_task::CancelTaskTool;
    let schema = tool.parameters_schema();
    let required = schema["required"].as_array().unwrap();
    assert!(required.iter().any(|v| v == "task_id"));
}

#[test]
fn test_list_tasks_schema() {
    let tool = ragent_core::tool::list_tasks::ListTasksTool;
    let schema = tool.parameters_schema();
    let props = schema["properties"].as_object().unwrap();
    assert!(props.contains_key("status"));
    assert!(props.contains_key("task_id"));
}

#[test]
fn test_cancel_task_permission() {
    let tool = ragent_core::tool::cancel_task::CancelTaskTool;
    assert_eq!(tool.permission_category(), "agent:spawn");
}

#[test]
fn test_list_tasks_permission() {
    let tool = ragent_core::tool::list_tasks::ListTasksTool;
    assert_eq!(tool.permission_category(), "agent:spawn");
}

// ── TaskStatus serialization ───────────────────────────────────

#[test]
fn test_task_status_all_variants_serialize() {
    assert_eq!(
        serde_json::to_string(&TaskStatus::Running).unwrap(),
        "\"running\""
    );
    assert_eq!(
        serde_json::to_string(&TaskStatus::Completed).unwrap(),
        "\"completed\""
    );
    assert_eq!(
        serde_json::to_string(&TaskStatus::Failed).unwrap(),
        "\"failed\""
    );
    assert_eq!(
        serde_json::to_string(&TaskStatus::Cancelled).unwrap(),
        "\"cancelled\""
    );
}

// ── Event variant serialization ────────────────────────────────

#[test]
fn test_subagent_events_serde() {
    use ragent_core::event::Event;

    let start = Event::SubagentStart {
        session_id: "s1".to_string(),
        task_id: "t1".to_string(),
        child_session_id: "c1".to_string(),
        agent: "explore".to_string(),
        task: "Find auth code".to_string(),
        background: true,
    };
    let json = serde_json::to_string(&start).unwrap();
    assert!(json.contains("\"subagent_start\""));
    assert!(json.contains("\"explore\""));

    let complete = Event::SubagentComplete {
        session_id: "s1".to_string(),
        task_id: "t1".to_string(),
        child_session_id: "c1".to_string(),
        summary: "Found 3 modules".to_string(),
        success: true,
        duration_ms: 1234,
    };
    let json = serde_json::to_string(&complete).unwrap();
    assert!(json.contains("\"subagent_complete\""));
    assert!(json.contains("\"Found 3 modules\""));

    let cancelled = Event::SubagentCancelled {
        session_id: "s1".to_string(),
        task_id: "t1".to_string(),
    };
    let json = serde_json::to_string(&cancelled).unwrap();
    assert!(json.contains("\"subagent_cancelled\""));
}
