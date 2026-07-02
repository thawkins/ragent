//! Tests for todo_write status-change summaries.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ragent_tools_extended::storage::{StorageBackend, TodoRow};
use ragent_tools_extended::todo::TodoWriteTool;
use ragent_tools_extended::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

/// In-memory mock storage for testing.
struct MockStorage {
    todos: Arc<Mutex<HashMap<String, TodoRow>>>,
}

impl MockStorage {
    fn new() -> Self {
        Self {
            todos: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn seed(&self, id: &str, session_id: &str, title: &str, status: &str) {
        let now = chrono::Utc::now().to_rfc3339();
        let row = TodoRow {
            id: id.to_string(),
            session_id: session_id.to_string(),
            title: title.to_string(),
            status: status.to_string(),
            description: String::new(),
            created_at: now.clone(),
            updated_at: now,
        };
        self.todos.lock().unwrap().insert(id.to_string(), row);
    }
}

impl StorageBackend for MockStorage {
    fn get_todos(
        &self,
        _session_id: &str,
        status_filter: Option<&str>,
    ) -> anyhow::Result<Vec<TodoRow>> {
        let lock = self.todos.lock().unwrap();
        let mut rows: Vec<TodoRow> = lock.values().cloned().collect();
        if let Some(filter) = status_filter {
            rows.retain(|r| r.status == filter);
        }
        Ok(rows)
    }

    fn create_todo(
        &self,
        id: &str,
        session_id: &str,
        title: &str,
        status: &str,
        description: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let row = TodoRow {
            id: id.to_string(),
            session_id: session_id.to_string(),
            title: title.to_string(),
            status: status.to_string(),
            description: description.to_string(),
            created_at: now.clone(),
            updated_at: now,
        };
        self.todos.lock().unwrap().insert(id.to_string(), row);
        Ok(())
    }

    fn update_todo(
        &self,
        id: &str,
        _session_id: &str,
        title: Option<&str>,
        status: Option<&str>,
        description: Option<&str>,
    ) -> anyhow::Result<bool> {
        let mut lock = self.todos.lock().unwrap();
        if let Some(row) = lock.get_mut(id) {
            if let Some(t) = title {
                row.title = t.to_string();
            }
            if let Some(s) = status {
                row.status = s.to_string();
            }
            if let Some(d) = description {
                row.description = d.to_string();
            }
            row.updated_at = chrono::Utc::now().to_rfc3339();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn delete_todo(&self, id: &str, _session_id: &str) -> anyhow::Result<bool> {
        Ok(self.todos.lock().unwrap().remove(id).is_some())
    }

    fn clear_todos(&self, _session_id: &str) -> anyhow::Result<usize> {
        let mut lock = self.todos.lock().unwrap();
        let count = lock.len();
        lock.clear();
        Ok(count)
    }

    fn get_memory(
        &self,
        _id: i64,
    ) -> anyhow::Result<Option<ragent_tools_extended::storage::MemoryRow>> {
        Ok(None)
    }

    fn get_memory_tags(&self, _id: i64) -> anyhow::Result<Vec<String>> {
        Ok(vec![])
    }

    fn search_memories(
        &self,
        _query: &str,
        _category: Option<&str>,
        _source: Option<&str>,
        _limit: usize,
        _min_confidence: f64,
    ) -> anyhow::Result<Vec<ragent_tools_extended::storage::MemoryRow>> {
        Ok(vec![])
    }

    fn list_memories(
        &self,
        _project: &str,
        _limit: usize,
    ) -> anyhow::Result<Vec<ragent_tools_extended::storage::MemoryRow>> {
        Ok(vec![])
    }

    fn store_memory_embedding(&self, _id: i64, _embedding_blob: &[u8]) -> anyhow::Result<bool> {
        Ok(false)
    }

    fn list_memory_embeddings(&self) -> anyhow::Result<Vec<(i64, Vec<u8>)>> {
        Ok(vec![])
    }

    fn search_memories_by_embedding(
        &self,
        _query_embedding: &[f32],
        _dimensions: usize,
        _limit: usize,
        _min_similarity: f32,
    ) -> anyhow::Result<Vec<ragent_tools_extended::storage::EmbeddingMatch>> {
        Ok(vec![])
    }
}

fn test_ctx(storage: Arc<dyn StorageBackend>) -> ToolContext {
    ToolContext {
        session_id: "sess-1".to_string(),
        working_dir: std::path::PathBuf::from("/tmp"),
        event_bus: Arc::new(EventBus::new(16)),
        storage: Some(storage),
        code_index: None,
        config: None,
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
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
