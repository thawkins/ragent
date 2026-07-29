//! Chrome DevTools Protocol (CDP) WebSocket client.
//!
//! Implements the low-level CDP wire protocol: JSON-RPC over WebSocket with
//! incremental message IDs, response correlation, and event fan-out. This
//! module is transport-only — it knows nothing about browser actions. The
//! higher-level [`super::BrowserTool`] wraps this client to implement the
//! `open`/`snapshot`/`click`/… action surface.
//!
//! # Protocol overview
//!
//! CDP uses a single WebSocket connection per target (browser tab). Each
//! command is a JSON object with `id`, `method`, and optional `params`. The
//! server responds with `{"id": <same>, "result": {...}}` on success or
//! `{"id": <same>, "error": {"code", "message", "data"}}` on failure. Events
//! are `{"method": "Page.loadEventFired", "params": {...}}` with no `id`.
//!
//! # Connection lifecycle
//!
//! 1. HTTP GET `http://<host>:<port>/json/version` → discover the
//!    `webSocketDebuggerUrl` for the browser-level endpoint.
//! 2. (Optional) `Target.createTarget` via the browser endpoint to open a new
//!    tab and receive its `targetId`.
//! 3. HTTP GET `http://<host>:<port>/json` → list open targets; pick one and
//!    use its `webSocketDebuggerUrl`.
//! 4. Open a WebSocket to the target's `webSocketDebuggerUrl`.
//! 5. Enable domains (`Page.enable`, `DOM.enable`, `Runtime.enable`, …).
//! 6. Send commands and correlate responses by `id`.
//!
//! # Design
//!
//! [`CdpConnection`] uses a task-based architecture: a background read loop
//! owns the WebSocket read half and dispatches responses to pending command
//! senders (oneshot channels) and events to a broadcast channel. The write
//! half is accessed via a channel guarded by a mutex for sequential sends.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;

use anyhow::{Result, bail};
use futures::SinkExt as _;
use futures::StreamExt as _;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use thiserror::Error;
use tokio::sync::{Mutex, broadcast, oneshot};
use tokio_tungstenite::tungstenite::Message as WsMessage;

/// Type alias for the pending-response map used by [`CdpConnection`].
type PendingMap = Arc<Mutex<HashMap<u64, oneshot::Sender<Result<Value>>>>>;

/// Default timeout for a single CDP command (30 seconds).
pub const DEFAULT_COMMAND_TIMEOUT_SECS: u64 = 30;

/// Default timeout for discovering the CDP endpoint (5 seconds).
pub const DEFAULT_DISCOVERY_TIMEOUT_SECS: u64 = 5;

/// CDP-specific errors.
#[derive(Debug, Error)]
pub enum CdpError {
    /// The CDP command returned an error response.
    #[error("CDP error {code}: {message}")]
    CommandError {
        /// CDP error code.
        code: i64,
        /// Human-readable error message from the browser.
        message: String,
    },
    /// The command timed out waiting for a response.
    #[error("CDP command timed out after {timeout_secs}s")]
    Timeout {
        /// The timeout duration in seconds.
        timeout_secs: u64,
    },
    /// The WebSocket connection was closed before a response arrived.
    #[error("CDP connection closed before response")]
    ConnectionClosed,
    /// Failed to discover or connect to the CDP endpoint.
    #[error("CDP connection failed: {0}")]
    ConnectionFailed(String),
    /// No suitable target (tab) was found.
    #[error("No CDP target found")]
    NoTarget,
}

/// Response from `GET /json/version` — the browser-level discovery endpoint.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VersionInfo {
    /// Browser version string (e.g. `"Chrome/131.0.6778.85"`).
    #[serde(rename = "Browser")]
    pub browser: String,
    /// V8 engine version.
    #[serde(default, rename = "V8")]
    pub v8: Option<String>,
    /// WebKit version.
    #[serde(default, rename = "WebKit")]
    pub webkit: Option<String>,
    /// Browser-level WebSocket debugger URL.
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: String,
    /// User agent string.
    #[serde(default, rename = "User-Agent")]
    pub user_agent: Option<String>,
}

/// Response from `GET /json` — list of available targets (tabs).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TargetInfo {
    /// Target id.
    pub id: String,
    /// Target type (e.g. `"page"`, `"background_page"`).
    #[serde(rename = "type")]
    pub target_type: String,
    /// Page title.
    pub title: String,
    /// Page URL.
    pub url: String,
    /// Per-target WebSocket debugger URL.
    #[serde(rename = "webSocketDebuggerUrl")]
    pub web_socket_debugger_url: String,
    /// Whether the tab is the active tab.
    #[serde(default)]
    pub attached: bool,
}

/// A CDP event received from the browser.
#[derive(Debug, Clone)]
pub struct CdpEvent {
    /// Event method (e.g. `"Page.loadEventFired"`).
    pub method: String,
    /// Event parameters.
    pub params: Value,
}

/// Low-level CDP WebSocket connection.
///
/// A background read loop owns the WebSocket read half and dispatches
/// responses to pending command senders (oneshot channels) and events to a
/// broadcast channel. The write half is accessed via an unbounded channel
/// guarded by a mutex for sequential command sends.
pub struct CdpConnection {
    /// Write channel — sends messages to the write task.
    write_tx: Mutex<tokio::sync::mpsc::UnboundedSender<WsMessage>>,
    /// Pending command responses keyed by command id.
    pending: PendingMap,
    /// Event broadcast channel.
    events: broadcast::Sender<CdpEvent>,
    /// Next command id.
    next_id: AtomicU64,
    /// Connection closed flag.
    closed: Arc<AtomicBool>,
}

impl CdpConnection {
    /// Connect to a CDP target via its WebSocket debugger URL.
    ///
    /// # Arguments
    ///
    /// * `ws_url` — the `webSocketDebuggerUrl` from `GET /json` or
    ///   `GET /json/version`.
    ///
    /// # Errors
    ///
    /// Returns an error if the WebSocket connection cannot be established.
    pub async fn connect(ws_url: &str) -> Result<Self> {
        use tokio_tungstenite::connect_async;

        let (ws_stream, _response) = connect_async(ws_url)
            .await
            .map_err(|e| CdpError::ConnectionFailed(e.to_string()))?;

        let (write_tx, write_rx) = tokio::sync::mpsc::unbounded_channel::<WsMessage>();
        let write_tx_clone = write_tx.clone();
        let (events_tx, _) = broadcast::channel::<CdpEvent>(256);
        let pending: PendingMap = Arc::new(Mutex::new(HashMap::new()));
        let closed = Arc::new(AtomicBool::new(false));

        let (mut writer, mut reader) = ws_stream.split();

        // Write loop: forward messages from write_tx to the WebSocket.
        let closed_w = closed.clone();
        tokio::spawn(async move {
            let mut write_rx = write_rx;
            while let Some(msg) = write_rx.recv().await {
                if writer.send(msg).await.is_err() {
                    break;
                }
            }
            closed_w.store(true, Ordering::SeqCst);
            let _ = writer.close().await;
        });

        // Read loop: dispatch responses and events.
        let pending_r = pending.clone();
        let events_r = events_tx.clone();
        let closed_r = closed.clone();
        tokio::spawn(async move {
            while let Some(msg) = reader.next().await {
                match msg {
                    Ok(WsMessage::Text(text)) => {
                        if let Ok(parsed) = serde_json::from_str::<Value>(&text) {
                            Self::dispatch_message(&parsed, &pending_r, &events_r).await;
                        }
                    }
                    Ok(WsMessage::Binary(data)) => {
                        if let Ok(parsed) = serde_json::from_slice::<Value>(&data) {
                            Self::dispatch_message(&parsed, &pending_r, &events_r).await;
                        }
                    }
                    Ok(WsMessage::Close(_)) | Err(_) => {
                        closed_r.store(true, Ordering::SeqCst);
                        break;
                    }
                    Ok(WsMessage::Ping(payload)) => {
                        let _ = write_tx_clone.send(WsMessage::Pong(payload));
                    }
                    Ok(_) => {}
                }
            }
            // Mark all pending commands as failed on disconnect.
            closed_r.store(true, Ordering::SeqCst);
            let mut pending = pending_r.lock().await;
            for (_id, sender) in pending.drain() {
                let _ = sender.send(Err(CdpError::ConnectionClosed.into()));
            }
        });

        Ok(Self {
            write_tx: Mutex::new(write_tx),
            pending,
            events: events_tx,
            next_id: AtomicU64::new(1),
            closed,
        })
    }

    /// Dispatch a parsed CDP message to the appropriate pending command or
    /// event channel.
    async fn dispatch_message(
        msg: &Value,
        pending: &PendingMap,
        events: &broadcast::Sender<CdpEvent>,
    ) {
        // Response to a command (has "id" and either "result" or "error").
        if let Some(id) = msg.get("id").and_then(Value::as_u64) {
            let result = if let Some(error) = msg.get("error") {
                let code = error.get("code").and_then(Value::as_i64).unwrap_or(-1);
                let message = error
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or("unknown error")
                    .to_string();
                Err(anyhow::anyhow!(CdpError::CommandError { code, message }))
            } else {
                Ok(msg.get("result").cloned().unwrap_or(Value::Null))
            };

            let mut pending = pending.lock().await;
            if let Some(sender) = pending.remove(&id) {
                let _ = sender.send(result);
            }
        } else if let Some(method) = msg.get("method").and_then(Value::as_str) {
            // CDP event (has "method" but no "id").
            let event = CdpEvent {
                method: method.to_string(),
                params: msg.get("params").cloned().unwrap_or(Value::Null),
            };
            let _ = events.send(event);
        }
    }

    /// Check if the connection is closed.
    #[must_use]
    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Subscribe to CDP events.
    pub fn subscribe(&self) -> broadcast::Receiver<CdpEvent> {
        self.events.subscribe()
    }

    /// Send a CDP command and wait for the response.
    ///
    /// # Arguments
    ///
    /// * `method` — the CDP method (e.g. `"Page.navigate"`).
    /// * `params` — optional parameters as a JSON value.
    /// * `timeout` — maximum time to wait for a response.
    ///
    /// # Errors
    ///
    /// Returns [`CdpError::CommandError`] if the browser returns an error,
    /// [`CdpError::Timeout`] if the response doesn't arrive in time, or
    /// [`CdpError::ConnectionClosed`] if the connection drops.
    pub async fn command(
        &self,
        method: &str,
        params: Option<Value>,
        timeout: Duration,
    ) -> Result<Value> {
        if self.is_closed() {
            bail!(CdpError::ConnectionClosed);
        }

        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let request = json!({
            "id": id,
            "method": method,
            "params": params.unwrap_or(Value::Object(serde_json::Map::new())),
        });

        let (tx, rx) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            pending.insert(id, tx);
        }

        let write_tx = self.write_tx.lock().await;
        let msg = WsMessage::Text(request.to_string().into());
        write_tx
            .send(msg)
            .map_err(|_| anyhow::anyhow!(CdpError::ConnectionClosed))?;
        drop(write_tx);

        match tokio::time::timeout(timeout, rx).await {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => bail!(CdpError::ConnectionClosed),
            Err(_) => {
                // Remove the pending entry on timeout.
                let mut pending = self.pending.lock().await;
                pending.remove(&id);
                bail!(CdpError::Timeout {
                    timeout_secs: timeout.as_secs(),
                });
            }
        }
    }

    /// Send a CDP command with the default timeout.
    ///
    /// # Errors
    ///
    /// See [`CdpConnection::command`].
    pub async fn command_default(&self, method: &str, params: Option<Value>) -> Result<Value> {
        self.command(
            method,
            params,
            Duration::from_secs(DEFAULT_COMMAND_TIMEOUT_SECS),
        )
        .await
    }

    /// Close the connection gracefully.
    pub fn close(&self) {
        self.closed.store(true, Ordering::SeqCst);
        if let Ok(write_tx) = self.write_tx.try_lock() {
            let _ = write_tx.send(WsMessage::Close(None));
        }
    }
}

/// Discover the CDP browser version info by querying `GET /json/version`.
///
/// # Arguments
///
/// * `http_endpoint` — the HTTP base URL (e.g. `"http://127.0.0.1:9222"`).
///
/// # Errors
///
/// Returns an error if the HTTP request fails or the response cannot be
/// parsed.
pub async fn discover_version(http_endpoint: &str) -> Result<VersionInfo> {
    let url = format!("{http_endpoint}/json/version");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(DEFAULT_DISCOVERY_TIMEOUT_SECS))
        .build()?;
    let resp = client.get(&url).send().await?;
    let info: VersionInfo = resp.json().await?;
    Ok(info)
}

/// List available CDP targets by querying `GET /json`.
///
/// Only targets of type `"page"` are returned by default.
///
/// # Arguments
///
/// * `http_endpoint` — the HTTP base URL (e.g. `"http://127.0.0.1:9222"`).
///
/// # Errors
///
/// Returns an error if the HTTP request fails or the response cannot be
/// parsed.
pub async fn list_targets(http_endpoint: &str) -> Result<Vec<TargetInfo>> {
    let url = format!("{http_endpoint}/json");
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(DEFAULT_DISCOVERY_TIMEOUT_SECS))
        .build()?;
    let resp = client.get(&url).send().await?;
    let targets: Vec<TargetInfo> = resp.json().await?;
    Ok(targets)
}

/// Find the first page-type target, or return [`CdpError::NoTarget`].
///
/// # Errors
///
/// Returns [`CdpError::NoTarget`] if no `"page"` target exists.
pub fn first_page_target(targets: &[TargetInfo]) -> Result<&TargetInfo> {
    targets
        .iter()
        .find(|t| t.target_type == "page")
        .ok_or_else(|| anyhow::anyhow!(CdpError::NoTarget))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_info_deserialises() {
        let json = r#"{
            "Browser": "Chrome/131.0.6778.85",
            "V8": "13.1.201.7",
            "WebKit": "537.36",
            "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/browser/abc-123",
            "User-Agent": "Mozilla/5.0"
        }"#;
        let info: VersionInfo = serde_json::from_str(json).unwrap();
        assert_eq!(info.browser, "Chrome/131.0.6778.85");
        assert_eq!(
            info.web_socket_debugger_url,
            "ws://127.0.0.1:9222/devtools/browser/abc-123"
        );
    }

    #[test]
    fn test_target_info_deserialises() {
        let json = r#"{
            "id": "target-1",
            "type": "page",
            "title": "Example",
            "url": "https://example.com",
            "webSocketDebuggerUrl": "ws://127.0.0.1:9222/devtools/page/target-1",
            "attached": false
        }"#;
        let target: TargetInfo = serde_json::from_str(json).unwrap();
        assert_eq!(target.id, "target-1");
        assert_eq!(target.target_type, "page");
        assert_eq!(target.url, "https://example.com");
    }

    #[test]
    fn test_first_page_target_finds_page() {
        let targets = vec![
            TargetInfo {
                id: "bg".to_string(),
                target_type: "background_page".to_string(),
                title: String::new(),
                url: String::new(),
                web_socket_debugger_url: String::new(),
                attached: false,
            },
            TargetInfo {
                id: "page1".to_string(),
                target_type: "page".to_string(),
                title: "Test".to_string(),
                url: "https://example.com".to_string(),
                web_socket_debugger_url: "ws://127.0.0.1:9222/devtools/page/page1".to_string(),
                attached: false,
            },
        ];
        let result = first_page_target(&targets).unwrap();
        assert_eq!(result.id, "page1");
    }

    #[test]
    fn test_first_page_target_no_page() {
        let targets = vec![TargetInfo {
            id: "bg".to_string(),
            target_type: "background_page".to_string(),
            title: String::new(),
            url: String::new(),
            web_socket_debugger_url: String::new(),
            attached: false,
        }];
        let result = first_page_target(&targets);
        assert!(result.is_err());
    }

    #[test]
    fn test_cdp_error_display() {
        let err = CdpError::CommandError {
            code: -32000,
            message: "Cannot navigate to invalid URL".to_string(),
        };
        assert!(err.to_string().contains("-32000"));
        assert!(err.to_string().contains("Cannot navigate"));
    }
}
