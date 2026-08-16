//! todo2tasks T-007: integration tests for the `task_create` tool
//! (`TaskCreateTool`).
//!
//! Verifies FR-009 (blocked_by validation), FR-011 (tool surface),
//! FR-012 (status defaults to pending), FR-006 (owner), FR-007
//! (active_form), FR-008 (metadata).

use std::sync::Arc;

use ragent_tools_extended::storage::StorageBackend;
use ragent_tools_extended::task::TaskCreateTool;
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

// ── Basic creation ───────────────────────────────────────────────────

/// Create a task with only the required fields; status should be "pending".
#[tokio::test]
async fn test_task_create_minimal() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let out = tool
        .execute(
            json!({
                "subject": "Write tests",
                "description": "Write unit tests for the auth module"
            }),
            &ctx,
        )
        .await
        .expect("task_create should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["subject"], "Write tests");
    assert_eq!(meta["description"], "Write unit tests for the auth module");
    assert_eq!(meta["status"], "pending");
    assert_eq!(meta["active_form"], serde_json::Value::Null);
    assert_eq!(meta["owner"], serde_json::Value::Null);
    assert_eq!(meta["metadata"], json!({}));
    assert_eq!(meta["blocked_by"], json!([]));
    assert_eq!(meta["blocks"], json!([]));
    assert_eq!(meta["is_blocked"], false);
    assert_eq!(meta["is_available"], true);

    // ID should start with "task-".
    let id = meta["id"].as_str().expect("id is string");
    assert!(
        id.starts_with("task-"),
        "id should start with 'task-', got: {id}"
    );

    // Content should mention the subject.
    assert!(
        out.content.contains("Write tests"),
        "content: {}",
        out.content
    );
    assert!(out.content.contains("pending"), "content: {}", out.content);
}

/// Create a task with all optional fields populated.
#[tokio::test]
async fn test_task_create_full() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Setup project", "completed");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let out = tool
        .execute(
            json!({
                "subject": "Implement auth",
                "description": "Add JWT authentication with refresh tokens",
                "active_form": "Implementing JWT auth",
                "owner": "coder-agent",
                "metadata": {"phase": "backend", "priority": "high"},
                "blocked_by": ["t1"]
            }),
            &ctx,
        )
        .await
        .expect("task_create should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["subject"], "Implement auth");
    assert_eq!(
        meta["description"],
        "Add JWT authentication with refresh tokens"
    );
    assert_eq!(meta["active_form"], "Implementing JWT auth");
    assert_eq!(meta["owner"], "coder-agent");
    assert_eq!(meta["status"], "pending");
    assert_eq!(meta["metadata"]["phase"], "backend");
    assert_eq!(meta["metadata"]["priority"], "high");
    assert_eq!(meta["blocked_by"], json!(["t1"]));
    // blocks is empty because no other task lists this new task in blocked_by.
    assert_eq!(meta["blocks"], json!([]));
    // is_blocked is false because t1 is completed.
    assert_eq!(meta["is_blocked"], false);
    // is_available is false because owner is set.
    assert_eq!(meta["is_available"], false);
}

// ── Required parameter validation ────────────────────────────────────

/// Missing `subject` should error.
#[tokio::test]
async fn test_task_create_missing_subject() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let err = tool
        .execute(json!({"description": "some description"}), &ctx)
        .await
        .expect_err("should fail without subject");

    assert!(
        err.to_string().contains("subject"),
        "error should mention subject: {err}"
    );
}

/// Missing `description` should error.
#[tokio::test]
async fn test_task_create_missing_description() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let err = tool
        .execute(json!({"subject": "Do something"}), &ctx)
        .await
        .expect_err("should fail without description");

    assert!(
        err.to_string().contains("description"),
        "error should mention description: {err}"
    );
}

/// Missing both required parameters should error.
#[tokio::test]
async fn test_task_create_missing_both_required() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let err = tool
        .execute(json!({}), &ctx)
        .await
        .expect_err("should fail without required params");

    assert!(
        err.to_string().contains("subject"),
        "error should mention subject: {err}"
    );
}

// ── Storage availability ─────────────────────────────────────────────

/// No storage should produce a clear error.
#[tokio::test]
async fn test_task_create_no_storage() {
    let ctx = ToolContext {
        storage: None,
        event_bus: Arc::new(EventBus::new(128)),
        session_id: "test-session".to_string(),
        working_dir: std::env::current_dir().unwrap(),
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    };
    let tool = TaskCreateTool;

    let err = tool
        .execute(json!({"subject": "Test", "description": "desc"}), &ctx)
        .await
        .expect_err("should fail without storage");

    assert!(
        err.to_string().contains("Storage is not available"),
        "error: {err}"
    );
}

// ── FR-009: blocked_by validation ────────────────────────────────────

/// blocked_by referencing a non-existent task should error (FR-009).
#[tokio::test]
async fn test_task_create_blocked_by_nonexistent() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let err = tool
        .execute(
            json!({
                "subject": "Dependent task",
                "description": "Depends on a non-existent task",
                "blocked_by": ["nonexistent-id"]
            }),
            &ctx,
        )
        .await
        .expect_err("should fail with non-existent blocked_by");

    assert!(
        err.to_string().contains("nonexistent-id"),
        "error should mention the missing id: {err}"
    );
    assert!(
        err.to_string().contains("blocked_by"),
        "error should mention blocked_by: {err}"
    );
}

/// blocked_by with one valid and one non-existent should error.
#[tokio::test]
async fn test_task_create_blocked_by_partial_invalid() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Valid task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let err = tool
        .execute(
            json!({
                "subject": "Dependent task",
                "description": "Partially invalid deps",
                "blocked_by": ["t1", "ghost-id"]
            }),
            &ctx,
        )
        .await
        .expect_err("should fail with partially invalid blocked_by");

    assert!(
        err.to_string().contains("ghost-id"),
        "error should mention the missing id: {err}"
    );
}

/// blocked_by referencing a task in a different session should error (FR-009).
#[tokio::test]
async fn test_task_create_blocked_by_cross_session() {
    let storage = MockStorage::new();
    // Seed a task in a DIFFERENT session.
    storage.seed("other-task", "other-session", "Other task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let err = tool
        .execute(
            json!({
                "subject": "Cross-session dep",
                "description": "Depends on a task in another session",
                "blocked_by": ["other-task"]
            }),
            &ctx,
        )
        .await
        .expect_err("should fail with cross-session blocked_by");

    assert!(
        err.to_string().contains("other-task"),
        "error should mention the cross-session id: {err}"
    );
}

/// blocked_by with all valid IDs should succeed.
#[tokio::test]
async fn test_task_create_blocked_by_all_valid() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "First", "pending");
    storage.seed("t2", "test-session", "Second", "completed");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let out = tool
        .execute(
            json!({
                "subject": "Dependent",
                "description": "Depends on t1 and t2",
                "blocked_by": ["t1", "t2"]
            }),
            &ctx,
        )
        .await
        .expect("task_create should succeed with valid blocked_by");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["blocked_by"], json!(["t1", "t2"]));
    // t1 is pending → is_blocked should be true.
    assert_eq!(meta["is_blocked"], true);
    assert_eq!(meta["is_available"], false);
}

/// Empty blocked_by array should succeed (no validation needed).
#[tokio::test]
async fn test_task_create_empty_blocked_by() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let out = tool
        .execute(
            json!({
                "subject": "Independent",
                "description": "No deps",
                "blocked_by": []
            }),
            &ctx,
        )
        .await
        .expect("task_create should succeed with empty blocked_by");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["blocked_by"], json!([]));
    assert_eq!(meta["is_available"], true);
}

// ── FR-008: metadata validation ──────────────────────────────────────

/// metadata that is not a JSON object should error.
#[tokio::test]
async fn test_task_create_metadata_not_object() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let err = tool
        .execute(
            json!({
                "subject": "Test",
                "description": "desc",
                "metadata": "not-an-object"
            }),
            &ctx,
        )
        .await
        .expect_err("should fail with string metadata");

    assert!(
        err.to_string().contains("metadata"),
        "error should mention metadata: {err}"
    );
    assert!(
        err.to_string().contains("object"),
        "error should mention object: {err}"
    );
}

/// metadata as a number should error.
#[tokio::test]
async fn test_task_create_metadata_number() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let err = tool
        .execute(
            json!({
                "subject": "Test",
                "description": "desc",
                "metadata": 42
            }),
            &ctx,
        )
        .await
        .expect_err("should fail with numeric metadata");

    assert!(err.to_string().contains("metadata"), "error: {err}");
}

/// metadata as an array should error.
#[tokio::test]
async fn test_task_create_metadata_array() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let err = tool
        .execute(
            json!({
                "subject": "Test",
                "description": "desc",
                "metadata": [1, 2, 3]
            }),
            &ctx,
        )
        .await
        .expect_err("should fail with array metadata");

    assert!(err.to_string().contains("metadata"), "error: {err}");
}

/// metadata as an empty object should succeed.
#[tokio::test]
async fn test_task_create_metadata_empty_object() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let out = tool
        .execute(
            json!({
                "subject": "Test",
                "description": "desc",
                "metadata": {}
            }),
            &ctx,
        )
        .await
        .expect("should succeed with empty metadata object");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["metadata"], json!({}));
}

/// metadata with nested values should be preserved verbatim.
#[tokio::test]
async fn test_task_create_metadata_nested() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let out = tool
        .execute(
            json!({
                "subject": "Test",
                "description": "desc",
                "metadata": {"tags": ["backend", "auth"], "sprint": 3, "nested": {"key": "val"}}
            }),
            &ctx,
        )
        .await
        .expect("should succeed with nested metadata");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["metadata"]["tags"], json!(["backend", "auth"]));
    assert_eq!(meta["metadata"]["sprint"], 3);
    assert_eq!(meta["metadata"]["nested"]["key"], "val");
}

// ── FR-012: status defaults to pending ───────────────────────────────

/// The created task must always have status "pending".
#[tokio::test]
async fn test_task_create_status_always_pending() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let out = tool
        .execute(
            json!({
                "subject": "Test",
                "description": "desc"
            }),
            &ctx,
        )
        .await
        .expect("task_create should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(
        meta["status"], "pending",
        "newly created task must have status 'pending'"
    );
}

// ── ID generation ────────────────────────────────────────────────────

/// Generated ID should start with "task-" prefix.
#[tokio::test]
async fn test_task_create_id_prefix() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let out = tool
        .execute(json!({"subject": "Test", "description": "desc"}), &ctx)
        .await
        .expect("task_create should succeed");

    let meta = out.metadata.expect("metadata present");
    let id = meta["id"].as_str().expect("id is string");
    assert!(
        id.starts_with("task-"),
        "id should start with 'task-', got: {id}"
    );
    // UUID part should be 32 chars (simple format).
    let uuid_part = &id["task-".len()..];
    assert_eq!(
        uuid_part.len(),
        32,
        "uuid part should be 32 chars, got: {uuid_part}"
    );
}

/// Two tasks should get different IDs.
#[tokio::test]
async fn test_task_create_unique_ids() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let out1 = tool
        .execute(json!({"subject": "First", "description": "desc"}), &ctx)
        .await
        .expect("first task_create should succeed");

    let out2 = tool
        .execute(json!({"subject": "Second", "description": "desc"}), &ctx)
        .await
        .expect("second task_create should succeed");

    let id1 = out1.metadata.unwrap()["id"].as_str().unwrap().to_string();
    let id2 = out2.metadata.unwrap()["id"].as_str().unwrap().to_string();
    assert_ne!(id1, id2, "two tasks should get different IDs");
}

// ── FR-006: owner ────────────────────────────────────────────────────

/// Owner is stored and returned.
#[tokio::test]
async fn test_task_create_owner() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let out = tool
        .execute(
            json!({
                "subject": "Test",
                "description": "desc",
                "owner": "build-agent"
            }),
            &ctx,
        )
        .await
        .expect("task_create should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["owner"], "build-agent");
    // With owner set, is_available should be false.
    assert_eq!(meta["is_available"], false);
}

// ── FR-007: active_form ──────────────────────────────────────────────

/// active_form is stored and returned.
#[tokio::test]
async fn test_task_create_active_form() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let out = tool
        .execute(
            json!({
                "subject": "Implement auth",
                "description": "desc",
                "active_form": "Implementing auth"
            }),
            &ctx,
        )
        .await
        .expect("task_create should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["active_form"], "Implementing auth");
    // Content should mention active form.
    assert!(
        out.content.contains("Implementing auth"),
        "content should show active_form: {}",
        out.content
    );
}

// ── Tool metadata ────────────────────────────────────────────────────

/// Tool name should be "task_create".
#[test]
fn test_task_create_tool_name() {
    let tool = TaskCreateTool;
    assert_eq!(tool.name(), "task_create");
}

/// Tool description should be non-empty and mention "Create".
#[test]
fn test_task_create_tool_description() {
    let tool = TaskCreateTool;
    let desc = tool.description();
    assert!(!desc.is_empty());
    assert!(desc.contains("Create"));
}

/// Tool schema should require subject and description.
#[test]
fn test_task_create_tool_schema() {
    let tool = TaskCreateTool;
    let schema = tool.parameters_schema();

    let required = schema["required"].as_array().expect("required is array");
    assert!(
        required.iter().any(|v| v.as_str() == Some("subject")),
        "schema should require subject"
    );
    assert!(
        required.iter().any(|v| v.as_str() == Some("description")),
        "schema should require description"
    );

    let props = schema["properties"]
        .as_object()
        .expect("properties is object");
    assert!(props.contains_key("subject"));
    assert!(props.contains_key("description"));
    assert!(props.contains_key("active_form"));
    assert!(props.contains_key("owner"));
    assert!(props.contains_key("metadata"));
    assert!(props.contains_key("blocked_by"));
}

/// Permission category should be "task".
#[test]
fn test_task_create_tool_permission_category() {
    let tool = TaskCreateTool;
    assert_eq!(tool.permission_category(), "task");
}

// ── blocks derivation on create ──────────────────────────────────────

/// Creating a task with blocked_by should set `blocks` on the referenced tasks.
/// The new task's own `blocks` should be empty.
#[tokio::test]
async fn test_task_create_blocks_derived_empty() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Blocker", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let out = tool
        .execute(
            json!({
                "subject": "Dependent",
                "description": "Depends on t1",
                "blocked_by": ["t1"]
            }),
            &ctx,
        )
        .await
        .expect("task_create should succeed");

    let meta = out.metadata.expect("metadata present");
    // The new task's blocks should be empty (no other task depends on it yet).
    assert_eq!(meta["blocks"], json!([]));
}

// ── Content format ───────────────────────────────────────────────────

/// Content should contain the task header and subject.
#[tokio::test]
async fn test_task_create_content_format() {
    let storage = MockStorage::new();
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskCreateTool;

    let out = tool
        .execute(
            json!({
                "subject": "My Task",
                "description": "Do the thing"
            }),
            &ctx,
        )
        .await
        .expect("task_create should succeed");

    assert!(
        out.content.contains("## Task `task-"),
        "content should have task header: {}",
        out.content
    );
    assert!(
        out.content.contains("My Task"),
        "content should have subject: {}",
        out.content
    );
    assert!(
        out.content.contains("Do the thing"),
        "content should have description: {}",
        out.content
    );
    assert!(
        out.content.contains("[available]"),
        "content should show [available] for a pending task with no deps: {}",
        out.content
    );
}
