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
pub const DEFAULT_MAX_BACKGROUND_TASKS: usize = 4;

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
    /// Task was cancelled before completion.
    Cancelled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
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

            let config = crate::config::Config::default();
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
                    let error_msg = e.to_string();
                    let cancelled = error_msg.contains("cancelled");
                    {
                        let mut t = tasks.write().await;
                        if let Some(entry) = t.get_mut(&tid) {
                            if cancelled {
                                entry.status = TaskStatus::Cancelled;
                            } else {
                                entry.status = TaskStatus::Failed;
                                entry.error = Some(error_msg.clone());
                            }
                            entry.completed_at = Some(Utc::now());
                        }
                    }
                    cancel_flags.write().await.remove(&tid);

                    if cancelled {
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
        let mut tasks = self.tasks.write().await;
        let mut completed = Vec::new();
        for entry in tasks.values_mut() {
            if entry.parent_session_id == parent_session_id
                && entry.background
                && !entry.reported
                && entry.status != TaskStatus::Running
                && entry.waiter_count == 0
            // Skip tasks with active waiters
            {
                entry.reported = true;
                completed.push(entry.clone());
            }
        }
        completed
    }

    /// Increments the waiter count for a task (called when wait_tasks starts waiting).
    pub async fn increment_waiter(&self, task_id: &str) {
        let mut tasks = self.tasks.write().await;
        if let Some(entry) = tasks.get_mut(task_id) {
            entry.waiter_count = entry.waiter_count.saturating_add(1);
            tracing::debug!(
                task_id,
                waiter_count = entry.waiter_count,
                "Incremented waiter count"
            );
        }
    }

    /// Decrements the waiter count for a task (called when wait_tasks completes).
    pub async fn decrement_waiter(&self, task_id: &str) {
        let mut tasks = self.tasks.write().await;
        if let Some(entry) = tasks.get_mut(task_id) {
            entry.waiter_count = entry.waiter_count.saturating_sub(1);
            tracing::debug!(
                task_id,
                waiter_count = entry.waiter_count,
                "Decremented waiter count"
            );
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
        let config = crate::config::Config::default();
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
