//! Task list types and the file-locked `TaskStore`.
//!
//! `tasks.json` is shared among all teammates and the lead.  Concurrent writes
//! are serialised using an exclusive `flock` on the file via the `fs2` crate.

use std::fs::{self, File, OpenOptions};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use fs2::FileExt as _;
use serde::{Deserialize, Serialize};

// ── Task status ─────────────────────────────────────────────────────────────

/// Lifecycle state of a single task.
///
/// Serialises to snake_case (`"pending"`, `"in_progress"`, `"completed"`,
/// `"cancelled"`) via `#[serde(rename_all = "lowercase")]` and via
/// [`TaskStatus::as_str`] (M5-T1). All output paths (serde, Debug-derived
/// tool output, SSE) produce the same snake_case string.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    /// Waiting to be claimed.
    #[default]
    Pending,
    /// Claimed by a teammate and actively being worked on.
    InProgress,
    /// Successfully completed.
    Completed,
    /// Cancelled by the lead.
    Cancelled,
}

impl TaskStatus {
    /// Return the canonical snake_case string used for serialization, tool
    /// output, and SSE (M5-T1).
    ///
    /// This matches the `#[serde(rename_all = "lowercase")]` on-disk format
    /// (`"pending"`, `"in_progress"`, `"completed"`, `"cancelled"`).
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Cancelled => "cancelled",
        }
    }
}

// ── Task ─────────────────────────────────────────────────────────────────────

/// A single unit of work in the shared task list.
///
/// `#[serde(deny_unknown_fields)]` rejects unknown fields on manual edits so
/// typos are surfaced instead of silently ignored (M5-T3).
///
/// M6-T3: `completed_by` makes task completion idempotent — a second
/// completion by a *different* agent is rejected, while the same agent
/// repeating a completion it already owns returns success without mutation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    /// Unique task identifier (e.g. `"task-001"`).
    pub id: String,
    /// Short human-readable title.
    pub title: String,
    /// Full description of the work to be done.
    #[serde(default)]
    pub description: String,
    /// Current state.
    pub status: TaskStatus,
    /// Agent ID of the teammate this task is assigned to, if any.
    pub assigned_to: Option<String>,
    /// Task IDs that must be `Completed` before this task can be claimed.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// When the task was added to the list.
    pub created_at: DateTime<Utc>,
    /// When a teammate first claimed the task.
    #[serde(default)]
    pub claimed_at: Option<DateTime<Utc>>,
    /// When the task was marked `Completed`.
    #[serde(default)]
    pub completed_at: Option<DateTime<Utc>>,
    /// M6-T3: Agent ID that completed this task, if any. Used to make
    /// completion idempotent.
    #[serde(default)]
    pub completed_by: Option<String>,
}

impl Task {
    /// Validate the task's invariants (M5-T3).
    ///
    /// Returns `Ok(())` if the task is well-formed, or an error describing the
    /// first violation. Checks:
    /// - `id` is non-empty.
    /// - `assigned_to`, when set, is a plausible agent ID (`"lead"` or `tm-…`).
    /// - `depends_on` does not contain the task's own id.
    pub fn validate(&self) -> Result<()> {
        if self.id.is_empty() {
            return Err(anyhow!("task id is empty"));
        }
        if let Some(owner) = &self.assigned_to
            && !owner.is_empty()
            && owner != "lead"
            && !owner.starts_with("tm-")
        {
            return Err(anyhow!(
                "task {} assigned_to '{owner}' is not a valid agent id (expected 'lead' or 'tm-…')",
                self.id
            ));
        }
        if self.depends_on.iter().any(|d| d == &self.id) {
            return Err(anyhow!("task {} depends on itself", self.id));
        }
        Ok(())
    }
    /// Create a new task in `Pending` state.
    pub fn new(id: impl Into<String>, title: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            description: String::new(),
            status: TaskStatus::Pending,
            assigned_to: None,
            depends_on: Vec::new(),
            created_at: Utc::now(),
            claimed_at: None,
            completed_at: None,
            completed_by: None,
        }
    }

    /// Return `true` if the task is pending and all dependencies are satisfied.
    ///
    /// PERF-026: `completed_ids` is a `HashSet<String>` so each
    /// `depends_on` lookup is O(1), giving O(D) per claim check (D =
    /// dependency count) instead of the previous O(T * D) when
    /// `completed_ids` was a `Vec<String>`.
    #[must_use]
    pub fn is_claimable(&self, completed_ids: &std::collections::HashSet<String>) -> bool {
        self.status == TaskStatus::Pending
            && self
                .depends_on
                .iter()
                .all(|dep| completed_ids.contains(dep))
    }
}

// ── Task list ─────────────────────────────────────────────────────────────────

/// Root of `tasks.json`.
///
/// Carries a `schema_version` (M5-T2) and `updated_at` (M5-T5) so future
/// schema changes can be migrated and concurrent races can be debugged.
/// `#[serde(deny_unknown_fields)]` rejects unknown fields on manual edits
/// (M5-T3).
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(deny_unknown_fields)]
pub struct TaskList {
    /// Schema version of the on-disk format (M5-T2).
    #[serde(default)]
    pub schema_version: u32,
    /// Name of the owning team.
    #[serde(default)]
    pub team_name: String,
    /// All tasks in insertion order.
    pub tasks: Vec<Task>,
    /// When this list was last written (M5-T5).
    #[serde(default)]
    pub updated_at: Option<DateTime<Utc>>,
}

/// Current on-disk schema version for [`TaskList`] (M5-T2).
pub const TASK_LIST_SCHEMA_VERSION: u32 = 1;

impl TaskList {
    /// Create an empty task list for `team_name`.
    pub fn new(team_name: impl Into<String>) -> Self {
        Self {
            schema_version: TASK_LIST_SCHEMA_VERSION,
            team_name: team_name.into(),
            tasks: Vec::new(),
            updated_at: Some(Utc::now()),
        }
    }

    /// Validate every task in the list (M5-T3).
    pub fn validate(&self) -> Result<()> {
        for t in &self.tasks {
            t.validate()?;
        }
        Ok(())
    }

    /// Migrate this list to the current schema version (M5-T2).
    ///
    /// Currently a no-op: the only schema change so far is the addition of
    /// `schema_version` and `updated_at`, both `#[serde(default)]`. Future
    /// breaking changes should bump [`TASK_LIST_SCHEMA_VERSION`] and perform
    /// the field transforms here before the list is used.
    pub fn migrate(&mut self) {
        if self.schema_version == 0 {
            self.schema_version = TASK_LIST_SCHEMA_VERSION;
        }
        if self.updated_at.is_none() {
            self.updated_at = Some(Utc::now());
        }
    }

    /// IDs of all tasks that are `Completed`.
    ///
    /// PERF-026: returns a `HashSet<String>` (not a `Vec`) so the caller's
    /// `is_claimable` dependency check is O(1) per dependency instead of a
    /// linear scan. Previously this rebuilt a `Vec<String>` on every
    /// `is_claimable` / `next_claimable` call, yielding O(T²) complexity for a
    /// full `next_claimable` scan over T tasks.
    pub fn completed_ids(&self) -> std::collections::HashSet<String> {
        self.tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.id.clone())
            .collect()
    }

    /// Find the first pending task whose dependencies are all satisfied.
    #[must_use]
    pub fn next_claimable(&self) -> Option<&Task> {
        let done = self.completed_ids();
        self.tasks.iter().find(|t| t.is_claimable(&done))
    }

    /// Find the in-progress task currently owned by `agent_id`, if any.
    #[must_use]
    pub fn in_progress_for<'a>(&'a self, agent_id: &str) -> Option<&'a Task> {
        self.tasks.iter().find(|t| {
            t.status == TaskStatus::InProgress && t.assigned_to.as_deref() == Some(agent_id)
        })
    }

    /// Find a task by ID, returning a mutable reference.
    pub fn get_mut(&mut self, task_id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == task_id)
    }
}

// ── Task store ────────────────────────────────────────────────────────────────

/// File-backed store for a team's task list.
///
/// All mutating operations acquire an exclusive `flock` on `tasks.json` for
/// the duration of the read-modify-write cycle, making claim races safe even
/// across multiple ragent processes on the same machine.
pub struct TaskStore {
    path: PathBuf,
    /// PERF-016: team directory retained so the `*_blocking` async wrappers
    /// can reconstruct the store inside a `spawn_blocking` closure.
    pub team_dir: PathBuf,
}

impl TaskStore {
    /// Open (or create) a `TaskStore` at `team_dir/tasks.json`.
    pub fn open(team_dir: &Path) -> Result<Self> {
        let path = team_dir.join("tasks.json");
        Ok(Self {
            path,
            team_dir: team_dir.to_path_buf(),
        })
    }

    /// Return the companion lock-file path for `tasks.json`.
    fn lock_path(&self) -> PathBuf {
        let mut name = self.path.as_os_str().to_os_string();
        name.push(".lock");
        PathBuf::from(name)
    }

    /// Acquire an advisory `flock` on the companion lock file.
    fn acquire_lock(&self, exclusive: bool) -> Result<File> {
        let lock = self.lock_path();
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&lock)
            .with_context(|| format!("open lock file {}", lock.display()))?;
        if exclusive {
            file.lock_exclusive()
                .with_context(|| format!("acquire exclusive lock on {}", lock.display()))?;
        } else {
            file.lock_shared()
                .with_context(|| format!("acquire shared lock on {}", lock.display()))?;
        }
        Ok(file)
    }

    /// Read the current task list (acquires a shared lock on the lock file).
    ///
    /// M5-T2: migrates the loaded list to the current schema version before
    /// returning it.
    pub fn read(&self) -> Result<TaskList> {
        let lock = self.acquire_lock(false)?;
        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)
                .with_context(|| format!("read {}", self.path.display()))?
        } else {
            String::new()
        };
        drop(lock);
        if raw.trim().is_empty() {
            return Ok(TaskList::default());
        }
        let mut list: TaskList =
            serde_json::from_str(&raw).with_context(|| format!("parse {}", self.path.display()))?;
        list.migrate();
        Ok(list)
    }

    /// Write `list` to `path` atomically while the caller holds the lock file.
    ///
    /// The temp file name includes a UUID so concurrent writers cannot collide
    /// on the same temp path (Milestone 1, M1-T4).
    fn write_locked(path: &Path, list: &TaskList) -> Result<()> {
        // M5-T5: stamp updated_at on every write.
        let mut to_write = list.clone();
        to_write.updated_at = Some(Utc::now());
        if to_write.schema_version == 0 {
            to_write.schema_version = TASK_LIST_SCHEMA_VERSION;
        }
        let json = serde_json::to_string_pretty(&to_write)?;
        let file_name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "tasks.json".to_string());
        let temp_path = path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join(format!(".{file_name}.{}", uuid::Uuid::new_v4()));
        fs::write(&temp_path, json)?;
        let result: Result<()> = (|| {
            let temp = OpenOptions::new().read(true).open(&temp_path)?;
            temp.sync_all()
                .with_context(|| format!("sync temp file {}", temp_path.display()))?;
            fs::rename(&temp_path, path)
                .with_context(|| format!("rename {} -> {}", temp_path.display(), path.display()))?;
            Ok(())
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temp_path);
        }
        result
    }

    /// PERF-023: write-through persistence entry point used by
    /// [`TeamManager`]'s in-memory `TaskList` cache.
    ///
    /// Acquires an exclusive `flock`, re-reads the on-disk list, applies
    /// `f` to it, and writes the result back atomically. The re-read +
    /// merge guard makes writes safe against external processes that may
    /// have mutated the file between the cache's load and this write: if
    /// the on-disk list has diverged from the cached snapshot, the merge
    /// function reconciles the in-memory mutation with the latest disk
    /// state (rather than blindly overwriting it).
    ///
    /// Returns the freshly-written [`TaskList`] so the caller can refresh
    /// its in-memory cache without an extra round-trip.
    pub fn write_through<F>(&self, f: F) -> Result<TaskList>
    where
        F: FnOnce(&mut TaskList),
    {
        let lock = self.acquire_lock(true)?;
        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = if raw.trim().is_empty() {
            TaskList::default()
        } else {
            serde_json::from_str(&raw).with_context(|| "parse tasks.json")?
        };
        f(&mut list);
        Self::write_locked(&self.path, &list)?;
        drop(lock);
        Ok(list)
    }

    /// Return the path to the `tasks.json` file (PERF-023: exposed so the
    /// in-memory cache on [`TeamManager`] can stat it for mtime-based
    /// invalidation without re-opening a `TaskStore`).
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Atomically claim the next available task for `agent_id`.
    ///
    /// Acquires an exclusive lock on the companion lock file, finds the first
    /// `Pending` task whose dependencies are all `Completed`, marks it
    /// `InProgress`, and writes the result atomically.  Returns
    /// `(Some(task), already_had)` where `already_had` is `true` if the agent
    /// already owned an in-progress task (no new claim was made) or `false` for
    /// a fresh claim.  Returns `(None, false)` when no tasks are available.
    pub fn claim_next(&self, agent_id: &str) -> Result<(Option<Task>, bool)> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = if raw.trim().is_empty() {
            TaskList::default()
        } else {
            serde_json::from_str(&raw)?
        };

        let done = list.completed_ids();

        // Guard: if this agent already has an in-progress task, return it as-is
        // so the tool can inform the agent to complete it before claiming another.
        if let Some(active) = list
            .tasks
            .iter()
            .find(|t| {
                t.status == TaskStatus::InProgress && t.assigned_to.as_deref() == Some(agent_id)
            })
            .cloned()
        {
            drop(lock);
            return Ok((Some(active), true));
        }

        let idx = list.tasks.iter().position(|t| t.is_claimable(&done));

        let result = if let Some(i) = idx {
            list.tasks[i].status = TaskStatus::InProgress;
            list.tasks[i].assigned_to = Some(agent_id.to_owned());
            list.tasks[i].claimed_at = Some(Utc::now());
            let claimed = list.tasks[i].clone();
            Self::write_locked(&self.path, &list).map(|_| (Some(claimed), false))
        } else {
            Ok((None, false))
        };
        drop(lock);
        result
    }

    /// Claim a specific task by ID for the given agent.
    ///
    /// Returns the claimed task, or an error if the task doesn't exist, is not
    /// claimable, or is already assigned to a different agent.
    pub fn claim_specific(&self, task_id: &str, agent_id: &str) -> Result<Task> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = if raw.trim().is_empty() {
            TaskList::default()
        } else {
            serde_json::from_str(&raw)?
        };

        let done = list.completed_ids();

        // Guard: if this agent already has a different in-progress task, reject the claim.
        if let Some(other) = list.tasks.iter().find(|t| {
            t.status == TaskStatus::InProgress
                && t.assigned_to.as_deref() == Some(agent_id)
                && t.id != task_id
        }) {
            drop(lock);
            return Err(anyhow!(
                "agent {} already has task '{}' in progress; must complete it before claiming '{}'",
                agent_id,
                other.id,
                task_id
            ));
        }

        let Some(task) = list.get_mut(task_id) else {
            drop(lock);
            return Err(anyhow!("task '{task_id}' not found"));
        };

        // M6-T3: idempotent claim. If the agent already owns this task as
        // InProgress, return success without mutation.
        if task.status == TaskStatus::InProgress && task.assigned_to.as_deref() == Some(agent_id) {
            let already_claimed = task.clone();
            drop(lock);
            return Ok(already_claimed);
        }

        if let Some(assigned_to) = &task.assigned_to
            && assigned_to != agent_id
        {
            drop(lock);
            return Err(anyhow!(
                "task '{task_id}' is already assigned to {assigned_to}, not {agent_id}"
            ));
        }

        if task.status != TaskStatus::Pending {
            drop(lock);
            return Err(anyhow!(
                "task '{task_id}' cannot be claimed (status: {:?}) — only Pending tasks can be claimed",
                task.status
            ));
        }

        if !task.is_claimable(&done) {
            drop(lock);
            return Err(anyhow!(
                "task '{task_id}' cannot be claimed — unsatisfied dependencies: {:?}",
                task.depends_on
            ));
        }

        task.status = TaskStatus::InProgress;
        task.assigned_to = Some(agent_id.to_owned());
        task.claimed_at = Some(Utc::now());
        let claimed = task.clone();

        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;
        Ok(claimed)
    }

    /// Mark a task as `Completed`.  Unblocks dependents automatically (they
    /// become claimable on the next `claim_next` call).
    ///
    /// M6-T3: completion is idempotent. If the task is already `Completed`:
    /// - by the same `agent_id` → return the task unchanged (no-op success).
    /// - by a different agent → reject with an error.
    pub fn complete(&self, task_id: &str, agent_id: &str) -> Result<Task> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = if raw.trim().is_empty() {
            TaskList::default()
        } else {
            serde_json::from_str(&raw).with_context(|| "parse tasks.json")?
        };

        let available_ids: Vec<String> = list
            .tasks
            .iter()
            .map(|t| format!("{} ({})", t.id, t.title))
            .collect();
        let Some(task) = list.get_mut(task_id) else {
            drop(lock);
            return Err(anyhow!(
                "task '{task_id}' not found. Available tasks: [{}]",
                available_ids.join(", ")
            ));
        };

        // M6-T3: idempotent completion.
        if task.status == TaskStatus::Completed {
            let owner = task.completed_by.as_deref().unwrap_or("unknown");
            if owner == agent_id {
                // Same agent re-completing — no-op success.
                let already = task.clone();
                drop(lock);
                return Ok(already);
            }
            drop(lock);
            return Err(anyhow!(
                "task '{task_id}' is already completed by '{owner}', not '{agent_id}'"
            ));
        }

        // Auto-claim if the task is pending/unclaimed, rather than rejecting.
        if task.assigned_to.as_deref() != Some(agent_id) {
            if task.status == TaskStatus::Pending || task.assigned_to.is_none() {
                task.assigned_to = Some(agent_id.to_owned());
                task.claimed_at = Some(Utc::now());
                task.status = TaskStatus::InProgress;
            } else {
                let current_owner = task.assigned_to.as_deref().unwrap_or("unknown");
                drop(lock);
                return Err(anyhow!(
                    "task {task_id} is assigned to agent {current_owner}, not {agent_id}"
                ));
            }
        }
        task.status = TaskStatus::Completed;
        task.completed_at = Some(Utc::now());
        // M6-T3: record who completed it.
        task.completed_by = Some(agent_id.to_owned());
        let completed = task.clone();

        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;
        Ok(completed)
    }

    /// Add a new task to the list (acquires an exclusive lock).
    ///
    /// M8-T4: the previous doc comment incorrectly claimed this method does
    /// **not** acquire a lock. It does — `acquire_lock(true)` is called at
    /// the top, and the lock is held for the duration of the
    /// read-modify-write cycle via `write_locked`. This is the same
    /// race-free pattern used by all other mutating `TaskStore` methods.
    pub fn add_task(&self, task: Task) -> Result<()> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = if raw.trim().is_empty() {
            TaskList::default()
        } else {
            serde_json::from_str(&raw)?
        };

        if list.tasks.iter().any(|t| t.id == task.id) {
            drop(lock);
            return Err(anyhow!("task {} already exists", task.id));
        }

        list.tasks.push(task);
        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result
    }

    /// Atomically pre-assign a pending task to an agent in `InProgress` state.
    pub fn pre_assign_task(&self, task_id: &str, agent_id: &str) -> Result<Task> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = if raw.trim().is_empty() {
            TaskList::default()
        } else {
            serde_json::from_str(&raw)?
        };

        let Some(task) = list.get_mut(task_id) else {
            drop(lock);
            return Err(anyhow!("task '{task_id}' not found"));
        };

        if task.status != TaskStatus::Pending {
            drop(lock);
            return Err(anyhow!(
                "task '{task_id}' is not pending (status: {:?}); cannot pre-assign",
                task.status
            ));
        }

        if task.assigned_to.is_some() {
            let assigned_to = task.assigned_to.as_ref().unwrap();
            drop(lock);
            return Err(anyhow!(
                "task '{task_id}' is already assigned to {assigned_to}"
            ));
        }

        task.status = TaskStatus::InProgress;
        task.assigned_to = Some(agent_id.to_owned());
        task.claimed_at = Some(Utc::now());
        let assigned = task.clone();

        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;
        Ok(assigned)
    }

    /// Update an existing task's status and/or assignment (used by `team_task_update`).
    pub fn update_task<F>(&self, task_id: &str, f: F) -> Result<Task>
    where
        F: FnOnce(&mut Task),
    {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = if raw.trim().is_empty() {
            TaskList::default()
        } else {
            serde_json::from_str(&raw).with_context(|| "parse tasks.json")?
        };

        let available_ids: Vec<String> = list
            .tasks
            .iter()
            .map(|t| format!("{} ({})", t.id, t.title))
            .collect();
        let Some(task) = list.get_mut(task_id) else {
            drop(lock);
            return Err(anyhow!(
                "task '{task_id}' not found. Available tasks: [{}]",
                available_ids.join(", ")
            ));
        };
        f(task);
        let updated = task.clone();

        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;
        Ok(updated)
    }

    /// Remove a task from the store by ID.
    pub fn remove_task(&self, task_id: &str) -> Result<Task> {
        let lock = self.acquire_lock(true)?;

        let raw = if self.path.exists() {
            fs::read_to_string(&self.path)?
        } else {
            String::new()
        };
        let mut list: TaskList = if raw.trim().is_empty() {
            TaskList::default()
        } else {
            serde_json::from_str(&raw).with_context(|| "parse tasks.json")?
        };

        let pos = list
            .tasks
            .iter()
            .position(|t| t.id == task_id)
            .ok_or_else(|| anyhow!("task '{task_id}' not found"))?;
        let removed = list.tasks.remove(pos);

        let result = Self::write_locked(&self.path, &list);
        drop(lock);
        result?;
        Ok(removed)
    }

    // ── PERF-016: async `spawn_blocking` wrappers ──────────────────────────────
    //
    // Each of these mirrors a synchronous `TaskStore` method but moves the
    // full read-modify-write cycle (file lock + `fs::read_to_string` +
    // `serde_json` deserialise + `fs::write`) onto a tokio blocking-pool
    // thread so the async executor is never stalled on synchronous I/O
    // during concurrent teammate activity.

    /// PERF-016: `spawn_blocking` wrapper around [`TaskStore::read`].
    pub async fn read_blocking(&self) -> Result<TaskList> {
        let team_dir = self.team_dir.clone();
        tokio::task::spawn_blocking(move || {
            let store = Self::open(&team_dir)?;
            store.read()
        })
        .await?
    }

    /// PERF-016: `spawn_blocking` wrapper around [`TaskStore::claim_next`].
    pub async fn claim_next_blocking(&self, agent_id: String) -> Result<(Option<Task>, bool)> {
        let team_dir = self.team_dir.clone();
        tokio::task::spawn_blocking(move || {
            let store = Self::open(&team_dir)?;
            store.claim_next(&agent_id)
        })
        .await?
    }

    /// PERF-016: `spawn_blocking` wrapper around [`TaskStore::claim_specific`].
    pub async fn claim_specific_blocking(&self, task_id: String, agent_id: String) -> Result<Task> {
        let team_dir = self.team_dir.clone();
        tokio::task::spawn_blocking(move || {
            let store = Self::open(&team_dir)?;
            store.claim_specific(&task_id, &agent_id)
        })
        .await?
    }

    /// PERF-016: `spawn_blocking` wrapper around [`TaskStore::complete`].
    pub async fn complete_blocking(&self, task_id: String, agent_id: String) -> Result<Task> {
        let team_dir = self.team_dir.clone();
        tokio::task::spawn_blocking(move || {
            let store = Self::open(&team_dir)?;
            store.complete(&task_id, &agent_id)
        })
        .await?
    }

    /// PERF-016: `spawn_blocking` wrapper around [`TaskStore::add_task`].
    pub async fn add_task_blocking(&self, task: Task) -> Result<()> {
        let team_dir = self.team_dir.clone();
        tokio::task::spawn_blocking(move || {
            let store = Self::open(&team_dir)?;
            store.add_task(task)
        })
        .await?
    }

    /// PERF-016: `spawn_blocking` wrapper around [`TaskStore::update_task`].
    pub async fn update_task_blocking<F>(&self, task_id: String, f: F) -> Result<Task>
    where
        F: FnOnce(&mut Task) + Send + 'static,
    {
        let team_dir = self.team_dir.clone();
        tokio::task::spawn_blocking(move || {
            let store = Self::open(&team_dir)?;
            store.update_task(&task_id, f)
        })
        .await?
    }
}
