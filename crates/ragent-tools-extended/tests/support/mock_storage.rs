//! Shared `MockStorage` test helper for todo-related integration tests.
//!
//! Provides an in-memory [`MockStorage`] that implements
//! [`ragent_tools_extended::storage::StorageBackend`] for the todo CRUD
//! methods.  Previously copy-pasted as identical `MockStorage` (in 3 test
//! files) and `DemoStorage` (in 1 example) — see `DUPPLAN.md` Milestone H.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use ragent_tools_extended::storage::{EmbeddingMatch, MemoryRow, StorageBackend, TaskRow};

/// In-memory mock storage for testing the todo tool.
///
/// All fields are wrapped in `Arc<Mutex<…>>` so the struct can be cheaply
/// cloned (via `Arc`) while keeping interior mutability across async calls.
#[derive(Clone)]
pub(crate) struct MockStorage {
    todos: Arc<Mutex<HashMap<String, TaskRow>>>,
}

impl MockStorage {
    /// Create a new empty mock storage.
    #[must_use]
    pub(crate) fn new() -> Self {
        Self {
            todos: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Seed a todo row directly into the in-memory store.
    ///
    /// Convenience method used by tests that need pre-populated state without
    /// going through the `create_task_simple` trait method.
    #[allow(dead_code)] // used by integration tests that opt in via this support helper
    pub(crate) fn seed(&self, id: &str, session_id: &str, title: &str, status: &str) {
        self.seed_task(id, session_id, title, status, "", None, None, &[]);
    }

    /// Seed a full task row with all Task-model fields (todo2tasks T-009).
    #[allow(dead_code)] // used by task tool integration tests
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn seed_task(
        &self,
        id: &str,
        session_id: &str,
        title: &str,
        status: &str,
        description: &str,
        active_form: Option<&str>,
        owner: Option<&str>,
        blocked_by: &[&str],
    ) {
        let now = chrono::Utc::now().to_rfc3339();
        let row = TaskRow {
            id: id.to_string(),
            session_id: session_id.to_string(),
            title: title.to_string(),
            status: status.to_string(),
            description: description.to_string(),
            created_at: now.clone(),
            updated_at: now,
            active_form: active_form.map(String::from),
            owner: owner.map(String::from),
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            blocked_by: blocked_by.iter().map(|s| s.to_string()).collect(),
        };
        self.todos.lock().unwrap().insert(id.to_string(), row);
    }

    /// Set the `created_at` timestamp on a seeded task (for ordering tests).
    #[allow(dead_code)] // used by task_list integration tests
    pub(crate) fn set_created_at(&self, id: &str, created_at: &str) {
        if let Some(row) = self.todos.lock().unwrap().get_mut(id) {
            row.created_at = created_at.to_string();
        }
    }

    /// Update the status of a seeded task (for auto-unblock tests).
    #[allow(dead_code)] // used by task_unblock integration tests
    pub(crate) fn set_status(&self, id: &str, status: &str) {
        if let Some(row) = self.todos.lock().unwrap().get_mut(id) {
            row.status = status.to_string();
        }
    }
}

impl Default for MockStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl StorageBackend for MockStorage {
    fn list_tasks(
        &self,
        session_id: &str,
        status_filter: Option<&str>,
    ) -> anyhow::Result<Vec<TaskRow>> {
        let lock = self.todos.lock().unwrap();
        let mut rows: Vec<TaskRow> = lock
            .values()
            .filter(|r| r.session_id == session_id)
            .cloned()
            .collect();
        if let Some(filter) = status_filter {
            rows.retain(|r| r.status == filter);
        }
        Ok(rows)
    }

    fn create_task_simple(
        &self,
        id: &str,
        session_id: &str,
        title: &str,
        status: &str,
        description: &str,
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let row = TaskRow {
            id: id.to_string(),
            session_id: session_id.to_string(),
            title: title.to_string(),
            status: status.to_string(),
            description: description.to_string(),
            created_at: now.clone(),
            updated_at: now,
            // T-001: new Task fields default to empty for legacy create path.
            active_form: None,
            owner: None,
            metadata: serde_json::Value::Object(serde_json::Map::new()),
            blocked_by: Vec::new(),
        };
        self.todos.lock().unwrap().insert(id.to_string(), row);
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    fn create_task(
        &self,
        id: &str,
        session_id: &str,
        subject: &str,
        description: &str,
        status: &str,
        active_form: Option<&str>,
        owner: Option<&str>,
        metadata: &serde_json::Value,
        blocked_by: &[String],
    ) -> anyhow::Result<()> {
        let now = chrono::Utc::now().to_rfc3339();
        let row = TaskRow {
            id: id.to_string(),
            session_id: session_id.to_string(),
            title: subject.to_string(),
            status: status.to_string(),
            description: description.to_string(),
            created_at: now.clone(),
            updated_at: now,
            active_form: active_form.map(String::from),
            owner: owner.map(String::from),
            metadata: metadata.clone(),
            blocked_by: blocked_by.to_vec(),
        };
        self.todos.lock().unwrap().insert(id.to_string(), row);
        Ok(())
    }

    fn update_task_simple(
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

    #[allow(clippy::type_complexity, clippy::too_many_arguments)]
    fn update_task(
        &self,
        id: &str,
        _session_id: &str,
        subject: Option<&str>,
        status: Option<&str>,
        description: Option<&str>,
        active_form: Option<Option<&str>>,
        owner: Option<Option<&str>>,
        metadata: Option<&serde_json::Value>,
        blocked_by: Option<&[String]>,
    ) -> anyhow::Result<bool> {
        let mut lock = self.todos.lock().unwrap();
        if let Some(row) = lock.get_mut(id) {
            if let Some(s) = subject {
                row.title = s.to_string();
            }
            if let Some(st) = status {
                row.status = st.to_string();
            }
            if let Some(d) = description {
                row.description = d.to_string();
            }
            if let Some(af) = active_form {
                row.active_form = af.map(String::from);
            }
            if let Some(o) = owner {
                row.owner = o.map(String::from);
            }
            if let Some(m) = metadata {
                row.metadata = m.clone();
            }
            if let Some(bb) = blocked_by {
                row.blocked_by = bb.to_vec();
            }
            row.updated_at = chrono::Utc::now().to_rfc3339();
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn delete_task(&self, id: &str, _session_id: &str) -> anyhow::Result<bool> {
        let mut lock = self.todos.lock().unwrap();
        Ok(lock.remove(id).is_some())
    }

    fn clear_tasks(&self, _session_id: &str) -> anyhow::Result<usize> {
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
