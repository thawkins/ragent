#![allow(clippy::assert_is_empty)]
//! todo2tasks T-010: integration tests for the `task_list` tool
//! (`TaskListTool`).
//!
//! Verifies FR-015: when `task_list` is called, the system returns all
//! session tasks ordered by `created_at`.  Each entry includes `id`,
//! `subject`, `status`, `owner`, `blocked_by`.  Optional `status`
//! filter: `pending` / `in_progress` / `completed` / `all` (default
//! `all`).

use std::sync::Arc;

use ragent_tools_extended::storage::StorageBackend;
use ragent_tools_extended::task::TaskListTool;
use ragent_tools_extended::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

#[path = "support/mock_storage.rs"]
mod mock_storage;
use mock_storage::MockStorage;

/// Build a `ToolContext` backed by the given storage.
fn test_ctx(storage: Arc<dyn StorageBackend>) -> ToolContext {
    ToolContext {
        storage: Some(storage),
        event_bus: Arc::new(EventBus::new(128)),
        session_id: "test-session".to_string(),
        working_dir: std::env::current_dir().unwrap(),
        code_index: None,
        config: None,
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    }
}

// ── Basic listing ───────────────────────────────────────────────────

/// List with no tasks should return "No tasks found".
#[tokio::test]
async fn test_task_list_empty() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    assert!(
        out.content.contains("No tasks found"),
        "content: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["count"], 0);
    assert_eq!(meta["status_filter"], "all");
    assert_eq!(meta["tasks"], json!([]));
}

/// List with a single task should return it with all FR-015 fields.
#[tokio::test]
async fn test_task_list_single() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Write tests", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    assert!(
        out.content.contains("Write tests"),
        "content: {}",
        out.content
    );
    assert!(out.content.contains("1 items"), "content: {}", out.content);

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["count"], 1);
    let tasks = meta["tasks"].as_array().expect("tasks array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], "t1");
    assert_eq!(tasks[0]["subject"], "Write tests");
    assert_eq!(tasks[0]["status"], "pending");
    assert_eq!(tasks[0]["owner"], serde_json::Value::Null);
    assert_eq!(tasks[0]["blocked_by"], json!([]));
}

/// List with multiple tasks should return all of them.
#[tokio::test]
async fn test_task_list_multiple() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "First", "pending");
    storage.seed("t2", "test-session", "Second", "in_progress");
    storage.seed("t3", "test-session", "Third", "completed");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["count"], 3);
    let tasks = meta["tasks"].as_array().expect("tasks array");
    assert_eq!(tasks.len(), 3);
}

// ── FR-015 fields ───────────────────────────────────────────────────

/// Each entry should include id, subject, status, owner, blocked_by.
#[tokio::test]
async fn test_task_list_includes_all_fields() {
    let storage = MockStorage::new();
    storage.seed_task(
        "t1",
        "test-session",
        "Implement feature",
        "in_progress",
        "desc",
        Some("Implementing feature"),
        Some("coder-agent"),
        &["t0"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    let meta = out.metadata.expect("metadata present");
    let task = &meta["tasks"][0];
    assert_eq!(task["id"], "t1");
    assert_eq!(task["subject"], "Implement feature");
    assert_eq!(task["status"], "in_progress");
    assert_eq!(task["owner"], "coder-agent");
    assert_eq!(task["blocked_by"], json!(["t0"]));
}

/// Entries should NOT include description, active_form, metadata, etc.
/// (FR-015 specifies id, subject, status, owner, blocked_by; T-006
/// adds derived `is_blocked` / `is_available` per FR-003/FR-005).
#[tokio::test]
async fn test_task_list_excludes_extra_fields() {
    let storage = MockStorage::new();
    storage.seed_task(
        "t1",
        "test-session",
        "Task",
        "pending",
        "a description",
        Some("active"),
        Some("owner"),
        &[],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    let meta = out.metadata.expect("metadata present");
    let task = &meta["tasks"][0];
    let obj = task.as_object().expect("task is object");
    // FR-015 fields + T-006 derived flags (FR-003, FR-005).
    assert_eq!(
        obj.len(),
        7,
        "should have exactly 7 fields, got: {:?}",
        obj.keys().collect::<Vec<_>>()
    );
    assert!(obj.contains_key("id"));
    assert!(obj.contains_key("subject"));
    assert!(obj.contains_key("status"));
    assert!(obj.contains_key("owner"));
    assert!(obj.contains_key("blocked_by"));
    assert!(obj.contains_key("is_blocked"));
    assert!(obj.contains_key("is_available"));
    assert!(!obj.contains_key("description"));
    assert!(!obj.contains_key("active_form"));
    assert!(!obj.contains_key("metadata"));
    assert!(!obj.contains_key("blocks"));
    assert!(!obj.contains_key("created_at"));
    assert!(!obj.contains_key("updated_at"));
}

// ── Status filtering ────────────────────────────────────────────────

/// Filter by "pending" should return only pending tasks.
#[tokio::test]
async fn test_task_list_filter_pending() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "First", "pending");
    storage.seed("t2", "test-session", "Second", "in_progress");
    storage.seed("t3", "test-session", "Third", "completed");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({"status": "pending"}), &ctx)
        .await
        .expect("task_list should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["count"], 1);
    assert_eq!(meta["status_filter"], "pending");
    let tasks = meta["tasks"].as_array().expect("tasks array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], "t1");
    assert_eq!(tasks[0]["status"], "pending");
}

/// Filter by "in_progress" should return only in_progress tasks.
#[tokio::test]
async fn test_task_list_filter_in_progress() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "First", "pending");
    storage.seed("t2", "test-session", "Second", "in_progress");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({"status": "in_progress"}), &ctx)
        .await
        .expect("task_list should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["count"], 1);
    let tasks = meta["tasks"].as_array().expect("tasks array");
    assert_eq!(tasks[0]["id"], "t2");
}

/// Filter by "completed" should return only completed tasks.
#[tokio::test]
async fn test_task_list_filter_completed() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "First", "pending");
    storage.seed("t2", "test-session", "Second", "completed");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({"status": "completed"}), &ctx)
        .await
        .expect("task_list should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["count"], 1);
    let tasks = meta["tasks"].as_array().expect("tasks array");
    assert_eq!(tasks[0]["id"], "t2");
}

/// Filter by "all" (explicit) should return all tasks.
#[tokio::test]
async fn test_task_list_filter_all_explicit() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "First", "pending");
    storage.seed("t2", "test-session", "Second", "in_progress");
    storage.seed("t3", "test-session", "Third", "completed");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({"status": "all"}), &ctx)
        .await
        .expect("task_list should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["count"], 3);
    assert_eq!(meta["status_filter"], "all");
}

/// Default (no status param) should be "all".
#[tokio::test]
async fn test_task_list_default_all() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "First", "pending");
    storage.seed("t2", "test-session", "Second", "completed");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["count"], 2);
    assert_eq!(meta["status_filter"], "all");
}

/// Invalid status filter should error.
#[tokio::test]
async fn test_task_list_invalid_status() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let err = tool
        .execute(json!({"status": "done"}), &ctx)
        .await
        .expect_err("should reject 'done' status");
    assert!(err.to_string().contains("Invalid status"), "error: {err}");
    assert!(err.to_string().contains("done"), "error: {err}");
}

/// "blocked" is not a valid filter for task_list (FR-015 specifies only
/// pending / in_progress / completed / all).
#[tokio::test]
async fn test_task_list_blocked_not_valid_filter() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let err = tool
        .execute(json!({"status": "blocked"}), &ctx)
        .await
        .expect_err("should reject 'blocked' status");
    assert!(err.to_string().contains("Invalid status"), "error: {err}");
}

// ── Ordering by created_at (FR-015) ─────────────────────────────────

/// Tasks should be ordered by created_at ascending.
#[tokio::test]
async fn test_task_list_ordered_by_created_at() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "First", "pending");
    storage.seed("t2", "test-session", "Second", "pending");
    storage.seed("t3", "test-session", "Third", "pending");
    // Set explicit timestamps in non-sorted order.
    storage.set_created_at("t3", "2026-08-16T10:00:00+00:00");
    storage.set_created_at("t1", "2026-08-16T08:00:00+00:00");
    storage.set_created_at("t2", "2026-08-16T09:00:00+00:00");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    let meta = out.metadata.expect("metadata present");
    let tasks = meta["tasks"].as_array().expect("tasks array");
    // Should be ordered: t1 (08:00) → t2 (09:00) → t3 (10:00).
    assert_eq!(tasks[0]["id"], "t1");
    assert_eq!(tasks[1]["id"], "t2");
    assert_eq!(tasks[2]["id"], "t3");
}

// ── Session scoping (FR-001) ────────────────────────────────────────

/// Tasks from other sessions should not appear.
#[tokio::test]
async fn test_task_list_session_scoped() {
    let storage = MockStorage::new();
    storage.seed("t1", "other-session", "Other session", "pending");
    storage.seed("t2", "test-session", "This session", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["count"], 1);
    let tasks = meta["tasks"].as_array().expect("tasks array");
    assert_eq!(tasks[0]["id"], "t2");
}

// ── Error cases ─────────────────────────────────────────────────────

/// No storage backend should error.
#[tokio::test]
async fn test_task_list_no_storage() {
    let ctx = ToolContext {
        storage: None,
        event_bus: Arc::new(EventBus::new(128)),
        session_id: "test-session".to_string(),
        working_dir: std::env::current_dir().unwrap(),
        code_index: None,
        config: None,
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    };
    let tool = TaskListTool;

    let err = tool
        .execute(json!({}), &ctx)
        .await
        .expect_err("should fail without storage");
    assert!(err.to_string().contains("Storage"), "error: {err}");
}

// ── Human-readable content ──────────────────────────────────────────

/// Content should include task IDs.
#[tokio::test]
async fn test_task_list_content_shows_id() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Task One", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    assert!(out.content.contains("ID: t1"), "content: {}", out.content);
}

/// Content should show owner when present.
#[tokio::test]
async fn test_task_list_content_shows_owner() {
    let storage = MockStorage::new();
    storage.seed_task(
        "t1",
        "test-session",
        "Task",
        "pending",
        "",
        None,
        Some("agent-1"),
        &[],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    assert!(
        out.content.contains("Owner: agent-1"),
        "content: {}",
        out.content
    );
}

/// Content should show blocked_by when present.
#[tokio::test]
async fn test_task_list_content_shows_blocked_by() {
    let storage = MockStorage::new();
    storage.seed_task(
        "t1",
        "test-session",
        "Task",
        "pending",
        "",
        None,
        None,
        &["t0"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    assert!(
        out.content.contains("Blocked by:"),
        "content: {}",
        out.content
    );
    assert!(out.content.contains("t0"), "content: {}", out.content);
}

/// Content for empty filtered list should mention the filter.
#[tokio::test]
async fn test_task_list_empty_filtered_content() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({"status": "completed"}), &ctx)
        .await
        .expect("task_list should succeed");

    assert!(
        out.content.contains("No tasks found"),
        "content: {}",
        out.content
    );
    assert!(
        out.content.contains("completed"),
        "content: {}",
        out.content
    );
}

/// Content for empty unfiltered list should just say "No tasks found."
#[tokio::test]
async fn test_task_list_empty_unfiltered_content() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    assert!(
        out.content.contains("No tasks found"),
        "content: {}",
        out.content
    );
    // Should NOT include a status qualifier.
    assert!(
        !out.content.contains("with status"),
        "content: {}",
        out.content
    );
}

// ── Tool metadata ────────────────────────────────────────────────��──

/// Tool name should be "task_list".
#[test]
fn test_task_list_tool_name() {
    let tool = TaskListTool;
    assert_eq!(tool.name(), "task_list");
}

/// Tool should have a non-empty description.
#[test]
fn test_task_list_tool_description() {
    let tool = TaskListTool;
    assert!(!tool.description().is_empty());
    assert!(tool.description().contains("status"));
}

/// Tool should have optional status in its schema.
#[test]
fn test_task_list_tool_schema() {
    let tool = TaskListTool;
    let schema = tool.parameters_schema();
    let props = schema["properties"].as_object().expect("properties");
    assert!(props.contains_key("status"));
    // status is NOT required (it's optional with default "all").
    let required = schema["required"].as_array();
    assert!(required.is_none() || required.unwrap().is_empty());
}

/// Permission category should match the existing todo tools.
#[test]
fn test_task_list_tool_permission_category() {
    let tool = TaskListTool;
    assert_eq!(tool.permission_category(), "task");
}

// ── Registry registration ───────────────────────────────────────────

/// `task_list` should be registered in the extended registry.
#[test]
fn test_task_list_registered() {
    let registry = ragent_tools_extended::create_extended_registry();
    assert!(
        registry.contains("task_list"),
        "task_list should be registered"
    );
}
