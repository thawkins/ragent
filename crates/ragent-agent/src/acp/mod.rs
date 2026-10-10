//! Agent Client Protocol (ACP) client (spec `openhands` FR-015, FR-022, FR-036).
//!
//! ragent drives an external coding agent (Claude Code, Codex, Gemini CLI, or a
//! custom ACP server) as a **subprocess** that speaks JSON-RPC 2.0 over its
//! standard input and output (FR-015). This module owns that transport:
//!
//! - [`AcpClient::spawn`] starts the agent's command and wires up the stdio
//!   pipes, a writer task (frames in) and a reader task (frames out).
//! - [`AcpClient::initialize`] performs the ACP `initialize` handshake.
//! - [`AcpClient::create_session`] opens a conversation (`session/new`).
//! - [`AcpClient::prompt`] relays one turn (`session/prompt`), renders each
//!   streamed `session/update` notification through a callback, and returns the
//!   turn's stop reason and accumulated text.
//! - [`relay_turn`] is the one-shot convenience the session processor uses.
//!
//! Failure handling is a first-class concern (FR-036): a non-zero subprocess
//! exit, a malformed (non-JSON) frame on stdout, a JSON-RPC error reply, or a
//! silent subprocess that outlives its turn budget all **fail the turn** - the
//! relay never blocks indefinitely waiting for more output. The child is started
//! with `kill_on_drop`, so a failed or dropped client tears the process down.

use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use serde::Serialize;
use serde_json::{Value, json};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::{Mutex, mpsc, oneshot};
use tracing::{debug, trace, warn};

use ragent_config::AcpAgentConfig;

/// ACP **server** endpoint on stdio, serving ACP-capable editors (spec
/// `openhands` FR-022, FR-028).
///
/// Compiled only behind the `acp-server` Cargo feature, and only run when
/// `acp.server_enabled` is set: the endpoint is off by default.
#[cfg(feature = "acp-server")]
pub mod server;

/// Convenience re-export of the ACP server stdio entry point (spec `openhands`
/// FR-022, FR-028).
#[cfg(feature = "acp-server")]
pub use server::serve_stdio as serve_acp_stdio;

/// The ACP wire protocol version ragent speaks.
///
/// ACP wire compatibility is negotiated during `initialize` via the
/// `protocolVersion` field; `1` is the current stable version.
pub const ACP_PROTOCOL_VERSION: u64 = 1;

/// How long the `initialize` handshake and `session/new` may take before the
/// turn is failed. Kept short: both are local subprocess round-trips.
const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(30);

/// How often the relay loop wakes to check a cancellation flag while no frame
/// or update has arrived.
const CANCEL_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// How many trailing stderr lines are retained for error context.
const STDERR_TAIL_LINES: usize = 20;

/// Why an ACP turn failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AcpErrorKind {
    /// The agent command could not be spawned (missing executable, permissions).
    Spawn,
    /// The JSON-RPC exchange failed: a `session/prompt` error reply, a closed
    /// channel, or a reply for an unknown request id.
    Protocol,
    /// A frame on stdout was not valid JSON-RPC (FR-036).
    MalformedFrame,
    /// The subprocess exited before the turn completed (FR-036). A non-zero
    /// exit is reported through [`AcpError::exit_code`].
    Exited,
    /// The subprocess produced no terminal frame within the turn budget
    /// (FR-036); the process is killed and the turn fails.
    Timeout,
    /// An I/O error writing to, or reading from, the subprocess.
    Io,
}

/// A structured ACP failure.
///
/// Implements [`std::error::Error`] so it travels inside an [`anyhow::Error`];
/// callers that need the structured cause downcast with
/// [`anyhow::Error::downcast_ref`]. This mirrors the
/// [`BackendError`](crate::backend::BackendError) shape used by the execution
/// backends.
#[derive(Debug)]
pub struct AcpError {
    agent: String,
    kind: AcpErrorKind,
    exit_code: Option<i32>,
    message: String,
}

impl AcpError {
    /// Build an ACP failure for `agent`.
    #[must_use]
    pub fn new(agent: impl Into<String>, kind: AcpErrorKind, message: impl Into<String>) -> Self {
        Self {
            agent: agent.into(),
            kind,
            exit_code: None,
            message: message.into(),
        }
    }

    /// Attach the subprocess exit code (used by [`AcpErrorKind::Exited`]).
    #[must_use]
    pub fn with_exit_code(mut self, code: Option<i32>) -> Self {
        self.exit_code = code;
        self
    }

    /// The ACP agent id that failed.
    #[must_use]
    pub fn agent(&self) -> &str {
        &self.agent
    }

    /// The coarse cause.
    #[must_use]
    pub const fn kind(&self) -> AcpErrorKind {
        self.kind
    }

    /// The subprocess exit code, when the failure was a process exit.
    #[must_use]
    pub const fn exit_code(&self) -> Option<i32> {
        self.exit_code
    }

    /// The human-readable message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for AcpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "ACP agent '{}' failed ({:?}): {}",
            self.agent, self.kind, self.message
        )
    }
}

impl std::error::Error for AcpError {}

/// One streamed update decoded from an ACP `session/update` notification.
///
/// Only the update kinds ragent renders are modelled explicitly; every other
/// kind is preserved as [`AcpUpdate::Other`] so an unknown future update is
/// surfaced rather than dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AcpUpdate {
    /// A chunk of the agent's user-visible message.
    AgentMessageChunk {
        /// The text fragment.
        text: String,
    },
    /// A chunk of the agent's chain-of-thought reasoning.
    AgentThoughtChunk {
        /// The reasoning text fragment.
        text: String,
    },
    /// A tool call the agent has begun.
    ToolCall {
        /// Provider-assigned call identifier.
        tool_call_id: String,
        /// Human-readable title.
        title: String,
        /// Lifecycle status (`pending`, `in_progress`, `completed`, `failed`).
        status: String,
    },
    /// A status/progress update for a previously announced tool call.
    ToolCallUpdate {
        /// Provider-assigned call identifier.
        tool_call_id: String,
        /// Updated title, when the agent supplied one.
        title: Option<String>,
        /// Updated status, when the agent supplied one.
        status: Option<String>,
    },
    /// An execution plan with a number of steps.
    Plan {
        /// Number of plan entries.
        entries: usize,
    },
    /// Any other update kind, carrying its raw `sessionUpdate` discriminator.
    Other {
        /// The `sessionUpdate` discriminator as received.
        kind: String,
    },
}

impl AcpUpdate {
    /// The `sessionUpdate` discriminator for this update.
    #[must_use]
    pub fn kind(&self) -> &str {
        match self {
            Self::AgentMessageChunk { .. } => "agent_message_chunk",
            Self::AgentThoughtChunk { .. } => "agent_thought_chunk",
            Self::ToolCall { .. } => "tool_call",
            Self::ToolCallUpdate { .. } => "tool_call_update",
            Self::Plan { .. } => "plan",
            Self::Other { kind } => kind,
        }
    }
}

/// The result of a completed ACP turn (`session/prompt`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AcpTurnOutcome {
    /// The agent's reported `stopReason` (empty when the agent omitted one).
    pub stop_reason: String,
    /// The concatenated text of every `agent_message_chunk` in the turn.
    pub text: String,
    /// How many streamed updates were delivered during the turn.
    pub updates: usize,
}

/// A frame routed from the reader task to the active turn's relay loop.
enum ReaderEvent {
    /// A decoded `session/update` notification to render.
    Update(AcpUpdate),
    /// The subprocess closed stdout (normally because it exited).
    Exited {
        /// The process exit code, when one was observed.
        code: Option<i32>,
        /// Trailing stderr lines captured for diagnostics.
        stderr: String,
    },
    /// A line on stdout was not valid JSON-RPC (FR-036).
    Malformed {
        /// The offending line.
        line: String,
    },
}

/// A JSON-RPC 2.0 request frame.
#[derive(Debug, Serialize)]
struct RpcRequest<'a> {
    jsonrpc: &'static str,
    id: u64,
    method: &'a str,
    params: Value,
}

/// A live connection to an external ACP agent.
///
/// The subprocess is spawned by [`AcpClient::spawn`]; [`AcpClient::prompt`]
/// relays turns over JSON-RPC on stdio and renders streamed updates. Dropping
/// the client kills the child (it is started with `kill_on_drop`).
pub struct AcpClient {
    agent_id: String,
    display_name: String,
    turn_timeout: Duration,
    child: Arc<Mutex<Child>>,
    /// Frames to write to the child's stdin, drained by the writer task.
    writer: mpsc::UnboundedSender<String>,
    /// Pending requests awaiting a JSON-RPC reply, keyed by request id.
    pending: Arc<
        std::sync::Mutex<std::collections::HashMap<u64, oneshot::Sender<Result<Value, AcpError>>>>,
    >,
    /// Streamed updates from the child, drained by the active turn.
    events: Mutex<mpsc::UnboundedReceiver<ReaderEvent>>,
    /// Next JSON-RPC request id.
    next_id: AtomicU64,
    /// Whether the connection is still considered live.
    alive: Arc<AtomicBool>,
    writer_task: Option<tokio::task::JoinHandle<()>>,
    reader_task: Option<tokio::task::JoinHandle<()>>,
    stderr_task: Option<tokio::task::JoinHandle<()>>,
}

impl AcpClient {
    /// Spawn `agent`'s command as a subprocess in `working_dir` and wire up the
    /// JSON-RPC transport (FR-015).
    ///
    /// `cwd` is overridden by `agent.cwd` when that is set.
    ///
    /// # Errors
    ///
    /// Returns [`AcpErrorKind::Spawn`] when the command cannot be started, and
    /// [`AcpErrorKind::Io`] when the child exposes no usable stdio pipes.
    pub async fn spawn(agent: &AcpAgentConfig, working_dir: &Path) -> Result<Self, AcpError> {
        let cwd = agent
            .cwd
            .as_deref()
            .map_or_else(|| working_dir.to_path_buf(), std::path::PathBuf::from);

        let mut cmd = Command::new(&agent.command);
        cmd.args(&agent.args)
            .envs(&agent.env)
            .current_dir(&cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            // The child must not outlive ragent holding its stdio pipe open.
            .kill_on_drop(true);

        let mut child = cmd.spawn().map_err(|e| {
            AcpError::new(
                &agent.id,
                AcpErrorKind::Spawn,
                format!("spawning '{}' failed: {e}", agent.command),
            )
        })?;

        let stdin = child.stdin.take().ok_or_else(|| {
            AcpError::new(&agent.id, AcpErrorKind::Io, "child stdin was not piped")
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            AcpError::new(&agent.id, AcpErrorKind::Io, "child stdout was not piped")
        })?;
        let stderr = child.stderr.take();

        let (writer_tx, mut writer_rx) = mpsc::unbounded_channel::<String>();
        let (events_tx, events_rx) = mpsc::unbounded_channel::<ReaderEvent>();

        let child = Arc::new(Mutex::new(child));
        let pending: Arc<
            std::sync::Mutex<
                std::collections::HashMap<u64, oneshot::Sender<Result<Value, AcpError>>>,
            >,
        > = Arc::new(std::sync::Mutex::new(std::collections::HashMap::new()));
        let alive = Arc::new(AtomicBool::new(true));

        // Writer task: serialise outgoing frames onto the child's stdin.
        let writer_task = tokio::spawn(async move {
            let mut stdin = stdin;
            while let Some(line) = writer_rx.recv().await {
                if stdin.write_all(line.as_bytes()).await.is_err() {
                    break;
                }
                if stdin.write_all(b"\n").await.is_err() {
                    break;
                }
                if stdin.flush().await.is_err() {
                    break;
                }
            }
            // Best-effort graceful close so the agent sees end-of-input.
            let _ = stdin.shutdown().await; // INTENTIONAL: best-effort close; a broken pipe just means the child already exited
        });

        // stderr drain task: keep the trailing lines for error context. Draining
        // prevents a chatty agent from blocking on a full stderr pipe.
        let stderr_tail: Arc<std::sync::Mutex<std::collections::VecDeque<String>>> =
            Arc::new(std::sync::Mutex::new(std::collections::VecDeque::new()));
        let stderr_task = stderr.map(|stderr| {
            let tail = Arc::clone(&stderr_tail);
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let mut guard = tail
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner);
                    if guard.len() == STDERR_TAIL_LINES {
                        guard.pop_front();
                    }
                    guard.push_back(line);
                }
            })
        });

        // Reader task: decode newline-delimited JSON-RPC frames from stdout and
        // route replies to their waiters and notifications to the turn loop.
        let reader_task = {
            let pending = Arc::clone(&pending);
            let events_tx = events_tx.clone();
            let writer_tx = writer_tx.clone();
            let child = Arc::clone(&child);
            let alive = Arc::clone(&alive);
            let agent_id = agent.id.clone();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stdout).lines();
                loop {
                    match lines.next_line().await {
                        Ok(Some(line)) => {
                            if line.trim().is_empty() {
                                continue;
                            }
                            if !handle_frame(&line, &pending, &events_tx, &writer_tx, &agent_id) {
                                // A malformed frame fails the turn (FR-036);
                                // stop reading further output. Wake the turn
                                // loop so it can surface the failure instead of
                                // being failed by the turn timeout: a `None`
                                // recv is treated as a premature exit.
                                alive.store(false, Ordering::SeqCst);
                                fail_pending(
                                    &pending,
                                    &agent_id,
                                    AcpErrorKind::MalformedFrame,
                                    None,
                                    "agent emitted a malformed frame before replying",
                                );
                                drop(events_tx);
                                break;
                            }
                        }
                        Ok(None) => {
                            // stdout closed: the child exited (or is exiting).
                            let code = {
                                let mut guard = child.lock().await;
                                guard.wait().await.ok().and_then(|status| status.code())
                            };
                            alive.store(false, Ordering::SeqCst);
                            fail_pending(
                                &pending,
                                &agent_id,
                                AcpErrorKind::Exited,
                                code,
                                "agent exited before replying",
                            );
                            let stderr = collect_stderr(&stderr_tail);
                            let _ = events_tx.send(ReaderEvent::Exited { code, stderr });
                            break;
                        }
                        Err(e) => {
                            alive.store(false, Ordering::SeqCst);
                            fail_pending(
                                &pending,
                                &agent_id,
                                AcpErrorKind::Exited,
                                None,
                                &format!("stdout read error: {e}"),
                            );
                            let stderr = collect_stderr(&stderr_tail);
                            let _ = events_tx.send(ReaderEvent::Exited { code: None, stderr });
                            break;
                        }
                    }
                }
            })
        };

        debug!(
            agent = %agent.id,
            command = %crate::sanitize::redact_secrets(&agent.command),
            "spawned ACP agent subprocess"
        );

        Ok(Self {
            agent_id: agent.id.clone(),
            display_name: agent.display_name().to_string(),
            turn_timeout: Duration::from_secs(agent.turn_timeout_secs.max(1)),
            child,
            writer: writer_tx,
            pending,
            events: Mutex::new(events_rx),
            next_id: AtomicU64::new(1),
            alive,
            writer_task: Some(writer_task),
            reader_task: Some(reader_task),
            stderr_task,
        })
    }

    /// The registered id of the agent this client drives.
    #[must_use]
    pub fn agent_id(&self) -> &str {
        &self.agent_id
    }

    /// The human-readable display name of the agent this client drives.
    #[must_use]
    pub fn display_name(&self) -> &str {
        &self.display_name
    }

    /// Whether the transport is still live.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }

    /// The timeout for the `initialize` and `session/new` handshakes.
    ///
    /// Bounded by both [`HANDSHAKE_TIMEOUT`] and the agent's turn budget, so a
    /// subprocess that never answers the handshake is failed by its own turn
    /// timeout rather than hanging for the full handshake window (FR-036).
    fn handshake_timeout(&self) -> Duration {
        HANDSHAKE_TIMEOUT.min(self.turn_timeout)
    }

    /// Perform the ACP `initialize` handshake.
    ///
    /// # Errors
    ///
    /// Fails with [`AcpErrorKind::Protocol`] on a JSON-RPC error reply,
    /// [`AcpErrorKind::Exited`] when the agent exits first, and
    /// [`AcpErrorKind::Timeout`] when the handshake does not complete within
    /// [`HANDSHAKE_TIMEOUT`].
    pub async fn initialize(&self) -> Result<Value, AcpError> {
        let params = json!({
            "protocolVersion": ACP_PROTOCOL_VERSION,
            "clientCapabilities": {
                "fs": { "readTextFile": false, "writeTextFile": false },
                "terminal": false,
            },
        });
        let result = self
            .request("initialize", params, self.handshake_timeout())
            .await?;
        if let Some(version) = result.get("protocolVersion").and_then(Value::as_u64)
            && version != ACP_PROTOCOL_VERSION
        {
            warn!(
                agent = %self.agent_id,
                negotiated = version,
                expected = ACP_PROTOCOL_VERSION,
                "ACP agent negotiated a different protocol version"
            );
        }
        Ok(result)
    }

    /// Open a new ACP conversation for `cwd` (`session/new`) and return its id.
    ///
    /// # Errors
    ///
    /// Fails with [`AcpErrorKind::Protocol`] when the agent returns no
    /// `sessionId`, and propagates transport failures from the request.
    pub async fn create_session(&self, cwd: &Path) -> Result<String, AcpError> {
        let params = json!({
            "cwd": cwd.to_string_lossy(),
            "mcpServers": [],
        });
        let result = self
            .request("session/new", params, self.handshake_timeout())
            .await?;
        result
            .get("sessionId")
            .and_then(Value::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                AcpError::new(
                    &self.agent_id,
                    AcpErrorKind::Protocol,
                    "session/new reply did not carry a sessionId",
                )
            })
    }

    /// Relay one turn (`session/prompt`) and render each streamed update.
    ///
    /// Each decoded [`AcpUpdate`] is passed to `on_update`; the returned
    /// [`AcpTurnOutcome`] carries the agent's stop reason and the accumulated
    /// assistant text.
    ///
    /// # Errors
    ///
    /// Fails the turn (FR-036) when the agent replies with a JSON-RPC error,
    /// exits non-zero before completing, emits a malformed frame, or produces
    /// no terminal frame within the turn budget. A raised `cancel` abort is also
    /// reported here.
    pub async fn prompt(
        &self,
        session_id: &str,
        text: &str,
        cancel: &AtomicBool,
        on_update: &mut (dyn FnMut(AcpUpdate) + Send),
    ) -> Result<AcpTurnOutcome, AcpError> {
        let params = json!({
            "sessionId": session_id,
            "prompt": [ { "type": "text", "text": text } ],
        });
        let (_id, mut reply) = self.send_request("session/prompt", params)?;
        let mut events = self.events.lock().await;
        let deadline = tokio::time::Instant::now() + self.turn_timeout;
        let mut outcome = AcpTurnOutcome::default();

        loop {
            if cancel.load(Ordering::Relaxed) {
                self.kill().await;
                return Err(AcpError::new(
                    &self.agent_id,
                    AcpErrorKind::Protocol,
                    "turn cancelled by user",
                ));
            }

            tokio::select! {
                biased;
                reply_result = &mut reply => {
                    // The agent may emit `session/update` notifications
                    // immediately before its terminal reply. The reader task
                    // forwards each update to the event channel before it
                    // resolves the reply waiter, so drain whatever is already
                    // queued here before completing - otherwise the final
                    // streamed chunk would race the reply and be dropped.
                    while let Ok(event) = events.try_recv() {
                        if let ReaderEvent::Update(update) = event {
                            outcome.updates += 1;
                            if let AcpUpdate::AgentMessageChunk { text } = &update {
                                outcome.text.push_str(text);
                            }
                            on_update(update);
                        }
                    }
                    return match reply_result {
                        Ok(Ok(value)) => {
                            outcome.stop_reason = value
                                .get("stopReason")
                                .and_then(Value::as_str)
                                .unwrap_or_default()
                                .to_string();
                            debug!(
                                agent = %self.agent_id,
                                stop_reason = %outcome.stop_reason,
                                updates = outcome.updates,
                                "ACP turn completed"
                            );
                            Ok(outcome)
                        }
                        Ok(Err(e)) => Err(e),
                        Err(_) => Err(AcpError::new(
                            &self.agent_id,
                            AcpErrorKind::Exited,
                            "agent closed the connection before replying to session/prompt",
                        )),
                    };
                }
                event = events.recv() => {
                    match event {
                        Some(ReaderEvent::Update(update)) => {
                            outcome.updates += 1;
                            if let AcpUpdate::AgentMessageChunk { text } = &update {
                                outcome.text.push_str(text);
                            }
                            on_update(update);
                        }
                        Some(ReaderEvent::Exited { code, stderr }) => {
                            return Err(self.exit_error(code, &stderr));
                        }
                        Some(ReaderEvent::Malformed { line }) => {
                            return Err(AcpError::new(
                                &self.agent_id,
                                AcpErrorKind::MalformedFrame,
                                format!(
                                    "malformed JSON-RPC frame on stdout: {}",
                                    truncate_for_log(&line)
                                ),
                            ));
                        }
                        None => {
                            return Err(AcpError::new(
                                &self.agent_id,
                                AcpErrorKind::Exited,
                                "agent output stream ended before the turn completed",
                            ));
                        }
                    }
                }
                () = tokio::time::sleep_until(deadline) => {
                    self.kill().await;
                    return Err(AcpError::new(
                        &self.agent_id,
                        AcpErrorKind::Timeout,
                        format!(
                            "agent produced no terminal frame within {}s; subprocess killed",
                            self.turn_timeout.as_secs()
                        ),
                    ));
                }
                () = tokio::time::sleep(CANCEL_POLL_INTERVAL) => {
                    // Re-check the cancellation flag without blocking on I/O;
                    // the top-of-loop check catches a flag already raised.
                    if cancel.load(Ordering::Relaxed) {
                        self.kill().await;
                        return Err(AcpError::new(
                            &self.agent_id,
                            AcpErrorKind::Protocol,
                            "turn cancelled by user",
                        ));
                    }
                }
            }
        }
    }

    /// Send a request and await its single JSON-RPC reply.
    async fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, AcpError> {
        let (id, reply) = self.send_request(method, params)?;
        match tokio::time::timeout(timeout, reply).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(AcpError::new(
                &self.agent_id,
                AcpErrorKind::Exited,
                format!("connection closed awaiting the '{method}' reply"),
            )),
            Err(_) => {
                self.discard_pending(id);
                Err(AcpError::new(
                    &self.agent_id,
                    AcpErrorKind::Timeout,
                    format!("no reply to '{method}' within {}s", timeout.as_secs()),
                ))
            }
        }
    }

    /// Drop the pending waiter for `id` (a request that was abandoned).
    fn discard_pending(&self, id: u64) {
        let mut pending = self
            .pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        pending.remove(&id);
    }

    /// Register a pending request and write its frame to the child.
    fn send_request(
        &self,
        method: &str,
        params: Value,
    ) -> Result<(u64, oneshot::Receiver<Result<Value, AcpError>>), AcpError> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = oneshot::channel();
        let frame = RpcRequest {
            jsonrpc: "2.0",
            id,
            method,
            params,
        };
        // Encode before registering the waiter, so a serialization failure cannot
        // leave an orphaned entry in the pending map.
        let line = serde_json::to_string(&frame).map_err(|e| {
            AcpError::new(
                &self.agent_id,
                AcpErrorKind::Protocol,
                format!("encoding '{method}' request failed: {e}"),
            )
        })?;
        {
            let mut pending = self
                .pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            pending.insert(id, tx);
        }
        if self.writer.send(line).is_err() {
            self.discard_pending(id);
            return Err(AcpError::new(
                &self.agent_id,
                AcpErrorKind::Exited,
                format!("agent stdin is closed; could not send '{method}'"),
            ));
        }
        Ok((id, rx))
    }

    /// Kill the subprocess and mark the transport dead.
    async fn kill(&self) {
        self.alive.store(false, Ordering::SeqCst);
        let mut guard = self.child.lock().await;
        let _ = guard.start_kill();
        let _ = guard.wait().await;
    }

    /// Terminate the agent subprocess and stop the transport tasks.
    ///
    /// Safe to call more than once.
    pub async fn shutdown(&mut self) {
        self.alive.store(false, Ordering::SeqCst);
        {
            let mut guard = self.child.lock().await;
            let _ = guard.start_kill();
            let _ = guard.wait().await;
        }
        // Dropping the writer sender ends the writer task; abort the reader and
        // stderr tasks so nothing lingers after a failed or finished turn.
        if let Some(task) = self.writer_task.take() {
            task.abort();
        }
        if let Some(task) = self.reader_task.take() {
            task.abort();
        }
        if let Some(task) = self.stderr_task.take() {
            task.abort();
        }
    }

    /// Build the failure for a subprocess exit.
    fn exit_error(&self, code: Option<i32>, stderr: &str) -> AcpError {
        let detail = if stderr.is_empty() {
            String::new()
        } else {
            format!("; stderr: {}", truncate_for_log(stderr))
        };
        let message = match code {
            Some(code) => {
                format!("agent exited with status {code} before completing the turn{detail}")
            }
            None => format!("agent terminated before completing the turn{detail}"),
        };
        AcpError::new(&self.agent_id, AcpErrorKind::Exited, message).with_exit_code(code)
    }
}

impl Drop for AcpClient {
    fn drop(&mut self) {
        // `kill_on_drop` tears the child down when its `Arc<Mutex<Child>>` is
        // dropped; issue a best-effort immediate kill here too so a failed turn
        // does not wait for the reader task to notice EOF.
        if let Ok(mut guard) = self.child.try_lock() {
            let _ = guard.start_kill();
        }
        if let Some(task) = self.writer_task.take() {
            task.abort();
        }
        if let Some(task) = self.reader_task.take() {
            task.abort();
        }
        if let Some(task) = self.stderr_task.take() {
            task.abort();
        }
    }
}

/// Decode one stdout line and route it, returning `false` when the line was a
/// malformed frame that must fail the turn (FR-036).
fn handle_frame(
    line: &str,
    pending: &std::sync::Mutex<
        std::collections::HashMap<u64, oneshot::Sender<Result<Value, AcpError>>>,
    >,
    events_tx: &mpsc::UnboundedSender<ReaderEvent>,
    writer_tx: &mpsc::UnboundedSender<String>,
    agent_id: &str,
) -> bool {
    let Ok(value) = serde_json::from_str::<Value>(line) else {
        let _ = events_tx.send(ReaderEvent::Malformed {
            line: line.to_string(),
        });
        return false;
    };

    // A request or notification from the agent carries a `method`.
    if let Some(method) = value.get("method").and_then(Value::as_str) {
        if value.get("id").is_some() {
            // A client-side request ragent does not implement: answer with a
            // JSON-RPC "method not found" so the agent is not left waiting.
            let id = value.get("id").cloned().unwrap_or(Value::Null);
            let reply = json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": { "code": -32601, "message": "client method not supported" },
            });
            let _ = writer_tx.send(reply.to_string());
        } else if method == "session/update" {
            let update = parse_update(&value);
            trace!(
                agent = agent_id,
                update = update.kind(),
                "ACP session update"
            );
            let _ = events_tx.send(ReaderEvent::Update(update));
        } else {
            trace!(
                agent = agent_id,
                method, "ignoring unhandled ACP notification"
            );
        }
        return true;
    }

    // Otherwise it is a response to one of our requests.
    if let Some(id) = value.get("id").and_then(Value::as_u64) {
        let waiter = {
            let mut pending = pending
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            pending.remove(&id)
        };
        if let Some(tx) = waiter {
            if let Some(error) = value.get("error") {
                let code = error.get("code").and_then(Value::as_i64).unwrap_or(0);
                let message = error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error");
                let _ = tx.send(Err(AcpError::new(
                    agent_id,
                    AcpErrorKind::Protocol,
                    format!("JSON-RPC error reply (code {code}): {message}"),
                )));
            } else {
                let result = value.get("result").cloned().unwrap_or(Value::Null);
                let _ = tx.send(Ok(result));
            }
        } else {
            trace!(
                agent = agent_id,
                id, "reply for an unknown request id ignored"
            );
        }
    }
    true
}

/// Decode an ACP `session/update` notification into an [`AcpUpdate`].
///
/// Exposed so the transport's frame model can be tested without a live
/// subprocess.
#[must_use]
pub fn parse_update(frame: &Value) -> AcpUpdate {
    let params = frame.get("params").unwrap_or(frame);
    let update = params.get("update").unwrap_or(params);
    let kind = update
        .get("sessionUpdate")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match kind {
        "agent_message_chunk" => AcpUpdate::AgentMessageChunk {
            text: extract_text(update.get("content")),
        },
        "agent_thought_chunk" => AcpUpdate::AgentThoughtChunk {
            text: extract_text(update.get("content")),
        },
        "tool_call" => AcpUpdate::ToolCall {
            tool_call_id: update
                .get("toolCallId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            title: update
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            status: update
                .get("status")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        },
        "tool_call_update" => AcpUpdate::ToolCallUpdate {
            tool_call_id: update
                .get("toolCallId")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            title: update
                .get("title")
                .and_then(Value::as_str)
                .map(str::to_string),
            status: update
                .get("status")
                .and_then(Value::as_str)
                .map(str::to_string),
        },
        "plan" => AcpUpdate::Plan {
            entries: update
                .get("entries")
                .and_then(Value::as_array)
                .map_or(0, Vec::len),
        },
        other => AcpUpdate::Other {
            kind: other.to_string(),
        },
    }
}

/// Extract concatenated text from an ACP `content` value, accepting both the
/// single `{ "type": "text", "text": ... }` block and an array of such blocks.
fn extract_text(content: Option<&Value>) -> String {
    match content {
        Some(Value::Object(obj)) => obj
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

/// Fail every pending request when the transport dies.
fn fail_pending(
    pending: &std::sync::Mutex<
        std::collections::HashMap<u64, oneshot::Sender<Result<Value, AcpError>>>,
    >,
    agent_id: &str,
    kind: AcpErrorKind,
    exit_code: Option<i32>,
    reason: &str,
) {
    let drained: Vec<_> = {
        let mut pending = pending
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        pending.drain().map(|(_, tx)| tx).collect()
    };
    for tx in drained {
        let error = AcpError::new(agent_id, kind, reason.to_string()).with_exit_code(exit_code);
        let _ = tx.send(Err(error));
    }
}

/// Snapshot the captured stderr tail as a single string.
fn collect_stderr(tail: &std::sync::Mutex<std::collections::VecDeque<String>>) -> String {
    let guard = tail
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    guard.iter().cloned().collect::<Vec<_>>().join("\n")
}

/// Truncate a string for a log line, keeping it single-line and bounded.
fn truncate_for_log(text: &str) -> String {
    const MAX: usize = 500;
    let one_line = text.replace(['\n', '\r'], " ");
    if one_line.len() <= MAX {
        one_line
    } else {
        let mut end = MAX;
        while end > 0 && !one_line.is_char_boundary(end) {
            end -= 1;
        }
        format!("{}...", &one_line[..end])
    }
}

/// Spawn `agent`, run the `initialize` / `session/new` / `session/prompt`
/// sequence for a single turn, then shut the subprocess down.
///
/// This is the entry point the session processor uses: each turn is a fresh
/// subprocess, so no state leaks between turns and a crashed agent cannot wedge
/// the session. [`AcpClient`] can be reused directly when a caller wants a
/// persistent connection.
///
/// # Errors
///
/// Returns the first [`AcpError`] from spawning, the handshake, session
/// creation, or the relayed turn. A turn failure never falls back to a local
/// provider (FR-022).
pub async fn relay_turn(
    agent: &AcpAgentConfig,
    working_dir: &Path,
    prompt: &str,
    cancel: &AtomicBool,
    on_update: &mut (dyn FnMut(AcpUpdate) + Send),
) -> Result<AcpTurnOutcome, AcpError> {
    let mut client = AcpClient::spawn(agent, working_dir).await?;
    let result = async {
        client.initialize().await?;
        let session_id = client.create_session(working_dir).await?;
        client.prompt(&session_id, prompt, cancel, on_update).await
    }
    .await;
    client.shutdown().await;
    result
}
