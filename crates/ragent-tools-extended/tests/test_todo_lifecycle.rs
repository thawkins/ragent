//! Tests for todo_read / todo_write full lifecycle.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ragent_tools_extended::storage::{EmbeddingMatch, MemoryRow, StorageBackend, TodoRow};
use ragent_tools_extended::todo::{TodoReadTool, TodoWriteTool};
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
        let mut lock = self.todos.lock().unwrap();
        Ok(lock.remove(id).is_some())
    }

    fn clear_todos(&self, _session_id: &str) -> anyhow::Result<usize> {
        let mut lock = self.todos.lock().unwrap();
        let count = lock.len();
        lock.clear();
        Ok(count)
    }

    fn get_memory(&self, _id: i64) -> anyhow::Result<Option<MemoryRow>> {
        Ok(None)
    }

    fn get_memory_tags(&self, _id: i64) -> anyhow::Result<Vec<String>> {
        Ok(Vec::new())
    }

    fn search_memories(
        &self,
        _query: &str,
        _category: Option<&str>,
        _source: Option<&str>,
        _limit: usize,
        _min_confidence: f64,
    ) -> anyhow::Result<Vec<MemoryRow>> {
        Ok(Vec::new())
    }

    fn list_memories(&self, _project: &str, _limit: usize) -> anyhow::Result<Vec<MemoryRow>> {
        Ok(Vec::new())
    }

    fn store_memory_embedding(&self, _id: i64, _embedding_blob: &[u8]) -> anyhow::Result<bool> {
        Ok(false)
    }

    fn list_memory_embeddings(&self) -> anyhow::Result<Vec<(i64, Vec<u8>)>> {
        Ok(Vec::new())
    }

    fn search_memories_by_embedding(
        &self,
        _query_embedding: &[f32],
        _dimensions: usize,
        _limit: usize,
        _min_similarity: f32,
    ) -> anyhow::Result<Vec<EmbeddingMatch>> {
        Ok(Vec::new())
    }
}

fn test_ctx(storage: Arc<dyn StorageBackend>) -> ToolContext {
    ToolContext {
        storage: Some(storage),
        event_bus: Arc::new(EventBus::new(128)),
        session_id: "test-session".to_string(),
        working_dir: std::env::current_dir().unwrap(),
        code_index: None,
    }
}

#[tokio::test]
async fn test_todo_full_lifecycle() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage.clone());
    let read_tool = TodoReadTool;
    let write_tool = TodoWriteTool;

    // ── 1. READ (empty) ──────────────────────────────────��──────────
    let out = read_tool
        .execute(json!({}), &ctx)
        .await
        .expect("read should succeed");
    assert!(
        out.content.contains("No TODO items found"),
        "empty read: {:?}",
        out.content
    );

    // ── 2. ADD ──────────────────────────────────────────────────────
    let out = write_tool
        .execute(
            json!({"action": "add", "id": "td-01", "title": "First task", "status": "pending"}),
            &ctx,
        )
        .await
        .expect("add should succeed");
    assert!(out.content.contains("Added todo 'td-01'"));
    assert!(out.content.contains("pending"));
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["action"], "add");
    assert_eq!(meta["count"], 1);

    // ── 3. READ (one item, all statuses) ────────────────────────────
    let out = read_tool
        .execute(json!({"status": "all"}), &ctx)
        .await
        .expect("read should succeed");
    assert!(out.content.contains("## TODOs (1 items)"));
    assert!(out.content.contains("First task"));
    assert!(out.content.contains("⏳"));

    // ── 4. UPDATE: pending -> in_progress ──────────────────────────
    let out = write_tool
        .execute(
            json!({"action": "update", "id": "td-01", "status": "in_progress"}),
            &ctx,
        )
        .await
        .expect("update should succeed");
    assert!(
        out.content.contains("(pending -> in_progress)"),
        "status change: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["action"], "update");
    assert_eq!(meta["old_status"], "pending");
    assert_eq!(meta["new_status"], "in_progress");

    // ── 5. ADD another (default status = pending) ─────────────────
    let out = write_tool
        .execute(
            json!({"action": "add", "id": "td-02", "title": "Second task"}),
            &ctx,
        )
        .await
        .expect("add should succeed");
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["count"], 2);

    // ── 6. READ with status filter: blocked (should be empty) ───────
    let out = read_tool
        .execute(json!({"status": "blocked"}), &ctx)
        .await
        .expect("read should succeed");
    assert!(
        out.content
            .contains("No TODO items found with status 'blocked'")
    );

    // ── 7. READ with status filter: in_progress ────────────────────
    let out = read_tool
        .execute(json!({"status": "in_progress"}), &ctx)
        .await
        .expect("read should succeed");
    assert!(out.content.contains("First task"));
    assert!(!out.content.contains("Second task"));

    // ── 8. UPDATE title only (no status change metadata) ─────────────
    let out = write_tool
        .execute(
            json!({"action": "update", "id": "td-01", "title": "First task (renamed)"}),
            &ctx,
        )
        .await
        .expect("update title should succeed");
    assert!(!out.content.contains("->")); // no status arrow
    let meta = out.metadata.expect("metadata");
    assert!(meta.get("old_status").is_none());
    assert!(meta.get("new_status").is_none());

    // ── 9. COMPLETE (in_progress -> done) ───────────────────────────
    let out = write_tool
        .execute(json!({"action": "complete", "id": "td-01"}), &ctx)
        .await
        .expect("complete should succeed");
    assert!(
        out.content.contains("(in_progress -> done)"),
        "complete: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["action"], "complete");
    assert_eq!(meta["old_status"], "in_progress");
    assert_eq!(meta["new_status"], "done");

    // ── 10. COMPLETE via "completed" alias ─────────────────────────
    let out = write_tool
        .execute(json!({"action": "completed", "id": "td-02"}), &ctx)
        .await
        .expect("completed alias should succeed");
    assert!(out.content.contains("(pending -> done)"));

    // ── 11. READ (2 items, 2 done) ────────────────────────────────
    let out = read_tool
        .execute(json!({}), &ctx)
        .await
        .expect("read should succeed");
    assert!(out.content.contains("## TODOs (2 items)"));
    assert!(out.content.contains("✅"));

    // ── 12. REMOVE one ─────────────────────────────────────────────
    let out = write_tool
        .execute(json!({"action": "remove", "id": "td-01"}), &ctx)
        .await
        .expect("remove should succeed");
    assert!(out.content.contains("Removed todo 'td-01'"));
    assert!(out.content.contains("First task (renamed)"));
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["count"], 1);

    // ── 13. CLEAR the remaining ─────────────────────────────────────
    let out = write_tool
        .execute(json!({"action": "clear"}), &ctx)
        .await
        .expect("clear should succeed");
    assert!(out.content.contains("Cleared 1 todo item"));
    assert!(out.content.contains("No TODO items found"));

    // ── 14. READ (empty again) ─────────────────────────────────────
    let out = read_tool
        .execute(json!({}), &ctx)
        .await
        .expect("read should succeed");
    assert!(out.content.contains("No TODO items found"));
}

#[tokio::test]
async fn test_todo_add_blocked_status() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage);
    let write_tool = TodoWriteTool;

    let out = write_tool
        .execute(
            json!({"action": "add", "id": "td-blk", "title": "Blocked task", "status": "blocked"}),
            &ctx,
        )
        .await
        .expect("add blocked should succeed");
    assert!(out.content.contains("status 'blocked'"));
    assert!(out.content.contains("🚫"));
}

#[tokio::test]
async fn test_todo_update_invalid_status_errors() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage);
    let write_tool = TodoWriteTool;

    let err = write_tool
        .execute(json!({"action": "add", "id": "td-x", "title": "X"}), &ctx)
        .await
        .expect("seed add ok");
    // force ignore
    drop(err);

    let res = write_tool
        .execute(
            json!({"action": "update", "id": "td-x", "status": "bogus"}),
            &ctx,
        )
        .await;
    assert!(res.is_err(), "expected error for invalid status");
    let err = res.unwrap_err().to_string();
    assert!(err.contains("Invalid status 'bogus'"));
}

#[tokio::test]
async fn test_todo_read_invalid_status_errors() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage);
    let read_tool = TodoReadTool;

    let res = read_tool.execute(json!({"status": "bogus"}), &ctx).await;
    assert!(res.is_err(), "expected error for invalid status filter");
    let err = res.unwrap_err().to_string();
    assert!(err.contains("Invalid status filter 'bogus'"));
}

#[tokio::test]
async fn test_todo_complete_nonexistent_errors() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage);
    let write_tool = TodoWriteTool;

    let res = write_tool
        .execute(json!({"action": "complete", "id": "no-such-id"}), &ctx)
        .await;
    assert!(res.is_err(), "expected error for nonexistent todo");
    let err = res.unwrap_err().to_string();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn test_todo_remove_nonexistent_errors() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage);
    let write_tool = TodoWriteTool;

    let res = write_tool
        .execute(json!({"action": "remove", "id": "no-such-id"}), &ctx)
        .await;
    assert!(res.is_err(), "expected error for nonexistent todo");
    let err = res.unwrap_err().to_string();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn test_todo_update_no_fields_errors() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage);
    let write_tool = TodoWriteTool;

    // seed
    let _ = write_tool
        .execute(json!({"action": "add", "id": "td-y", "title": "Y"}), &ctx)
        .await;

    let res = write_tool
        .execute(json!({"action": "update", "id": "td-y"}), &ctx)
        .await;
    assert!(
        res.is_err(),
        "expected error when no fields provided for update"
    );
    let err = res.unwrap_err().to_string();
    assert!(err.contains("At least one of title, status, or description"));
}
