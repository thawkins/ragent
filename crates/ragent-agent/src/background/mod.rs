//! Background task service for the `bg` tool (M3).
//!
//! [`BackgroundTaskService`] owns a map of running [`BackgroundCommand`]s and
//! persists their lifecycle to `ragent_storage::Storage`. It also publishes
//! session-scoped events via the [`EventBus`] so the TUI and HTTP consumers
//! can show live progress.
//!
//! ## Wake/notify hooks (T-023)
//!
//! When a background task finishes, the service records the completion in an
//! in-memory queue and calls [`tokio::sync::Notify::notify_one`] on a shared
//! [`Notify`] handle. The session processor (or any external waiter such as the
//! TUI/CLI idle loop) can await [`BackgroundTaskService::completion_notify`] to
//! resume the session the moment a long-running task completes. Completed tasks
//! are drained via [`BackgroundTaskService::drain_completed`] and injected into
//! the agent conversation, mirroring the sub-agent `task_manager` pattern.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use chrono::Utc;
use ragent_storage::storage::{BackgroundTaskRow, Storage};
use ragent_tools_core::bg::BackgroundCommand;
use ragent_types::event::{Event, EventBus};
use tokio::sync::Notify;
use tracing::{debug, warn};
use uuid::Uuid;

/// Default output buffer flush interval in seconds.
const FLUSH_INTERVAL_SECS: u64 = 2;

/// Number of tail lines retained for completion injection.
const COMPLETION_TAIL_LINES: usize = 20;

/// A completed background task surfaced to the session processor for injection
/// into the agent conversation.
#[derive(Debug, Clone)]
pub struct CompletedBgTask {
    /// Unique task identifier.
    pub task_id: String,
    /// Session that owns this task.
    pub session_id: String,
    /// Shell command that was executed.
    pub command: String,
    /// Final status: `completed`, `failed`, or `cancelled`.
    pub status: String,
    /// Exit code, if the process exited normally.
    pub exit_code: Option<i32>,
    /// Tail of the combined stdout/stderr output.
    pub tail: String,
}

/// Shared completion sink: a queue of finished task ids plus a [`Notify`] used
/// to wake external waiters (TUI idle loop, CLI runner). Owned by
/// [`BackgroundTaskService`] and cloned into each `flush_task` so the
/// background flush coroutine can record completions without holding a
/// reference to the full service.
struct CompletionSink {
    queue: Mutex<VecDeque<String>>,
    notify: Arc<Notify>,
    has_pending: AtomicBool,
}

impl CompletionSink {
    fn record(&self, task_id: &str) {
        self.queue
            .lock()
            .expect("background completion queue poisoned")
            .push_back(task_id.to_string());
        self.has_pending.store(true, Ordering::Relaxed);
        self.notify.notify_one();
        debug!(task_id = %task_id, "Background completion recorded and notify fired");
    }
}

/// Shared background task manager.
pub struct BackgroundTaskService {
    storage: Arc<Storage>,
    event_bus: Arc<EventBus>,
    /// In-memory handles keyed by task id.
    tasks: Mutex<HashMap<String, BackgroundCommand>>,
    /// Session id associated with each spawned task (for drain filtering).
    sessions: Mutex<HashMap<String, String>>,
    /// Shared completion queue + notify, also handed to `flush_task`.
    completion_sink: Arc<CompletionSink>,
    /// Task ids that have already been surfaced via `drain_completed`, so the
    /// in-memory done-scan does not re-surface them on the next drain.
    drained_ids: Mutex<HashSet<String>>,
}

impl BackgroundTaskService {
    /// Create a new service bound to `storage` and `event_bus`.
    pub fn new(storage: Arc<Storage>, event_bus: Arc<EventBus>) -> Self {
        let notify = Arc::new(Notify::new());
        Self {
            storage,
            event_bus,
            tasks: Mutex::new(HashMap::new()),
            sessions: Mutex::new(HashMap::new()),
            completion_sink: Arc::new(CompletionSink {
                queue: Mutex::new(VecDeque::new()),
                notify,
                has_pending: AtomicBool::new(false),
            }),
            drained_ids: Mutex::new(HashSet::new()),
        }
    }

    /// Return a shared [`Notify`] that is signalled whenever a background task
    /// completes. External waiters (TUI idle loop, CLI runner, HTTP handler)
    /// can `await` this to resume the session the moment a long-running task
    /// finishes.
    #[must_use]
    pub fn completion_notify(&self) -> Arc<Notify> {
        Arc::clone(&self.completion_sink.notify)
    }

    /// Fast-path check: returns `true` when at least one completed background
    /// task is queued and not yet drained into the conversation.
    #[must_use]
    pub fn has_pending_completions(&self) -> bool {
        self.completion_sink.has_pending.load(Ordering::Relaxed)
    }

    /// Spawn a background shell command for `session_id`.
    ///
    /// The command is validated and executed in `working_dir`. A row is
    /// inserted immediately; a background flush task keeps the stored
    /// stdout/stderr/progress up to date.
    pub async fn spawn(
        &self,
        session_id: &str,
        command: &str,
        working_dir: &PathBuf,
    ) -> Result<String> {
        let task_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        let row = BackgroundTaskRow {
            id: task_id.clone(),
            session_id: session_id.to_string(),
            command: command.to_string(),
            status: "running".to_string(),
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            progress_json: "{}".to_string(),
            created_at: now.clone(),
            updated_at: now.clone(),
            completed_at: None,
        };

        let storage = Arc::clone(&self.storage);
        Storage::write_async(storage, move |s| s.create_background_task(&row)).await?;

        let cmd = BackgroundCommand::spawn(
            task_id.clone(),
            command.to_string(),
            working_dir.clone(),
            Some(Arc::clone(&self.event_bus)),
        )
        .await?;

        self.tasks
            .lock()
            .expect("background task map poisoned")
            .insert(task_id.clone(), cmd.clone());
        self.sessions
            .lock()
            .expect("background session map poisoned")
            .insert(task_id.clone(), session_id.to_string());

        self.event_bus.publish(Event::BackgroundTaskSpawned {
            session_id: session_id.to_string(),
            task_id: task_id.clone(),
            command: command.to_string(),
        });

        let flush_storage = Arc::clone(&self.storage);
        let sink = Arc::clone(&self.completion_sink);
        tokio::spawn(Self::flush_task(cmd.clone(), flush_storage, sink));

        debug!(
            task_id = %task_id,
            session_id = %session_id,
            command = %command,
            "Background task spawned"
        );
        Ok(task_id)
    }

    /// List tasks, optionally filtering by session id and/or status.
    pub async fn list(
        &self,
        session_id: Option<&str>,
        status: Option<&str>,
        limit: usize,
    ) -> Result<Vec<BackgroundTaskRow>> {
        let session_id = session_id.map(std::string::ToString::to_string);
        let status = status.map(std::string::ToString::to_string);
        let storage = Arc::clone(&self.storage);
        let rows = Storage::write_async(storage, move |s| {
            s.list_background_tasks(session_id.as_deref(), status.as_deref(), limit)
        })
        .await?;
        Ok(rows)
    }

    /// Return a single task row, overlaying in-memory state when present.
    pub async fn status(&self, task_id: &str) -> Result<BackgroundTaskRow> {
        let task_id_owned = task_id.to_string();
        let storage = Arc::clone(&self.storage);
        let mut row = Storage::write_async(storage, move |s| s.get_background_task(&task_id_owned))
            .await?
            .with_context(|| format!("Background task not found: {task_id}"))?;

        if let Some(cmd) = self
            .tasks
            .lock()
            .expect("background task map poisoned")
            .get(task_id)
        {
            row.status = cmd.status();
            row.exit_code = cmd.exit_code().map(i64::from);
            let (stdout, stderr) = cmd.output();
            row.stdout = stdout;
            row.stderr = stderr;
            row.progress_json = cmd.progress().to_string();
        }
        Ok(row)
    }

    /// Return the full stdout and stderr for a task.
    pub async fn output(&self, task_id: &str) -> Result<(BackgroundTaskRow, String, String)> {
        let row = self.status(task_id).await?;
        Ok((row.clone(), row.stdout, row.stderr))
    }

    /// Return the last `n` lines of output for a task.
    pub async fn tail(&self, task_id: &str, n: usize) -> Result<String> {
        let row = self.status(task_id).await?;
        if let Some(cmd) = self
            .tasks
            .lock()
            .expect("background task map poisoned")
            .get(task_id)
        {
            Ok(cmd.tail(n))
        } else {
            let combined = format!("{}{}", row.stdout, row.stderr);
            let lines: Vec<&str> = combined.lines().collect();
            let start = lines.len().saturating_sub(n);
            Ok(lines[start..].join("\n"))
        }
    }

    /// Cancel a running background task.
    pub async fn cancel(&self, task_id: &str) -> Result<()> {
        let cmd = {
            let tasks = self.tasks.lock().expect("background task map poisoned");
            tasks.get(task_id).cloned()
        };
        let Some(cmd) = cmd else {
            bail!("Background task is not running: {task_id}");
        };

        cmd.cancel().await?;
        let task_id_owned = task_id.to_string();
        let storage = Arc::clone(&self.storage);
        let now = Utc::now().to_rfc3339();
        Storage::write_async(storage, move |s| {
            s.update_background_task_status(&task_id_owned, "cancelled", None, Some(&now))
        })
        .await?;
        self.event_bus.publish(Event::BackgroundTaskUpdated {
            session_id: "background".to_string(),
            task_id: task_id.to_string(),
            status: "cancelled".to_string(),
            progress: None,
        });
        Ok(())
    }

    /// Wait for a task to finish, up to `timeout_secs`.
    pub async fn wait(&self, task_id: &str, timeout_secs: u64) -> Result<BackgroundTaskRow> {
        let cmd = {
            let tasks = self.tasks.lock().expect("background task map poisoned");
            tasks.get(task_id).cloned()
        };
        if let Some(cmd) = cmd {
            cmd.wait(timeout_secs).await?;
        }
        self.status(task_id).await
    }

    /// Remove completed/failed/cancelled tasks older than `older_than_minutes`.
    pub async fn cleanup(
        &self,
        session_id: Option<&str>,
        older_than_minutes: i64,
        completed_only: bool,
    ) -> Result<usize> {
        let session_id = session_id.map(std::string::ToString::to_string);
        let storage = Arc::clone(&self.storage);
        let count = Storage::write_async(storage, move |s| {
            s.cleanup_background_tasks(session_id.as_deref(), older_than_minutes, completed_only)
        })
        .await?;

        // Drop in-memory handles and session mappings for finished tasks.
        let done_ids: Vec<String> = {
            let tasks = self.tasks.lock().expect("background task map poisoned");
            tasks
                .iter()
                .filter(|(_, cmd)| cmd.is_done())
                .map(|(id, _)| id.clone())
                .collect()
        };
        let mut tasks = self.tasks.lock().expect("background task map poisoned");
        let mut sessions = self
            .sessions
            .lock()
            .expect("background session map poisoned");
        for id in &done_ids {
            tasks.remove(id);
            sessions.remove(id);
        }

        Ok(count)
    }

    /// Drain completed background tasks for `session_id` that have not yet been
    /// injected into the conversation.
    ///
    /// Returns one [`CompletedBgTask`] per finished task, in completion order.
    /// After draining, the task is removed from the completion queue (but its
    /// row remains in storage for history). The session processor calls this
    /// between agent-loop steps to surface finished work to the model.
    ///
    /// In addition to the explicit completion queue (fed by `flush_task`), this
    /// also scans the in-memory command handles for any task that has finished
    /// but whose completion record has not yet been queued — this closes the
    /// race between `wait()` returning (`done == true`) and `flush_task`
    /// recording the completion.
    pub async fn drain_completed(&self, session_id: &str) -> Vec<CompletedBgTask> {
        if !self.has_pending_completions() && !self.has_done_in_memory(session_id) {
            return Vec::new();
        }

        // Candidate ids from the explicit completion queue.
        let mut candidate_ids: Vec<String> = {
            let mut queue = self
                .completion_sink
                .queue
                .lock()
                .expect("background completion queue poisoned");
            let mut drained = Vec::new();
            while let Some(id) = queue.pop_front() {
                drained.push(id);
            }
            drained
        };

        // Also scan in-memory handles for done tasks belonging to this session
        // that are not already candidates and have not already been drained.
        // This closes the flush_task race.
        {
            let tasks = self.tasks.lock().expect("background task map poisoned");
            let sessions = self
                .sessions
                .lock()
                .expect("background session map poisoned");
            let drained = self
                .drained_ids
                .lock()
                .expect("background drained set poisoned");
            for (id, cmd) in tasks.iter() {
                if cmd.is_done()
                    && sessions.get(id).map(String::as_str) == Some(session_id)
                    && !candidate_ids.iter().any(|c| c == id)
                    && !drained.contains(id)
                {
                    candidate_ids.push(id.clone());
                }
            }
        }

        let mut results = Vec::new();
        for id in &candidate_ids {
            // Only surface tasks belonging to this session.
            let belongs = {
                let sessions = self
                    .sessions
                    .lock()
                    .expect("background session map poisoned");
                sessions.get(id).map(String::as_str) == Some(session_id)
            };
            if !belongs {
                // Re-queue for the correct session's next drain.
                self.completion_sink
                    .queue
                    .lock()
                    .expect("background completion queue poisoned")
                    .push_back(id.clone());
                continue;
            }
            if let Ok(row) = self.status(id).await {
                let tail = {
                    let tasks = self.tasks.lock().expect("background task map poisoned");
                    tasks
                        .get(id)
                        .map(|cmd| cmd.tail(COMPLETION_TAIL_LINES))
                        .unwrap_or_else(|| {
                            let combined = format!("{}{}", row.stdout, row.stderr);
                            let lines: Vec<&str> = combined.lines().collect();
                            let start = lines.len().saturating_sub(COMPLETION_TAIL_LINES);
                            lines[start..].join("\n")
                        })
                };
                results.push(CompletedBgTask {
                    task_id: row.id.clone(),
                    session_id: row.session_id,
                    command: row.command,
                    status: row.status,
                    exit_code: row.exit_code.map(|c| c as i32),
                    tail,
                });
                // Mark as drained so the next drain does not re-surface it.
                self.drained_ids
                    .lock()
                    .expect("background drained set poisoned")
                    .insert(row.id);
            }
        }

        // Update the fast-path flag.
        let remaining = self
            .completion_sink
            .queue
            .lock()
            .expect("background completion queue poisoned")
            .len();
        let any_done = self.has_done_in_memory(session_id);
        self.completion_sink
            .has_pending
            .store(remaining > 0 || any_done, Ordering::Relaxed);

        results
    }

    /// Returns `true` if any in-memory command handle for `session_id` is done
    /// and has not already been drained.
    fn has_done_in_memory(&self, session_id: &str) -> bool {
        let tasks = self.tasks.lock().expect("background task map poisoned");
        let sessions = self
            .sessions
            .lock()
            .expect("background session map poisoned");
        let drained = self
            .drained_ids
            .lock()
            .expect("background drained set poisoned");
        tasks.iter().any(|(id, cmd)| {
            cmd.is_done()
                && sessions.get(id).map(String::as_str) == Some(session_id)
                && !drained.contains(id)
        })
    }

    /// Flush the current stdout/stderr/progress of a running task to storage.
    async fn flush_task(cmd: BackgroundCommand, storage: Arc<Storage>, sink: Arc<CompletionSink>) {
        let task_id = cmd.id().to_string();
        while !cmd.is_done() {
            tokio::time::sleep(tokio::time::Duration::from_secs(FLUSH_INTERVAL_SECS)).await;
            let (stdout, stderr) = cmd.output();
            let progress = cmd.progress();
            let progress_json = progress.to_string();
            let task_id_flush = task_id.clone();
            if let Err(e) = Storage::write_async(Arc::clone(&storage), move |s| {
                s.set_background_task_output(&task_id_flush, &stdout, &stderr, &progress_json)
            })
            .await
            {
                warn!(task_id = %task_id, error = %e, "Failed to flush background task output");
            }
        }

        // Final flush once the task finishes.
        let (stdout, stderr) = cmd.output();
        let progress = cmd.progress();
        let status = cmd.status();
        let exit_code = cmd.exit_code().map(i64::from);
        let completed_at = if cmd.is_done() {
            Some(Utc::now().to_rfc3339())
        } else {
            None
        };
        let task_id_final = task_id.clone();
        let progress_json = progress.to_string();
        if let Err(e) = Storage::write_async(Arc::clone(&storage), move |s| {
            s.set_background_task_output(&task_id_final, &stdout, &stderr, &progress_json)
        })
        .await
        {
            warn!(task_id = %task_id, error = %e, "Failed to flush final background task output");
        }
        let task_id_status = task_id.clone();
        if let Err(e) = Storage::write_async(Arc::clone(&storage), move |s| {
            s.update_background_task_status(
                &task_id_status,
                &status,
                exit_code,
                completed_at.as_deref(),
            )
        })
        .await
        {
            warn!(task_id = %task_id, error = %e, "Failed to update final background task status");
        }

        // T-023: record the completion so the session processor (and any
        // external waiter on `completion_notify`) can resume and inject the
        // result into the conversation.
        sink.record(&task_id);
    }
}
