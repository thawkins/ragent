//! Task tools for the todo2tasks migration (T-007, T-008, T-009, T-010, …).
//!
//! This module hosts the new `task_*` tool surface that replaces the
//! legacy session task tools with four single-purpose tools mirroring
//! the article's TaskCreate / TaskUpdate / TaskGet / TaskList design.
//!
//! ## Current status
//!
//! - **T-006** — Auto-unblock evaluation: `is_available` / `is_blocked`
//!   derived at read time and surfaced in `task_get` / `task_list`
//!   output (FR-003, FR-005).
//! - **T-007** — `TaskCreateTool` implemented (FR-009, FR-011, FR-012).
//! - **T-008/T-017** — `TaskUpdateTool` implemented with `status=blocked`
//!   rejection (FR-005) and `blocked_by` existence validation (FR-009).
//! - **T-009** — `TaskGetTool` implemented (FR-011, FR-014).
//! - **T-010** — `TaskListTool` implemented (FR-011, FR-015).
//! - T-011 — completed: all four tools registered in
//!   `create_extended_registry()` and hardwired auto-approve covers
//!   `task_*` (FR-011, FR-017).

use std::collections::HashMap;

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::storage::TaskRow;

// ── DAG derivation (T-006, FR-003, FR-005) ──────────────────────────

/// Derived DAG fields for a single task, computed at read time from the
/// full session task set (todo2tasks T-006, FR-003, FR-005).
///
/// Mirrors `ragent_storage::TaskDerived` but works with the
/// tools-extended `TaskRow` type (which uses `serde_json::Value` for
/// `metadata` rather than `String`).
#[derive(Debug, Clone, Default)]
pub(crate) struct TaskDagInfo {
    /// Task IDs that this task blocks (inverse of `blocked_by`).
    blocks: Vec<String>,
    /// Derived blocked flag (FR-005): status is `"pending"` and at
    /// least one `blocked_by` ID is not `"completed"`.
    is_blocked: bool,
    /// Derived available flag (FR-003): status is `"pending"`, owner
    /// is empty, and all `blocked_by` IDs are `"completed"` (or
    /// `blocked_by` is empty).
    is_available: bool,
}

/// Computes derived DAG fields for all tasks in a session (T-006,
/// FR-003, FR-005).
///
/// Given a slice of [`TaskRow`] values (typically from
/// `StorageBackend::list_tasks`), this function:
///
/// 1. Builds a status lookup map (`id` → `status`) for O(1)
///    `blocked_by` resolution.
/// 2. Computes `is_blocked` per FR-005: status is `"pending"` **and**
///    at least one `blocked_by` ID is not `"completed"`.
/// 3. Computes `is_available` per FR-003: status is `"pending"`,
///    owner is `None`/empty, and all `blocked_by` IDs are
///    `"completed"` (or `blocked_by` is empty).
/// 4. Computes the inverse edge (`blocks`): if task `B` lists `A` in
///    `B.blocked_by`, then `A`'s `blocks` set includes `B`.
///
/// This is the tool-layer equivalent of
/// `ragent_storage::compute_task_dag`, adapted for the
/// tools-extended `TaskRow` type.
pub(crate) fn compute_dag(tasks: &[TaskRow]) -> HashMap<String, TaskDagInfo> {
    // Build id → status lookup for O(1) blocked_by resolution.
    let status_map: HashMap<&str, &str> = tasks
        .iter()
        .map(|t| (t.id.as_str(), t.status.as_str()))
        .collect();

    let mut dag: HashMap<String, TaskDagInfo> = HashMap::with_capacity(tasks.len());

    // First pass: compute is_blocked and is_available for each task.
    for t in tasks {
        let mut info = TaskDagInfo::default();

        if t.status == "pending" {
            // is_blocked: at least one blocker is not completed.
            let any_unfinished = t.blocked_by.iter().any(|dep_id| {
                status_map
                    .get(dep_id.as_str())
                    .is_none_or(|s| *s != "completed")
            });
            info.is_blocked = !t.blocked_by.is_empty() && any_unfinished;

            // is_available: owner is empty and all blockers are completed.
            let owner_empty = t.owner.as_deref().is_none_or(|o| o.is_empty());
            let all_blockers_done = t.blocked_by.iter().all(|dep_id| {
                status_map
                    .get(dep_id.as_str())
                    .is_some_and(|s| *s == "completed")
            });
            info.is_available = owner_empty && all_blockers_done;
        }

        dag.insert(t.id.clone(), info);
    }

    // Second pass: compute inverse edges (blocks).
    // If B lists A in blocked_by, then A blocks B.
    for t in tasks {
        for dep_id in &t.blocked_by {
            if let Some(info) = dag.get_mut(dep_id)
                && !info.blocks.contains(&t.id)
            {
                info.blocks.push(t.id.clone());
            }
        }
    }

    // Sort blocks vectors for deterministic output.
    for info in dag.values_mut() {
        info.blocks.sort();
    }

    dag
}

// ── TaskCreateTool (T-007, FR-009, FR-011, FR-012) ───────────────────

/// Creates a new task in the current session (todo2tasks T-007,
/// FR-009, FR-011, FR-012).
///
/// Required parameters: `subject` (imperative title) and `description`
/// (free-text, carries acceptance criteria).
///
/// Optional parameters:
/// - `active_form` — present-continuous phrase for progress indicators
///   (FR-007).
/// - `owner` — free-form label naming the responsible agent/worker
///   (FR-006).
/// - `metadata` — arbitrary JSON object of key-value pairs (FR-008).
/// - `blocked_by` — array of task IDs that must reach `completed` before
///   this task (FR-001).  Each ID is validated to exist in the current
///   session (FR-009).
///
/// The tool generates a unique ID (`task-<uuid>`), sets `status` to
/// `"pending"`, and returns the created task record.
pub struct TaskCreateTool;

#[async_trait::async_trait]
impl Tool for TaskCreateTool {
    fn name(&self) -> &'static str {
        "task_create"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &'static str {
        "Create a new task in the current session. \
         Required: subject (imperative title), description (acceptance criteria). \
         Optional: active_form (present-continuous phrase), owner (agent label), \
         metadata (JSON object), blocked_by (array of task IDs that must complete first). \
         Returns the created task with status 'pending'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "subject": {
                    "type": "string",
                    "description": "Imperative title for the task (e.g. 'Implement JWT auth')."
                },
                "description": {
                    "type": "string",
                    "description": "Free-text description carrying acceptance criteria."
                },
                "active_form": {
                    "type": "string",
                    "description": "Present-continuous phrase shown in progress indicators (e.g. 'Implementing JWT auth')."
                },
                "owner": {
                    "type": "string",
                    "description": "Free-form label naming the agent/worker responsible for this task."
                },
                "metadata": {
                    "type": "object",
                    "description": "Arbitrary JSON object of key-value pairs (feature, phase, priority, ...).",
                    "additionalProperties": true
                },
                "blocked_by": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Task IDs that must reach 'completed' before this task can start."
                }
            },
            "required": ["subject", "description"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "task"
    }

    /// # Errors
    ///
    /// Returns an error if storage is unavailable, required parameters
    /// are missing, `metadata` is not a JSON object, `blocked_by`
    /// references a non-existent task (FR-009), or the storage backend
    /// fails.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let storage = ctx.storage.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Storage is not available. Cannot create a task without a storage backend."
            )
        })?;

        let subject = input["subject"].as_str().ok_or_else(|| {
            anyhow::anyhow!(
                "Missing required 'subject' parameter. Provide an imperative title for the task."
            )
        })?;

        let description = input["description"].as_str().ok_or_else(|| {
            anyhow::anyhow!(
                "Missing required 'description' parameter. Provide a description with acceptance criteria."
            )
        })?;

        let active_form = input["active_form"].as_str();
        let owner = input["owner"].as_str();

        // metadata: must be a JSON object if present (FR-008).
        let metadata = if let Some(meta_val) = input.get("metadata") {
            if !meta_val.is_object() {
                anyhow::bail!(
                    "Invalid 'metadata' parameter: expected a JSON object, got {}.",
                    type_str(meta_val)
                );
            }
            meta_val.clone()
        } else {
            json!({})
        };

        // blocked_by: array of task IDs (FR-001, FR-009).
        let blocked_by: Vec<String> = if let Some(arr) = input.get("blocked_by") {
            if !arr.is_array() {
                anyhow::bail!(
                    "Invalid 'blocked_by' parameter: expected an array of task ID strings."
                );
            }
            arr.as_array()
                .expect("checked is_array above")
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else {
            Vec::new()
        };

        // Validate blocked_by references exist in this session (FR-009).
        if !blocked_by.is_empty() {
            let existing = storage.list_tasks(&ctx.session_id, None).map_err(|e| {
                anyhow::anyhow!("Failed to read tasks for blocked_by validation: {e}")
            })?;
            let existing_ids: std::collections::HashSet<&str> =
                existing.iter().map(|t| t.id.as_str()).collect();
            let missing: Vec<&str> = blocked_by
                .iter()
                .map(String::as_str)
                .filter(|id| !existing_ids.contains(*id))
                .collect();
            if !missing.is_empty() {
                anyhow::bail!(
                    "blocked_by references non-existent task(s): {}. \
                     All blocked_by IDs must exist in the current session.",
                    missing.join(", ")
                );
            }
        }

        // Generate a unique task ID (FR-011).
        let id = generate_task_id();

        // Persist the task with status = "pending" (FR-011).
        storage
            .create_task(
                &id,
                &ctx.session_id,
                subject,
                description,
                "pending",
                active_form,
                owner,
                &metadata,
                &blocked_by,
            )
            .map_err(|e| anyhow::anyhow!("Failed to create task: {e}"))?;

        // Read back the created task to return the full record.
        let all_tasks = storage
            .list_tasks(&ctx.session_id, None)
            .map_err(|e| anyhow::anyhow!("Failed to read back created task: {e}"))?;

        let task = all_tasks
            .iter()
            .find(|t| t.id == id)
            .ok_or_else(|| anyhow::anyhow!("Created task '{id}' was not found in storage"))?;

        // Compute derived DAG fields for the response (T-006).
        let dag = compute_dag(&all_tasks);
        let info = dag.get(&task.id).cloned().unwrap_or_default();

        let record = format_task_record(task, &info);
        let metadata_response = json!({
            "id": task.id,
            "subject": task.title,
            "description": task.description,
            "active_form": task.active_form,
            "status": task.status,
            "owner": task.owner,
            "metadata": task.metadata,
            "blocked_by": task.blocked_by,
            "blocks": info.blocks,
            "is_blocked": info.is_blocked,
            "is_available": info.is_available,
            "created_at": task.created_at,
            "updated_at": task.updated_at,
        });

        Ok(ToolOutput {
            content: record,
            metadata: Some(metadata_response),
        })
    }
}

/// Generates a unique ID for a Task item.
pub(crate) fn generate_task_id() -> String {
    format!("task-{}", uuid::Uuid::new_v4().simple())
}

/// Returns a human-readable type name for a JSON value (for error messages).
fn type_str(v: &Value) -> &'static str {
    match v {
        Value::Null => "null",
        Value::Bool(_) => "boolean",
        Value::Number(_) => "number",
        Value::String(_) => "string",
        Value::Array(_) => "array",
        Value::Object(_) => "object",
    }
}

// ── TaskUpdateTool (T-008, T-017, FR-004, FR-005, FR-009) ────────────

/// Valid status values for `task_update` (FR-005: `blocked` is NOT
/// a storable status — it is derived at read time).
const UPDATE_STATUSES: &[&str] = &["pending", "in_progress", "completed"];

/// Detects whether adding proposed `blocked_by` edges would create a
/// dependency cycle (FR-004).
///
/// Given the current set of tasks, the task being updated (`task_id`),
/// its proposed new `blocked_by` list, and a map of other tasks whose
/// `blocked_by` lists will gain `task_id` (from `add_blocks`), this
/// function builds a temporary adjacency map and runs DFS to check if
/// any cycle is reachable starting from `task_id`.
///
/// A cycle exists if following `blocked_by` edges transitively from
/// `task_id` leads back to `task_id`.  Self-references (a task listing
/// its own ID in `blocked_by`) are also cycles.
///
/// Returns `Some(cycle_path)` if a cycle is detected (a vector of task
/// IDs forming the cycle, starting and ending at `task_id`), or `None`
/// if the graph is acyclic.
fn detect_cycle(
    tasks: &[TaskRow],
    task_id: &str,
    proposed_blocked_by: &[String],
    // task_id → new IDs to merge into that task's blocked_by (from add_blocks)
    add_blocks_targets: &HashMap<String, Vec<String>>,
) -> Option<Vec<String>> {
    // Build adjacency map: task_id → set of IDs it depends on (blocked_by).
    let mut adj: HashMap<&str, Vec<String>> = HashMap::new();
    for t in tasks {
        adj.insert(t.id.as_str(), t.blocked_by.clone());
    }
    // Apply proposed changes for the updated task.
    if let Some(entry) = adj.get_mut(task_id) {
        for id in proposed_blocked_by {
            if !entry.contains(id) {
                entry.push(id.clone());
            }
        }
    }
    // Apply proposed changes from add_blocks: each target gains task_id.
    for target_id in add_blocks_targets.keys() {
        if let Some(entry) = adj.get_mut(target_id.as_str())
            && !entry.contains(&task_id.to_string())
        {
            entry.push(task_id.to_string());
        }
    }

    // DFS from task_id following blocked_by edges. If we reach task_id
    // again, we've found a cycle.
    fn dfs(
        adj: &HashMap<&str, Vec<String>>,
        current: &str,
        task_id: &str,
        visiting: &mut std::collections::HashSet<String>,
        path: &mut Vec<String>,
    ) -> Option<Vec<String>> {
        for dep in adj.get(current).into_iter().flatten() {
            if dep == task_id {
                // Found cycle back to start.
                let mut cycle = path.clone();
                cycle.push(dep.clone());
                return Some(cycle);
            }
            if visiting.contains(dep) {
                // Already in current DFS path — inner cycle.
                let mut cycle = path.clone();
                cycle.push(dep.clone());
                return Some(cycle);
            }
            visiting.insert(dep.clone());
            path.push(dep.clone());
            if let Some(c) = dfs(adj, dep, task_id, visiting, path) {
                return Some(c);
            }
            path.pop();
            visiting.remove(dep);
        }
        None
    }

    let mut visiting = std::collections::HashSet::new();
    let mut path = vec![task_id.to_string()];
    dfs(&adj, task_id, task_id, &mut visiting, &mut path)
}

/// Updates an existing task in the current session (todo2tasks T-008,
/// T-017, FR-004, FR-005, FR-009).
///
/// Required parameter: `task_id`.
///
/// Optional parameters:
/// - `status` — must be one of `pending`, `in_progress`, `completed`.
///   The value `blocked` is **rejected** with a clear error (FR-005):
///   blocked-ness is a derived state, not a stored status.
/// - `subject` — new imperative title.
/// - `description` — new description text.
/// - `active_form` — new present-continuous phrase (pass `""` to clear).
/// - `owner` — new owner label (pass `""` to clear).
/// - `metadata` — new JSON object (full replacement).
/// - `add_blocked_by` — array of task IDs to merge into the existing
///   `blocked_by` list.  Each ID is validated to exist in the current
///   session (FR-009).  Self-references are rejected.
/// - `add_blocks` — array of task IDs that should be blocked by this
///   task.  For each ID `B` in the list, `task_id` is added to `B`'s
///   `blocked_by` list.  Each ID is validated to exist in the current
///   session (FR-009).  Self-references are rejected.
///
/// Both `add_blocked_by` and `add_blocks` are checked for dependency
/// cycles before any edges are persisted (FR-004).  If a cycle is
/// detected the update is rejected with an error describing the cycle.
///
/// On a `status` transition to `completed`, the auto-unblock evaluation
/// is implicit: derived `is_available` / `is_blocked` fields are
/// recomputed at read time by `task_get` / `task_list` (FR-003).  The
/// response metadata also includes an `unblocked` array listing task IDs
/// that became available as a result of this completion.
pub struct TaskUpdateTool;

#[async_trait::async_trait]
impl Tool for TaskUpdateTool {
    fn name(&self) -> &'static str {
        "task_update"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &'static str {
        "Update an existing task in the current session. \
         Required: task_id. Optional: status (pending, in_progress, completed — \
         NOT 'blocked'), subject, description, active_form, owner, metadata, \
         add_blocked_by (array of task IDs to add as dependencies), \
         add_blocks (array of task IDs that should be blocked by this task). \
         'blocked' status is rejected — blocked-ness is derived from blocked_by. \
         Cycle-creating edges are rejected (FR-004). \
         Completing a task auto-evaluates dependent tasks (FR-003). \
         Returns the updated task record."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The ID of the task to update."
                },
                "status": {
                    "type": "string",
                    "description": "New status: pending, in_progress, or completed. 'blocked' is not allowed — it is a derived state.",
                    "enum": ["pending", "in_progress", "completed"]
                },
                "subject": {
                    "type": "string",
                    "description": "New imperative title for the task."
                },
                "description": {
                    "type": "string",
                    "description": "New description text."
                },
                "active_form": {
                    "type": "string",
                    "description": "New present-continuous phrase. Pass an empty string to clear."
                },
                "owner": {
                    "type": "string",
                    "description": "New owner label. Pass an empty string to clear."
                },
                "metadata": {
                    "type": "object",
                    "description": "New metadata JSON object (full replacement).",
                    "additionalProperties": true
                },
                "add_blocked_by": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Task IDs to add to this task's blocked_by list. Each ID must exist in the current session."
                },
                "add_blocks": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Task IDs that should be blocked by this task. Each ID must exist in the current session. Adding B here adds this task's ID to B's blocked_by list."
                }
            },
            "required": ["task_id"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "task"
    }

    /// # Errors
    ///
    /// Returns an error if storage is unavailable, `task_id` is missing,
    /// `status` is `"blocked"` or any other invalid value (FR-005),
    /// `add_blocked_by` or `add_blocks` references a non-existent or
    /// cross-session task (FR-009), `add_blocked_by` or `add_blocks`
    /// contains a self-reference, adding edges would create a dependency
    /// cycle (FR-004), `metadata` is not a JSON object, or the storage
    /// backend fails.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let storage = ctx.storage.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Storage is not available. Cannot update a task without a storage backend."
            )
        })?;

        let task_id = input["task_id"].as_str().ok_or_else(|| {
            anyhow::anyhow!(
                "Missing required 'task_id' parameter. Provide the ID of the task to update."
            )
        })?;

        // ── T-017: Reject status=blocked with a clear error (FR-005) ──
        let status = input.get("status").and_then(|v| v.as_str());
        if let Some(s) = status {
            if s == "blocked" {
                anyhow::bail!(
                    "Invalid status 'blocked': 'blocked' is not a valid status. \
                     Blocked-ness is derived from the blocked_by list at read time, \
                     not stored as a status. Valid statuses are: {}.",
                    UPDATE_STATUSES.join(", ")
                );
            }
            if !UPDATE_STATUSES.contains(&s) {
                anyhow::bail!(
                    "Invalid status '{}'. Valid statuses are: {}.",
                    s,
                    UPDATE_STATUSES.join(", ")
                );
            }
        }

        // ── Parse optional fields ──────────────────────────────────────
        let subject = input.get("subject").and_then(|v| v.as_str());

        let description = input.get("description").and_then(|v| v.as_str());

        // active_form: string present → Some(Some(v)); empty string →
        // Some(None) (clear); absent → None (unchanged).
        let active_form = if let Some(val) = input.get("active_form") {
            val.as_str()
                .map(|s| if s.is_empty() { None } else { Some(s) })
        } else {
            None
        };

        // owner: same semantics as active_form.
        let owner = if let Some(val) = input.get("owner") {
            val.as_str()
                .map(|s| if s.is_empty() { None } else { Some(s) })
        } else {
            None
        };

        // metadata: must be a JSON object if present (FR-008).
        let metadata = if let Some(meta_val) = input.get("metadata") {
            if !meta_val.is_object() {
                anyhow::bail!(
                    "Invalid 'metadata' parameter: expected a JSON object, got {}.",
                    type_str(meta_val)
                );
            }
            Some(meta_val)
        } else {
            None
        };

        // ── add_blocked_by: merge into existing blocked_by (FR-009) ────
        let add_blocked_by: Vec<String> = if let Some(arr) = input.get("add_blocked_by") {
            if !arr.is_array() {
                anyhow::bail!(
                    "Invalid 'add_blocked_by' parameter: expected an array of task ID strings."
                );
            }
            arr.as_array()
                .expect("checked is_array above")
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else {
            Vec::new()
        };

        // ── add_blocks: inverse edges — target tasks gain task_id ────
        let add_blocks: Vec<String> = if let Some(arr) = input.get("add_blocks") {
            if !arr.is_array() {
                anyhow::bail!(
                    "Invalid 'add_blocks' parameter: expected an array of task ID strings."
                );
            }
            arr.as_array()
                .expect("checked is_array above")
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else {
            Vec::new()
        };

        // Fetch the existing task so we can merge blocked_by.
        let all_tasks = storage
            .list_tasks(&ctx.session_id, None)
            .map_err(|e| anyhow::anyhow!("Failed to read tasks: {e}"))?;

        let existing_task = all_tasks
            .iter()
            .find(|t| t.id == task_id)
            .ok_or_else(|| anyhow::anyhow!("Task '{task_id}' not found in this session"))?;

        // Validate add_blocked_by references exist in this session (FR-009).
        if !add_blocked_by.is_empty() {
            let existing_ids: std::collections::HashSet<&str> =
                all_tasks.iter().map(|t| t.id.as_str()).collect();
            let missing: Vec<&str> = add_blocked_by
                .iter()
                .map(String::as_str)
                .filter(|id| !existing_ids.contains(*id))
                .collect();
            if !missing.is_empty() {
                anyhow::bail!(
                    "add_blocked_by references non-existent task(s): {}. \
                     All blocked_by IDs must exist in the current session.",
                    missing.join(", ")
                );
            }
        }

        // Reject self-references in add_blocked_by (FR-004 trivial cycle).
        if add_blocked_by.iter().any(|id| id == task_id) {
            anyhow::bail!(
                "add_blocked_by contains a self-reference: task '{task_id}' cannot \
                 depend on itself. This would create a trivial dependency cycle."
            );
        }

        // Validate add_blocks references exist in this session (FR-009).
        if !add_blocks.is_empty() {
            let existing_ids: std::collections::HashSet<&str> =
                all_tasks.iter().map(|t| t.id.as_str()).collect();
            let missing: Vec<&str> = add_blocks
                .iter()
                .map(String::as_str)
                .filter(|id| !existing_ids.contains(*id))
                .collect();
            if !missing.is_empty() {
                anyhow::bail!(
                    "add_blocks references non-existent task(s): {}. \
                     All blocks IDs must exist in the current session.",
                    missing.join(", ")
                );
            }
        }

        // Reject self-references in add_blocks (FR-004 trivial cycle).
        if add_blocks.iter().any(|id| id == task_id) {
            anyhow::bail!(
                "add_blocks contains a self-reference: task '{task_id}' cannot \
                 block itself. This would create a trivial dependency cycle."
            );
        }

        // Merge: existing blocked_by ∪ add_blocked_by, deduplicated.
        let merged_blocked_by: Option<Vec<String>> = if add_blocked_by.is_empty() {
            None
        } else {
            let mut merged = existing_task.blocked_by.clone();
            for id in &add_blocked_by {
                if !merged.contains(id) {
                    merged.push(id.clone());
                }
            }
            merged.sort();
            Some(merged)
        };

        // Build add_blocks target map for cycle detection: each target
        // task's blocked_by will gain task_id.
        let add_blocks_targets: HashMap<String, Vec<String>> = if add_blocks.is_empty() {
            HashMap::new()
        } else {
            let mut map = HashMap::new();
            for target_id in &add_blocks {
                // Find the target task's existing blocked_by.
                let target_bb = all_tasks
                    .iter()
                    .find(|t| &t.id == target_id)
                    .map(|t| t.blocked_by.clone())
                    .unwrap_or_default();
                map.insert(target_id.clone(), target_bb);
            }
            map
        };

        // ── FR-004: Cycle detection ──────────────────────────────────
        // Check if the proposed edges (add_blocked_by on this task +
        // add_blocks on other tasks) would create a dependency cycle.
        if !add_blocked_by.is_empty() || !add_blocks.is_empty() {
            let proposed_bb = merged_blocked_by
                .as_deref()
                .unwrap_or(&existing_task.blocked_by);
            if let Some(cycle) = detect_cycle(&all_tasks, task_id, proposed_bb, &add_blocks_targets)
            {
                anyhow::bail!(
                    "Dependency cycle detected (FR-004): {}. \
                     Adding these edges would create a circular dependency. \
                     The update was rejected and no changes were persisted.",
                    cycle.join(" → ")
                );
            }
        }

        // ── Persist the update ─────────────────────────────────────────
        storage
            .update_task(
                task_id,
                &ctx.session_id,
                subject,
                status,
                description,
                active_form,
                owner,
                metadata,
                merged_blocked_by.as_deref(),
            )
            .map_err(|e| anyhow::anyhow!("Failed to update task: {e}"))?;

        // ── Persist add_blocks: add task_id to each target's blocked_by ─
        for target_id in &add_blocks {
            let target_task = all_tasks
                .iter()
                .find(|t| &t.id == target_id)
                .ok_or_else(|| {
                    anyhow::anyhow!("add_blocks target '{target_id}' disappeared before persisting")
                })?;
            let mut new_bb = target_task.blocked_by.clone();
            if !new_bb.contains(&task_id.to_string()) {
                new_bb.push(task_id.to_string());
                new_bb.sort();
            }
            storage
                .update_task(
                    target_id,
                    &ctx.session_id,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    Some(&new_bb),
                )
                .map_err(|e| {
                    anyhow::anyhow!("Failed to update blocks target '{target_id}': {e}")
                })?;
        }

        // Read back the updated task to return the full record.
        let all_tasks = storage
            .list_tasks(&ctx.session_id, None)
            .map_err(|e| anyhow::anyhow!("Failed to read back updated task: {e}"))?;

        let task = all_tasks
            .iter()
            .find(|t| t.id == task_id)
            .ok_or_else(|| anyhow::anyhow!("Updated task '{task_id}' was not found in storage"))?;

        // Compute derived DAG fields for the response (T-006).
        let dag = compute_dag(&all_tasks);
        let info = dag.get(&task.id).cloned().unwrap_or_default();

        // ── FR-003: Auto-unblock evaluation on completion ────────────
        // When status transitions to `completed`, compute which tasks
        // became available (unblocked) as a result.  This is informational
        // — the derived `is_available` field is always recomputed at read
        // time by `compute_dag`.  Here we identify the dependent tasks
        // that are now fully unblocked.
        let unblocked: Vec<String> = if status == Some("completed") {
            let status_map: HashMap<&str, &str> = all_tasks
                .iter()
                .map(|t| (t.id.as_str(), t.status.as_str()))
                .collect();
            all_tasks
                .iter()
                .filter(|t| {
                    t.id != task_id
                        && t.status == "pending"
                        && t.blocked_by.contains(&task_id.to_string())
                })
                .filter(|t| {
                    t.blocked_by.iter().all(|dep_id| {
                        status_map
                            .get(dep_id.as_str())
                            .is_some_and(|s| *s == "completed")
                    })
                })
                .map(|t| t.id.clone())
                .collect()
        } else {
            Vec::new()
        };

        let record = format_task_record(task, &info);
        let metadata_response = json!({
            "id": task.id,
            "subject": task.title,
            "description": task.description,
            "active_form": task.active_form,
            "status": task.status,
            "owner": task.owner,
            "metadata": task.metadata,
            "blocked_by": task.blocked_by,
            "blocks": info.blocks,
            "is_blocked": info.is_blocked,
            "is_available": info.is_available,
            "unblocked": unblocked,
            "created_at": task.created_at,
            "updated_at": task.updated_at,
        });

        Ok(ToolOutput {
            content: record,
            metadata: Some(metadata_response),
        })
    }
}

// ── TaskGetTool (T-009, FR-014) ─────────────────────────────────────

/// Retrieves the full record of a single task by ID (todo2tasks T-009,
/// FR-014).
///
/// Required parameter: `task_id`.  Returns the full task record:
/// `id`, `subject`, `description`, `active_form`, `status`, `owner`,
/// `metadata`, `blocked_by`, `blocks` (derived), `created_at`,
/// `updated_at`.
///
/// The `blocks` field is derived at read time — it lists every task
/// ID in the session that includes this task in its `blocked_by`.
/// This satisfies FR-014 and mirrors the `compute_task_dag` logic
/// from T-004.
pub struct TaskGetTool;

#[async_trait::async_trait]
impl Tool for TaskGetTool {
    fn name(&self) -> &'static str {
        "task_get"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &'static str {
        "Retrieve the full record of a single task by its task_id. \
         Returns: id, subject, description, active_form, status, owner, \
         metadata, blocked_by, blocks (derived), created_at, updated_at. \
         Required parameter: task_id."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The ID of the task to retrieve."
                }
            },
            "required": ["task_id"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "task"
    }

    /// # Errors
    ///
    /// Returns an error if storage is unavailable, `task_id` is missing,
    /// or the storage backend fails.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let storage = ctx.storage.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Storage is not available. Cannot retrieve task without a storage backend."
            )
        })?;

        let task_id = input["task_id"].as_str().ok_or_else(|| {
            anyhow::anyhow!(
                "Missing required 'task_id' parameter. Provide the ID of the task to retrieve."
            )
        })?;

        // Fetch all tasks for the session so we can compute derived
        // DAG fields: `blocks` (inverse of blocked_by), `is_blocked`,
        // and `is_available` (FR-003, FR-005, FR-014).
        let all_tasks = storage
            .list_tasks(&ctx.session_id, None)
            .map_err(|e| anyhow::anyhow!("Failed to read tasks: {e}"))?;

        let task = all_tasks
            .iter()
            .find(|t| t.id == task_id)
            .ok_or_else(|| anyhow::anyhow!("Task '{task_id}' not found in this session"))?;

        // Compute derived DAG fields for all session tasks (T-006).
        let dag = compute_dag(&all_tasks);
        let info = dag.get(&task.id).cloned().unwrap_or_default();

        // Build the full record (FR-014) with derived annotations.
        let record = format_task_record(task, &info);
        let metadata = json!({
            "id": task.id,
            "subject": task.title,
            "description": task.description,
            "active_form": task.active_form,
            "status": task.status,
            "owner": task.owner,
            "metadata": task.metadata,
            "blocked_by": task.blocked_by,
            "blocks": info.blocks,
            "is_blocked": info.is_blocked,
            "is_available": info.is_available,
            "created_at": task.created_at,
            "updated_at": task.updated_at,
        });

        Ok(ToolOutput {
            content: record,
            metadata: Some(metadata),
        })
    }
}

// ── TaskListTool (T-010, FR-015) ────────────────────────────────────

/// Valid status filter values for `task_list` (FR-015).
const LIST_STATUSES: &[&str] = &["pending", "in_progress", "completed", "all"];

/// Lists all tasks for the current session, optionally filtered by
/// status (todo2tasks T-010, FR-015).
///
/// Optional parameter: `status` (`pending` / `in_progress` /
/// `completed` / `all`, default `all`).  Returns all session tasks
/// ordered by `created_at`.  Each entry includes `id`, `subject`,
/// `status`, `owner`, `blocked_by`.
pub struct TaskListTool;

#[async_trait::async_trait]
impl Tool for TaskListTool {
    fn name(&self) -> &'static str {
        "task_list"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &'static str {
        "List all tasks for the current session, optionally filtered by status. \
         Each entry includes id, subject, status, owner, and blocked_by. \
         Tasks are ordered by created_at. \
         Optional 'status' parameter: pending, in_progress, completed, or all (default: all)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "status": {
                    "type": "string",
                    "description": "Filter by status: pending, in_progress, completed, or all (default: all)",
                    "enum": ["pending", "in_progress", "completed", "all"]
                }
            },
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "task"
    }

    /// # Errors
    ///
    /// Returns an error if storage is unavailable, the `status` filter
    /// is invalid, or the storage backend fails.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let storage = ctx.storage.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Storage is not available. Cannot list tasks without a storage backend."
            )
        })?;

        let status_filter = input["status"].as_str().unwrap_or("all");

        if !LIST_STATUSES.contains(&status_filter) {
            anyhow::bail!(
                "Invalid status filter '{}'. Must be one of: {}",
                status_filter,
                LIST_STATUSES.join(", ")
            );
        }

        // Fetch ALL tasks (unfiltered) so the DAG derivation is correct
        // even when a status filter is applied — `is_blocked` and
        // `is_available` depend on the full session task set (T-006,
        // FR-003, FR-005).
        let all_tasks = storage
            .list_tasks(&ctx.session_id, None)
            .map_err(|e| anyhow::anyhow!("Failed to read tasks: {e}"))?;

        // Compute derived DAG fields from the full task set.
        let dag = compute_dag(&all_tasks);

        // Apply status filter after DAG computation.
        let mut tasks: Vec<TaskRow> = if status_filter == "all" {
            all_tasks
        } else {
            all_tasks
                .into_iter()
                .filter(|t| t.status == status_filter)
                .collect()
        };

        // Order by created_at (FR-015).
        tasks.sort_by(|a, b| a.created_at.cmp(&b.created_at));

        let content = format_task_list(&tasks, status_filter, &dag);

        let tasks_meta: Vec<Value> = tasks
            .iter()
            .map(|t| {
                let info = dag.get(&t.id).cloned().unwrap_or_default();
                json!({
                    "id": t.id,
                    "subject": t.title,
                    "status": t.status,
                    "owner": t.owner,
                    "blocked_by": t.blocked_by,
                    "is_blocked": info.is_blocked,
                    "is_available": info.is_available,
                })
            })
            .collect();

        let metadata = json!({
            "count": tasks.len(),
            "status_filter": status_filter,
            "tasks": tasks_meta,
        });

        Ok(ToolOutput {
            content,
            metadata: Some(metadata),
        })
    }
}

// ── Formatting helpers ──────────────────────────────────────────────

/// Formats a single task as a human-readable markdown record
/// (FR-014) with derived DAG annotations (FR-003, FR-005).
///
/// When `info.is_blocked` is true, a `[blocked by #id, …]` annotation
/// is rendered.  When `info.is_available` is true, an `[available]`
/// annotation is rendered.  The `blocks` list is shown when non-empty.
pub(crate) fn format_task_record(task: &TaskRow, info: &TaskDagInfo) -> String {
    let status_icon = match task.status.as_str() {
        "pending" => "⏳",
        "in_progress" => "🔄",
        "completed" => "✅",
        "done" => "✅",
        "blocked" => "🚫",
        _ => "❓",
    };

    let mut out = String::new();
    out.push_str(&format!("## Task `{}`\n\n", task.id));
    out.push_str(&format!(
        "- {} **{}** `[{}]`\n",
        status_icon, task.title, task.status
    ));

    // Derived annotations (FR-003, FR-005).
    if info.is_blocked {
        let deps: String = task
            .blocked_by
            .iter()
            .map(|d| format!("#{d}"))
            .collect::<Vec<_>>()
            .join(", ");
        out.push_str(&format!("  [blocked by {deps}]\n"));
    } else if info.is_available {
        out.push_str("  [available]\n");
    }

    if let Some(ref active) = task.active_form
        && !active.is_empty()
    {
        out.push_str(&format!("  Active form: {}\n", active));
    }

    if !task.description.is_empty() {
        out.push_str(&format!("  Description: {}\n", task.description));
    }

    if let Some(ref owner) = task.owner
        && !owner.is_empty()
    {
        out.push_str(&format!("  Owner: {}\n", owner));
    }

    if !task.blocked_by.is_empty() {
        out.push_str(&format!("  Blocked by: {}\n", task.blocked_by.join(", ")));
    }

    if !info.blocks.is_empty() {
        out.push_str(&format!("  Blocks: {}\n", info.blocks.join(", ")));
    }

    out.push_str(&format!("  Created: {}\n", task.created_at));
    out.push_str(&format!("  Updated: {}\n", task.updated_at));

    out
}

/// Formats a list of tasks as human-readable markdown (FR-015) with
/// derived DAG annotations (FR-003, FR-005).
///
/// Each entry shows `id`, `subject` (title), `status`, `owner`,
/// `blocked_by`, and derived `[available]` / `[blocked by #id, …]`
/// annotations.
fn format_task_list(
    tasks: &[TaskRow],
    status_filter: &str,
    dag: &HashMap<String, TaskDagInfo>,
) -> String {
    let mut output = String::new();
    if tasks.is_empty() {
        output.push_str("No tasks found");
        if status_filter != "all" {
            output.push_str(&format!(" with status '{status_filter}'"));
        }
        output.push('.');
    } else {
        output.push_str(&format!("## Tasks ({} items)\n\n", tasks.len()));
        for task in tasks {
            let info = dag.get(&task.id).cloned().unwrap_or_default();
            let status_icon = match task.status.as_str() {
                "pending" => "⏳",
                "in_progress" => "🔄",
                "completed" => "✅",
                "done" => "✅",
                "blocked" => "🚫",
                _ => "❓",
            };
            output.push_str(&format!(
                "- {} **{}** `[{}]`\n",
                status_icon, task.title, task.status
            ));
            output.push_str(&format!("  ID: {}\n", task.id));

            // Derived annotations (FR-003, FR-005).
            if info.is_blocked {
                let deps: String = task
                    .blocked_by
                    .iter()
                    .map(|d| format!("#{d}"))
                    .collect::<Vec<_>>()
                    .join(", ");
                output.push_str(&format!("  [blocked by {deps}]\n"));
            } else if info.is_available {
                output.push_str("  [available]\n");
            }

            if let Some(ref owner) = task.owner
                && !owner.is_empty()
            {
                output.push_str(&format!("  Owner: {}\n", owner));
            }
            if !task.blocked_by.is_empty() {
                output.push_str(&format!("  Blocked by: {}\n", task.blocked_by.join(", ")));
            }
        }
    }
    output
}
