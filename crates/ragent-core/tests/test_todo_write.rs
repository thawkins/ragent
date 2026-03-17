//! Tests for the `todo_write` tool.

use std::sync::Arc;
use std::path::PathBuf;

use serde_json::json;

use ragent_core::event::EventBus;
use ragent_core::storage::Storage;
use ragent_core::tool::{Tool, ToolContext};
use ragent_core::tool::todo::TodoWriteTool;

fn make_ctx(storage: Arc<Storage>) -> ToolContext {
    ToolContext {
        session_id: "test-session".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: Some(storage),
        task_manager: None,
            lsp_manager: None,
            active_model: None,
    }
}

fn setup() -> Arc<Storage> {
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    storage
        .create_session("test-session", "/tmp")
        .expect("create session");
    storage
}

// ── Metadata ────────────────────────────────────────────────────────

#[test]
fn test_todo_write_name() {
    assert_eq!(TodoWriteTool.name(), "todo_write");
}

#[test]
fn test_todo_write_description() {
    assert!(!TodoWriteTool.description().is_empty());
}

#[test]
fn test_todo_write_permission() {
    assert_eq!(TodoWriteTool.permission_category(), "todo");
}

#[test]
fn test_todo_write_schema_has_action_required() {
    let schema = TodoWriteTool.parameters_schema();
    assert_eq!(schema["type"], "object");
    let required = schema["required"].as_array().unwrap();
    assert!(required.contains(&json!("action")));
}

// ── Add action ──────────────────────────────────────────────────────

#[tokio::test]
async fn test_add_basic() {
    let storage = setup();
    let ctx = make_ctx(storage.clone());

    let result = TodoWriteTool
        .execute(json!({"action": "add", "title": "Fix bug"}), &ctx)
        .await
        .unwrap();

    assert!(result.content.contains("Added todo"));
    assert!(result.content.contains("Fix bug"));
    assert_eq!(result.metadata.as_ref().unwrap()["count"], 1);
    assert_eq!(result.metadata.as_ref().unwrap()["action"], "add");

    let todos = storage.get_todos("test-session", None).unwrap();
    assert_eq!(todos.len(), 1);
    assert_eq!(todos[0].title, "Fix bug");
    assert_eq!(todos[0].status, "pending");
}

#[tokio::test]
async fn test_add_with_status_and_description() {
    let storage = setup();
    let ctx = make_ctx(storage.clone());

    let result = TodoWriteTool
        .execute(
            json!({"action": "add", "title": "Deploy", "status": "blocked", "description": "Waiting on CI"}),
            &ctx,
        )
        .await
        .unwrap();

    assert!(result.content.contains("blocked"));
    let todos = storage.get_todos("test-session", None).unwrap();
    assert_eq!(todos[0].status, "blocked");
    assert_eq!(todos[0].description, "Waiting on CI");
}

#[tokio::test]
async fn test_add_with_custom_id() {
    let storage = setup();
    let ctx = make_ctx(storage.clone());

    TodoWriteTool
        .execute(json!({"action": "add", "title": "My task", "id": "my-custom-id"}), &ctx)
        .await
        .unwrap();

    let todos = storage.get_todos("test-session", None).unwrap();
    assert_eq!(todos[0].id, "my-custom-id");
}

#[tokio::test]
async fn test_add_missing_title() {
    let storage = setup();
    let ctx = make_ctx(storage);

    let err = TodoWriteTool
        .execute(json!({"action": "add"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("title"));
}

#[tokio::test]
async fn test_add_empty_title() {
    let storage = setup();
    let ctx = make_ctx(storage);

    let err = TodoWriteTool
        .execute(json!({"action": "add", "title": "  "}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("empty"));
}

#[tokio::test]
async fn test_add_invalid_status() {
    let storage = setup();
    let ctx = make_ctx(storage);

    let err = TodoWriteTool
        .execute(json!({"action": "add", "title": "X", "status": "invalid"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Invalid status"));
}

// ── Update action ───────────────────────────────────────────────────

#[tokio::test]
async fn test_update_status() {
    let storage = setup();
    storage.create_todo("t1", "test-session", "Task", "pending", "").unwrap();
    let ctx = make_ctx(storage.clone());

    let result = TodoWriteTool
        .execute(json!({"action": "update", "id": "t1", "status": "done"}), &ctx)
        .await
        .unwrap();

    assert!(result.content.contains("Updated todo 't1'"));
    let todos = storage.get_todos("test-session", None).unwrap();
    assert_eq!(todos[0].status, "done");
}

#[tokio::test]
async fn test_update_title_and_description() {
    let storage = setup();
    storage.create_todo("t1", "test-session", "Old", "pending", "old desc").unwrap();
    let ctx = make_ctx(storage.clone());

    TodoWriteTool
        .execute(
            json!({"action": "update", "id": "t1", "title": "New", "description": "new desc"}),
            &ctx,
        )
        .await
        .unwrap();

    let todos = storage.get_todos("test-session", None).unwrap();
    assert_eq!(todos[0].title, "New");
    assert_eq!(todos[0].description, "new desc");
}

#[tokio::test]
async fn test_update_missing_id() {
    let storage = setup();
    let ctx = make_ctx(storage);

    let err = TodoWriteTool
        .execute(json!({"action": "update", "status": "done"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("id"));
}

#[tokio::test]
async fn test_update_no_fields() {
    let storage = setup();
    storage.create_todo("t1", "test-session", "Task", "pending", "").unwrap();
    let ctx = make_ctx(storage);

    let err = TodoWriteTool
        .execute(json!({"action": "update", "id": "t1"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("At least one"));
}

#[tokio::test]
async fn test_update_nonexistent() {
    let storage = setup();
    let ctx = make_ctx(storage);

    let err = TodoWriteTool
        .execute(json!({"action": "update", "id": "nope", "status": "done"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not found"));
}

#[tokio::test]
async fn test_update_invalid_status() {
    let storage = setup();
    storage.create_todo("t1", "test-session", "Task", "pending", "").unwrap();
    let ctx = make_ctx(storage);

    let err = TodoWriteTool
        .execute(json!({"action": "update", "id": "t1", "status": "all"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Invalid status"));
}

// ── Remove action ───────────────────────────────────────────────────

#[tokio::test]
async fn test_remove() {
    let storage = setup();
    storage.create_todo("t1", "test-session", "Task", "pending", "").unwrap();
    let ctx = make_ctx(storage.clone());

    let result = TodoWriteTool
        .execute(json!({"action": "remove", "id": "t1"}), &ctx)
        .await
        .unwrap();

    assert!(result.content.contains("Removed todo 't1'"));
    assert_eq!(result.metadata.as_ref().unwrap()["count"], 0);
    assert!(storage.get_todos("test-session", None).unwrap().is_empty());
}

#[tokio::test]
async fn test_remove_missing_id() {
    let storage = setup();
    let ctx = make_ctx(storage);

    let err = TodoWriteTool
        .execute(json!({"action": "remove"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("id"));
}

#[tokio::test]
async fn test_remove_nonexistent() {
    let storage = setup();
    let ctx = make_ctx(storage);

    let err = TodoWriteTool
        .execute(json!({"action": "remove", "id": "nope"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not found"));
}

// ── Clear action ────────────────────────────────────────────────────

#[tokio::test]
async fn test_clear() {
    let storage = setup();
    storage.create_todo("t1", "test-session", "A", "pending", "").unwrap();
    storage.create_todo("t2", "test-session", "B", "done", "").unwrap();
    let ctx = make_ctx(storage.clone());

    let result = TodoWriteTool
        .execute(json!({"action": "clear"}), &ctx)
        .await
        .unwrap();

    assert!(result.content.contains("Cleared 2 todo items"));
    assert_eq!(result.metadata.as_ref().unwrap()["count"], 0);
    assert!(storage.get_todos("test-session", None).unwrap().is_empty());
}

#[tokio::test]
async fn test_clear_empty() {
    let storage = setup();
    let ctx = make_ctx(storage);

    let result = TodoWriteTool
        .execute(json!({"action": "clear"}), &ctx)
        .await
        .unwrap();

    assert!(result.content.contains("Cleared 0 todo items"));
}

#[tokio::test]
async fn test_clear_session_isolation() {
    let storage = setup();
    storage.create_session("other", "/tmp").unwrap();
    storage.create_todo("t1", "test-session", "Mine", "pending", "").unwrap();
    storage.create_todo("t2", "other", "Theirs", "pending", "").unwrap();
    let ctx = make_ctx(storage.clone());

    TodoWriteTool
        .execute(json!({"action": "clear"}), &ctx)
        .await
        .unwrap();

    // My todos cleared, other session untouched
    assert!(storage.get_todos("test-session", None).unwrap().is_empty());
    assert_eq!(storage.get_todos("other", None).unwrap().len(), 1);
}

// ── Error cases ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_missing_action() {
    let storage = setup();
    let ctx = make_ctx(storage);

    let err = TodoWriteTool
        .execute(json!({}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("action"));
}

#[tokio::test]
async fn test_invalid_action() {
    let storage = setup();
    let ctx = make_ctx(storage);

    let err = TodoWriteTool
        .execute(json!({"action": "destroy"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Invalid action"));
}

#[tokio::test]
async fn test_no_storage() {
    let ctx = ToolContext {
        session_id: "s".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        task_manager: None,
            lsp_manager: None,
            active_model: None,
    };

    let err = TodoWriteTool
        .execute(json!({"action": "add", "title": "X"}), &ctx)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("Storage is not available"));
}

// ── Registry ────────────────────────────────────────────────────────

#[test]
fn test_todo_write_in_registry() {
    let registry = ragent_core::tool::create_default_registry();
    assert!(registry.get("todo_write").is_some());
    assert_eq!(registry.list().len(), 31);
}
