//! Shared `MockStorage` test helper for todo-related integration tests.
//!
//! Provides an in-memory [`MockStorage`] that implements
//! [`ragent_tools_extended::storage::StorageBackend`] for the todo CRUD
//! methods.  Previously copy-pasted as identical `MockStorage` (in 3 test
//! files) and `DemoStorage` (in 1 example) — see `DUPPLAN.md` Milestone H.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ragent_tools_extended::storage::{EmbeddingMatch, MemoryRow, StorageBackend, TodoRow};

/// In-memory mock storage for testing the todo tool.
///
/// All fields are wrapped in `Arc<Mutex<…>>` so the struct can be cheaply
/// cloned (via `Arc`) while keeping interior mutability across async calls.
#[derive(Clone)]
pub struct MockStorage {
    todos: Arc<Mutex<HashMap<String, TodoRow>>>,
}

impl MockStorage {
    /// Create a new empty mock storage.
    #[must_use]
    pub fn new() -> Self {
        Self {
            todos: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Seed a todo row directly into the in-memory store.
    ///
    /// Convenience method used by tests that need pre-populated state without
    /// going through the `create_todo` trait method.
    #[allow(dead_code)]
    pub fn seed(&self, id: &str, session_id: &str, title: &str, status: &str) {
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

impl Default for MockStorage {
    fn default() -> Self {
        Self::new()
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
