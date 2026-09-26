//! HTTP transport MCP client.
//!
//! Provides [`HttpMcpClient`], a lightweight JSON-RPC client that talks to an MCP
//! server over plain HTTP (or HTTPS) using the workspace `reqwest` client. It
//! implements [`McpClientBackend`] so it can be used interchangeably with the
//! stdio-based [`McpClient`](super::McpClient).
//!
//! Authentication and custom headers are supplied once at construction time via
//! the `headers` map and are attached to every outgoing request.
//!
//! # Auto-reconnect (FR-014)
//!
//! When a request fails with a network error or non-2xx response, the client
//! retries up to 3 times with exponential backoff (1s, 2s, 4s). If all retries
//! are exhausted, the client marks itself as `disconnected` and emits a
//! `tracing::warn!`. On the next tool invocation that targets this server, the
//! `disconnected` flag is cleared and the request is attempted again.

use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::sleep;

use super::{McpClientBackend, McpToolDef};

/// Backoff delays between retry attempts (1s, 2s, 4s).
const RETRY_BACKOFFS: &[Duration] = &[
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

/// How long [`probe`] waits for an endpoint's `initialize` before giving up.
///
/// Deliberately short: the probe runs on the startup path and a miss is the
/// common case (nothing is listening), so it must not inherit the retry/backoff
/// envelope used for servers ragent owns.
const PROBE_TIMEOUT: Duration = Duration::from_millis(800);

/// The process-wide MCP HTTP client.
///
/// `reqwest::Client::new()` must be called from inside a Tokio runtime context:
/// it builds the connection pool's `PollEvented` TCP sockets eagerly, so calling
/// it outside a runtime panics with "there is no reactor running". A client
/// constructed lazily on first use is therefore built from the async request
/// path (which always runs on the runtime), never from `app::init`/`Config`
/// construction. Sharing one client also lets the connection pool be reused
/// across every MCP HTTP server instead of one pool per server.
fn shared_client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(reqwest::Client::new)
}

/// Parse a JSON-RPC reply body into its `result`, unwrapping a Streamable-HTTP
/// Server-Sent-Events frame first when the server answered with one.
///
/// A Streamable-HTTP endpoint may reply either with a bare JSON-RPC object or
/// with an `event: message` / `data: {...}` SSE frame (the MongoDB MCP server on
/// its 2025-era sessionful path does the latter). Both carry the same
/// [`JsonRpcResponse`]; this normalises them so the plain-JSON-RPC client needs
/// no SSE parser.
fn parse_jsonrpc_result(method: &str, text: &str) -> Result<Value> {
    let payload = unwrap_sse_frame(text);
    let parsed: JsonRpcResponse<Value> = serde_json::from_str(&payload)
        .with_context(|| format!("invalid JSON-RPC response for '{}': {}", method, text))?;

    if let Some(error) = parsed.error {
        anyhow::bail!(
            "JSON-RPC error for '{}': code {} - {}",
            method,
            error.code,
            error.message
        );
    }

    parsed
        .result
        .context(format!("JSON-RPC response for '{}' missing result", method))
}

/// Return the JSON payload of `text`: the joined `data:` lines of an SSE
/// frame, or the input unchanged when it is already a bare JSON document.
///
/// The SSE grammar concatenates every `data:` line of an event with `\n`
/// between them, so a server that splits a large JSON-RPC payload across
/// several `data:` lines must not be reduced to its first line.
fn unwrap_sse_frame(text: &str) -> std::borrow::Cow<'_, str> {
    let trimmed = text.trim_start();
    if !trimmed.starts_with("event:") && !trimmed.starts_with("data:") {
        return std::borrow::Cow::Borrowed(text.trim());
    }
    let data_lines: Vec<&str> = trimmed
        .lines()
        .filter_map(|line| line.strip_prefix("data:"))
        .map(str::trim)
        .collect();
    match data_lines.len() {
        0 => std::borrow::Cow::Borrowed(text.trim()),
        1 => std::borrow::Cow::Borrowed(data_lines[0]),
        _ => std::borrow::Cow::Owned(data_lines.join("\n")),
    }
}

/// JSON-RPC 2.0 request envelope.
#[derive(Debug, Serialize)]
struct JsonRpcRequest<T> {
    jsonrpc: String,
    id: u64,
    method: String,
    params: T,
}

/// JSON-RPC 2.0 error payload.
#[derive(Debug, Deserialize)]
struct JsonRpcError {
    code: i64,
    message: String,
}

/// JSON-RPC 2.0 response envelope.
#[derive(Debug, Deserialize)]
struct JsonRpcResponse<T> {
    #[serde(default)]
    result: Option<T>,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

/// One live MCP Streamable-HTTP endpoint discovered by [`probe`].
///
/// Carries the exact endpoint that answered and the `mcp-session-id` that
/// endpoint issued for the probe's `initialize`, so a caller can adopt the
/// running server (build a client with [`HttpMcpClient::new`] +
/// [`HttpMcpClient::with_session_id`]) without repeating the handshake.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LiveEndpoint {
    /// The MCP endpoint URL that answered (e.g. `http://127.0.0.1:3000/mcp`).
    pub url: String,
    /// The `mcp-session-id` the endpoint issued, when it is sessionful.
    pub session_id: Option<String>,
}

/// Probe whether an already-running MCP server is reachable at `url`.
///
/// Performs the MCP `initialize` handshake the Streamable-HTTP transport
/// requires and returns the endpoint plus its session id when a live server
/// answered. Returns `None` for a URL this is not HTTP/HTTPS, or when nothing
/// answered / the endpoint is not an MCP server (so the caller falls through to
/// starting its own instance).
///
/// A bare TCP-listening socket is *not* enough: `initialize` must round-trip, so
/// an unrelated process squatting the port is rejected.
pub async fn probe(url: &str) -> Option<LiveEndpoint> {
    if !(url.starts_with("http://") || url.starts_with("https://")) {
        return None;
    }
    let mut client = HttpMcpClient::new(url, HashMap::new());
    // A single attempt on a short timeout: a probe asks "is something already
    // listening here?" and a miss must fall through to a spawn quickly. The
    // client's own retry/backoff envelope (1s + 2s + 4s) is right for a server
    // ragent owns and wrong for a speculative probe.
    match tokio::time::timeout(PROBE_TIMEOUT, client.initialize()).await {
        Ok(Ok(_)) => Some(LiveEndpoint {
            url: url.to_string(),
            session_id: client.session_id().map(str::to_string),
        }),
        Ok(Err(error)) => {
            tracing::debug!(url = %crate::sanitize::redact_secrets(url), error = %error, "MCP endpoint probe failed");
            None
        }
        Err(_) => {
            tracing::debug!(url = %crate::sanitize::redact_secrets(url), "MCP endpoint probe timed out");
            None
        }
    }
}

/// MCP client that sends JSON-RPC requests over HTTP.
///
/// Each instance targets a single MCP server URL. The client is stateless apart
/// from the monotonically increasing JSON-RPC request id and the
/// `disconnected` flag, so reconnecting is simply a matter of clearing the
/// flag and issuing the next request.
#[derive(Debug)]
pub struct HttpMcpClient {
    /// Base URL for the MCP HTTP endpoint.
    url: String,
    /// Optional custom headers attached to every request.
    headers: HashMap<String, String>,
    /// Underlying reqwest client. `None` means "resolve [`shared_client`] on
    /// first use", so constructing the client is safe outside a Tokio runtime.
    client: Option<reqwest::Client>,
    /// The `mcp-session-id` a sessionful Streamable-HTTP server issued during
    /// `initialize`, replayed on every later request. `None` for a stateless
    /// server, or before [`HttpMcpClient::initialize`] has run.
    session_id: Option<String>,
    /// Next JSON-RPC request id.
    next_id: AtomicU64,
    /// `true` when the server has been marked disconnected after exhausting
    /// all retry attempts (FR-014). Cleared on the next invocation to give
    /// the server another chance.
    disconnected: AtomicBool,
}

impl HttpMcpClient {
    /// Create a new HTTP MCP client for the given URL and optional headers.
    ///
    /// # Arguments
    ///
    /// * `url` — MCP server HTTP endpoint, e.g. `http://localhost:3000/mcp`.
    /// * `headers` — map of header names to values sent with every request.
    ///
    /// # Examples
    ///
    /// ```
    /// use std::collections::HashMap;
    /// use ragent_agent::mcp::http::HttpMcpClient;
    ///
    /// let client = HttpMcpClient::new("http://localhost:3000/mcp", HashMap::new());
    /// ```
    #[must_use]
    pub fn new(url: impl Into<String>, headers: HashMap<String, String>) -> Self {
        Self {
            url: url.into(),
            headers,
            client: None,
            session_id: None,
            next_id: AtomicU64::new(1),
            disconnected: AtomicBool::new(false),
        }
    }

    /// The reqwest client for this instance: the injected one, or the shared
    /// process-wide client resolved on first use.
    fn client(&self) -> &reqwest::Client {
        self.client.as_ref().unwrap_or_else(|| shared_client())
    }

    /// Replace the underlying `reqwest::Client`.
    ///
    /// Useful in tests or when the caller needs custom timeouts / middleware.
    #[must_use]
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = Some(client);
        self
    }

    /// Attach an already-negotiated `mcp-session-id`, so this client joins a
    /// session opened elsewhere (see [`Self::initialize`] and
    /// [`McpClient::adopt_connected`](super::McpClient::adopt_connected)).
    #[must_use]
    pub fn with_session_id(mut self, session_id: Option<String>) -> Self {
        self.session_id = session_id;
        self
    }

    /// Returns `true` if the client is currently in a disconnected state
    /// (FR-014). The flag is cleared on the next request attempt.
    #[must_use]
    pub fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::SeqCst)
    }

    /// The `mcp-session-id` sent with every request, when the server is a
    /// sessionful Streamable-HTTP server.
    ///
    /// A server that issues a session refuses every request other than
    /// `initialize` unless it carries the id, so a client that only speaks the
    /// plain JSON-RPC subset ([`Self::post_once`]) must replay the session the
    /// [`initialize`](Self::initialize) call negotiated.
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        self.session_id.as_deref()
    }

    /// Perform the MCP `initialize` handshake, capturing any `mcp-session-id`
    /// the server returns for use by later requests.
    ///
    /// This is the subset handshake a sessionful Streamable-HTTP server requires
    /// before it will answer `tools/list`: a bare [`list_tools`] on a fresh
    /// client is rejected. Returns the server's negotiated `protocolVersion`
    /// when it answered with a JSON-RPC result.
    ///
    /// # Errors
    ///
    /// Returns an error when the request fails, the status is not 2xx, or the
    /// response is not a parseable JSON-RPC reply.
    ///
    /// [`list_tools`]: McpClientBackend::list_tools
    pub async fn initialize(&mut self) -> Result<Value> {
        let params = serde_json::json!({
            "protocolVersion": "2024-11-05",
            "capabilities": {},
            "clientInfo": { "name": "ragent", "version": env!("CARGO_PKG_VERSION") }
        });
        let (result, session_id) = self
            .post_once_capture_session("initialize", &params)
            .await?;
        if session_id.is_some() {
            self.session_id = session_id;
        }
        Ok(result)
    }

    /// Build and send a single JSON-RPC POST request (no retries).
    ///
    /// Returns the parsed JSON-RPC `result` on success, or an error on network
    /// failure, non-2xx status, or JSON-RPC error.
    async fn post_once<T: Serialize + Send + Sync>(
        &self,
        method: &str,
        params: &T,
    ) -> Result<Value> {
        self.post_once_capture_session(method, params)
            .await
            .map(|(result, _)| result)
    }

    /// As [`Self::post_once`], but also returns the `mcp-session-id` response
    /// header when the server issued one (only `initialize` does).
    async fn post_once_capture_session<T: Serialize + Send + Sync>(
        &self,
        method: &str,
        params: &T,
    ) -> Result<(Value, Option<String>)> {
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id,
            method: method.to_string(),
            params,
        };
        let body =
            serde_json::to_string(&request).context("failed to serialize JSON-RPC request")?;

        let mut builder = self
            .client()
            .post(&self.url)
            .header("Content-Type", "application/json")
            // A sessionful Streamable-HTTP server (the MongoDB MCP server among
            // them) only answers a negotiated session; advertising the SSE
            // media type lets such a server reply with an `event: message`
            // frame, which is unwrapped below.
            .header("Accept", "application/json, text/event-stream");

        if let Some(session_id) = &self.session_id {
            builder = builder.header("mcp-session-id", session_id);
        }

        for (key, value) in &self.headers {
            builder = builder.header(key, value);
        }

        let response = builder
            .body(body)
            .send()
            .await
            .context("failed to send HTTP MCP request")?;

        let status = response.status();
        let session_id = response
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let text = response
            .text()
            .await
            .context("failed to read HTTP MCP response body")?;

        if !status.is_success() {
            anyhow::bail!(
                "HTTP MCP request '{}' failed with status {}: {}",
                method,
                status,
                text
            );
        }

        let parsed = parse_jsonrpc_result(method, &text)?;
        Ok((parsed, session_id))
    }

    /// Build and send a JSON-RPC POST request with auto-reconnect (FR-014).
    ///
    /// On network error or non-2xx response, the request is retried up to 3
    /// times with exponential backoff (1s, 2s, 4s). If all retries are
    /// exhausted, the client is marked as `disconnected` and a
    /// `tracing::warn!` is emitted. On the next invocation the `disconnected`
    /// flag is cleared and the request is attempted again.
    async fn post<T: Serialize + Send + Sync>(&self, method: &str, params: T) -> Result<Value> {
        // Clear the disconnected flag — we're giving the server another chance.
        if self.disconnected.swap(false, Ordering::SeqCst) {
            tracing::info!(
                url = %self.url,
                method,
                "Retrying HTTP MCP server after disconnect"
            );
        }

        let mut last_error: Option<anyhow::Error> = None;

        let mut attempts: Vec<Option<Duration>> = vec![None];
        attempts.extend(RETRY_BACKOFFS.iter().copied().map(Some));

        for (attempt, backoff) in attempts.into_iter().enumerate() {
            if let Some(delay) = backoff {
                tracing::warn!(
                    url = %self.url,
                    method,
                    attempt,
                    delay_ms = delay.as_millis(),
                    "HTTP MCP request failed, retrying with backoff"
                );
                sleep(delay).await;
            }

            match self.post_once(method, &params).await {
                Ok(value) => return Ok(value),
                Err(e) => {
                    last_error = Some(e);
                }
            }
        }

        // All retries exhausted — mark as disconnected.
        self.disconnected.store(true, Ordering::SeqCst);
        tracing::warn!(
            url = %self.url,
            method,
            "HTTP MCP server marked disconnected after exhausting all retry attempts"
        );

        Err(last_error
            .unwrap_or_else(|| anyhow::anyhow!("HTTP MCP request failed with no error captured")))
    }
}

#[async_trait]
impl McpClientBackend for HttpMcpClient {
    async fn list_tools(&self) -> Vec<McpToolDef> {
        match self.post("tools/list", serde_json::json!({})).await {
            Ok(value) => parse_tool_list(value),
            Err(error) => {
                tracing::warn!(error = %error, "failed to list HTTP MCP tools");
                Vec::new()
            }
        }
    }

    async fn list_tools_for_server(&self, _server_id: &str) -> Vec<McpToolDef> {
        // HTTP clients target a single configured URL; the server id is implicit.
        self.list_tools().await
    }

    async fn refresh_tools(&mut self) -> Result<()> {
        // Stateless HTTP transport: nothing to refresh beyond re-listing tools.
        Ok(())
    }

    async fn refresh_tools_for_server(&mut self, _server_id: &str) -> Result<Vec<McpToolDef>> {
        Ok(self.list_tools().await)
    }

    async fn call_tool(&self, _server_id: &str, tool_name: &str, input: Value) -> Result<Value> {
        let arguments = super::normalize_mcp_arguments(input);

        let params = serde_json::json!({
            "name": tool_name,
            "arguments": arguments,
        });

        self.post("tools/call", params).await
    }

    async fn call_tool_by_name(&self, tool_name: &str, input: Value) -> Result<Value> {
        self.call_tool("", tool_name, input).await
    }
}

/// Parse the `tools/list` JSON-RPC result into [`McpToolDef`]s.
fn parse_tool_list(value: Value) -> Vec<McpToolDef> {
    let tools = value
        .get("tools")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    tools
        .into_iter()
        .filter_map(|tool| {
            let name = tool.get("name")?.as_str()?.to_string();
            let description = tool
                .get("description")
                .and_then(|d| d.as_str())
                .unwrap_or("")
                .to_string();
            let parameters = tool
                .get("inputSchema")
                .cloned()
                .unwrap_or_else(|| Value::Object(serde_json::Map::new()));
            Some(McpToolDef {
                name,
                description,
                parameters,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_tool_list_extracts_tools() {
        let value = serde_json::json!({
            "tools": [
                {
                    "name": "echo",
                    "description": "Echoes input",
                    "inputSchema": {"type": "object", "properties": {"msg": {"type": "string"}}}
                }
            ]
        });
        let tools = parse_tool_list(value);
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "echo");
        assert_eq!(tools[0].description, "Echoes input");
    }

    #[test]
    fn parse_tool_list_handles_empty_result() {
        let tools = parse_tool_list(serde_json::json!({}));
        assert!(tools.is_empty());
    }

    #[test]
    fn new_client_starts_connected() {
        let client = HttpMcpClient::new("http://localhost:9999", HashMap::new());
        assert!(!client.is_disconnected());
    }

    #[test]
    fn unwrap_sse_frame_joins_multiline_data_fields() {
        // The SSE grammar concatenates the `data:` lines of one event with
        // `\n`; a server that splits a payload across lines must round-trip.
        let frame = "event: message\ndata: {\"jsonrpc\":\ndata: \"2.0\",\"result\":1}\n\n";
        assert_eq!(
            unwrap_sse_frame(frame),
            "{\"jsonrpc\":\n\"2.0\",\"result\":1}"
        );
    }

    #[test]
    fn unwrap_sse_frame_keeps_bare_json_and_single_data_line() {
        let bare = "{\"result\": 1}";
        assert_eq!(unwrap_sse_frame(bare), bare);
        let framed = "event: message\ndata: {\"result\": 1}\n\n";
        assert_eq!(unwrap_sse_frame(framed), "{\"result\": 1}");
    }
}
