//! Tests for todo_write status-change summaries.

use std::sync::Arc;

use ragent_tools_extended::storage::StorageBackend;
use ragent_tools_extended::todo::TodoWriteTool;
use ragent_tools_extended::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

#[path = "support/mock_storage.rs"]
mod mock_storage;
use mock_storage::MockStorage;

/// In-memory mock storage for testing.
fn test_ctx(storage: Arc<dyn StorageBackend>) -> ToolContext {
    ToolContext {
        session_id: "sess-1".to_string(),
        working_dir: std::path::PathBuf::from("/tmp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: Some(storage),
        code_index: None,
        config: None,
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    }
}

#[tokio::test]
async fn test_todo_update_shows_status_change() {
    let storage = Arc::new(MockStorage::new());
    storage.seed("todo-1", "sess-1", "My task", "pending");

    let tool = TodoWriteTool;
    let output = tool
        .execute(
            json!({
                "action": "update",
                "id": "todo-1",
                "status": "in_progress"
            }),
            &test_ctx(storage),
        )
        .await
        .unwrap();

    assert!(
        output
            .content
            .contains("Updated todo 'todo-1' (pending -> in_progress)")
    );
}

#[tokio::test]
async fn test_todo_complete_shows_status_change() {
    let storage = Arc::new(MockStorage::new());
    storage.seed("todo-1", "sess-1", "My task", "in_progress");

    let tool = TodoWriteTool;
    let output = tool
        .execute(
            json!({
                "action": "complete",
                "id": "todo-1"
            }),
            &test_ctx(storage),
        )
        .await
        .unwrap();

    assert!(
        output
            .content
            .contains("Marked todo 'todo-1' as done (in_progress -> done)")
    );
}

#[tokio::test]
async fn test_todo_update_without_status_omits_arrow() {
    let storage = Arc::new(MockStorage::new());
    storage.seed("todo-1", "sess-1", "My task", "pending");

    let tool = TodoWriteTool;
    let output = tool
        .execute(
            json!({
                "action": "update",
                "id": "todo-1",
                "title": "Renamed task"
            }),
            &test_ctx(storage),
        )
        .await
        .unwrap();

    assert!(output.content.contains("Updated todo 'todo-1'"));
    assert!(!output.content.contains("->"));
}

#[tokio::test]
async fn test_todo_complete_from_done_still_shows_arrow() {
    let storage = Arc::new(MockStorage::new());
    storage.seed("todo-1", "sess-1", "My task", "done");

    let tool = TodoWriteTool;
    let output = tool
        .execute(
            json!({
                "action": "done",
                "id": "todo-1"
            }),
            &test_ctx(storage),
        )
        .await
        .unwrap();

    assert!(
        output
            .content
            .contains("Marked todo 'todo-1' as done (done -> done)")
    );
}

#[tokio::test]
async fn test_todo_update_nonexistent_returns_error() {
    let storage = Arc::new(MockStorage::new());

    let tool = TodoWriteTool;
    let result = tool
        .execute(
            json!({
                "action": "update",
                "id": "missing",
                "status": "done"
            }),
            &test_ctx(storage),
        )
        .await;

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn test_todo_update_includes_status_metadata() {
    let storage = Arc::new(MockStorage::new());
    storage.seed("todo-1", "sess-1", "My task", "pending");

    let tool = TodoWriteTool;
    let output = tool
        .execute(
            json!({
                "action": "update",
                "id": "todo-1",
                "status": "in_progress"
            }),
            &test_ctx(storage),
        )
        .await
        .unwrap();

    let meta = output.metadata.unwrap();
    assert_eq!(meta["action"], "update");
    assert_eq!(meta["old_status"], "pending");
    assert_eq!(meta["new_status"], "in_progress");
}

#[tokio::test]
async fn test_todo_complete_includes_status_metadata() {
    let storage = Arc::new(MockStorage::new());
    storage.seed("todo-1", "sess-1", "My task", "in_progress");

    let tool = TodoWriteTool;
    let output = tool
        .execute(
            json!({
                "action": "complete",
                "id": "todo-1"
            }),
            &test_ctx(storage),
        )
        .await
        .unwrap();

    let meta = output.metadata.unwrap();
    assert_eq!(meta["action"], "complete");
    assert_eq!(meta["old_status"], "in_progress");
    assert_eq!(meta["new_status"], "done");
}

#[tokio::test]
async fn test_todo_update_title_only_omits_status_metadata() {
    let storage = Arc::new(MockStorage::new());
    storage.seed("todo-1", "sess-1", "My task", "pending");

    let tool = TodoWriteTool;
    let output = tool
        .execute(
            json!({
                "action": "update",
                "id": "todo-1",
                "title": "Renamed task"
            }),
            &test_ctx(storage),
        )
        .await
        .unwrap();

    let meta = output.metadata.unwrap();
    assert_eq!(meta["action"], "update");
    assert!(!meta.as_object().unwrap().contains_key("old_status"));
    assert!(!meta.as_object().unwrap().contains_key("new_status"));
}

#[tokio::test]
async fn test_todo_add_omits_status_metadata() {
    let storage = Arc::new(MockStorage::new());

    let tool = TodoWriteTool;
    let output = tool
        .execute(
            json!({
                "action": "add",
                "title": "New task",
                "status": "pending"
            }),
            &test_ctx(storage),
        )
        .await
        .unwrap();

    let meta = output.metadata.unwrap();
    assert_eq!(meta["action"], "add");
    assert!(!meta.as_object().unwrap().contains_key("old_status"));
    assert!(!meta.as_object().unwrap().contains_key("new_status"));
}
