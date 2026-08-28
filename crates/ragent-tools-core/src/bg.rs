//! Background shell command runner for the `bg` tool (M3).
//!
//! [`BackgroundCommand`] spawns a shell command in the background, captures
//! stdout/stderr incrementally, parses `JCODE_PROGRESS` lines, and exposes
//! `status`, `output`, `tail`, and `cancel` operations. Persistence and task
//! lifecycle management live in `ragent_agent::background`.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::{Context, Result, bail};
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStderr, ChildStdout};
use tokio::sync::Notify;
use tracing::{debug, trace, warn};

use crate::bash;
use crate::event::{Event, EventBus};

/// Default maximum number of bytes retained in the in-memory output buffers.
const MAX_BUFFER_BYTES: usize = 1_000_000;

/// Shared mutable state for a running background command.
struct Inner {
    /// Process handle; replaced with `None` once the child has been awaited.
    child: Option<Child>,
    /// Captured stdout so far.
    stdout: String,
    /// Captured stderr so far.
    stderr: String,
    /// Merged JSON object built from `JCODE_PROGRESS` lines.
    progress: Value,
    /// Current lifecycle status.
    status: String,
    /// Exit code, when known.
    exit_code: Option<i32>,
    /// Whether the task was explicitly cancelled by the user.
    cancelled: bool,
    /// Set to `true` when the reader/waiter tasks finish.
    done: bool,
    /// Total bytes dropped from the combined buffers due to the size cap.
    bytes_dropped: usize,
    /// R-17: Notified by `waiter_task` when `done` is set so `cancel()` can
    /// block on it instead of polling every 50 ms.
    done_notify: Arc<Notify>,
    /// R-9: Notified by `cancel()` to wake the `waiter_task` out of
    /// `child.wait().await` so it can kill the process.
    cancel_notify: Arc<Notify>,
}

/// A handle to a command running in the background.
///
/// Cloning the handle is cheap (it shares the same `Arc<Mutex<Inner>>`). All
/// methods are synchronous reads except for [`BackgroundCommand::cancel`] and
/// [`BackgroundCommand::wait`], which interact with the underlying process.
#[derive(Clone)]
pub struct BackgroundCommand {
    id: String,
    command: String,
    inner: Arc<Mutex<Inner>>,
}

impl BackgroundCommand {
    /// Lock the shared inner state, returning a tool error if the mutex was
    /// poisoned rather than panicking.
    fn lock_inner(&self) -> anyhow::Result<std::sync::MutexGuard<'_, Inner>> {
        self.inner
            .lock()
            .map_err(|_| anyhow::anyhow!("background inner lock poisoned"))
    }

    /// Spawn a new background shell command.
    ///
    /// `id` is the caller-assigned task identifier (usually a UUID). `command`
    /// is run through the same shell discovery and security checks as the
    /// [`bash`](crate::bash) tool. `session_id` identifies the owning session so
    /// lifecycle events can be routed to the correct TUI panel.
    ///
    /// # Errors
    ///
    /// Returns an error if the command fails security validation or the child
    /// process cannot be spawned.
    pub async fn spawn(
        id: String,
        command: String,
        working_dir: PathBuf,
        event_bus: Option<Arc<EventBus>>,
        session_id: String,
    ) -> Result<Self> {
        if !working_dir.exists() {
            bail!(
                "Working directory does not exist: {}",
                working_dir.display()
            );
        }

        let mut child = bash::spawn_background_shell(&command, &working_dir)
            .await
            .with_context(|| format!("Failed to spawn background command: {command}"))?;

        let stdout = child
            .stdout
            .take()
            .context("Failed to acquire stdout pipe for background command")?;
        let stderr = child
            .stderr
            .take()
            .context("Failed to acquire stderr pipe for background command")?;

        let done_notify = Arc::new(Notify::new());
        let cancel_notify = Arc::new(Notify::new());

        let inner = Arc::new(Mutex::new(Inner {
            child: Some(child),
            stdout: String::new(),
            stderr: String::new(),
            progress: Value::Object(Default::default()),
            status: "running".to_string(),
            exit_code: None,
            cancelled: false,
            done: false,
            bytes_dropped: 0,
            done_notify: Arc::clone(&done_notify),
            cancel_notify: Arc::clone(&cancel_notify),
        }));

        let handle = Self {
            id: id.clone(),
            command: command.clone(),
            inner: Arc::clone(&inner),
        };

        let id_reader = id.clone();
        let command_reader = command.clone();
        let session_id_reader = session_id.clone();
        let bus_reader = event_bus.clone();
        tokio::spawn(Self::reader_task(
            Arc::clone(&inner),
            stdout,
            stderr,
            id_reader,
            command_reader,
            session_id_reader,
            bus_reader,
        ));

        tokio::spawn(Self::waiter_task(
            Arc::clone(&inner),
            id,
            command,
            session_id,
            event_bus,
            done_notify,
            cancel_notify,
        ));

        Ok(handle)
    }

    /// Return the task id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// Return the shell command string.
    #[must_use]
    pub fn command(&self) -> &str {
        &self.command
    }

    /// Return the current status: `running`, `completed`, `failed`, or
    /// `cancelled`.
    #[must_use]
    pub fn status(&self) -> String {
        self.lock_inner()
            .map(|inner| inner.status.clone())
            .unwrap_or_default()
    }

    /// Return the exit code, if the process has finished.
    #[must_use]
    pub fn exit_code(&self) -> Option<i32> {
        self.lock_inner().ok().and_then(|inner| inner.exit_code)
    }

    /// Return `true` once the process has exited (naturally or via cancel).
    #[must_use]
    pub fn is_done(&self) -> bool {
        self.lock_inner().is_ok_and(|inner| inner.done)
    }

    /// Return the full captured stdout and stderr.
    #[must_use]
    pub fn output(&self) -> (String, String) {
        self.lock_inner()
            .map(|inner| (inner.stdout.clone(), inner.stderr.clone()))
            .unwrap_or_default()
    }

    /// Return the last `n` lines of combined stdout/stderr.
    ///
    /// Output is interleaved in the order lines were appended. The default
    /// `n = 20` matches common `tail` usage.
    #[must_use]
    pub fn tail(&self, n: usize) -> String {
        let Ok(inner) = self.lock_inner() else {
            return String::new();
        };
        let combined = format!("{}{}", inner.stdout, inner.stderr);
        let lines: Vec<&str> = combined.lines().collect();
        let start = lines.len().saturating_sub(n);
        lines[start..].join("\n")
    }

    /// Return the merged `JCODE_PROGRESS` JSON object.
    #[must_use]
    pub fn progress(&self) -> Value {
        self.lock_inner()
            .map(|inner| inner.progress.clone())
            .unwrap_or(Value::Null)
    }

    /// Request cancellation by killing the child process.
    ///
    /// This is a best-effort operation; if the process has already exited it
    /// is a no-op. The status is updated to `cancelled` once the waiter task
    /// observes the exit. The child handle remains in [`Inner`] so the waiter
    /// task can reap it; we only signal `start_kill` here.
    pub async fn cancel(&self) -> Result<()> {
        {
            let mut inner = self.lock_inner()?;
            inner.cancelled = true;
            if let Some(child) = inner.child.as_mut() {
                // `start_kill` sends the signal without awaiting.
                let _ = child.start_kill();
            }
            // R-9: Wake the waiter task so it stops blocking on `child.wait()`
            // and can observe the killed process exiting.
            inner.cancel_notify.notify_one();
        }
        // R-17: Block on the Notify signalled by `waiter_task` when `done` is
        // set, instead of polling `is_done()` every 50 ms (200 wakeups for a
        // 10 s timeout). Falls back to a deadline in case the waiter task
        // itself panicked and never sets `done`.
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(10);
        let done_notify = {
            let inner = self.lock_inner()?;
            Arc::clone(&inner.done_notify)
        };
        loop {
            if self.is_done() {
                return Ok(());
            }
            // Use a short timeout so we re-check `is_done` even if the notify
            // is missed (e.g. waiter panicked before signalling).
            tokio::select! {
                _ = done_notify.notified() => {}
                _ = tokio::time::sleep_until(deadline) => {
                    if self.is_done() {
                        return Ok(());
                    }
                    bail!(
                        "Timed out waiting for cancelled background task {} to exit",
                        self.id
                    );
                }
            }
        }
    }

    /// Block until the process exits or the timeout elapses.
    ///
    /// Returns `Ok(())` if the process finished before the timeout. If the
    /// timeout is reached the command is **not** cancelled; callers should
    /// use [`BackgroundCommand::cancel`] if they want to stop it.
    pub async fn wait(&self, timeout_secs: u64) -> Result<()> {
        let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(timeout_secs);
        let done_notify = {
            let inner = self.lock_inner()?;
            Arc::clone(&inner.done_notify)
        };
        loop {
            if self.is_done() {
                return Ok(());
            }
            // R-17: Block on the Notify instead of polling every 200 ms.
            tokio::select! {
                _ = done_notify.notified() => {}
                _ = tokio::time::sleep_until(deadline) => {
                    if self.is_done() {
                        return Ok(());
                    }
                    bail!("Timeout waiting for background task {}", self.id);
                }
            }
        }
    }

    async fn reader_task(
        inner: Arc<Mutex<Inner>>,
        stdout: ChildStdout,
        stderr: ChildStderr,
        task_id: String,
        command: String,
        session_id: String,
        event_bus: Option<Arc<EventBus>>,
    ) {
        let stdout_reader = BufReader::new(stdout);
        let stderr_reader = BufReader::new(stderr);

        let inner_stdout = Arc::clone(&inner);
        let event_bus_stdout = event_bus.clone();
        let task_id_stdout = task_id.clone();
        let session_id_stdout = session_id.clone();
        let command_stdout = command.clone();
        let stdout_handle = tokio::spawn(async move {
            let mut lines = stdout_reader.lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let formatted = format!("{line}\n");
                        Self::append_line(
                            &inner_stdout,
                            &formatted,
                            true,
                            &task_id_stdout,
                            &command_stdout,
                            &session_id_stdout,
                            event_bus_stdout.as_ref(),
                        );
                    }
                    Ok(None) => break,
                    Err(e) => {
                        debug!(error = %e, %task_id_stdout, "stdout reader error");
                        break;
                    }
                }
            }
        });

        let inner_stderr = Arc::clone(&inner);
        let task_id_stderr = task_id.clone();
        let session_id_stderr = session_id.clone();
        let stderr_handle = tokio::spawn(async move {
            let mut lines = stderr_reader.lines();
            loop {
                match lines.next_line().await {
                    Ok(Some(line)) => {
                        let formatted = format!("{line}\n");
                        Self::append_line(
                            &inner_stderr,
                            &formatted,
                            false,
                            &task_id_stderr,
                            &command,
                            &session_id_stderr,
                            event_bus.as_ref(),
                        );
                    }
                    Ok(None) => break,
                    Err(e) => {
                        debug!(error = %e, %task_id_stderr, "stderr reader error");
                        break;
                    }
                }
            }
        });

        let _ = tokio::join!(stdout_handle, stderr_handle);
        debug!(%task_id, "Background command reader task finished");
    }

    fn append_line(
        inner: &Mutex<Inner>,
        line: &str,
        is_stdout: bool,
        task_id: &str,
        command: &str,
        session_id: &str,
        event_bus: Option<&Arc<EventBus>>,
    ) {
        let trimmed = line.trim_start();
        let mut progress_update: Option<Value> = None;
        let mut is_progress = false;
        if let Some(rest) = trimmed.strip_prefix("JCODE_PROGRESS") {
            let rest = rest.trim_start();
            if let Ok(value) = serde_json::from_str::<Value>(rest) {
                progress_update = Some(value);
                is_progress = true;
            }
        }

        let mut guard = match inner.lock() {
            Ok(g) => g,
            Err(_) => {
                warn!(%task_id, "background inner lock poisoned in append_line");
                return;
            }
        };
        let dropped = guard.bytes_dropped;

        // T-022: successfully parsed `JCODE_PROGRESS` lines are consumed into
        // the progress object and not appended to the stdout/stderr buffers,
        // so the normal output stream stays clean for the model.
        let new_dropped = if is_progress {
            dropped
        } else if is_stdout {
            Self::append_with_cap(&mut guard.stdout, line, dropped)
        } else {
            Self::append_with_cap(&mut guard.stderr, line, dropped)
        };
        guard.bytes_dropped = new_dropped;
        if let Some(value) = progress_update {
            if let (Some(existing), Value::Object(new)) = (guard.progress.as_object_mut(), value) {
                for (k, v) in new {
                    existing.insert(k, v);
                }
            }
            progress_update = Some(guard.progress.clone());
        }
        if let (Some(bus), Some(progress)) = (event_bus, progress_update) {
            bus.publish(Event::BackgroundTaskUpdated {
                session_id: session_id.to_string(),
                task_id: task_id.to_string(),
                status: guard.status.clone(),
                progress: Some(progress),
            });
        }

        let status = guard.status.clone();
        drop(guard);
        // Per-line: use trace! (non-hot path) to avoid log flooding when
        // RUST_LOG=debug is set on a long-running command.
        trace!(
            %task_id,
            command = %command,
            is_stdout,
            status,
            "Background command line captured"
        );
    }

    fn append_with_cap(buffer: &mut String, line: &str, dropped: usize) -> usize {
        let mut new_dropped = dropped;
        let total_after = buffer.len().saturating_add(line.len());
        if total_after > MAX_BUFFER_BYTES {
            let overflow = total_after - MAX_BUFFER_BYTES;
            if overflow >= buffer.len() {
                new_dropped = new_dropped.saturating_add(buffer.len());
                buffer.clear();
            } else {
                // Drop the oldest `overflow` bytes. Using `drain(..overflow)`
                // is O(overflow) and shifts the remainder once, whereas
                // `replace_range` was O(buffer.len()) per call (O(n²) overall
                // when trimming repeatedly under a high-output command).
                new_dropped = new_dropped.saturating_add(overflow);
                buffer.drain(..overflow);
            }
        }
        buffer.push_str(line);
        new_dropped
    }

    async fn waiter_task(
        inner: Arc<Mutex<Inner>>,
        task_id: String,
        command: String,
        session_id: String,
        event_bus: Option<Arc<EventBus>>,
        done_notify: Arc<Notify>,
        cancel_notify: Arc<Notify>,
    ) {
        // R-9: Take the child out of `Inner` so we can call `child.wait()`
        // (async-aware) instead of polling `try_wait()` every 100 ms. The
        // `cancel()` path signals `cancel_notify`; we `select!` on both so
        // cancel wakes the waiter immediately rather than waiting for the
        // next poll tick.
        let mut child = match inner.lock() {
            Ok(mut g) => g.child.take(),
            Err(_) => {
                warn!(%task_id, "background inner lock poisoned in waiter_task");
                return;
            }
        };

        let exit_status: Option<std::process::ExitStatus> = if let Some(ref mut child) = child {
            loop {
                tokio::select! {
                    status = child.wait() => {
                        break status.ok();
                    }
                    // R-9: cancel() called start_kill() and notified us. Kill
                    // if not already killed, then keep waiting — `child.wait()`
                    // will return once the killed process exits.
                    _ = cancel_notify.notified() => {
                        let _ = child.start_kill();
                    }
                }
            }
        } else {
            // Child was already taken (e.g. by a prior loop or cancel path).
            None
        };

        // Drop the child to close process resources.
        drop(child);

        let (status, exit_code) = match exit_status {
            Some(status) if status.success() => ("completed", status.code()),
            Some(status) => ("failed", status.code()),
            None => ("failed", None),
        };

        let mut guard = match inner.lock() {
            Ok(g) => g,
            Err(_) => {
                warn!(%task_id, "background inner lock poisoned finalising");
                return;
            }
        };
        if guard.cancelled {
            guard.status = "cancelled".to_string();
        } else {
            guard.status = status.to_string();
        }
        guard.exit_code = exit_code;
        guard.done = true;
        // R-17: Wake any caller blocked in `cancel()` or `wait()`.
        done_notify.notify_waiters();
        let final_status = guard.status.clone();
        let progress = guard.progress.clone();
        drop(guard);

        if let Some(bus) = event_bus {
            bus.publish(Event::BackgroundTaskUpdated {
                session_id: session_id.clone(),
                task_id: task_id.clone(),
                status: final_status.clone(),
                progress: Some(progress),
            });
            bus.publish(Event::BackgroundTaskCompleted {
                session_id: session_id.clone(),
                task_id: task_id.clone(),
                status: final_status,
                exit_code,
            });
        }

        debug!(
            %task_id,
            command = %command,
            status,
            exit_code = ?exit_code,
            "Background command finished"
        );
    }
}
