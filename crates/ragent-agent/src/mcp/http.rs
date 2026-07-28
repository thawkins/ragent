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
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use tokio::time::sleep;

use super::{McpClientBackend, McpToolDef};

/// Backoff delays between retry attempts (1s, 2s, 4s).
const RETRY_BACKOFFS: &[Duration] = &[
    Duration::from_secs(1),
    Duration::from_secs(2),
    Duration::from_secs(4),
];

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
    /// Underlying reqwest client.
    client: reqwest::Client,
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
            client: reqwest::Client::new(),
            next_id: AtomicU64::new(1),
            disconnected: AtomicBool::new(false),
        }
    }

    /// Replace the underlying `reqwest::Client`.
    ///
    /// Useful in tests or when the caller needs custom timeouts / middleware.
    #[must_use]
    pub fn with_client(mut self, client: reqwest::Client) -> Self {
        self.client = client;
        self
    }

    /// Returns `true` if the client is currently in a disconnected state
    /// (FR-014). The flag is cleared on the next request attempt.
    #[must_use]
    pub fn is_disconnected(&self) -> bool {
        self.disconnected.load(Ordering::SeqCst)
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
            .client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json");

        for (key, value) in &self.headers {
            builder = builder.header(key, value);
        }

        let response = builder
            .body(body)
            .send()
            .await
            .context("failed to send HTTP MCP request")?;

        let status = response.status();
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

        let parsed: JsonRpcResponse<Value> = serde_json::from_str(&text)
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
        let arguments = match input {
            Value::Object(map) => map,
            Value::Null => Map::new(),
            other => {
                let mut map = Map::new();
                map.insert("value".to_string(), other);
                map
            }
        };

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
                .unwrap_or_else(|| Value::Object(Map::new()));
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
}
