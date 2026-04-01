//! Task list types and the file-locked `TaskStore`.
//!
//! `tasks.json` is shared among all teammates and the lead.  Concurrent writes
//! are serialised using an exclusive `flock` on the file via the `fs2` crate.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Utc};
use fs2::FileExt as _;
use serde::{Deserialize, Serialize};

// ── Task status ─────────────────────────────────────────────────────────────

/// Lifecycle state of a single task.
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

// ── Task ─────────────────────────────────────────────────────────────────────

/// A single unit of work in the shared task list.
#[derive(Debug, Clone, Serialize, Deserialize)]
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
    pub claimed_at: Option<DateTime<Utc>>,
    /// When the task was marked `Completed`.
    pub completed_at: Option<DateTime<Utc>>,
}

impl Task {
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
        }
    }

    /// Return `true` if the task is pending and all dependencies are satisfied.
    pub fn is_claimable(&self, completed_ids: &[String]) -> bool {
        self.status == TaskStatus::Pending
            && self
                .depends_on
                .iter()
                .all(|dep| completed_ids.contains(dep))
    }
}

// ── Task list ─────────────────────────────────────────────────────────────────

/// Root of `tasks.json`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct TaskList {
    /// Name of the owning team.
    pub team_name: String,
    /// All tasks in insertion order.
    pub tasks: Vec<Task>,
}

impl TaskList {
    /// Create an empty task list for `team_name`.
    pub fn new(team_name: impl Into<String>) -> Self {
        Self {
            team_name: team_name.into(),
            tasks: Vec::new(),
        }
    }

    /// IDs of all tasks that are `Completed`.
    fn completed_ids(&self) -> Vec<String> {
        self.tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .map(|t| t.id.clone())
            .collect()
    }

    /// Find the first pending task whose dependencies are all satisfied.
    pub fn next_claimable(&self) -> Option<&Task> {
        let done = self.completed_ids();
        self.tasks.iter().find(|t| t.is_claimable(&done))
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
}

impl TaskStore {
    /// Open (or create) a `TaskStore` at `team_dir/tasks.json`.
    pub fn open(team_dir: &Path) -> Result<Self> {
        let path = team_dir.join("tasks.json");
        Ok(Self { path })
    }

    /// Read the current task list without acquiring a lock.
    pub fn read(&self) -> Result<TaskList> {
        if !self.path.exists() {
            return Ok(TaskList::default());
        }
        let raw = fs::read_to_string(&self.path)
            .with_context(|| format!("read {}", self.path.display()))?;
        if raw.trim().is_empty() {
            return Ok(TaskList::default());
        }
        serde_json::from_str(&raw)
            .with_context(|| format!("parse {}", self.path.display()))
    }

    /// Write the task list (caller must hold the lock).
    fn write_locked(file: &mut File, list: &TaskList) -> Result<()> {
        let json = serde_json::to_string_pretty(list)?;
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(json.as_bytes())?;
        file.flush()?;
        Ok(())
    }

    /// Atomically claim the next available task for `agent_id`.
    ///
    /// Acquires an exclusive file lock, finds the first `Pending` task whose
    /// dependencies are all `Completed`, marks it `InProgress`, and releases
    /// the lock.  Returns the claimed task, or `None` if no task is available.
    pub fn claim_next(&self, agent_id: &str) -> Result<Option<Task>> {
        // Open (or create) the file for read+write.
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
            .with_context(|| format!("open task store {}", self.path.display()))?;

        file.lock_exclusive()
            .with_context(|| "acquire exclusive lock on tasks.json")?;

        // Read current contents (may be empty on first use).
        let mut raw = String::new();
        file.read_to_string(&mut raw)?;
        let mut list: TaskList = if raw.trim().is_empty() {
            TaskList::default()
        } else {
            serde_json::from_str(&raw)?
        };

        let done = list.completed_ids();
        let idx = list
            .tasks
            .iter()
            .position(|t| t.is_claimable(&done));

        if let Some(i) = idx {
            list.tasks[i].status = TaskStatus::InProgress;
            list.tasks[i].assigned_to = Some(agent_id.to_owned());
            list.tasks[i].claimed_at = Some(Utc::now());
            let claimed = list.tasks[i].clone();
            Self::write_locked(&mut file, &list)?;
            file.unlock()?;
            Ok(Some(claimed))
        } else {
            file.unlock()?;
            Ok(None)
        }
    }

    /// Mark a task as `Completed`.  Unblocks dependents automatically (they
    /// become claimable on the next `claim_next` call).
    pub fn complete(&self, task_id: &str, agent_id: &str) -> Result<Task> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
            .with_context(|| format!("open task store {}", self.path.display()))?;

        file.lock_exclusive()?;

        let mut raw = String::new();
        file.read_to_string(&mut raw)?;
        let mut list: TaskList = if raw.trim().is_empty() {
            TaskList::default()
        } else {
            serde_json::from_str(&raw)
                .with_context(|| "parse tasks.json")?
        };

        let available_ids: Vec<String> = list.tasks.iter()
            .map(|t| format!("{} ({})", t.id, t.title))
            .collect();
        let task = list
            .get_mut(task_id)
            .ok_or_else(|| anyhow!(
                "task '{task_id}' not found. Available tasks: [{}]",
                available_ids.join(", ")
            ))?;

        // Auto-claim if the task is pending/unclaimed, rather than rejecting
        if task.assigned_to.as_deref() != Some(agent_id) {
            if task.status == TaskStatus::Pending || task.assigned_to.is_none() {
                task.assigned_to = Some(agent_id.to_owned());
                task.claimed_at = Some(Utc::now());
                task.status = TaskStatus::InProgress;
            } else {
                let current_owner = task.assigned_to.as_deref().unwrap_or("unknown");
                file.unlock()?;
                return Err(anyhow!(
                    "task {task_id} is assigned to agent {current_owner}, not {agent_id}"
                ));
            }
        }
        task.status = TaskStatus::Completed;
        task.completed_at = Some(Utc::now());
        let completed = task.clone();

        Self::write_locked(&mut file, &list)?;
        file.unlock()?;
        Ok(completed)
    }

    /// Add a new task to the list (lead-only operation; not file-locked because
    /// the lead is the only writer of new tasks).
    pub fn add_task(&self, task: Task) -> Result<()> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
            .with_context(|| format!("open task store {}", self.path.display()))?;

        file.lock_exclusive()?;

        let mut raw = String::new();
        file.read_to_string(&mut raw)?;
        let mut list: TaskList = if raw.trim().is_empty() {
            TaskList::default()
        } else {
            serde_json::from_str(&raw)?
        };

        if list.tasks.iter().any(|t| t.id == task.id) {
            file.unlock()?;
            return Err(anyhow!("task {} already exists", task.id));
        }

        list.tasks.push(task);
        Self::write_locked(&mut file, &list)?;
        file.unlock()?;
        Ok(())
    }

    /// Update an existing task's status and/or assignment (used by `team_task_update`).
    pub fn update_task<F>(&self, task_id: &str, f: F) -> Result<Task>
    where
        F: FnOnce(&mut Task),
    {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
            .with_context(|| format!("open task store {}", self.path.display()))?;

        file.lock_exclusive()?;

        let mut raw = String::new();
        file.read_to_string(&mut raw)?;
        let mut list: TaskList = if raw.trim().is_empty() {
            TaskList::default()
        } else {
            serde_json::from_str(&raw)
                .with_context(|| "parse tasks.json")?
        };

        let available_ids: Vec<String> = list.tasks.iter()
            .map(|t| format!("{} ({})", t.id, t.title))
            .collect();
        let task = list
            .get_mut(task_id)
            .ok_or_else(|| anyhow!(
                "task '{task_id}' not found. Available tasks: [{}]",
                available_ids.join(", ")
            ))?;
        f(task);
        let updated = task.clone();

        Self::write_locked(&mut file, &list)?;
        file.unlock()?;
        Ok(updated)
    }

    /// Remove a task from the store by ID.
    ///
    /// Used by the `TaskCreated` quality-gate hook to reject a newly created
    /// task.  Returns the removed task, or an error if the ID is not found.
    pub fn remove_task(&self, task_id: &str) -> Result<Task> {
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.path)
            .with_context(|| format!("open task store {}", self.path.display()))?;

        file.lock_exclusive()?;

        let mut raw = String::new();
        file.read_to_string(&mut raw)?;
        let mut list: TaskList = if raw.trim().is_empty() {
            TaskList::default()
        } else {
            serde_json::from_str(&raw)
                .with_context(|| "parse tasks.json")?
        };

        let pos = list
            .tasks
            .iter()
            .position(|t| t.id == task_id)
            .ok_or_else(|| anyhow!("task '{task_id}' not found"))?;
        let removed = list.tasks.remove(pos);

        Self::write_locked(&mut file, &list)?;
        file.unlock()?;
        Ok(removed)
    }
}
