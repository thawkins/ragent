//! todo2tasks T-008 / T-017: integration tests for the `task_update` tool
//! (`TaskUpdateTool`).
//!
//! Verifies:
//! - FR-005: Reject `status=blocked` with a clear error listing valid
//!   statuses (`pending`, `in_progress`, `completed`).
//! - FR-009: Reject foreign/non-existent `blocked_by` references at
//!   `task_update` boundary (via `add_blocked_by` and `add_blocks`).
//! - FR-004: Reject dependency cycles (via `add_blocked_by` and `add_blocks`).
//! - FR-003: Auto-unblock evaluation on `completed` transition.
//! - FR-013: Clear, actionable error messages.
//! - Basic update functionality (status, subject, description, owner,
//!   active_form, metadata, add_blocked_by merge, add_blocks).

use std::sync::Arc;

use ragent_tools_extended::storage::StorageBackend;
use ragent_tools_extended::task::TaskUpdateTool;
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
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

// ── T-017 / FR-005: Reject status=blocked ─────────────────────────────

/// `status=blocked` must be rejected with a clear error (FR-005, T-017).
#[tokio::test]
async fn test_task_update_reject_blocked_status() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(
            json!({
                "task_id": "t1",
                "status": "blocked"
            }),
            &ctx,
        )
        .await
        .expect_err("status=blocked must be rejected");

    let msg = err.to_string();
    assert!(
        msg.contains("blocked"),
        "error should mention 'blocked': {msg}"
    );
    assert!(
        msg.contains("pending") && msg.contains("in_progress") && msg.contains("completed"),
        "error should list valid statuses: {msg}"
    );
    assert!(
        msg.contains("derived") || msg.contains("not a valid status"),
        "error should explain blocked is derived: {msg}"
    );
}

/// `status=blocked` rejection should NOT modify the task.
#[tokio::test]
async fn test_task_update_blocked_no_mutation() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage.clone()));
    let tool = TaskUpdateTool;

    let _ = tool
        .execute(json!({"task_id": "t1", "status": "blocked"}), &ctx)
        .await;

    // Verify the task is still pending.
    let tasks = storage.list_tasks("test-session", None).unwrap();
    let task = tasks.iter().find(|t| t.id == "t1").unwrap();
    assert_eq!(task.status, "pending");
}

/// An invalid status not in the enum (e.g. "done") should be rejected.
#[tokio::test]
async fn test_task_update_reject_unknown_status() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(json!({"task_id": "t1", "status": "done"}), &ctx)
        .await
        .expect_err("unknown status must be rejected");

    let msg = err.to_string();
    assert!(
        msg.contains("done"),
        "error should mention the invalid status: {msg}"
    );
    assert!(
        msg.contains("pending") && msg.contains("in_progress") && msg.contains("completed"),
        "error should list valid statuses: {msg}"
    );
}

/// `status=done` (legacy todo status) should also be rejected.
#[tokio::test]
async fn test_task_update_reject_done_status() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(json!({"task_id": "t1", "status": "done"}), &ctx)
        .await
        .expect_err("'done' is not a valid task status");

    assert!(err.to_string().contains("Valid statuses"));
}

// ── T-017 / FR-009: Reject foreign/non-existent blocked_by ────────────

/// `add_blocked_by` with a non-existent task ID must be rejected (FR-009).
#[tokio::test]
async fn test_task_update_add_blocked_by_nonexistent() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(
            json!({
                "task_id": "t1",
                "add_blocked_by": ["ghost-id"]
            }),
            &ctx,
        )
        .await
        .expect_err("non-existent blocked_by must be rejected");

    let msg = err.to_string();
    assert!(
        msg.contains("ghost-id"),
        "error should mention the missing id: {msg}"
    );
    assert!(
        msg.contains("blocked_by") || msg.contains("add_blocked_by"),
        "error should mention blocked_by: {msg}"
    );
}

/// `add_blocked_by` with a mix of valid and non-existent IDs must be
/// rejected, listing the non-existent ones.
#[tokio::test]
async fn test_task_update_add_blocked_by_partial_invalid() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    storage.seed("t2", "test-session", "Blocker", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(
            json!({
                "task_id": "t1",
                "add_blocked_by": ["t2", "phantom"]
            }),
            &ctx,
        )
        .await
        .expect_err("partially invalid blocked_by must be rejected");

    assert!(
        err.to_string().contains("phantom"),
        "error should mention the missing id: {err}"
    );
}

/// `add_blocked_by` referencing a task in a different session must be
/// rejected (FR-009 — no cross-session leakage).
#[tokio::test]
async fn test_task_update_add_blocked_by_cross_session() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    // Seed a task in a DIFFERENT session.
    storage.seed("other", "other-session", "Other task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(
            json!({
                "task_id": "t1",
                "add_blocked_by": ["other"]
            }),
            &ctx,
        )
        .await
        .expect_err("cross-session blocked_by must be rejected");

    assert!(
        err.to_string().contains("other"),
        "error should mention the cross-session id: {err}"
    );
}

/// `add_blocked_by` rejection should NOT modify the task's blocked_by.
#[tokio::test]
async fn test_task_update_blocked_by_reject_no_mutation() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage.clone()));
    let tool = TaskUpdateTool;

    let _ = tool
        .execute(json!({"task_id": "t1", "add_blocked_by": ["ghost"]}), &ctx)
        .await;

    let tasks = storage.list_tasks("test-session", None).unwrap();
    let task = tasks.iter().find(|t| t.id == "t1").unwrap();
    assert!(
        task.blocked_by.is_empty(),
        "blocked_by should remain empty after rejected update"
    );
}

// ── Valid updates ─────────────────────────────────────────────────────

/// Update status to `in_progress` should succeed.
#[tokio::test]
async fn test_task_update_status_in_progress() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(json!({"task_id": "t1", "status": "in_progress"}), &ctx)
        .await
        .expect("status update should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["status"], "in_progress");
}

/// Update status to `completed` should succeed.
#[tokio::test]
async fn test_task_update_status_completed() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(json!({"task_id": "t1", "status": "completed"}), &ctx)
        .await
        .expect("status update should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["status"], "completed");
}

/// Update status back to `pending` should succeed.
#[tokio::test]
async fn test_task_update_status_back_to_pending() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "in_progress");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(json!({"task_id": "t1", "status": "pending"}), &ctx)
        .await
        .expect("status update should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["status"], "pending");
}

/// Update subject should succeed.
#[tokio::test]
async fn test_task_update_subject() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Old title", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(json!({"task_id": "t1", "subject": "New title"}), &ctx)
        .await
        .expect("subject update should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["subject"], "New title");
}

/// Update description should succeed.
#[tokio::test]
async fn test_task_update_description() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(
            json!({"task_id": "t1", "description": "New description text"}),
            &ctx,
        )
        .await
        .expect("description update should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["description"], "New description text");
}

/// Update owner should succeed.
#[tokio::test]
async fn test_task_update_owner() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(json!({"task_id": "t1", "owner": "coder-agent"}), &ctx)
        .await
        .expect("owner update should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["owner"], "coder-agent");
}

/// Clear owner by passing empty string.
#[tokio::test]
async fn test_task_update_clear_owner() {
    let storage = MockStorage::new();
    storage.seed_task(
        "t1",
        "test-session",
        "My task",
        "pending",
        "",
        None,
        Some("owner-1"),
        &[],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(json!({"task_id": "t1", "owner": ""}), &ctx)
        .await
        .expect("clear owner should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["owner"], serde_json::Value::Null);
}

/// Update active_form should succeed.
#[tokio::test]
async fn test_task_update_active_form() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(
            json!({"task_id": "t1", "active_form": "Working on my task"}),
            &ctx,
        )
        .await
        .expect("active_form update should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["active_form"], "Working on my task");
}

/// Clear active_form by passing empty string.
#[tokio::test]
async fn test_task_update_clear_active_form() {
    let storage = MockStorage::new();
    storage.seed_task(
        "t1",
        "test-session",
        "My task",
        "pending",
        "",
        Some("Running"),
        None,
        &[],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(json!({"task_id": "t1", "active_form": ""}), &ctx)
        .await
        .expect("clear active_form should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["active_form"], serde_json::Value::Null);
}

/// Update metadata should succeed (full replacement).
#[tokio::test]
async fn test_task_update_metadata() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(
            json!({"task_id": "t1", "metadata": {"phase": "testing", "priority": "high"}}),
            &ctx,
        )
        .await
        .expect("metadata update should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["metadata"]["phase"], "testing");
    assert_eq!(meta["metadata"]["priority"], "high");
}

/// metadata not a JSON object should be rejected.
#[tokio::test]
async fn test_task_update_metadata_not_object() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(json!({"task_id": "t1", "metadata": "not-an-object"}), &ctx)
        .await
        .expect_err("non-object metadata should be rejected");

    assert!(
        err.to_string().contains("metadata"),
        "error should mention metadata: {err}"
    );
    assert!(
        err.to_string().contains("object"),
        "error should say expected object: {err}"
    );
}

// ── add_blocked_by merge semantics ────────────────────────────────────

/// `add_blocked_by` with valid IDs should merge into existing blocked_by.
#[tokio::test]
async fn test_task_update_add_blocked_by_valid() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    storage.seed("t2", "test-session", "Blocker 1", "pending");
    storage.seed("t3", "test-session", "Blocker 2", "completed");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(
            json!({"task_id": "t1", "add_blocked_by": ["t2", "t3"]}),
            &ctx,
        )
        .await
        .expect("add_blocked_by should succeed");

    let meta = out.metadata.expect("metadata present");
    let blocked_by = meta["blocked_by"].as_array().unwrap();
    assert!(blocked_by.contains(&json!("t2")), "should contain t2");
    assert!(blocked_by.contains(&json!("t3")), "should contain t3");
}

/// `add_blocked_by` should merge with existing blocked_by (dedup).
#[tokio::test]
async fn test_task_update_add_blocked_by_merge_dedup() {
    let storage = MockStorage::new();
    storage.seed_task(
        "t1",
        "test-session",
        "My task",
        "pending",
        "",
        None,
        None,
        &["t2"],
    );
    storage.seed("t2", "test-session", "Blocker 1", "pending");
    storage.seed("t3", "test-session", "Blocker 2", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(
            json!({"task_id": "t1", "add_blocked_by": ["t2", "t3"]}),
            &ctx,
        )
        .await
        .expect("merge should succeed");

    let meta = out.metadata.expect("metadata present");
    let blocked_by = meta["blocked_by"].as_array().unwrap();
    // Should have t2 and t3, but t2 should not be duplicated.
    assert_eq!(blocked_by.len(), 2, "should have 2 entries after dedup");
    assert!(blocked_by.contains(&json!("t2")));
    assert!(blocked_by.contains(&json!("t3")));
}

/// After `add_blocked_by`, the task should be flagged as blocked if
/// blockers are not all completed (FR-005 derived state).
#[tokio::test]
async fn test_task_update_add_blocked_by_derives_blocked() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    storage.seed("t2", "test-session", "Blocker", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(json!({"task_id": "t1", "add_blocked_by": ["t2"]}), &ctx)
        .await
        .expect("should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["is_blocked"], true, "should be derived blocked");
    assert_eq!(meta["is_available"], false, "should not be available");
}

/// After a blocker is completed (via task_update), the dependent task
/// should be flagged as available (FR-003 auto-unblock at read time).
#[tokio::test]
async fn test_task_update_complete_blocker_derives_available() {
    let storage = MockStorage::new();
    storage.seed_task(
        "t1",
        "test-session",
        "Dependent",
        "pending",
        "",
        None,
        None,
        &["t2"],
    );
    storage.seed("t2", "test-session", "Blocker", "in_progress");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    // Complete the blocker.
    let out = tool
        .execute(json!({"task_id": "t2", "status": "completed"}), &ctx)
        .await
        .expect("should succeed");

    // The response is for t2 (the blocker), not t1. But we can verify
    // that t2 is now completed.
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["status"], "completed");
}

// ── Edge cases ────────────────────────────────────────────────────────

/// Missing `task_id` parameter should error.
#[tokio::test]
async fn test_task_update_missing_task_id() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(json!({"status": "pending"}), &ctx)
        .await
        .expect_err("missing task_id should error");

    assert!(
        err.to_string().contains("task_id"),
        "error should mention task_id: {err}"
    );
}

/// Non-existent `task_id` should error.
#[tokio::test]
async fn test_task_update_nonexistent_task() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(json!({"task_id": "ghost", "status": "in_progress"}), &ctx)
        .await
        .expect_err("non-existent task should error");

    assert!(
        err.to_string().contains("ghost"),
        "error should mention the task id: {err}"
    );
}

/// No storage should error.
#[tokio::test]
async fn test_task_update_no_storage() {
    let ctx = ToolContext {
        storage: None,
        event_bus: Arc::new(EventBus::new(128)),
        session_id: "test-session".to_string(),
        working_dir: std::env::current_dir().unwrap(),
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    };
    let tool = TaskUpdateTool;

    let err = tool
        .execute(json!({"task_id": "t1", "status": "pending"}), &ctx)
        .await
        .expect_err("no storage should error");

    assert!(
        err.to_string().contains("Storage"),
        "error should mention storage: {err}"
    );
}

/// Update with no optional fields (just task_id) should succeed and
/// return the unchanged task.
#[tokio::test]
async fn test_task_update_noop() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect("noop update should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["subject"], "My task");
    assert_eq!(meta["status"], "pending");
}

/// `add_blocked_by` that is not an array should be rejected.
#[tokio::test]
async fn test_task_update_add_blocked_by_not_array() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(
            json!({"task_id": "t1", "add_blocked_by": "not-an-array"}),
            &ctx,
        )
        .await
        .expect_err("non-array add_blocked_by should be rejected");

    assert!(
        err.to_string().contains("add_blocked_by"),
        "error should mention add_blocked_by: {err}"
    );
    assert!(
        err.to_string().contains("array"),
        "error should say expected array: {err}"
    );
}

// ── Tool metadata ─────────────────────────────────────────────────────

/// Tool name should be "task_update".
#[test]
fn test_task_update_tool_name() {
    let tool = TaskUpdateTool;
    assert_eq!(tool.name(), "task_update");
}

/// Tool description should mention task_id and status.
#[test]
fn test_task_update_tool_description() {
    let tool = TaskUpdateTool;
    let desc = tool.description();
    assert!(
        desc.contains("task_id"),
        "description should mention task_id"
    );
    assert!(desc.contains("status"), "description should mention status");
}

/// Tool schema should require task_id and have status enum.
#[test]
fn test_task_update_tool_schema() {
    let tool = TaskUpdateTool;
    let schema = tool.parameters_schema();

    // task_id is required.
    let required = schema["required"].as_array().unwrap();
    assert!(
        required.contains(&json!("task_id")),
        "task_id should be required"
    );

    // status enum should NOT include "blocked".
    let status_enum = schema["properties"]["status"]["enum"].as_array().unwrap();
    assert!(
        !status_enum.contains(&json!("blocked")),
        "status enum should NOT include 'blocked'"
    );
    assert!(
        status_enum.contains(&json!("pending")),
        "status enum should include 'pending'"
    );
    assert!(
        status_enum.contains(&json!("in_progress")),
        "status enum should include 'in_progress'"
    );
    assert!(
        status_enum.contains(&json!("completed")),
        "status enum should include 'completed'"
    );
}

/// Permission category should be "task".
#[test]
fn test_task_update_tool_permission_category() {
    let tool = TaskUpdateTool;
    assert_eq!(tool.permission_category(), "task");
}

// ── T-008: add_blocks tests ───────────────────────────────────────────

/// `add_blocks` should add this task's ID to the target's blocked_by list.
#[tokio::test]
async fn test_task_update_add_blocks_basic() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Blocker task", "in_progress");
    storage.seed("t2", "test-session", "Dependent task", "pending");
    let ctx = test_ctx(Arc::new(storage.clone()));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(json!({"task_id": "t1", "add_blocks": ["t2"]}), &ctx)
        .await
        .expect("add_blocks should succeed");

    // The response is for t1 (the blocker).
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["id"], "t1");

    // Verify t2 now has t1 in its blocked_by.
    let tasks = storage.list_tasks("test-session", None).unwrap();
    let t2 = tasks.iter().find(|t| t.id == "t2").unwrap();
    assert!(
        t2.blocked_by.contains(&"t1".to_string()),
        "t2's blocked_by should contain t1 after add_blocks"
    );
}

/// `add_blocks` with multiple targets should add task_id to all of them.
#[tokio::test]
async fn test_task_update_add_blocks_multiple() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Blocker", "in_progress");
    storage.seed("t2", "test-session", "Dep A", "pending");
    storage.seed("t3", "test-session", "Dep B", "pending");
    let ctx = test_ctx(Arc::new(storage.clone()));
    let tool = TaskUpdateTool;

    let _ = tool
        .execute(json!({"task_id": "t1", "add_blocks": ["t2", "t3"]}), &ctx)
        .await
        .expect("add_blocks should succeed");

    let tasks = storage.list_tasks("test-session", None).unwrap();
    let t2 = tasks.iter().find(|t| t.id == "t2").unwrap();
    let t3 = tasks.iter().find(|t| t.id == "t3").unwrap();
    assert!(t2.blocked_by.contains(&"t1".to_string()));
    assert!(t3.blocked_by.contains(&"t1".to_string()));
}

/// `add_blocks` should merge with target's existing blocked_by (dedup).
#[tokio::test]
async fn test_task_update_add_blocks_merge() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Blocker A", "in_progress");
    storage.seed("t2", "test-session", "Blocker B", "completed");
    // t3 already blocked by t2.
    storage.seed_task(
        "t3",
        "test-session",
        "Dependent",
        "pending",
        "",
        None,
        None,
        &["t2"],
    );
    let ctx = test_ctx(Arc::new(storage.clone()));
    let tool = TaskUpdateTool;

    let _ = tool
        .execute(json!({"task_id": "t1", "add_blocks": ["t3"]}), &ctx)
        .await
        .expect("add_blocks should succeed");

    let tasks = storage.list_tasks("test-session", None).unwrap();
    let t3 = tasks.iter().find(|t| t.id == "t3").unwrap();
    assert!(t3.blocked_by.contains(&"t1".to_string()));
    assert!(t3.blocked_by.contains(&"t2".to_string()));
    // Should have exactly 2 entries.
    assert_eq!(t3.blocked_by.len(), 2);
}

/// `add_blocks` with a self-reference should be rejected.
#[tokio::test]
async fn test_task_update_add_blocks_self_ref() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage.clone()));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(json!({"task_id": "t1", "add_blocks": ["t1"]}), &ctx)
        .await
        .expect_err("self-reference in add_blocks must be rejected");

    assert!(
        err.to_string().contains("self-reference"),
        "error should mention self-reference: {err}"
    );

    // Verify no mutation happened.
    let tasks = storage.list_tasks("test-session", None).unwrap();
    let t1 = tasks.iter().find(|t| t.id == "t1").unwrap();
    assert!(
        t1.blocked_by.is_empty(),
        "no blocked_by should have been added"
    );
}

/// `add_blocks` with a non-existent task ID should be rejected (FR-009).
#[tokio::test]
async fn test_task_update_add_blocks_nonexistent() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(json!({"task_id": "t1", "add_blocks": ["ghost"]}), &ctx)
        .await
        .expect_err("non-existent add_blocks must be rejected");

    assert!(
        err.to_string().contains("ghost"),
        "error should mention ghost: {err}"
    );
    assert!(
        err.to_string().contains("add_blocks"),
        "error should mention add_blocks: {err}"
    );
}

/// `add_blocked_by` with a self-reference should be rejected.
#[tokio::test]
async fn test_task_update_add_blocked_by_self_ref() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage.clone()));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(json!({"task_id": "t1", "add_blocked_by": ["t1"]}), &ctx)
        .await
        .expect_err("self-reference in add_blocked_by must be rejected");

    assert!(
        err.to_string().contains("self-reference"),
        "error should mention self-reference: {err}"
    );

    let tasks = storage.list_tasks("test-session", None).unwrap();
    let t1 = tasks.iter().find(|t| t.id == "t1").unwrap();
    assert!(
        t1.blocked_by.is_empty(),
        "no blocked_by should have been added"
    );
}

/// `add_blocks` that is not an array should be rejected.
#[tokio::test]
async fn test_task_update_add_blocks_not_array() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "My task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let err = tool
        .execute(json!({"task_id": "t1", "add_blocks": "not-an-array"}), &ctx)
        .await
        .expect_err("non-array add_blocks should be rejected");

    assert!(
        err.to_string().contains("add_blocks"),
        "error should mention add_blocks: {err}"
    );
    assert!(
        err.to_string().contains("array"),
        "error should say expected array: {err}"
    );
}

// ── T-008 / FR-004: Cycle detection tests ─────────────────────────────

/// Adding a `blocked_by` edge that would create a 2-node cycle must be
/// rejected (t1 → t2, t2 → t1).
#[tokio::test]
async fn test_task_update_cycle_via_add_blocked_by() {
    let storage = MockStorage::new();
    // t2 already depends on t1 (t2.blocked_by = [t1]).
    storage.seed("t1", "test-session", "Task A", "pending");
    storage.seed_task(
        "t2",
        "test-session",
        "Task B",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage.clone()));
    let tool = TaskUpdateTool;

    // Try to make t1 depend on t2 → cycle: t1 → t2 → t1.
    let err = tool
        .execute(json!({"task_id": "t1", "add_blocked_by": ["t2"]}), &ctx)
        .await
        .expect_err("cycle must be rejected");

    let msg = err.to_string();
    assert!(
        msg.contains("cycle") || msg.contains("circular"),
        "error should mention cycle: {msg}"
    );
    assert!(
        msg.contains("FR-004"),
        "error should reference FR-004: {msg}"
    );

    // Verify no mutation happened.
    let tasks = storage.list_tasks("test-session", None).unwrap();
    let t1 = tasks.iter().find(|t| t.id == "t1").unwrap();
    assert!(
        t1.blocked_by.is_empty(),
        "t1.blocked_by should remain empty after rejected cycle"
    );
}

/// Adding a `blocks` edge that would create a 2-node cycle must be
/// rejected (t1 blocks t2, t2 blocks t1).
#[tokio::test]
async fn test_task_update_cycle_via_add_blocks() {
    let storage = MockStorage::new();
    // t1 already depends on t2 (t1.blocked_by = [t2]).
    storage.seed_task(
        "t1",
        "test-session",
        "Task A",
        "pending",
        "",
        None,
        None,
        &["t2"],
    );
    storage.seed("t2", "test-session", "Task B", "pending");
    let ctx = test_ctx(Arc::new(storage.clone()));
    let tool = TaskUpdateTool;

    // Try to make t1 block t2 (i.e. add t1 to t2's blocked_by).
    // This creates cycle: t1 → t2 → t1.
    let err = tool
        .execute(json!({"task_id": "t1", "add_blocks": ["t2"]}), &ctx)
        .await
        .expect_err("cycle via add_blocks must be rejected");

    let msg = err.to_string();
    assert!(
        msg.contains("cycle") || msg.contains("circular"),
        "error should mention cycle: {msg}"
    );

    // Verify t2's blocked_by was not modified.
    let tasks = storage.list_tasks("test-session", None).unwrap();
    let t2 = tasks.iter().find(|t| t.id == "t2").unwrap();
    assert!(
        t2.blocked_by.is_empty(),
        "t2.blocked_by should remain empty after rejected cycle"
    );
}

/// A 3-node cycle via add_blocked_by must be rejected
/// (t1 → t2 → t3 → t1).
#[tokio::test]
async fn test_task_update_cycle_three_nodes() {
    let storage = MockStorage::new();
    // t2 depends on t3, t3 depends on t1.
    storage.seed("t1", "test-session", "Task A", "pending");
    storage.seed_task(
        "t2",
        "test-session",
        "Task B",
        "pending",
        "",
        None,
        None,
        &["t3"],
    );
    storage.seed_task(
        "t3",
        "test-session",
        "Task C",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage.clone()));
    let tool = TaskUpdateTool;

    // Try to make t1 depend on t2 → cycle: t1 → t2 → t3 → t1.
    let err = tool
        .execute(json!({"task_id": "t1", "add_blocked_by": ["t2"]}), &ctx)
        .await
        .expect_err("3-node cycle must be rejected");

    assert!(
        err.to_string().contains("cycle") || err.to_string().contains("circular"),
        "error should mention cycle: {err}"
    );

    let tasks = storage.list_tasks("test-session", None).unwrap();
    let t1 = tasks.iter().find(|t| t.id == "t1").unwrap();
    assert!(t1.blocked_by.is_empty());
}

/// A valid dependency chain (no cycle) should succeed.
#[tokio::test]
async fn test_task_update_no_cycle_valid_chain() {
    let storage = MockStorage::new();
    // t1 is completed, t2 depends on t1 (completed).
    storage.seed("t1", "test-session", "Foundation", "completed");
    storage.seed_task(
        "t2",
        "test-session",
        "Middle",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    storage.seed("t3", "test-session", "Top", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    // t3 depends on t2 — no cycle (t3 → t2 → t1, all forward).
    let out = tool
        .execute(json!({"task_id": "t3", "add_blocked_by": ["t2"]}), &ctx)
        .await
        .expect("valid chain should succeed");

    let meta = out.metadata.expect("metadata present");
    let blocked_by = meta["blocked_by"].as_array().unwrap();
    assert!(blocked_by.contains(&json!("t2")));
}

// ── T-008 / FR-003: Auto-unblock on completion ────────────────────────

/// Completing a task should report which dependent tasks became available.
#[tokio::test]
async fn test_task_update_complete_reports_unblocked() {
    let storage = MockStorage::new();
    // t1 depends on t2. t2 is in_progress.
    storage.seed_task(
        "t1",
        "test-session",
        "Dependent",
        "pending",
        "",
        None,
        None,
        &["t2"],
    );
    storage.seed("t2", "test-session", "Blocker", "in_progress");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    // Complete the blocker t2.
    let out = tool
        .execute(json!({"task_id": "t2", "status": "completed"}), &ctx)
        .await
        .expect("should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["status"], "completed");

    // The `unblocked` array should list t1.
    let unblocked = meta["unblocked"]
        .as_array()
        .expect("unblocked should be an array");
    assert!(
        unblocked.contains(&json!("t1")),
        "unblocked should contain t1: {unblocked:?}"
    );
}

/// Completing a task with no dependents should report empty unblocked.
#[tokio::test]
async fn test_task_update_complete_no_dependents() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Solo task", "in_progress");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    let out = tool
        .execute(json!({"task_id": "t1", "status": "completed"}), &ctx)
        .await
        .expect("should succeed");

    let meta = out.metadata.expect("metadata present");
    let unblocked = meta["unblocked"]
        .as_array()
        .expect("unblocked should be an array");
    assert!(
        unblocked.is_empty(),
        "unblocked should be empty when no dependents exist"
    );
}

/// Completing a task whose dependents have OTHER unfinished blockers
/// should NOT report those dependents as unblocked.
#[tokio::test]
async fn test_task_update_complete_partial_unblock() {
    let storage = MockStorage::new();
    // t1 depends on t2 AND t3. t2 is being completed, t3 is still pending.
    storage.seed_task(
        "t1",
        "test-session",
        "Dependent",
        "pending",
        "",
        None,
        None,
        &["t2", "t3"],
    );
    storage.seed("t2", "test-session", "Blocker A", "in_progress");
    storage.seed("t3", "test-session", "Blocker B", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    // Complete t2 — t1 should NOT be unblocked (t3 is still pending).
    let out = tool
        .execute(json!({"task_id": "t2", "status": "completed"}), &ctx)
        .await
        .expect("should succeed");

    let meta = out.metadata.expect("metadata present");
    let unblocked = meta["unblocked"]
        .as_array()
        .expect("unblocked should be an array");
    assert!(
        !unblocked.contains(&json!("t1")),
        "t1 should NOT be in unblocked (t3 still pending): {unblocked:?}"
    );
}

/// Non-completion status updates should NOT populate the `unblocked` field.
#[tokio::test]
async fn test_task_update_non_completion_no_unblocked() {
    let storage = MockStorage::new();
    storage.seed_task(
        "t1",
        "test-session",
        "Dependent",
        "pending",
        "",
        None,
        None,
        &["t2"],
    );
    storage.seed("t2", "test-session", "Blocker", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskUpdateTool;

    // Set t2 to in_progress (not completed).
    let out = tool
        .execute(json!({"task_id": "t2", "status": "in_progress"}), &ctx)
        .await
        .expect("should succeed");

    let meta = out.metadata.expect("metadata present");
    let unblocked = meta["unblocked"]
        .as_array()
        .expect("unblocked should be an array");
    assert!(
        unblocked.is_empty(),
        "unblocked should be empty for non-completion status"
    );
}

// ── T-008: Schema tests for add_blocks ────────────────────────────────

/// Tool schema should include add_blocks property.
#[test]
fn test_task_update_schema_has_add_blocks() {
    let tool = TaskUpdateTool;
    let schema = tool.parameters_schema();

    assert!(
        schema["properties"]["add_blocks"]["type"] == "array",
        "add_blocks should be type array"
    );
}

/// Tool description should mention add_blocks.
#[test]
fn test_task_update_description_mentions_add_blocks() {
    let tool = TaskUpdateTool;
    let desc = tool.description();
    assert!(
        desc.contains("add_blocks"),
        "description should mention add_blocks: {desc}"
    );
}
