//! Tests for the `todo_read` tool.
//!
//! Validates that TODO items can be listed from storage, with optional
//! status filtering, correct markdown formatting, and error handling.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::json;

use ragent_core::event::EventBus;
use ragent_core::storage::Storage;
use ragent_core::tool::todo::TodoReadTool;
use ragent_core::tool::{Tool, ToolContext};

fn make_ctx_with_storage(storage: Arc<Storage>) -> ToolContext {
    ToolContext {
        session_id: "test-session".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: Some(storage),
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
    }
}

fn make_ctx_no_storage() -> ToolContext {
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

fn setup_storage() -> Arc<Storage> {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    storage
        .create_session("test-session", "/tmp")
        .expect("create session");
    storage
}

// ── Basic metadata ──────────────────────────────────────────────────

#[test]
fn test_todo_read_name() {
    let tool = TodoReadTool;
    assert_eq!(tool.name(), "todo_read");
}

#[test]
fn test_todo_read_description() {
    let tool = TodoReadTool;
    assert!(!tool.description().is_empty());
}

#[test]
fn test_todo_read_permission() {
    let tool = TodoReadTool;
    assert_eq!(tool.permission_category(), "todo");
}

#[test]
fn test_todo_read_schema() {
    let tool = TodoReadTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["status"].is_object());
}

// ── Execution tests ─────────────────────────────────────────────────

#[tokio::test]
async fn test_todo_read_empty_list() {
    let storage = setup_storage();
    let ctx = make_ctx_with_storage(storage);
    let tool = TodoReadTool;

    let result = tool.execute(json!({}), &ctx).await.unwrap();
    assert!(result.content.contains("No TODO items found"));
    assert_eq!(result.metadata.as_ref().unwrap()["count"], 0);
}

#[tokio::test]
async fn test_todo_read_with_items() {
    let storage = setup_storage();
    storage
        .create_todo("t1", "test-session", "Fix bug", "pending", "A nasty bug")
        .unwrap();
    storage
        .create_todo("t2", "test-session", "Write tests", "done", "")
        .unwrap();

    let ctx = make_ctx_with_storage(storage);
    let tool = TodoReadTool;

    let result = tool.execute(json!({}), &ctx).await.unwrap();
    assert!(result.content.contains("2 items"));
    assert!(result.content.contains("Fix bug"));
    assert!(result.content.contains("Write tests"));
    assert!(result.content.contains("⏳")); // pending icon
    assert!(result.content.contains("✅")); // done icon
    assert_eq!(result.metadata.as_ref().unwrap()["count"], 2);
}

#[tokio::test]
async fn test_todo_read_filter_pending() {
    let storage = setup_storage();
    storage
        .create_todo("t1", "test-session", "Fix bug", "pending", "")
        .unwrap();
    storage
        .create_todo("t2", "test-session", "Write tests", "done", "")
        .unwrap();

    let ctx = make_ctx_with_storage(storage);
    let tool = TodoReadTool;

    let result = tool
        .execute(json!({"status": "pending"}), &ctx)
        .await
        .unwrap();
    assert!(result.content.contains("1 items"));
    assert!(result.content.contains("Fix bug"));
    assert!(!result.content.contains("Write tests"));
    assert_eq!(
        result.metadata.as_ref().unwrap()["status_filter"],
        "pending"
    );
}

#[tokio::test]
async fn test_todo_read_filter_done() {
    let storage = setup_storage();
    storage
        .create_todo("t1", "test-session", "Fix bug", "pending", "")
        .unwrap();
    storage
        .create_todo("t2", "test-session", "Write tests", "done", "")
        .unwrap();

    let ctx = make_ctx_with_storage(storage);
    let tool = TodoReadTool;

    let result = tool.execute(json!({"status": "done"}), &ctx).await.unwrap();
    assert!(result.content.contains("1 items"));
    assert!(result.content.contains("Write tests"));
    assert!(!result.content.contains("Fix bug"));
}

#[tokio::test]
async fn test_todo_read_filter_all() {
    let storage = setup_storage();
    storage
        .create_todo("t1", "test-session", "Fix bug", "pending", "")
        .unwrap();
    storage
        .create_todo("t2", "test-session", "Write tests", "done", "")
        .unwrap();

    let ctx = make_ctx_with_storage(storage);
    let tool = TodoReadTool;

    let result = tool.execute(json!({"status": "all"}), &ctx).await.unwrap();
    assert!(result.content.contains("2 items"));
}

#[tokio::test]
async fn test_todo_read_invalid_status() {
    let storage = setup_storage();
    let ctx = make_ctx_with_storage(storage);
    let tool = TodoReadTool;

    let result = tool.execute(json!({"status": "invalid"}), &ctx).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Invalid status filter")
    );
}

#[tokio::test]
async fn test_todo_read_no_storage() {
    let ctx = make_ctx_no_storage();
    let tool = TodoReadTool;

    let result = tool.execute(json!({}), &ctx).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("Storage is not available")
    );
}

#[tokio::test]
async fn test_todo_read_session_isolation() {
    let storage = setup_storage();
    storage.create_session("other-session", "/tmp").unwrap();
    storage
        .create_todo("t1", "test-session", "My task", "pending", "")
        .unwrap();
    storage
        .create_todo("t2", "other-session", "Other task", "pending", "")
        .unwrap();

    let ctx = make_ctx_with_storage(storage);
    let tool = TodoReadTool;

    let result = tool.execute(json!({}), &ctx).await.unwrap();
    assert!(result.content.contains("My task"));
    assert!(!result.content.contains("Other task"));
    assert_eq!(result.metadata.as_ref().unwrap()["count"], 1);
}

#[tokio::test]
async fn test_todo_read_description_displayed() {
    let storage = setup_storage();
    storage
        .create_todo(
            "t1",
            "test-session",
            "Fix bug",
            "pending",
            "This is a detailed description",
        )
        .unwrap();

    let ctx = make_ctx_with_storage(storage);
    let tool = TodoReadTool;

    let result = tool.execute(json!({}), &ctx).await.unwrap();
    assert!(result.content.contains("This is a detailed description"));
}

#[tokio::test]
async fn test_todo_read_empty_filter_message() {
    let storage = setup_storage();
    storage
        .create_todo("t1", "test-session", "Fix bug", "done", "")
        .unwrap();

    let ctx = make_ctx_with_storage(storage);
    let tool = TodoReadTool;

    let result = tool
        .execute(json!({"status": "pending"}), &ctx)
        .await
        .unwrap();
    assert!(
        result
            .content
            .contains("No TODO items found with status 'pending'")
    );
}

// ── Registry integration ────────────────────────────────────────────

#[test]
fn test_todo_read_in_registry() {
    let registry = ragent_core::tool::create_default_registry();
    assert!(registry.get("todo_read").is_some());
    assert_eq!(registry.list().len(), 31);
}
