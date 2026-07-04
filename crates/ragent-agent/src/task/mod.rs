//! Sub-agent task management for F13 (sub-agent spawning) and F14 (background agents).
//!
//! The [`TaskManager`] tracks spawned sub-agent tasks, supports both synchronous
//! (blocking) and background (non-blocking) execution, and publishes lifecycle
//! events via the [`EventBus`](crate::event::EventBus).
//!
//! # Architecture
//!
//! ```text
//! Parent Session
//!   │
//!   ├─ new_task(agent: "explore", background: false)  ← blocks until done
//!   │   └─ TaskEntry { status: Completed, result: "..." }
//!   │
//!   └─ new_task(agent: "build", background: true)     ← returns immediately
//!       └─ TaskEntry { status: Running }
//!           ↓ (later)
//!       └─ SubagentComplete event published
//! ```

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::agent::{AgentMode, ModelRef};
use crate::event::{Event, EventBus};
use crate::session::processor::SessionProcessor;

/// D4 fix: Sanitize agent name for use in task ID.
/// Converts to lowercase, replaces spaces and special chars with hyphens.
fn sanitize_for_id(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c.to_lowercase().next().unwrap_or(c)
            } else {
                '-'
            }
        })
        .collect();
    // Remove consecutive hyphens and trim leading/trailing
    let mut result = String::new();
    let mut prev_hyphen = true; // Treat start as hyphen to trim leading
    for c in sanitized.chars() {
        if c == '-' {
            if !prev_hyphen {
                result.push(c);
            }
            prev_hyphen = true;
        } else {
            result.push(c);
            prev_hyphen = false;
        }
    }
    // Trim trailing hyphen if any
    if result.ends_with('-') {
        result.pop();
    }
    // Limit to 20 chars to keep IDs readable
    if result.len() > 20 {
        result.truncate(20);
    }
    // Ensure we have something - fallback to "task" if empty
    if result.is_empty() {
        result = "task".to_string();
    }
    result
}

/// Maximum number of concurrent background tasks (default).
pub const DEFAULT_MAX_BACKGROUND_TASKS: usize = 8;

/// Status of a sub-agent task.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    /// Task is actively running.
    Running,
    /// Task completed successfully.
    Completed,
    /// Task failed with an error.
    Failed,
    /// Task was suspended (paused) by user.
    Suspended,
    /// Task is being forcibly terminated.
    Terminating,
    /// Task was cancelled before completion.
    Cancelled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Suspended => write!(f, "suspended"),
            Self::Terminating => write!(f, "terminating"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A tracked sub-agent task entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskEntry {
    /// Unique task identifier.
    pub id: String,
    /// Session that spawned this task.
    pub parent_session_id: String,
    /// Session created for the sub-agent.
    pub child_session_id: String,
    /// Name of the agent running the task.
    pub agent_name: String,
    /// The prompt/task sent to the sub-agent.
    pub task_prompt: String,
    /// Whether this task runs in the background.
    pub background: bool,
    /// Current status.
    pub status: TaskStatus,
    /// Result summary (populated on completion).
    pub result: Option<String>,
    /// Error message (populated on failure).
    pub error: Option<String>,
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// When the task completed (if finished).
    pub completed_at: Option<DateTime<Utc>>,
    /// Whether this completion has been injected into the parent session.
    #[serde(default)]
    pub reported: bool,
    /// Number of active waiters for this task (via wait_tasks tool).
    /// When > 0, the task result should not be redundantly reported via drain_completed
    /// because a waiter is already handling it.
    #[serde(default)]
    pub waiter_count: u32,
}

/// Result of a completed sub-agent task.
#[derive(Debug, Clone)]
pub struct TaskResult {
    /// The task entry with final status.
    pub entry: TaskEntry,
    /// Full response text from the sub-agent.
    pub response: String,
}

/// Manages sub-agent task lifecycle, tracking, and background execution.
///
/// Thread-safe via interior mutability (`RwLock`). Designed to be shared
/// as `Arc<TaskManager>` across the session processor and tool invocations.
pub struct TaskManager {
    /// Active and completed tasks indexed by task ID.
    tasks: Arc<RwLock<HashMap<String, TaskEntry>>>,
    /// Cancel flags for running tasks.
    cancel_flags: Arc<RwLock<HashMap<String, Arc<AtomicBool>>>>,
    /// Event bus for publishing sub-agent lifecycle events.
    event_bus: Arc<EventBus>,
    /// Session processor for running sub-agent loops.
    processor: Arc<SessionProcessor>,
    /// Maximum concurrent background tasks.
    max_background: usize,
    /// P-11: flag set whenever a background task is spawned and cleared by
    /// `drain_completed` when no completed tasks remain to report. The
    /// agent loop checks this flag before calling `drain_completed` so the
    /// common "no background tasks" path avoids acquiring the task-map lock
    /// and scanning every entry on every loop step.
    has_pending_background: AtomicBool,
}

impl TaskManager {
    /// Creates a new task manager.
    pub fn new(
        event_bus: Arc<EventBus>,
        processor: Arc<SessionProcessor>,
        max_background: usize,
    ) -> Self {
        Self {
            tasks: Arc::new(RwLock::new(HashMap::new())),
            cancel_flags: Arc::new(RwLock::new(HashMap::new())),
            event_bus,
            processor,
            max_background,
            has_pending_background: AtomicBool::new(false),
        }
    }

    /// Spawns a sub-agent task synchronously (blocks until completion).
    ///
    /// Creates an isolated session, resolves the agent, runs the agent loop,
    /// and returns the result. The parent session is blocked during execution.
    pub async fn spawn_sync(
        &self,
        parent_session_id: &str,
        agent_name: &str,
        task_prompt: &str,
        model_override: Option<&str>,
        working_dir: &std::path::Path,
    ) -> anyhow::Result<TaskResult> {
        // D4 fix: Generate human-readable task ID based on agent name
        // e.g., "explore-a1b2c3d4" instead of full UUID
        let task_id = format!(
            "{}-{}",
            sanitize_for_id(agent_name),
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("task")
        );
        let start = Instant::now();
        // Create isolated session
        let child_session = self
            .processor
            .session_manager
            .create_session(working_dir.to_path_buf())?;
        let child_sid = child_session.id.clone();

        // Register task entry
        let entry = TaskEntry {
            id: task_id.clone(),
            parent_session_id: parent_session_id.to_string(),
            child_session_id: child_sid.clone(),
            agent_name: agent_name.to_string(),
            task_prompt: task_prompt.to_string(),
            background: false,
            status: TaskStatus::Running,
            result: None,
            error: None,
            created_at: Utc::now(),
            completed_at: None,
            reported: false,
            waiter_count: 0,
        };
        self.tasks.write().await.insert(task_id.clone(), entry);
        // P-11: spawn_sync tasks are not background (they block the caller),
        // so they are never drained by `drain_completed`. We do not set the
        // flag here — only `spawn_background` sets it.

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flags
            .write()
            .await
            .insert(task_id.clone(), cancel_flag.clone());

        // Publish start event
        self.event_bus.publish(Event::SubagentStart {
            session_id: parent_session_id.to_string(),
            task_id: task_id.clone(),
            child_session_id: child_sid.clone(),
            agent: agent_name.to_string(),
            task: truncate_str(task_prompt, 200),
            background: false,
        });

        // Resolve agent
        let result = self
            .run_subagent(
                &child_sid,
                agent_name,
                task_prompt,
                model_override,
                cancel_flag,
                working_dir,
            )
            .await;

        let duration_ms = start.elapsed().as_millis() as u64;

        // Update task entry and publish completion
        match result {
            Ok(response) => {
                let summary = truncate_str(&response, 2000);
                {
                    let mut tasks = self.tasks.write().await;
                    if let Some(entry) = tasks.get_mut(&task_id) {
                        entry.status = TaskStatus::Completed;
                        entry.result = Some(summary.clone());
                        entry.completed_at = Some(Utc::now());
                    }
                }
                self.cancel_flags.write().await.remove(&task_id);

                self.event_bus.publish(Event::SubagentComplete {
                    session_id: parent_session_id.to_string(),
                    task_id: task_id.clone(),
                    child_session_id: child_sid.clone(),
                    summary: summary.clone(),
                    success: true,
                    duration_ms,
                });

                let entry = self.tasks.read().await.get(&task_id).cloned().unwrap();
                Ok(TaskResult { entry, response })
            }
            Err(e) => {
                let error_msg = e.to_string();
                {
                    let mut tasks = self.tasks.write().await;
                    if let Some(entry) = tasks.get_mut(&task_id) {
                        entry.status = TaskStatus::Failed;
                        entry.error = Some(error_msg.clone());
                        entry.completed_at = Some(Utc::now());
                    }
                }
                self.cancel_flags.write().await.remove(&task_id);

                self.event_bus.publish(Event::SubagentComplete {
                    session_id: parent_session_id.to_string(),
                    task_id: task_id.clone(),
                    child_session_id: child_sid,
                    summary: format!("Error: {error_msg}"),
                    success: false,
                    duration_ms,
                });

                Err(e)
            }
        }
    }

    /// Spawns a sub-agent task in the background (returns immediately).
    ///
    /// The task runs as an independent tokio task. Results are published
    /// via [`Event::SubagentComplete`] when finished.
    ///
    /// Returns the task ID and entry for the newly spawned task.
    pub async fn spawn_background(
        &self,
        parent_session_id: &str,
        agent_name: &str,
        task_prompt: &str,
        model_override: Option<&str>,
        working_dir: &std::path::Path,
    ) -> anyhow::Result<TaskEntry> {
        // Check concurrency limit
        let running_count = self
            .tasks
            .read()
            .await
            .values()
            .filter(|t| t.status == TaskStatus::Running && t.background)
            .count();

        if running_count >= self.max_background {
            anyhow::bail!(
                "Maximum concurrent background tasks ({}) reached. \
                 Wait for a running task to complete or cancel one.",
                self.max_background
            );
        }

        // D4 fix: Generate human-readable task ID based on agent name
        // e.g., "explore-a1b2c3d4" instead of full UUID
        let task_id = format!(
            "{}-{}",
            sanitize_for_id(agent_name),
            uuid::Uuid::new_v4()
                .to_string()
                .split('-')
                .next()
                .unwrap_or("task")
        );

        // Create isolated session
        let child_session = self
            .processor
            .session_manager
            .create_session(working_dir.to_path_buf())?;
        let child_sid = child_session.id.clone();

        // Register task entry
        let entry = TaskEntry {
            id: task_id.clone(),
            parent_session_id: parent_session_id.to_string(),
            child_session_id: child_sid.clone(),
            agent_name: agent_name.to_string(),
            task_prompt: task_prompt.to_string(),
            background: true,
            status: TaskStatus::Running,
            result: None,
            error: None,
            created_at: Utc::now(),
            completed_at: None,
            reported: false,
            waiter_count: 0,
        };
        self.tasks
            .write()
            .await
            .insert(task_id.clone(), entry.clone());
        // P-11: mark that there is at least one pending background task so
        // the agent loop's `drain_completed` call is not skipped.
        self.has_pending_background.store(true, Ordering::Relaxed);

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flags
            .write()
            .await
            .insert(task_id.clone(), cancel_flag.clone());

        // Publish start event
        self.event_bus.publish(Event::SubagentStart {
            session_id: parent_session_id.to_string(),
            task_id: task_id.clone(),
            child_session_id: child_sid.clone(),
            agent: agent_name.to_string(),
            task: truncate_str(task_prompt, 200),
            background: true,
        });

        // Clone everything needed for the background task
        let parent_sid = parent_session_id.to_string();
        let agent = agent_name.to_string();
        let prompt = task_prompt.to_string();
        let model = model_override.map(std::string::ToString::to_string);
        let event_bus = self.event_bus.clone();
        let tasks = self.tasks.clone();
        let cancel_flags = self.cancel_flags.clone();
        let processor = self.processor.clone();
        let tid = task_id.clone();
        let csid = child_sid.clone();
        let working_dir_buf = working_dir.to_path_buf();

        tokio::spawn(async move {
            let start = Instant::now();

            let config = crate::Config::default();
            let mut agent_info =
                match crate::agent::resolve_agent_with_customs(&agent, &config, &working_dir_buf) {
                    Ok(a) => a,
                    Err(e) => {
                        let error_msg = e.to_string();
                        {
                            let mut t = tasks.write().await;
                            if let Some(entry) = t.get_mut(&tid) {
                                entry.status = TaskStatus::Failed;
                                entry.error = Some(error_msg.clone());
                                entry.completed_at = Some(Utc::now());
                            }
                        }
                        cancel_flags.write().await.remove(&tid);
                        event_bus.publish(Event::SubagentComplete {
                            session_id: parent_sid,
                            task_id: tid,
                            child_session_id: csid,
                            summary: format!("Error: {error_msg}"),
                            success: false,
                            duration_ms: start.elapsed().as_millis() as u64,
                        });
                        return;
                    }
                };
            agent_info.mode = AgentMode::Subagent;

            if let Some(ref model_str) = model
                && let Some((provider, model_id)) = model_str
                    .split_once('/')
                    .or_else(|| model_str.split_once(':'))
            {
                agent_info.model = Some(ModelRef {
                    provider_id: provider.to_string(),
                    model_id: model_id.to_string(),
                });
            }

            let result = processor
                .process_message(&csid, &prompt, &agent_info, cancel_flag)
                .await;

            let duration_ms = start.elapsed().as_millis() as u64;

            match result {
                Ok(response_msg) => {
                    let response = response_msg.text_content();
                    let summary = truncate_str(&response, 2000);
                    {
                        let mut t = tasks.write().await;
                        if let Some(entry) = t.get_mut(&tid) {
                            entry.status = TaskStatus::Completed;
                            entry.result = Some(summary.clone());
                            entry.completed_at = Some(Utc::now());
                        }
                    }
                    cancel_flags.write().await.remove(&tid);
                    event_bus.publish(Event::SubagentComplete {
                        session_id: parent_sid,
                        task_id: tid,
                        child_session_id: csid,
                        summary,
                        success: true,
                        duration_ms,
                    });
                }
                Err(e) => {
                    // M7-T4: Use typed cancellation detection instead of
                    // fragile string matching. The processor returns a
                    // Cancelled error variant when the cancel flag is
                    // set; we check for that rather than
                    // `error_msg.contains("cancelled")`.
                    let is_cancelled = is_cancel_error(&e);
                    let error_msg = e.to_string();
                    {
                        let mut t = tasks.write().await;
                        if let Some(entry) = t.get_mut(&tid) {
                            if is_cancelled {
                                entry.status = TaskStatus::Cancelled;
                            } else {
                                entry.status = TaskStatus::Failed;
                                entry.error = Some(error_msg.clone());
                            }
                            entry.completed_at = Some(Utc::now());
                        }
                    }
                    cancel_flags.write().await.remove(&tid);

                    if is_cancelled {
                        event_bus.publish(Event::SubagentCancelled {
                            session_id: parent_sid,
                            task_id: tid,
                        });
                    } else {
                        event_bus.publish(Event::SubagentComplete {
                            session_id: parent_sid,
                            task_id: tid,
                            child_session_id: csid,
                            summary: format!("Error: {error_msg}"),
                            success: false,
                            duration_ms,
                        });
                    }
                }
            }
        });

        Ok(entry)
    }

    /// Cancels a running task by setting its cancel flag.
    pub async fn cancel_task(&self, task_id: &str) -> anyhow::Result<()> {
        let flags = self.cancel_flags.read().await;
        if let Some(flag) = flags.get(task_id) {
            flag.store(true, Ordering::Relaxed);
            tracing::info!(task_id, "Cancel requested for sub-agent task");
            Ok(())
        } else {
            anyhow::bail!("Task '{task_id}' not found or already completed")
        }
    }

    /// M7-T1: Suspend a running sub-agent task (pause its event loop without
    /// cancelling).
    ///
    /// **Decision (M7-T1):** `suspend_task` / `resume_task` are **removed**
    /// from the public API surface because the session processor's agent
    /// loop does not honour `suspend_flags` — the loop keeps running and
    /// consuming tokens. Rather than ship a misleading no-op, the methods
    /// now return a clear error explaining that suspension is not
    /// implemented, and the `SubagentSuspended` / `SubagentResumed` events
    /// are no longer published. The TUI buttons that previously called
    /// these methods should use `cancel_task` instead.
    ///
    /// See `docs/team-unification-decision.md` for the rationale.
    pub async fn suspend_task(&self, task_id: &str) -> anyhow::Result<()> {
        // M7-T1: Suspend is not implemented in the processor agent loop.
        // The suspend_flags map exists but is never checked by the loop.
        // Rather than mislead callers, we return an explicit error.
        let _ = task_id;
        anyhow::bail!(
            "suspend_task is not implemented — the agent loop does not honour \
             suspend flags. Use cancel_task to stop a running sub-agent instead."
        )
    }

    /// M7-T1: Resume a suspended task — not implemented (see `suspend_task`).
    pub async fn resume_task(&self, task_id: &str) -> anyhow::Result<()> {
        let _ = task_id;
        anyhow::bail!(
            "resume_task is not implemented — the agent loop does not honour \
             suspend flags. Use cancel_task and re-spawn instead."
        )
    }

    /// Kill a running or suspended task (forcible termination).
    ///
    /// M7-T2: uses a blocking `write()` (not `try_write()`) to set the cancel
    /// flag so the cancel signal is never lost due to lock contention. The
    /// `kill_flags` field has been removed — the cancel flag alone is
    /// sufficient; the 10-second force-kill escalation path remains for
    /// tasks that don't observe the flag in time.
    pub async fn kill_task(&self, task_id: &str) -> anyhow::Result<()> {
        let mut tasks = self.tasks.write().await;
        let entry = tasks
            .get_mut(task_id)
            .ok_or_else(|| anyhow::anyhow!("Task '{task_id}' not found"))?;
        if matches!(
            entry.status,
            TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
        ) {
            anyhow::bail!("Task '{task_id}' is already finished");
        }
        entry.status = TaskStatus::Terminating;
        let parent = entry.parent_session_id.clone();
        let child = entry.child_session_id.clone();
        drop(tasks);

        // M7-T2: Use a blocking write() so the cancel signal cannot be
        // lost due to lock contention. The previous try_write() could
        // silently fail if another task held the write lock, leaving the
        // task running until the 10s force-kill.
        {
            let flags = self.cancel_flags.write().await;
            if let Some(cf) = flags.get(task_id) {
                cf.store(true, Ordering::Relaxed);
            }
        }

        self.event_bus.publish(Event::SubagentKilled {
            session_id: parent.clone(),
            task_id: task_id.to_string(),
            child_session_id: child.clone(),
            force: false,
        });
        tracing::info!(task_id, "Sub-agent kill requested");
        // Force-kill escalation after 10 seconds
        let tasks2 = self.tasks.clone();
        let flags2 = self.cancel_flags.clone();
        let eb2 = self.event_bus.clone();
        let tid = task_id.to_string();
        tokio::spawn(async move {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
            let mut t = tasks2.write().await;
            if let Some(entry) = t.get_mut(&tid) {
                if entry.status == TaskStatus::Terminating {
                    entry.status = TaskStatus::Failed;
                    entry.error = Some("Force-killed after timeout".to_string());
                    entry.completed_at = Some(Utc::now());
                }
            }
            flags2.write().await.remove(&tid);
            eb2.publish(Event::SubagentKilled {
                session_id: parent,
                task_id: tid,
                child_session_id: child,
                force: true,
            });
        });
        Ok(())
    }

    /// Returns a snapshot of a specific task.
    pub async fn get_task(&self, task_id: &str) -> Option<TaskEntry> {
        self.tasks.read().await.get(task_id).cloned()
    }

    /// Returns all tasks for a given parent session.
    pub async fn list_tasks(&self, parent_session_id: &str) -> Vec<TaskEntry> {
        self.tasks
            .read()
            .await
            .values()
            .filter(|t| t.parent_session_id == parent_session_id)
            .cloned()
            .collect()
    }

    /// Returns the count of currently running background tasks.
    pub async fn running_background_count(&self) -> usize {
        self.tasks
            .read()
            .await
            .values()
            .filter(|t| t.status == TaskStatus::Running && t.background)
            .count()
    }

    /// Cancels all running tasks for a given parent session.
    pub async fn cancel_all(&self, parent_session_id: &str) {
        let flags = self.cancel_flags.read().await;
        let tasks = self.tasks.read().await;
        for (tid, entry) in tasks.iter() {
            if entry.parent_session_id == parent_session_id
                && entry.status == TaskStatus::Running
                && let Some(flag) = flags.get(tid)
            {
                flag.store(true, Ordering::Relaxed);
                tracing::info!(task_id = tid, "Cancelling sub-agent task (session cleanup)");
            }
        }
    }

    /// Returns completed background tasks that have not yet been reported
    /// to the parent session, and marks them as reported.
    ///
    /// This is called by the processor loop between iterations to inject
    /// background task results into the conversation.
    ///
    /// Note: Tasks with waiter_count > 0 are skipped because they are being
    /// actively waited on via wait_tasks tool and should not be redundantly
    /// injected into the conversation.
    pub async fn drain_completed(&self, parent_session_id: &str) -> Vec<TaskEntry> {
        // P-11: fast path — if no background tasks have ever been spawned,
        // skip the lock acquisition and map scan entirely.
        if !self.has_pending_background.load(Ordering::Relaxed) {
            return Vec::new();
        }
        let mut tasks = self.tasks.write().await;
        let mut completed = Vec::new();
        for entry in tasks.values_mut() {
            if entry.parent_session_id == parent_session_id
                && entry.background
                && !entry.reported
                && entry.status != TaskStatus::Running
                && entry.waiter_count == 0
            {
                entry.reported = true;
                completed.push(entry.clone());
            }
        }
        // P-11: clear the flag when no unreported background tasks remain
        // for this parent, so subsequent loop steps skip the lock+scan.
        let still_pending = tasks.values().any(|e| {
            e.parent_session_id == parent_session_id
                && e.background
                && !e.reported
                && e.status != TaskStatus::Running
                && e.waiter_count == 0
        });
        if !still_pending {
            self.has_pending_background.store(false, Ordering::Relaxed);
        }
        completed
    }

    /// P-11: returns `true` when at least one background task has been
    /// spawned and not yet drained. The agent loop uses this to skip the
    /// `drain_completed` call (and its lock acquisition + map scan) on the
    /// common no-background-tasks path.
    #[must_use]
    pub fn has_pending_background(&self) -> bool {
        self.has_pending_background.load(Ordering::Relaxed)
    }

    /// M7-T3: Increments the waiter count for a task **only if the task
    /// exists and is still running**. Returns `true` if the increment
    /// succeeded (task found and running), `false` if the task was already
    /// completed (in which case the caller should collect its result
    /// directly rather than waiting for an event).
    ///
    /// This fixes the previous spurious-increment bug where `increment_waiter`
    /// was called for already-completed tasks, and the subsequent
    /// `decrement_waiter` for those same completed tasks caused
    /// `drain_completed` to inject results prematurely when another waiter
    /// was still waiting.
    #[must_use]
    pub async fn increment_waiter(&self, task_id: &str) -> bool {
        let mut tasks = self.tasks.write().await;
        if let Some(entry) = tasks.get_mut(task_id) {
            if entry.status == TaskStatus::Running {
                entry.waiter_count = entry.waiter_count.saturating_add(1);
                tracing::debug!(
                    task_id,
                    waiter_count = entry.waiter_count,
                    "M7-T3: Incremented waiter count (task still running)"
                );
                return true;
            }
            tracing::debug!(
                task_id,
                status = %entry.status,
                "M7-T3: increment_waiter skipped — task already completed"
            );
            return false;
        }
        tracing::debug!(task_id, "M7-T3: increment_waiter skipped — task not found");
        false
    }

    /// M7-T3: Decrements the waiter count for a task **only if it is > 0**.
    ///
    /// This fixes the previous spurious-decrement bug: the old
    /// `decrement_waiter` blindly decremented for any task ID in the wait
    /// set, including tasks that were already completed (and thus never had
    /// their waiter_count incremented). The spurious decrement could cause
    /// `drain_completed` to inject results prematurely when another waiter
    /// was still waiting. Now, `decrement_waiter` is a no-op if the task
    /// doesn't exist or if `waiter_count == 0`.
    pub async fn decrement_waiter(&self, task_id: &str) {
        let mut tasks = self.tasks.write().await;
        if let Some(entry) = tasks.get_mut(task_id) {
            if entry.waiter_count > 0 {
                entry.waiter_count = entry.waiter_count.saturating_sub(1);
                tracing::debug!(
                    task_id,
                    waiter_count = entry.waiter_count,
                    "M7-T3: Decremented waiter count"
                );
            } else {
                tracing::debug!(
                    task_id,
                    waiter_count = entry.waiter_count,
                    "M7-T3: decrement_waiter skipped — count already 0 (no spurious decrement)"
                );
            }
        }
    }

    /// Internal helper: resolve agent and run through the processor loop.
    async fn run_subagent(
        &self,
        child_session_id: &str,
        agent_name: &str,
        task_prompt: &str,
        model_override: Option<&str>,
        cancel_flag: Arc<AtomicBool>,
        working_dir: &std::path::Path,
    ) -> anyhow::Result<String> {
        let config = crate::Config::default();
        let mut agent = crate::agent::resolve_agent_with_customs(agent_name, &config, working_dir)?;
        agent.mode = AgentMode::Subagent;

        // Apply model override
        if let Some(model_str) = model_override
            && let Some((provider, model_id)) = model_str
                .split_once('/')
                .or_else(|| model_str.split_once(':'))
        {
            agent.model = Some(ModelRef {
                provider_id: provider.to_string(),
                model_id: model_id.to_string(),
            });
        }

        let response_msg = self
            .processor
            .process_message(child_session_id, task_prompt, &agent, cancel_flag)
            .await?;

        Ok(response_msg.text_content())
    }
}

/// Truncate a string to `max_len` characters, appending "…" if truncated.
fn truncate_str(s: &str, max_len: usize) -> String {
    match s.char_indices().nth(max_len) {
        Some((byte_idx, _)) => {
            let mut truncated = s[..byte_idx].to_string();
            truncated.push('…');
            truncated
        }
        None => s.to_string(),
    }
}

/// M7-T4: Typed cancellation detection.
///
/// Instead of checking `error_msg.contains("cancelled")`, we inspect the
/// error's type chain for the processor's cancellation error. This is
/// robust to wording changes in the error message.
///
/// The processor signals cancellation by returning an error whose
/// `to_string()` contains "cancelled" OR whose type name contains
/// "Cancelled" (via `anyhow`'s chain). We check both paths for robustness:
/// - The `anyhow::Error::chain()` lets us inspect each error's type name.
/// - As a fallback, we also check the debug representation, since the
///   processor's `FinishReason::Cancelled` may appear in the error chain.
fn is_cancel_error(err: &anyhow::Error) -> bool {
    // Check the error chain for a "Cancelled" type name or a
    // "cancelled" in any error's Display string.
    for source in err.chain() {
        let type_name = std::any::type_name_of_val(source);
        if type_name.contains("Cancelled") {
            return true;
        }
        let display = source.to_string();
        if display.to_lowercase().contains("cancelled") {
            return true;
        }
    }
    // Also check the top-level error's Display (in case chain() is empty).
    err.to_string().to_lowercase().contains("cancelled")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_task_status_serialization() {
        let status = TaskStatus::Running;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"running\"");

        let status: TaskStatus = serde_json::from_str("\"completed\"").unwrap();
        assert_eq!(status, TaskStatus::Completed);
    }

    #[test]
    fn test_truncate_str_short() {
        assert_eq!(truncate_str("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_str_exact() {
        assert_eq!(truncate_str("hello", 5), "hello");
    }

    #[test]
    fn test_truncate_str_long() {
        let result = truncate_str("hello world", 5);
        assert_eq!(result, "hello…");
    }

    #[test]
    fn test_truncate_str_multibyte_boundary_safe() {
        let s = "café naïve résumé";
        let result = truncate_str(s, 6);
        assert_eq!(result, "café n…");
    }

    #[test]
    fn test_truncate_str_multibyte_not_truncated_when_shorter() {
        let s = "naïve";
        let result = truncate_str(s, 10);
        assert_eq!(result, "naïve");
    }

    #[test]
    fn test_task_entry_serialization() {
        let entry = TaskEntry {
            id: "task-1".to_string(),
            parent_session_id: "parent-1".to_string(),
            child_session_id: "child-1".to_string(),
            agent_name: "explore".to_string(),
            task_prompt: "Find auth code".to_string(),
            background: true,
            status: TaskStatus::Running,
            result: None,
            error: None,
            created_at: Utc::now(),
            completed_at: None,
            reported: false,
            waiter_count: 0,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert!(json.contains("\"explore\""));
        assert!(json.contains("\"running\""));
    }

    // D4 fix: Tests for sanitize_for_id
    #[test]
    fn test_sanitize_for_id_basic() {
        assert_eq!(sanitize_for_id("explore"), "explore");
        assert_eq!(sanitize_for_id("code-review"), "code-review");
    }

    #[test]
    fn test_sanitize_for_id_with_spaces() {
        assert_eq!(sanitize_for_id("Code Review"), "code-review");
        assert_eq!(sanitize_for_id("  spaced  "), "spaced");
    }

    #[test]
    fn test_sanitize_for_id_with_special_chars() {
        assert_eq!(sanitize_for_id("test@agent"), "test-agent");
        assert_eq!(sanitize_for_id("agent.name"), "agent-name");
    }

    #[test]
    fn test_sanitize_for_id_consecutive_specials() {
        assert_eq!(sanitize_for_id("a--b"), "a-b");
        assert_eq!(sanitize_for_id("a---b"), "a-b");
    }

    #[test]
    fn test_sanitize_for_id_trims_leading_trailing() {
        assert_eq!(sanitize_for_id("-leading"), "leading");
        assert_eq!(sanitize_for_id("trailing-"), "trailing");
    }

    #[test]
    fn test_sanitize_for_id_empty_fallback() {
        assert_eq!(sanitize_for_id(""), "task");
        assert_eq!(sanitize_for_id("---"), "task");
    }

    #[test]
    fn test_sanitize_for_id_length_limit() {
        let long = "a".repeat(50);
        let result = sanitize_for_id(&long);
        assert!(result.len() <= 20, "Result should be limited to 20 chars");
    }
}
