//! Remote execution backend: drive a ragent server over its REST+SSE API
//! (spec `openhands` T-003; FR-001, FR-004, FR-009, FR-021, FR-031, FR-034).
//!
//! A `remote` backend points at a second ragent server (usually on another
//! host) and runs the session **against that server** instead of on the local
//! machine. Unlike [`ContainerBackend`](super::ContainerBackend), which still
//! executes tools locally through a sandbox, the remote backend does not
//! execute tools at all: it relays the whole turn to the remote server's REST
//! API and mirrors the server's server-sent event stream back into the local
//! event bus (FR-021).
//!
//! ## Turn flow
//!
//! ```text
//! POST <url>/sessions            { "directory": "<dir>" }   -> { "id": ... }
//! GET  <url>/sessions/<id>/messages  (SSE; POST body { "content": prompt })
//! POST <url>/sessions/<id>/permission/<req_id>  { "decision": "allow" }
//! ```
//!
//! The `send_message` route both starts the turn and streams the resulting
//! events, so one request drives the whole turn ([`relay_turn`]). The stream is
//! decoded frame by frame into [`RemoteUpdate`]s; the session dispatch layer
//! re-publishes each one onto the local bus with the **local** session id so
//! the TUI renders the remote turn exactly like a local one
//! ([`crate::session::remote_dispatch`]).
//!
//! ## Unreachability (FR-034)
//!
//! A connection that cannot be reached before the turn starts is a
//! provisioning failure ([`RemoteErrorKind::Unreachable`]) and fails the turn
//! with no local execution (FR-031). A stream that drops **part-way through** a
//! turn is reported as [`RemoteErrorKind::Protocol`] and likewise never
//! re-runs the turn's tools locally (FR-034). The relay is bounded by
//! [`TURN_TIMEOUT`] and polls a cancellation flag, so an unresponsive server
//! cannot hang the session.

use std::collections::HashMap;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use futures::StreamExt as _;
use serde::Serialize;
use serde_json::Value;

use ragent_config::{BackendConfig, ExecutionBackendKind};

use super::{BackendError, BackendErrorKind, BackendToolCall, ExecutionBackend};
use crate::tool::ToolOutput;

/// Timeout for a non-streaming remote request (session create, permission
/// reply). The streaming turn is bounded by [`TURN_TIMEOUT`] instead.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Overall budget for one relayed turn before it is failed as a timeout.
const TURN_TIMEOUT: Duration = Duration::from_mins(15);

/// How often the streaming loop wakes to re-check the cancellation flag while
/// no frame has arrived.
const POLL_INTERVAL: Duration = Duration::from_millis(250);

/// Separator joining the parts of a remote-session cache key. A control
/// character cannot appear in a URL or session id, so it cannot collide.
const KEY_SEP: char = '\u{1f}';

/// Why a remote turn step failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoteErrorKind {
    /// The remote server could not be reached (connection refused, DNS, TLS,
    /// no URL configured). Fails the turn without local execution (FR-031).
    Unreachable,
    /// The remote server answered but the exchange failed: a non-2xx status, a
    /// dropped stream part-way through the turn, or a JSON-RPC-shaped error
    /// body (FR-034).
    Protocol,
    /// The turn exceeded [`TURN_TIMEOUT`] without completing (FR-034).
    Timeout,
    /// The user cancelled the turn.
    Cancelled,
}

/// A structured remote-backend failure.
///
/// Implements [`std::error::Error`] so it travels inside an [`anyhow::Error`];
/// callers that need the structured cause downcast it with
/// [`anyhow::Error::downcast_ref`]. This mirrors the
/// [`AcpError`](crate::acp::AcpError) and [`BackendError`] shapes.
#[derive(Debug)]
pub struct RemoteError {
    kind: RemoteErrorKind,
    status: Option<u16>,
    message: String,
}

impl RemoteError {
    /// Build a remote failure.
    #[must_use]
    pub fn new(kind: RemoteErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            status: None,
            message: message.into(),
        }
    }

    /// Attach the HTTP status code that produced the failure.
    #[must_use]
    pub fn with_status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
    }

    /// The coarse cause.
    #[must_use]
    pub const fn kind(&self) -> RemoteErrorKind {
        self.kind
    }

    /// The HTTP status code, when the failure came from a response.
    #[must_use]
    pub const fn status(&self) -> Option<u16> {
        self.status
    }

    /// The human-readable message.
    #[must_use]
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl std::fmt::Display for RemoteError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "remote backend failed ({:?}): {}",
            self.kind, self.message
        )
    }
}

impl std::error::Error for RemoteError {}

impl From<RemoteError> for BackendError {
    /// Map a remote failure onto the backend error taxonomy.
    ///
    /// `Unreachable` is a provisioning failure (the remote could not be
    /// provisioned, FR-031); a protocol fault or timeout is a mid-turn failure
    /// (FR-034). Neither ever falls back to host execution.
    fn from(error: RemoteError) -> Self {
        let kind = match error.kind {
            RemoteErrorKind::Unreachable => BackendErrorKind::Provision,
            RemoteErrorKind::Protocol | RemoteErrorKind::Timeout => BackendErrorKind::Protocol,
            RemoteErrorKind::Cancelled => BackendErrorKind::Protocol,
        };
        // `Self` here is `RemoteError`; the backend kind must be named
        // explicitly, so the lint's `Self` suggestion does not apply.
        #[allow(clippy::use_self)]
        BackendError::new(ExecutionBackendKind::Remote, kind, error.message)
    }
}

/// One decoded update from the remote event stream.
///
/// The remote server serialises its `Event`s with the event names produced by
/// [`ragent_server::sse`](https://docs.rs/ragent-server); this enum decodes the
/// subset the TUI renders and preserves every other kind as [`RemoteUpdate::Other`]
/// so an unknown future event is surfaced rather than dropped.
#[allow(clippy::derive_partial_eq_without_eq)]
#[derive(Debug, Clone, PartialEq)]
pub enum RemoteUpdate {
    /// A remote session was created (`session_created`).
    SessionCreated,
    /// The remote turn's assistant message started (`message_start`).
    MessageStart {
        /// Remote message id.
        message_id: String,
    },
    /// A chunk of assistant text (`text_delta`).
    TextDelta {
        /// The text fragment.
        text: String,
    },
    /// A chunk of chain-of-thought text (`reasoning_delta`).
    ReasoningDelta {
        /// The reasoning fragment.
        text: String,
    },
    /// A tool call began (`tool_call_start`).
    ToolCallStart {
        /// Remote call id.
        call_id: String,
        /// Tool name.
        tool: String,
    },
    /// A tool call finished (`tool_call_end`).
    ToolCallEnd {
        /// Remote call id.
        call_id: String,
        /// Tool name.
        tool: String,
        /// Error text on failure.
        error: Option<String>,
        /// Wall-clock duration in milliseconds.
        duration_ms: u64,
    },
    /// A tool result (`tool_result`).
    ToolResult {
        /// Remote call id.
        call_id: String,
        /// Tool name.
        tool: String,
        /// Result content.
        content: String,
        /// Full result line count.
        content_line_count: usize,
        /// Structured metadata.
        metadata: Option<Value>,
        /// Whether the tool succeeded.
        success: bool,
    },
    /// A remote notice (`agent_notice`).
    AgentNotice {
        /// The notice text.
        message: String,
    },
    /// A remote error (`agent_error`).
    AgentError {
        /// The error text.
        error: String,
    },
    /// The remote turn's assistant message ended (`message_end`).
    MessageEnd {
        /// Remote message id.
        message_id: String,
        /// Terminal reason label.
        reason: String,
    },
    /// A remote tool is requesting permission (`permission_requested`).
    PermissionRequested {
        /// Remote request id.
        request_id: String,
        /// Permission kind.
        permission: String,
        /// Human-readable description.
        description: String,
        /// Optional pick-list.
        options: Vec<String>,
    },
    /// A remote tool is asking a question (`question_requested`).
    QuestionRequested {
        /// Remote request id.
        request_id: String,
        /// The question text.
        question: String,
        /// Optional pick-list.
        options: Vec<String>,
    },
    /// Any other remote event, carrying its event name.
    Other {
        /// The remote `event:` name as received.
        kind: String,
    },
}

/// The result of a completed remote turn.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RemoteTurnOutcome {
    /// The concatenated text of every `text_delta` in the turn.
    pub text: String,
    /// The terminal reason label (`stop`, `cancelled`, ...); empty if the stream
    /// ended without a `message_end`.
    pub stop_reason: String,
    /// How many streamed updates were delivered during the turn.
    pub updates: usize,
}

/// A reusable handle to one remote ragent server.
///
/// The handle performs no work at construction: it holds the connection
/// descriptor and a lazily-built HTTP client (built on the first async call, so
/// it is never constructed outside a Tokio runtime). One handle is cached per
/// `(url, id)` for the life of the process, so a session reuses one connection
/// pool instead of building a client per dispatch (FR-009).
pub struct RemoteBackend {
    id: String,
    name: String,
    url: Option<String>,
    api_key: Option<String>,
    client: OnceLock<reqwest::Client>,
}

impl std::fmt::Debug for RemoteBackend {
    /// Render the handle without its bearer key, so a log line or panic message
    /// cannot leak the credential resolved from the store (FR-035).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RemoteBackend")
            .field("id", &self.id)
            .field("name", &self.name)
            .field("url", &self.url)
            .field("api_key", &self.api_key.as_ref().map(|_| "[REDACTED]"))
            .finish()
    }
}

impl RemoteBackend {
    /// Build a remote adapter from a backend descriptor.
    #[must_use]
    pub fn from_descriptor(descriptor: &BackendConfig) -> Self {
        let url = descriptor
            .url
            .as_deref()
            .map(str::trim)
            .filter(|u| !u.is_empty())
            .map(|u| u.trim_end_matches('/').to_string());
        let api_key = descriptor
            .api_key
            .as_deref()
            .map(str::trim)
            .filter(|k| !k.is_empty())
            .map(str::to_string);
        Self {
            id: descriptor.id.clone(),
            name: descriptor.display_name().to_string(),
            url,
            api_key,
            client: OnceLock::new(),
        }
    }

    /// Build a remote adapter whose bearer key is resolved from the encrypted
    /// credential store instead of from a literal descriptor field (FR-010).
    ///
    /// The descriptor names the credential in `credentials` (non-secret
    /// identifiers); the value is resolved from `resolver` at spawn time and
    /// registered with the shared redaction registry, so it is masked in
    /// diagnostics (FR-035). When the descriptor already carries a literal
    /// `api_key`, that value wins and no store read is performed. A named
    /// credential the store does not hold is a
    /// [`RemoteErrorKind::Unreachable`] provisioning failure, so the turn never
    /// proceeds unauthenticated.
    ///
    /// # Errors
    ///
    /// Returns a [`RemoteError`] when the store cannot be read or a named
    /// credential is absent.
    pub fn resolve_credentials(
        descriptor: &BackendConfig,
        resolver: &dyn super::secrets::SecretResolver,
    ) -> Result<Self, RemoteError> {
        let backend = Self::from_descriptor(descriptor);
        if backend.api_key.is_some() || descriptor.credentials.is_empty() {
            return Ok(backend);
        }
        let secrets =
            super::secrets::resolve_secret_names(&descriptor.credentials, &backend.name, resolver)
                .map_err(|error| {
                    RemoteError::new(RemoteErrorKind::Unreachable, error.to_string())
                })?;
        // The first named credential is the bearer key.
        Ok(Self {
            api_key: secrets.first().map(|secret| secret.value.clone()),
            ..backend
        })
    }

    /// The registered backend id.
    #[must_use]
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The display name.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The configured base URL, if any.
    #[must_use]
    pub fn url(&self) -> Option<&str> {
        self.url.as_deref()
    }

    /// Whether a base URL is configured.
    #[must_use]
    pub fn has_url(&self) -> bool {
        self.url.is_some()
    }

    /// Whether a bearer key is configured, without revealing it (FR-010,
    /// FR-035).
    #[must_use]
    pub fn has_bearer_key(&self) -> bool {
        self.api_key.is_some()
    }

    /// The shared HTTP client, built on first use (inside the runtime).
    fn client(&self) -> &reqwest::Client {
        self.client.get_or_init(|| {
            reqwest::Client::builder()
                .connect_timeout(Duration::from_secs(10))
                .build()
                .unwrap_or_else(|error| {
                    // `build` can only fail on an invalid timeout (here a constant),
                    // so this is unreachable in practice; a plain client is still
                    // usable, but log rather than silently dropping the timeout.
                    tracing::warn!(%error, "remote backend: HTTP client build failed; using a default client");
                    reqwest::Client::new()
                })
        })
    }

    /// The required base URL, or an [`RemoteErrorKind::Unreachable`] failure.
    fn base_url(&self) -> Result<&str, RemoteError> {
        self.url.as_deref().ok_or_else(|| {
            RemoteError::new(
                RemoteErrorKind::Unreachable,
                "the remote backend has no base URL configured; set `url` on the \
                 backend descriptor. No tool is executed locally (FR-031).",
            )
        })
    }

    /// Apply bearer auth when a key is configured.
    fn with_auth(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.api_key.as_deref() {
            Some(key) => request.bearer_auth(key),
            None => request,
        }
    }

    /// Create (or reuse) a remote session rooted at `directory` (FR-009).
    ///
    /// The remote session id is cached per `(url, local_session_id)` so a second
    /// turn continues the same remote conversation instead of opening a new one.
    ///
    /// # Errors
    ///
    /// Returns [`RemoteErrorKind::Unreachable`] when the server cannot be
    /// reached and [`RemoteErrorKind::Protocol`] on a non-2xx answer.
    pub async fn create_session(
        &self,
        local_session_id: &str,
        directory: &Path,
    ) -> Result<String, RemoteError> {
        if let Some(remote_id) = cached_remote_session(self.id.as_str(), local_session_id) {
            return Ok(remote_id);
        }
        let base = self.base_url()?;
        let body = CreateSessionBody {
            directory: directory.display().to_string(),
        };
        let request = self
            .with_auth(self.client().post(format!("{base}/sessions")))
            .json(&body)
            .timeout(REQUEST_TIMEOUT);
        let response = request.send().await.map_err(|e| {
            RemoteError::new(
                RemoteErrorKind::Unreachable,
                format!("cannot reach the remote server at '{base}': {e}"),
            )
        })?;
        let status = response.status();
        if !status.is_success() {
            let detail = response.text().await.unwrap_or_default();
            return Err(RemoteError::new(
                RemoteErrorKind::Protocol,
                format!(
                    "the remote server rejected session creation (HTTP {}): {}",
                    status.as_u16(),
                    detail.trim()
                ),
            )
            .with_status(status.as_u16()));
        }
        let value: Value = response.json().await.map_err(|e| {
            RemoteError::new(
                RemoteErrorKind::Protocol,
                format!("the remote server returned an invalid session body: {e}"),
            )
        })?;
        let remote_id = value
            .get("id")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                RemoteError::new(
                    RemoteErrorKind::Protocol,
                    "the remote server's session body carried no `id`",
                )
            })?
            .to_string();
        remember_remote_session(self.id.as_str(), local_session_id, &remote_id);
        Ok(remote_id)
    }

    /// Open the SSE turn stream for `remote_session` and return the response.
    ///
    /// The request is not bounded by a total timeout here: the turn budget is
    /// enforced by [`relay_turn`]'s own deadline loop, which can also observe a
    /// cancellation request.
    ///
    /// # Errors
    ///
    /// Returns [`RemoteErrorKind::Unreachable`] when the server cannot be
    /// reached and [`RemoteErrorKind::Protocol`] on a non-2xx answer.
    pub async fn open_turn(
        &self,
        remote_session: &str,
        prompt: &str,
    ) -> Result<reqwest::Response, RemoteError> {
        let base = self.base_url()?;
        let url = format!("{base}/sessions/{remote_session}/messages");
        let body = SendMessageBody {
            content: prompt.to_string(),
        };
        let response = self
            .with_auth(self.client().post(url))
            .json(&body)
            .send()
            .await
            .map_err(|e| {
                RemoteError::new(
                    RemoteErrorKind::Unreachable,
                    format!("cannot reach the remote server at '{base}' to drive the turn: {e}"),
                )
            })?;
        let status = response.status();
        if !status.is_success() {
            let detail = response.text().await.unwrap_or_default();
            return Err(RemoteError::new(
                RemoteErrorKind::Protocol,
                format!(
                    "the remote server rejected the turn (HTTP {}): {}",
                    status.as_u16(),
                    detail.trim()
                ),
            )
            .with_status(status.as_u16()));
        }
        Ok(response)
    }

    /// Forward a permission reply to the remote server (used to answer a remote
    /// tool's `permission_requested`).
    ///
    /// # Errors
    ///
    /// Returns a [`RemoteError`] when the reply cannot be delivered; the caller
    /// logs it and continues, since the remote turn times out on its own.
    pub async fn reply_permission(
        &self,
        remote_session: &str,
        request_id: &str,
        allowed: bool,
    ) -> Result<(), RemoteError> {
        let base = self.base_url()?;
        let url = format!("{base}/sessions/{remote_session}/permission/{request_id}");
        let body = PermissionReplyBody {
            decision: if allowed { "allow" } else { "deny" },
        };
        let response = self
            .with_auth(self.client().post(url))
            .json(&body)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|e| {
                RemoteError::new(
                    RemoteErrorKind::Unreachable,
                    format!("cannot deliver a permission reply to '{base}': {e}"),
                )
            })?;
        if !response.status().is_success() {
            return Err(RemoteError::new(
                RemoteErrorKind::Protocol,
                format!(
                    "the remote server rejected the permission reply (HTTP {})",
                    response.status().as_u16()
                ),
            ));
        }
        Ok(())
    }
}

#[async_trait]
impl ExecutionBackend for RemoteBackend {
    fn kind(&self) -> ExecutionBackendKind {
        ExecutionBackendKind::Remote
    }

    /// Refuse every local tool dispatch.
    ///
    /// A session on the remote backend does not execute tools locally at all:
    /// the turn is relayed to the remote server, which runs the tools in its own
    /// environment. Reaching this method means a tool dispatch leaked past the
    /// session-level remote relay, so the call is refused rather than run on the
    /// host (FR-021, FR-031).
    async fn execute_tool(&self, call: BackendToolCall<'_>) -> anyhow::Result<ToolOutput> {
        let tool = call.tool.name().to_string();
        tracing::warn!(
            backend = "remote",
            tool = %tool,
            "tool dispatch reached the remote backend; refusing (remote turns are relayed, not run locally)"
        );
        Err(BackendError::new(
            ExecutionBackendKind::Remote,
            BackendErrorKind::Unavailable,
            format!(
                "the remote execution backend relays turns to '{}' over REST+SSE and does not \
                 execute the '{tool}' tool locally (FR-021, FR-031).",
                self.name
            ),
        )
        .into())
    }
}

/// Request body for `POST /sessions`.
#[derive(Serialize)]
struct CreateSessionBody {
    directory: String,
}

/// Request body for `POST /sessions/{id}/messages`.
#[derive(Serialize)]
struct SendMessageBody {
    content: String,
}

/// Request body for `POST /sessions/{id}/permission/{req_id}`.
#[derive(Serialize)]
struct PermissionReplyBody {
    decision: &'static str,
}

/// Relay one turn to `backend` and render each streamed update through
/// `on_update` (FR-021).
///
/// # Errors
///
/// Returns a [`RemoteError`] when the remote server is unreachable, the stream
/// fails part-way through the turn, a frame is malformed, the turn times out, or
/// the turn is cancelled (FR-034). On every failure the turn's tools are never
/// re-executed locally.
pub async fn relay_turn(
    backend: &RemoteBackend,
    local_session_id: &str,
    directory: &Path,
    prompt: &str,
    cancel: &std::sync::atomic::AtomicBool,
    on_update: &mut (dyn FnMut(RemoteUpdate) + Send),
) -> Result<RemoteTurnOutcome, RemoteError> {
    use std::sync::atomic::Ordering;

    let remote_session = backend.create_session(local_session_id, directory).await?;
    let response = backend.open_turn(&remote_session, prompt).await?;

    let mut stream = Box::pin(response.bytes_stream());
    let mut buffer = String::new();
    let mut outcome = RemoteTurnOutcome::default();
    let deadline = tokio::time::Instant::now() + TURN_TIMEOUT;

    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(RemoteError::new(
                RemoteErrorKind::Cancelled,
                "the remote turn was cancelled by the user",
            ));
        }
        if tokio::time::Instant::now() >= deadline {
            return Err(RemoteError::new(
                RemoteErrorKind::Timeout,
                format!(
                    "the remote turn did not complete within {}s",
                    TURN_TIMEOUT.as_secs()
                ),
            ));
        }

        match tokio::time::timeout(POLL_INTERVAL, stream.next()).await {
            // No frame yet: loop to re-check cancellation and the deadline.
            Err(_elapsed) => continue,
            // The server closed the stream.
            Ok(None) => {
                return Err(RemoteError::new(
                    RemoteErrorKind::Protocol,
                    "the remote server closed the event stream before the turn completed",
                ));
            }
            Ok(Some(Err(e))) => {
                return Err(RemoteError::new(
                    RemoteErrorKind::Protocol,
                    format!("the remote event stream failed part-way through the turn: {e}"),
                ));
            }
            Ok(Some(Ok(bytes))) => {
                buffer.push_str(&String::from_utf8_lossy(&bytes));
                for frame in drain_frames(&mut buffer) {
                    let Some((name, data)) = parse_sse_frame(&frame) else {
                        continue;
                    };
                    let update = decode_update(&name, &data);
                    if let RemoteUpdate::TextDelta { text } = &update {
                        outcome.text.push_str(text);
                    }
                    let terminal = match &update {
                        RemoteUpdate::MessageEnd { message_id, reason } => {
                            // The AGENTS.md acknowledgment exchange uses the
                            // `init` message id; it is not the real turn end.
                            if message_id == "init" {
                                false
                            } else {
                                // The reason is already decoded from this frame, so
                                // no second parse of `data` is needed.
                                outcome.stop_reason = if reason.is_empty() {
                                    "stop".to_string()
                                } else {
                                    reason.clone()
                                };
                                true
                            }
                        }
                        _ => false,
                    };
                    outcome.updates += 1;
                    on_update(update);
                    if terminal {
                        return Ok(outcome);
                    }
                }
            }
        }
    }
}

/// Split `buffer` into complete SSE frames, leaving any partial tail in place.
fn drain_frames(buffer: &mut String) -> Vec<String> {
    let mut frames = Vec::new();
    // Frames are separated by a blank line; accept both `\n\n` and the CRLF
    // form a proxy might emit.
    while let Some(index) = find_frame_boundary(buffer) {
        let frame: String = buffer.drain(..index.1).collect();
        frames.push(frame);
    }
    frames
}

/// Return the `(start, end)` byte range of the next complete frame, if any.
fn find_frame_boundary(buffer: &str) -> Option<(usize, usize)> {
    let lf = buffer.find("\n\n").map(|i| (i, i + 2));
    let crlf = buffer.find("\r\n\r\n").map(|i| (i, i + 4));
    match (lf, crlf) {
        (Some(a), Some(b)) => Some(if a.0 <= b.0 { a } else { b }),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Parse one SSE frame into its `(event, data)` pair.
///
/// Multiple `data:` lines are concatenated with `\n`, per the SSE grammar.
fn parse_sse_frame(frame: &str) -> Option<(String, String)> {
    let mut event: Option<String> = None;
    let mut data_lines: Vec<&str> = Vec::new();
    for line in frame.lines() {
        if let Some(rest) = line.strip_prefix("event:") {
            event = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("data:") {
            data_lines.push(rest.strip_prefix(' ').unwrap_or(rest));
        }
    }
    if data_lines.is_empty() {
        return None;
    }
    // Default the event name to the payload's `type` tag (the server serialises
    // `Event` with `#[serde(tag = "type")]`, so the body is self-describing).
    let data = data_lines.join("\n");
    let name = event.unwrap_or_else(|| {
        serde_json::from_str::<Value>(&data)
            .ok()
            .and_then(|v| v.get("type").and_then(Value::as_str).map(str::to_string))
            .unwrap_or_default()
    });
    Some((name, data))
}

/// Decode one `(event, data)` pair into a [`RemoteUpdate`].
///
/// Public for tests and for consumers that need to interpret a captured remote
/// stream without a live connection.
#[must_use]
pub fn parse_remote_update(name: &str, data: &str) -> RemoteUpdate {
    decode_update(name, data)
}

/// Split an in-memory SSE body into complete frames (public for tests).
#[must_use]
pub fn split_sse_frames(input: &str) -> Vec<String> {
    let mut buffer = input.to_string();
    drain_frames(&mut buffer)
}

/// Decode one `(event, data)` pair into a [`RemoteUpdate`].
fn decode_update(name: &str, data: &str) -> RemoteUpdate {
    let value: Value = match serde_json::from_str(data) {
        Ok(value) => value,
        Err(_) => {
            return RemoteUpdate::Other {
                kind: format!("malformed:{name}"),
            };
        }
    };
    let string = |key: &str| {
        value
            .get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    match name {
        "session_created" => RemoteUpdate::SessionCreated,
        "message_start" => RemoteUpdate::MessageStart {
            message_id: string("message_id"),
        },
        "text_delta" => RemoteUpdate::TextDelta {
            text: string("text"),
        },
        "reasoning_delta" => RemoteUpdate::ReasoningDelta {
            text: string("text"),
        },
        "tool_call_start" => RemoteUpdate::ToolCallStart {
            call_id: string("call_id"),
            tool: string("tool"),
        },
        "tool_call_end" => RemoteUpdate::ToolCallEnd {
            call_id: string("call_id"),
            tool: string("tool"),
            error: value
                .get("error")
                .and_then(Value::as_str)
                .map(str::to_string),
            duration_ms: value
                .get("duration_ms")
                .and_then(Value::as_u64)
                .unwrap_or(0),
        },
        "tool_result" => RemoteUpdate::ToolResult {
            call_id: string("call_id"),
            tool: string("tool"),
            content: string("content"),
            content_line_count: value
                .get("content_line_count")
                .and_then(Value::as_u64)
                .unwrap_or(0) as usize,
            metadata: value.get("metadata").cloned(),
            success: value
                .get("success")
                .and_then(Value::as_bool)
                .unwrap_or(false),
        },
        "agent_notice" => RemoteUpdate::AgentNotice {
            message: string("message"),
        },
        "agent_error" => RemoteUpdate::AgentError {
            error: string("error"),
        },
        "message_end" => RemoteUpdate::MessageEnd {
            message_id: string("message_id"),
            reason: string("reason"),
        },
        "permission_requested" => RemoteUpdate::PermissionRequested {
            request_id: string("request_id"),
            permission: string("permission"),
            description: string("description"),
            options: parse_options(&value),
        },
        "question_requested" => RemoteUpdate::QuestionRequested {
            request_id: string("request_id"),
            question: string("question"),
            options: parse_options(&value),
        },
        other => RemoteUpdate::Other {
            kind: other.to_string(),
        },
    }
}

/// Extract a `question`/`permission` request's optional pick-list.
fn parse_options(value: &Value) -> Vec<String> {
    value
        .get("options")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

/// Process-wide map of `(backend id, local session id)` -> remote session
/// id, so a second turn continues the same remote conversation.
type SessionCache = Mutex<HashMap<String, String>>;

fn session_cache() -> &'static SessionCache {
    static CACHE: OnceLock<SessionCache> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn session_cache_key(backend_id: &str, local_session_id: &str) -> String {
    format!("{backend_id}{KEY_SEP}{local_session_id}")
}

/// Look up the cached remote session id for a local session.
#[must_use]
pub fn cached_remote_session(backend_id: &str, local_session_id: &str) -> Option<String> {
    let cache = session_cache();
    let guard = match cache.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard
        .get(&session_cache_key(backend_id, local_session_id))
        .cloned()
}

/// Remember the remote session id that backs a local session.
pub fn remember_remote_session(backend_id: &str, local_session_id: &str, remote_id: &str) {
    let cache = session_cache();
    let mut guard = match cache.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.insert(
        session_cache_key(backend_id, local_session_id),
        remote_id.to_string(),
    );
}

/// Forget a local session's remote mapping (used by tests and session teardown).
pub fn forget_remote_session(backend_id: &str, local_session_id: &str) {
    let cache = session_cache();
    let mut guard = match cache.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    guard.remove(&session_cache_key(backend_id, local_session_id));
}

/// Resolve the remote descriptor a config selects, if the active backend is
/// `remote` (FR-001, FR-004, FR-009).
///
/// Returns the configured descriptor when one is present; a bare `remote`
/// label with no registered entry yields [`BackendConfig::default`] (no URL), so
/// the backend fails at provisioning with a clear message rather than silently
/// resolving to host execution (FR-031).
#[must_use]
pub fn remote_backend_config(config: &ragent_config::Config) -> Option<BackendConfig> {
    if config.effective_execution_backend() != ExecutionBackendKind::Remote {
        return None;
    }
    Some(
        config
            .effective_backend_config()
            .cloned()
            .unwrap_or_default(),
    )
}

/// Validate that a config names a usable remote backend, for tests and callers
/// that want a descriptive error before a turn starts.
///
/// # Errors
///
/// Returns [`RemoteErrorKind::Unreachable`] when no base URL is configured.
pub fn validate_remote_backend(descriptor: &BackendConfig) -> Result<(), RemoteError> {
    let backend = RemoteBackend::from_descriptor(descriptor);
    backend.base_url().map(|_| ())
}
