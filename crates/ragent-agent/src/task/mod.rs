//! Sub-agent task management for F13 (sub-agent spawning) and F14 (background agents).
//!
//! The [`AgentManager`] tracks spawned sub-agent tasks, supports both synchronous
//! (blocking) and background (non-blocking) execution, and publishes lifecycle
//! events via the [`EventBus`](crate::event::EventBus).
//!
//! # Architecture
//!
//! ```text
//! Parent Session
//!   │
//!   ├─ new_agent(agent: "explore", background: false)  ← blocks until done
//!   │   └─ TaskEntry { status: Completed, result: "..." }
//!   │
//!   └─ new_agent(agent: "build", background: true)     ← returns immediately
//!       └─ TaskEntry { status: Running }
//!           ↓ (later)
//!       └─ SubagentComplete event published
//! ```

use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

use chrono::{DateTime, Utc};
use dashmap::DashMap;
use serde::{Deserialize, Serialize};

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

/// D4 fix: Generate a human-readable task ID based on agent name.
/// e.g., "explore-a1b2c3d4" instead of full UUID.
fn make_task_id(agent_name: &str) -> String {
    format!(
        "{}-{}",
        sanitize_for_id(agent_name),
        uuid::Uuid::new_v4()
            .to_string()
            .split('-')
            .next()
            .unwrap_or("task")
    )
}

/// Build a `TaskEntry` for a newly spawned sub-agent task.
fn build_task_entry(
    task_id: &str,
    parent_session_id: &str,
    child_sid: &str,
    agent_name: &str,
    task_prompt: &str,
    background: bool,
) -> TaskEntry {
    TaskEntry {
        id: task_id.to_string(),
        parent_session_id: parent_session_id.to_string(),
        child_session_id: child_sid.to_string(),
        agent_name: agent_name.to_string(),
        task_prompt: task_prompt.to_string(),
        background,
        status: TaskStatus::Running,
        result: None,
        error: None,
        created_at: Utc::now(),
        completed_at: None,
        reported: false,
        waiter_count: 0,
        output_file: None,
        report_status: ReportStatus::Complete,
    }
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

/// How a completed sub-agent run actually ended, from the perspective of
/// the parent that collects the result. Serialised onto [`TaskEntry`] so
/// wait/list tools can flag results that may be incomplete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReportStatus {
    /// The provider finished the reply naturally (normal case).
    #[default]
    Complete,
    /// The reply was cut but the task layer successfully asked the model to
    /// continue and recovered the remainder.
    Continued,
    /// The reply was cut and no continuation attempt recovered it.
    Truncated,
}

impl ReportStatus {
    /// Stable label for serialisation into tool outputs / event payloads.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Continued => "continued",
            Self::Truncated => "truncated",
        }
    }
}

impl std::fmt::Display for ReportStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
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
    ///
    /// Stored as `Arc<str>` so that cloning a `TaskEntry` (e.g. in
    /// `list_agents`, `drain_completed`) is a cheap pointer bump rather
    /// than a full string copy (FR-006, FR-016).
    pub result: Option<Arc<str>>,
    /// Error message (populated on failure).
    ///
    /// Stored as `Arc<str>` for the same reason as `result`.
    pub error: Option<Arc<str>>,
    /// When the task was created.
    pub created_at: DateTime<Utc>,
    /// When the task completed (if finished).
    pub completed_at: Option<DateTime<Utc>>,
    /// Whether this completion has been injected into the parent session.
    #[serde(default)]
    pub reported: bool,
    /// Number of active waiters for this task (via wait_agents tool).
    /// When > 0, the task result should not be redundantly reported via drain_completed
    /// because a waiter is already handling it.
    #[serde(default)]
    pub waiter_count: u32,
    /// Path to the durable on-disk copy of this agent's FULL untruncated
    /// output (`log/subagents/<id>.md` under the working directory).
    ///
    /// In-memory copies (`result`, `SubagentComplete::summary`) and the
    /// `wait_agents` tool output are all subject to truncation before they
    /// reach the parent model's context; this file is the recovery path —
    /// a sub-agent finding can never be silently lost because it is always
    /// readable from disk via the `read` tool.
    #[serde(default)]
    pub output_file: Option<PathBuf>,
    /// Whether the final reply was delivered intact (`Complete`), was
    /// salvaged by the truncation-continuation retry (`Continued`), or
    /// could not be finished because the provider kept cutting it
    /// (`Truncated`).
    #[serde(default)]
    pub report_status: ReportStatus,
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
/// Thread-safe via interior mutability (`DashMap`). Designed to be shared
/// as `Arc<AgentManager>` across the session processor and tool invocations.
pub struct AgentManager {
    /// Active and completed tasks indexed by task ID.
    ///
    /// PERF (FR-016): `DashMap` replaces `RwLock<HashMap>` to eliminate
    /// reader-writer lock contention on read-heavy paths (`list_agents`,
    /// `running_background_count`, `drain_completed`). Shard-based
    /// concurrent access means readers on one task never block readers on
    /// another.
    tasks: Arc<DashMap<String, TaskEntry>>,
    /// Cancel flags for running tasks.
    ///
    /// PERF (FR-016): `DashMap` replaces `RwLock<HashMap>` for the same
    /// lock-contention reason as `tasks`.
    cancel_flags: Arc<DashMap<String, Arc<AtomicBool>>>,
    /// Event bus for publishing sub-agent lifecycle events.
    event_bus: Arc<EventBus>,
    /// Session processor for running sub-agent loops.
    processor: Arc<SessionProcessor>,
    /// Maximum concurrent background tasks.
    max_background: usize,
    /// Maximum runtime in seconds for a background sub-agent task before it
    /// is forcibly cancelled.
    background_timeout_secs: u64,
    /// P-11: flag set whenever a background task is spawned and cleared by
    /// `drain_completed` when no completed tasks remain to report. The
    /// agent loop checks this flag before calling `drain_completed` so the
    /// common "no background tasks" path avoids acquiring the task-map lock
    /// and scanning every entry on every loop step.
    has_pending_background: AtomicBool,
}

impl AgentManager {
    /// Creates a new task manager.
    pub fn new(
        event_bus: Arc<EventBus>,
        processor: Arc<SessionProcessor>,
        max_background: usize,
        background_timeout_secs: u64,
    ) -> Self {
        Self {
            tasks: Arc::new(DashMap::new()),
            cancel_flags: Arc::new(DashMap::new()),
            event_bus,
            processor,
            max_background,
            background_timeout_secs,
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
        let task_id = make_task_id(agent_name);
        let start = Instant::now();
        // Create isolated session
        let child_session = self
            .processor
            .session_manager
            .create_session(working_dir.to_path_buf())?;
        let child_sid = child_session.id.clone();

        // Register task entry
        let entry = build_task_entry(
            &task_id,
            parent_session_id,
            &child_sid,
            agent_name,
            task_prompt,
            false,
        );
        self.tasks.insert(task_id.clone(), entry);
        // P-11: spawn_sync tasks are not background (they block the caller),
        // so they are never drained by `drain_completed`. We do not set the
        // flag here — only `spawn_background` sets it.

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flags
            .insert(task_id.clone(), cancel_flag.clone());

        // Publish start event
        self.event_bus.publish(Event::SubagentStart {
            session_id: parent_session_id.to_string(),
            task_id: task_id.clone(),
            child_session_id: child_sid.clone(),
            agent: agent_name.to_string(),
            task: truncate_str(task_prompt, 200).into_owned(),
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
                let summary = truncate_str(&response, 2000).into_owned();
                {
                    if let Some(mut entry) = self.tasks.get_mut(&task_id) {
                        entry.status = TaskStatus::Completed;
                        entry.result = Some(Arc::from(response.as_str()));
                        entry.completed_at = Some(Utc::now());
                    }
                }
                self.cancel_flags.remove(&task_id);
                self.event_bus.publish(Event::SubagentComplete {
                    session_id: parent_session_id.to_string(),
                    task_id: task_id.clone(),
                    child_session_id: child_sid.clone(),
                    summary,
                    success: true,
                    duration_ms,
                    finish_reason: "stop".to_string(),
                });

                let entry = self
                    .tasks
                    .get(&task_id)
                    .map(|r| r.value().clone())
                    .ok_or_else(|| anyhow::anyhow!("task {task_id} vanished after completion"))?;
                Ok(TaskResult { entry, response })
            }
            Err(e) => {
                let error_msg = e.to_string();
                {
                    if let Some(mut entry) = self.tasks.get_mut(&task_id) {
                        entry.status = TaskStatus::Failed;
                        entry.error = Some(Arc::from(error_msg.as_str()));
                        entry.completed_at = Some(Utc::now());
                    }
                }
                self.cancel_flags.remove(&task_id);
                self.event_bus.publish(Event::SubagentComplete {
                    session_id: parent_session_id.to_string(),
                    task_id: task_id.clone(),
                    child_session_id: child_sid,
                    summary: format!("Error: {error_msg}"),
                    success: false,
                    duration_ms,
                    finish_reason: "error".to_string(),
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
            .iter()
            .filter(|r| r.status == TaskStatus::Running && r.background)
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
        let task_id = make_task_id(agent_name);

        // Create isolated session
        let child_session = self
            .processor
            .session_manager
            .create_session(working_dir.to_path_buf())?;
        let child_sid = child_session.id.clone();

        // Register task entry
        let entry = build_task_entry(
            &task_id,
            parent_session_id,
            &child_sid,
            agent_name,
            task_prompt,
            true,
        );
        self.tasks.insert(task_id.clone(), entry.clone());
        // P-11: mark that there is at least one pending background task so
        // the agent loop's `drain_completed` call is not skipped.
        self.has_pending_background.store(true, Ordering::Relaxed);

        let cancel_flag = Arc::new(AtomicBool::new(false));
        self.cancel_flags
            .insert(task_id.clone(), cancel_flag.clone());

        // Publish start event
        self.event_bus.publish(Event::SubagentStart {
            session_id: parent_session_id.to_string(),
            task_id: task_id.clone(),
            child_session_id: child_sid.clone(),
            agent: agent_name.to_string(),
            task: truncate_str(task_prompt, 200).into_owned(),
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
        let background_timeout_secs = self.background_timeout_secs;
        let tid = task_id.clone();
        let csid = child_sid.clone();
        let working_dir_buf = working_dir.to_path_buf();
        let cancel_flag_outer = cancel_flag.clone();

        tokio::spawn(async move {
            let start = Instant::now();

            // Run the sub-agent logic in a nested task so that panics inside
            // process_message (or agent resolution) are caught as a JoinError
            // rather than silently aborting this background task and leaving
            // the parent wait_agents call stalled forever.
            let csid_inner = csid.clone();
            let inner = tokio::spawn(async move {
                let config = processor.load_config_cached();
                let mut agent_info = match crate::agent::resolve_agent_with_customs_and_model(
                    &agent,
                    &config,
                    &working_dir_buf,
                    &processor.provider_registry,
                ) {
                    Ok(a) => Arc::unwrap_or_clone(a),
                    Err(e) => return Err(e),
                };
                agent_info.mode = AgentMode::Subagent;
                agent_info.stall_timeout_secs = Some(background_timeout_secs);

                // Apply explicit model override if provided.
                if let Some(ref model_str) = model
                    && let Some((provider, model_id)) = model_str
                        .split_once('/')
                        .or_else(|| model_str.split_once(':'))
                {
                    agent_info.model = Some(ModelRef {
                        provider_id: provider.to_string(),
                        model_id: model_id.to_string(),
                    });
                } else if !agent_info.model_pinned || agent_info.model.is_none() {
                    // No explicit override: fall back to the user's persisted
                    // `selected_model` setting (same path the TUI uses via
                    // `apply_selected_model_and_thinking`). Without this, the
                    // agent would get `resolve_default_model`'s first-provider
                    // pick (Anthropic), which typically has no API key configured.
                    if let Ok(Some(model_str)) = processor
                        .session_manager
                        .storage()
                        .get_setting("selected_model")
                    {
                        if let Some((provider, model_id)) = model_str
                            .split_once('/')
                            .or_else(|| model_str.split_once(':'))
                        {
                            tracing::info!(
                                agent = %agent_info.name,
                                selected_model = %model_str,
                                "Applied persisted selected_model to background agent"
                            );
                            agent_info.model = Some(ModelRef {
                                provider_id: provider.to_string(),
                                model_id: model_id.to_string(),
                            });
                        }
                    }
                }

                processor
                    .process_message(&csid_inner, &prompt, &agent_info, cancel_flag.clone())
                    .await
                    .map(|msg| msg.text_content())
            });

            let abort_handle = inner.abort_handle();
            let result = match tokio::time::timeout(
                tokio::time::Duration::from_secs(background_timeout_secs),
                inner,
            )
            .await
            {
                Ok(Ok(Ok(response))) => Ok(response),
                Ok(Ok(Err(e))) => Err(e),
                Ok(Err(join_err)) => {
                    if join_err.is_panic() {
                        Err(anyhow::anyhow!("sub-agent task panicked"))
                    } else if join_err.is_cancelled() {
                        Err(anyhow::anyhow!("sub-agent task cancelled"))
                    } else {
                        Err(anyhow::anyhow!("sub-agent task aborted: {join_err}"))
                    }
                }
                Err(_) => {
                    abort_handle.abort();
                    cancel_flag_outer.store(true, Ordering::Relaxed);
                    Err(anyhow::anyhow!(
                        "background agent timed out after {background_timeout_secs}s"
                    ))
                }
            };

            let duration_ms = start.elapsed().as_millis() as u64;

            match result {
                Ok(response) => {
                    let summary = truncate_str(&response, 2000).into_owned();
                    {
                        if let Some(mut entry) = tasks.get_mut(&tid) {
                            entry.status = TaskStatus::Completed;
                            entry.result = Some(Arc::from(response.as_str()));
                            entry.completed_at = Some(Utc::now());
                        }
                    }
                    cancel_flags.remove(&tid);
                    event_bus.publish(Event::SubagentComplete {
                        session_id: parent_sid,
                        task_id: tid,
                        child_session_id: csid,
                        summary,
                        success: true,
                        duration_ms,
                        finish_reason: "stop".to_string(),
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
                        if let Some(mut entry) = tasks.get_mut(&tid) {
                            if is_cancelled {
                                entry.status = TaskStatus::Cancelled;
                            } else {
                                entry.status = TaskStatus::Failed;
                                entry.error = Some(Arc::from(error_msg.as_str()));
                            }
                            entry.completed_at = Some(Utc::now());
                        }
                    }
                    cancel_flags.remove(&tid);

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
                            finish_reason: "error".to_string(),
                        });
                    }
                }
            }
        });

        Ok(entry)
    }

    /// Cancels a running task by setting its cancel flag.
    pub async fn cancel_agent(&self, task_id: &str) -> anyhow::Result<()> {
        // PERF (FR-016): DashMap — get returns a short-lived shard guard.
        if let Some(flag) = self.cancel_flags.get(task_id) {
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
    /// these methods should use `cancel_agent` instead.
    ///
    /// See `docs/team-unification-decision.md` for the rationale.
    pub async fn suspend_task(&self, task_id: &str) -> anyhow::Result<()> {
        // M7-T1: Suspend is not implemented in the processor agent loop.
        // The suspend_flags map exists but is never checked by the loop.
        // Rather than mislead callers, we return an explicit error.
        let _ = task_id;
        anyhow::bail!(
            "suspend_task is not implemented — the agent loop does not honour \
               suspend flags. Use cancel_agent to stop a running sub-agent instead."
        )
    }

    /// M7-T1: Resume a suspended task — not implemented (see `suspend_task`).
    pub async fn resume_task(&self, task_id: &str) -> anyhow::Result<()> {
        let _ = task_id;
        anyhow::bail!(
            "resume_task is not implemented — the agent loop does not honour \
               suspend flags. Use cancel_agent and re-spawn instead."
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
        // PERF (FR-016): DashMap — get_mut returns a short-lived shard guard.
        let mut entry = self
            .tasks
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
        drop(entry);

        // PERF (FR-016): DashMap — no async write guard needed; the cancel
        // flag is set atomically regardless of shard contention.
        {
            if let Some(cf) = self.cancel_flags.get(task_id) {
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
            // PERF (FR-016): DashMap — get_mut returns a short-lived guard.
            if let Some(mut entry) = tasks2.get_mut(&tid) {
                if entry.status == TaskStatus::Terminating {
                    entry.status = TaskStatus::Failed;
                    entry.error = Some(Arc::from("Force-killed after timeout"));
                    entry.completed_at = Some(Utc::now());
                }
            }
            flags2.remove(&tid);
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
        self.tasks.get(task_id).map(|r| r.value().clone())
    }

    /// Returns all tasks for a given parent session.
    pub async fn list_agents(&self, parent_session_id: &str) -> Vec<TaskEntry> {
        self.tasks
            .iter()
            .filter(|r| r.parent_session_id == parent_session_id)
            .map(|r| r.value().clone())
            .collect()
    }

    /// Returns the count of currently running background tasks.
    pub async fn running_background_count(&self) -> usize {
        self.tasks
            .iter()
            .filter(|r| r.status == TaskStatus::Running && r.background)
            .count()
    }

    /// Cancels all running tasks for a given parent session.
    pub async fn cancel_all(&self, parent_session_id: &str) {
        // PERF (FR-016): DashMap — iterate tasks; for each running task,
        // look up its cancel flag by task ID. No global read lock held
        // across both maps.
        for entry in self.tasks.iter() {
            if entry.parent_session_id == parent_session_id
                && entry.status == TaskStatus::Running
                && let Some(flag) = self.cancel_flags.get(entry.key())
            {
                flag.store(true, Ordering::Relaxed);
                tracing::info!(
                    task_id = entry.key(),
                    "Cancelling sub-agent task (session cleanup)"
                );
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
    /// actively waited on via wait_agents tool and should not be redundantly
    /// injected into the conversation.
    pub async fn drain_completed(&self, parent_session_id: &str) -> Vec<TaskEntry> {
        // P-11: fast path — if no background tasks have ever been spawned,
        // skip the lock acquisition and map scan entirely.
        if !self.has_pending_background.load(Ordering::Relaxed) {
            return Vec::new();
        }
        // PERF (FR-016): DashMap — iter_mut yields short-lived RefMut
        // guards one shard at a time. No global write lock held across
        // the entire scan.
        let mut completed = Vec::new();
        for mut entry in self.tasks.iter_mut() {
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
        let still_pending = self.tasks.iter().any(|e| {
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
        // PERF (FR-016): DashMap — get_mut returns a short-lived shard guard.
        if let Some(mut entry) = self.tasks.get_mut(task_id) {
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
        // PERF (FR-016): DashMap — get_mut returns a short-lived shard guard.
        if let Some(mut entry) = self.tasks.get_mut(task_id) {
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
        let config = self.processor.load_config_cached();
        let agent = crate::agent::resolve_agent_with_customs_and_model(
            agent_name,
            &config,
            working_dir,
            &self.processor.provider_registry,
        )?;
        let mut agent = Arc::unwrap_or_clone(agent);
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
        } else if !agent.model_pinned || agent.model.is_none() {
            // No explicit override: fall back to the user's persisted
            // `selected_model` setting so sub-agents use the same provider
            // the user configured in the TUI.
            if let Ok(Some(model_str)) = self
                .processor
                .session_manager
                .storage()
                .get_setting("selected_model")
            {
                if let Some((provider, model_id)) = model_str
                    .split_once('/')
                    .or_else(|| model_str.split_once(':'))
                {
                    tracing::info!(
                        agent = %agent.name,
                        selected_model = %model_str,
                        "Applied persisted selected_model to sub-agent"
                    );
                    agent.model = Some(ModelRef {
                        provider_id: provider.to_string(),
                        model_id: model_id.to_string(),
                    });
                }
            }
        }

        let response_msg = self
            .processor
            .process_message(child_session_id, task_prompt, &agent, cancel_flag)
            .await?;

        Ok(response_msg.text_content())
    }

    /// Test-only helper: insert an already-completed task entry directly into
    /// the task map without spawning a real sub-agent.
    #[doc(hidden)]
    pub async fn seed_completed_for_test(&self, entry: TaskEntry) {
        self.tasks.insert(entry.id.clone(), entry);
    }
}

/// Truncate a string to `max_len` characters, appending "…" if truncated.
///
/// Returns a borrowed slice when no truncation is needed, avoiding an
/// allocation on the common no-op path.
fn truncate_str(s: &str, max_len: usize) -> std::borrow::Cow<'_, str> {
    match s.char_indices().nth(max_len) {
        Some((byte_idx, _)) => {
            let mut truncated = String::from(&s[..byte_idx]);
            truncated.push('\u{2026}');
            std::borrow::Cow::Owned(truncated)
        }
        None => std::borrow::Cow::Borrowed(s),
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
    // "cancelled" in any error's Display string. `chain()` includes
    // the top-level error itself, so the extra top-level check below
    // is unnecessary.
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
    false
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
        assert_eq!(truncate_str("hello", 10).as_ref(), "hello");
    }

    #[test]
    fn test_truncate_str_exact() {
        assert_eq!(truncate_str("hello", 5).as_ref(), "hello");
    }

    #[test]
    fn test_truncate_str_long() {
        let result = truncate_str("hello world", 5);
        assert_eq!(result.as_ref(), "hello\u{2026}");
    }

    #[test]
    fn test_truncate_str_multibyte_boundary_safe() {
        let s = "caf\u{e9} na\u{ef}ve r\u{e9}sum\u{e9}";
        let result = truncate_str(s, 6);
        assert_eq!(result.as_ref(), "caf\u{e9} n\u{2026}");
    }

    #[test]
    fn test_truncate_str_multibyte_not_truncated_when_shorter() {
        let s = "na\u{ef}ve";
        let result = truncate_str(s, 10);
        assert_eq!(result.as_ref(), "na\u{ef}ve");
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
            output_file: None,
            report_status: ReportStatus::Complete,
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

    /// The `SubagentComplete` event `summary` remains truncated to 2000
    /// chars for TUI display — this is separate from the full result.
    #[test]
    fn test_event_summary_is_short() {
        let long = "z".repeat(10_000);
        let summary = truncate_str(&long, 2000).into_owned();
        assert!(summary.len() < long.len());
        assert!(summary.ends_with('…'));
    }
}
