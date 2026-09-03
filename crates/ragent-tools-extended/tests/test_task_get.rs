#![allow(clippy::assert_is_empty)]
//! todo2tasks T-009: integration tests for the `task_get` tool
//! (`TaskGetTool`).
//!
//! Verifies FR-014: when `task_get` is called with a `task_id`, the
//! system returns the full task record — `id`, `subject`,
//! `description`, `active_form`, `status`, `owner`, `metadata`,
//! `blocked_by`, `blocks` (derived), `created_at`, `updated_at`.

use std::sync::Arc;

use ragent_tools_extended::storage::StorageBackend;
use ragent_tools_extended::task::TaskGetTool;
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

// ── Basic retrieval ─────────────────────────────────────────────────

/// Retrieve a simple task with only the legacy fields populated.
#[tokio::test]
async fn test_task_get_basic() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Write tests", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect("task_get should succeed");

    assert!(
        out.content.contains("Write tests"),
        "content: {}",
        out.content
    );
    assert!(out.content.contains("pending"), "content: {}", out.content);
    assert!(out.content.contains("t1"), "content: {}", out.content);

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["id"], "t1");
    assert_eq!(meta["subject"], "Write tests");
    assert_eq!(meta["status"], "pending");
    assert_eq!(meta["description"], "");
    assert_eq!(meta["active_form"], serde_json::Value::Null);
    assert_eq!(meta["owner"], serde_json::Value::Null);
    assert_eq!(meta["blocked_by"], json!([]));
    assert_eq!(meta["blocks"], json!([]));
}

/// Retrieve a task with all Task-model fields populated.
#[tokio::test]
async fn test_task_get_full_record() {
    let storage = MockStorage::new();
    storage.seed_task(
        "t2",
        "test-session",
        "Implement auth",
        "in_progress",
        "Add JWT authentication",
        Some("Implementing JWT auth"),
        Some("coder-agent"),
        &["t1", "t3"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("task_get should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["id"], "t2");
    assert_eq!(meta["subject"], "Implement auth");
    assert_eq!(meta["description"], "Add JWT authentication");
    assert_eq!(meta["active_form"], "Implementing JWT auth");
    assert_eq!(meta["status"], "in_progress");
    assert_eq!(meta["owner"], "coder-agent");
    assert_eq!(meta["blocked_by"], json!(["t1", "t3"]));
    // blocks is empty because no other task lists t2 in blocked_by.
    assert_eq!(meta["blocks"], json!([]));
}

// ── Derived blocks field ────────────────────────────────────────────

/// `blocks` should list all tasks that depend on this task.
#[tokio::test]
async fn test_task_get_derived_blocks() {
    let storage = MockStorage::new();
    // t1 is the task we'll retrieve.
    storage.seed("t1", "test-session", "Setup CI", "completed");
    // t2 and t3 both depend on t1.
    storage.seed_task(
        "t2",
        "test-session",
        "Run tests",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    storage.seed_task(
        "t3",
        "test-session",
        "Deploy",
        "pending",
        "",
        None,
        None,
        &["t1", "t2"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect("task_get should succeed");

    let meta = out.metadata.expect("metadata present");
    let blocks = meta["blocks"].as_array().expect("blocks should be array");
    assert_eq!(blocks.len(), 2, "t1 should block both t2 and t3");
    let block_ids: Vec<&str> = blocks.iter().map(|v| v.as_str().unwrap()).collect();
    assert!(block_ids.contains(&"t2"));
    assert!(block_ids.contains(&"t3"));
}

/// `blocks` should be empty when no other task depends on this one.
#[tokio::test]
async fn test_task_get_blocks_empty() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Standalone task", "pending");
    storage.seed("t2", "test-session", "Other task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect("task_get should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["blocks"], json!([]));
}

/// `blocks` should include tasks that transitively depend on this one
/// (directly — `blocks` is the direct inverse, not transitive).
#[tokio::test]
async fn test_task_get_blocks_direct_only() {
    let storage = MockStorage::new();
    // t1 → t2 → t3 (t3 depends on t2, t2 depends on t1)
    storage.seed("t1", "test-session", "First", "completed");
    storage.seed_task(
        "t2",
        "test-session",
        "Second",
        "in_progress",
        "",
        None,
        None,
        &["t1"],
    );
    storage.seed_task(
        "t3",
        "test-session",
        "Third",
        "pending",
        "",
        None,
        None,
        &["t2"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    // t1's blocks should be [t2] only (not t3, since t3 depends on t2 not t1).
    let out = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect("task_get should succeed");
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["blocks"], json!(["t2"]));

    // t2's blocks should be [t3].
    let out = tool
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("task_get should succeed");
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["blocks"], json!(["t3"]));
}

// ── Error cases ─────────────────────────────────────────────────────

/// Missing `task_id` parameter should error.
#[tokio::test]
async fn test_task_get_missing_task_id() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let err = tool
        .execute(json!({}), &ctx)
        .await
        .expect_err("should fail without task_id");
    assert!(err.to_string().contains("task_id"), "error: {err}");
}

/// Non-existent task ID should error.
#[tokio::test]
async fn test_task_get_nonexistent() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Real task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let err = tool
        .execute(json!({"task_id": "nonexistent"}), &ctx)
        .await
        .expect_err("should fail for non-existent task");
    assert!(err.to_string().contains("nonexistent"), "error: {err}");
    assert!(err.to_string().contains("not found"), "error: {err}");
}

/// No storage backend should error.
#[tokio::test]
async fn test_task_get_no_storage() {
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
    let tool = TaskGetTool;

    let err = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect_err("should fail without storage");
    assert!(err.to_string().contains("Storage"), "error: {err}");
}

// ── Session scoping (FR-001) ────────────────────────────────────────

/// Tasks from other sessions should not be visible.
#[tokio::test]
async fn test_task_get_session_scoped() {
    let storage = MockStorage::new();
    storage.seed("t1", "other-session", "Other session task", "pending");
    storage.seed("t2", "test-session", "This session task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    // t1 belongs to "other-session" — should not be found.
    let err = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect_err("should not find task from other session");
    assert!(err.to_string().contains("not found"), "error: {err}");

    // t2 belongs to "test-session" — should be found.
    let out = tool
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("should find task in this session");
    assert_eq!(out.metadata.unwrap()["id"], "t2");
}

// ── Human-readable content ──────────────────────────────────────────

/// Content should include active_form when present.
#[tokio::test]
async fn test_task_get_content_active_form() {
    let storage = MockStorage::new();
    storage.seed_task(
        "t1",
        "test-session",
        "Implement feature",
        "in_progress",
        "",
        Some("Implementing feature"),
        None,
        &[],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect("task_get should succeed");

    assert!(
        out.content.contains("Implementing feature"),
        "content: {}",
        out.content
    );
}

/// Content should include owner when present.
#[tokio::test]
async fn test_task_get_content_owner() {
    let storage = MockStorage::new();
    storage.seed_task(
        "t1",
        "test-session",
        "Review PR",
        "pending",
        "",
        None,
        Some("reviewer-bot"),
        &[],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect("task_get should succeed");

    assert!(
        out.content.contains("reviewer-bot"),
        "content: {}",
        out.content
    );
}

/// Content should include blocked_by and blocks annotations.
#[tokio::test]
async fn test_task_get_content_deps() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Setup", "completed");
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    // t1's content should show Blocks: t2
    let out = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect("task_get should succeed");
    assert!(out.content.contains("Blocks:"), "content: {}", out.content);
    assert!(out.content.contains("t2"), "content: {}", out.content);

    // t2's content should show Blocked by: t1
    let out = tool
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("task_get should succeed");
    assert!(
        out.content.contains("Blocked by:"),
        "content: {}",
        out.content
    );
    assert!(out.content.contains("t1"), "content: {}", out.content);
}

/// Content should include description when present.
#[tokio::test]
async fn test_task_get_content_description() {
    let storage = MockStorage::new();
    storage.seed_task(
        "t1",
        "test-session",
        "Write docs",
        "pending",
        "Document the new API endpoints",
        None,
        None,
        &[],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect("task_get should succeed");

    assert!(
        out.content.contains("Document the new API endpoints"),
        "content: {}",
        out.content
    );
}

// ── Tool metadata ───────────────────────────────────────────────────

/// Tool name should be "task_get".
#[test]
fn test_task_get_tool_name() {
    let tool = TaskGetTool;
    assert_eq!(tool.name(), "task_get");
}

/// Tool should have a non-empty description.
#[test]
fn test_task_get_tool_description() {
    let tool = TaskGetTool;
    assert!(!tool.description().is_empty());
    assert!(tool.description().contains("task_id"));
}

/// Tool should require task_id in its schema.
#[test]
fn test_task_get_tool_schema() {
    let tool = TaskGetTool;
    let schema = tool.parameters_schema();
    let props = schema["properties"].as_object().expect("properties");
    assert!(props.contains_key("task_id"));
    let required = schema["required"].as_array().expect("required");
    assert!(
        required.iter().any(|v| v == "task_id"),
        "task_id should be required"
    );
}

/// Permission category should match the existing todo tools.
#[test]
fn test_task_get_tool_permission_category() {
    let tool = TaskGetTool;
    assert_eq!(tool.permission_category(), "task");
}

// ── Status display ───���──────────────────────────────────────────────

/// Completed status should display correctly.
#[tokio::test]
async fn test_task_get_completed_status() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Done task", "completed");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect("task_get should succeed");

    assert!(
        out.content.contains("completed"),
        "content: {}",
        out.content
    );
    assert!(out.content.contains("✅"), "content: {}", out.content);
}

/// In-progress status should display correctly.
#[tokio::test]
async fn test_task_get_in_progress_status() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Active task", "in_progress");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect("task_get should succeed");

    assert!(
        out.content.contains("in_progress"),
        "content: {}",
        out.content
    );
    assert!(out.content.contains("🔄"), "content: {}", out.content);
}

// ── Registry registration ───────────────────────────────────────────

/// `task_get` should be registered in the extended registry.
#[test]
fn test_task_get_registered() {
    let registry = ragent_tools_extended::create_extended_registry();
    assert!(
        registry.contains("task_get"),
        "task_get should be registered"
    );
}

// ── Empty session ───────────────────────────────────────────────────

/// Getting a task from an empty session should error.
#[tokio::test]
async fn test_task_get_empty_session() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let err = tool
        .execute(json!({"task_id": "anything"}), &ctx)
        .await
        .expect_err("should fail in empty session");
    assert!(err.to_string().contains("not found"), "error: {err}");
}
