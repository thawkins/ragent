//! Standalone demonstration of TodoReadTool / TodoWriteTool.
//! Shows the exact markdown output the MessageWidget would receive.
//!
//! Run with:
//!   cargo run --example todo_cycle

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ragent_tools_extended::storage::{EmbeddingMatch, MemoryRow, StorageBackend, TodoRow};
use ragent_tools_extended::todo::{TodoReadTool, TodoWriteTool};
use ragent_tools_extended::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

struct DemoStorage {
    todos: Arc<Mutex<HashMap<String, TodoRow>>>,
}

impl DemoStorage {
    fn new() -> Self {
        Self {
            todos: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

impl StorageBackend for DemoStorage {
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
            if let Some(t) = title { row.title = t.to_string(); }
            if let Some(s) = status { row.status = s.to_string(); }
            if let Some(d) = description { row.description = d.to_string(); }
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

    fn get_memory(&self, _id: i64) -> anyhow::Result<Option<MemoryRow>> { Ok(None) }
    fn get_memory_tags(&self, _id: i64) -> anyhow::Result<Vec<String>> { Ok(Vec::new()) }
    fn search_memories(
        &self, _q: &str, _c: Option<&str>, _s: Option<&str>, _l: usize, _m: f64
    ) -> anyhow::Result<Vec<MemoryRow>> { Ok(Vec::new()) }
    fn list_memories(&self, _p: &str, _l: usize) -> anyhow::Result<Vec<MemoryRow>> { Ok(Vec::new()) }
    fn store_memory_embedding(&self, _id: i64, _blob: &[u8]) -> anyhow::Result<bool> { Ok(false) }
    fn list_memory_embeddings(&self) -> anyhow::Result<Vec<(i64, Vec<u8>)>> { Ok(Vec::new()) }
    fn search_memories_by_embedding(
        &self, _qe: &[f32], _d: usize, _l: usize, _m: f32
    ) -> anyhow::Result<Vec<EmbeddingMatch>> { Ok(Vec::new()) }
}

fn make_ctx(storage: Arc<dyn StorageBackend>) -> ToolContext {
    ToolContext {
        storage: Some(storage),
        event_bus: Arc::new(EventBus::new(128)),
        session_id: "demo-session".to_string(),
        working_dir: std::env::current_dir().unwrap(),
        code_index: None,
    }
}

fn print_tool_call(name: &str, params: &serde_json::Value) {
    println!("┌──────────────────────────────────────────────────────────────┐");
    println!("│  TOOL CALL: {:48} │", name);
    println!("│  PARAMS:  {:50} │", params.to_string());
    println!("└──────────────────────────────────────────────────────────────┘");
}

fn print_output(content: &str) {
    println!("  📝  MessageWidget would render:");
    println!("  ╭���───────────────────────────────────────────────────────────╮");
    for line in content.lines() {
        println!("  │ {:58} │", line.chars().take(58).collect::<String>());
    }
    println!("  ╰────────────────────────────────────────────────────────────╯\n");
}

#[tokio::main]
async fn main() {
    let storage: Arc<dyn StorageBackend> = Arc::new(DemoStorage::new());
    let read_tool = TodoReadTool;
    let write_tool = TodoWriteTool;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║         TODO TOOL CYCLE — MESSAGEWIDGET PREVIEW              ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // 1. READ empty
    print_tool_call("todo_read", &json!({}));
    let out = read_tool.execute(json!({}), &make_ctx(storage.clone())).await.unwrap();
    print_output(&out.content);

    // 2. ADD (pending)
    print_tool_call("todo_write", &json!({"action":"add","id":"task-001","title":"Implement OAuth2"}));
    let out = write_tool.execute(
        json!({"action":"add","id":"task-001","title":"Implement OAuth2"}),
        &make_ctx(storage.clone())
    ).await.unwrap();
    print_output(&out.content);

    // 3. UPDATE → in_progress
    print_tool_call("todo_write", &json!({"action":"update","id":"task-001","status":"in_progress"}));
    let out = write_tool.execute(
        json!({"action":"update","id":"task-001","status":"in_progress"}),
        &make_ctx(storage.clone())
    ).await.unwrap();
    print_output(&out.content);

    // 4. ADD blocked task
    print_tool_call("todo_write", &json!({"action":"add","id":"task-002","title":"Refactor DB","status":"blocked"}));
    let out = write_tool.execute(
        json!({"action":"add","id":"task-002","title":"Refactor DB","status":"blocked"}),
        &make_ctx(storage.clone())
    ).await.unwrap();
    print_output(&out.content);

    // 5. UPDATE title only
    print_tool_call("todo_write", &json!({"action":"update","id":"task-001","title":"Implement OAuth2 (with refresh tokens)"}));
    let out = write_tool.execute(
        json!({"action":"update","id":"task-001","title":"Implement OAuth2 (with refresh tokens)"}),
        &make_ctx(storage.clone())
    ).await.unwrap();
    print_output(&out.content);

    // 6. READ all
    print_tool_call("todo_read", &json!({"status":"all"}));
    let out = read_tool.execute(json!({"status":"all"}), &make_ctx(storage.clone())).await.unwrap();
    print_output(&out.content);

    // 7. COMPLETE → done
    print_tool_call("todo_write", &json!({"action":"complete","id":"task-001"}));
    let out = write_tool.execute(
        json!({"action":"complete","id":"task-001"}),
        &make_ctx(storage.clone())
    ).await.unwrap();
    print_output(&out.content);

    // 8. READ by status
    print_tool_call("todo_read", &json!({"status":"done"}));
    let out = read_tool.execute(json!({"status":"done"}), &make_ctx(storage.clone())).await.unwrap();
    print_output(&out.content);

    // 9. REMOVE
    print_tool_call("todo_write", &json!({"action":"remove","id":"task-002"}));
    let out = write_tool.execute(
        json!({"action":"remove","id":"task-002"}),
        &make_ctx(storage.clone())
    ).await.unwrap();
    print_output(&out.content);

    // 10. CLEAR
    print_tool_call("todo_write", &json!({"action":"clear"}));
    let out = write_tool.execute(
        json!({"action":"clear"}),
        &make_ctx(storage.clone())
    ).await.unwrap();
    print_output(&out.content);

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║              STATE CYCLE COMPLETE ✅                          ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}
