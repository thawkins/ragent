//! Demonstration: exercise todo_read / todo_write through a full lifecycle.
//! Run with: cargo test -p ragent-tools-extended --test test_todo_demo -- --nocapture

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ragent_tools_extended::storage::{EmbeddingMatch, MemoryRow, StorageBackend, TodoRow};
use ragent_tools_extended::todo::{TodoReadTool, TodoWriteTool};
use ragent_tools_extended::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

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

fn ctx(storage: Arc<dyn StorageBackend>) -> ToolContext {
    ToolContext {
        storage: Some(storage),
        event_bus: Arc::new(EventBus::new(128)),
        session_id: "demo-session".to_string(),
        working_dir: std::env::current_dir().unwrap(),
        code_index: None,
        config: None,
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    }
}

#[tokio::test]
async fn demo_todo_lifecycle() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let read_tool = TodoReadTool;
    let write_tool = TodoWriteTool;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           TODO TOOL DEMONSTRATION — FULL LIFECYCLE           ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // ── Step 1: READ empty list ───────────────────────────────────
    println!("[STEP 1] todo_read (empty list)");
    let out = read_tool
        .execute(json!({}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 2: ADD a new todo ────────────────────────────────────
    println!("[STEP 2] todo_write action=add (default status: pending)");
    let out = write_tool
        .execute(
            json!({"action": "add", "id": "task-001", "title": "Implement OAuth2 login", "description": "Add Google and GitHub providers"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 3: ADD another with explicit status ──────────────────
    println!("[STEP 3] todo_write action=add status=blocked");
    let out = write_tool
        .execute(
            json!({"action": "add", "id": "task-002", "title": "Refactor database layer", "status": "blocked"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 4: READ all ──────────────────────────────────────────
    println!("[STEP 4] todo_read status=all");
    let out = read_tool
        .execute(json!({"status": "all"}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 5: UPDATE status (pending → in_progress) ─────────────
    println!("[STEP 5] todo_write action=update status=in_progress");
    let out = write_tool
        .execute(
            json!({"action": "update", "id": "task-001", "status": "in_progress"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 6: READ filtered by in_progress ──────────────────────
    println!("[STEP 6] todo_read status=in_progress");
    let out = read_tool
        .execute(json!({"status": "in_progress"}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 7: UPDATE title only (no status change) ──────────────
    println!("[STEP 7] todo_write action=update title only");
    let out = write_tool
        .execute(
            json!({"action": "update", "id": "task-001", "title": "Implement OAuth2 login (with refresh tokens)"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 8: COMPLETE (in_progress → done) ─────────────────────
    println!("[STEP 8] todo_write action=complete");
    let out = write_tool
        .execute(
            json!({"action": "complete", "id": "task-001"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 9: READ filtered by done ─────────────────────────────
    println!("[STEP 9] todo_read status=done");
    let out = read_tool
        .execute(json!({"status": "done"}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 10: COMPLETE via alias "completed" ──────────────────
    println!("[STEP 10] todo_write action=completed (alias for complete)");
    let out = write_tool
        .execute(
            json!({"action": "completed", "id": "task-002"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 11: READ all (both done) ─────────────────────────────
    println!("[STEP 11] todo_read status=all (final state)");
    let out = read_tool
        .execute(json!({"status": "all"}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 12: REMOVE one ──────────────────────────────────────
    println!("[STEP 12] todo_write action=remove");
    let out = write_tool
        .execute(
            json!({"action": "remove", "id": "task-001"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 13: CLEAR remaining ──────────────────────────────────
    println!("[STEP 13] todo_write action=clear");
    let out = write_tool
        .execute(json!({"action": "clear"}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 14: READ empty ───────────────────────────────────────
    println!("[STEP 14] todo_read (verify empty)");
    let out = read_tool
        .execute(json!({}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    LIFECYCLE COMPLETE ✅                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}
