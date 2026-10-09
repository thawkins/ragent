//! MCP notification push-event adapter (spec `piegap` FR-003).
//!
//! When a configured MCP server pushes a notification frame, this adapter
//! normalizes the notification into a [`TriggerEnvelope`] and routes it through
//! the same [`TriggerRuntime`] as dynamic rules, with deduplication and cycle
//! suppression.
//!
//! ## Injection modes
//!
//! Per-server configuration (`McpNotificationMode` in `ragent-config`) selects
//! one of two injection behaviours:
//!
//! - **`inject_summary`** - inject a bounded summary into the parent chat
//!   *without* a model call. The summary is produced by normalizing the
//!   notification payload and is capped at [`TriggerEnvelope::SUMMARY_MAX`]
//!   characters.
//!
//! - **`inject_and_run`** - inject a prompt and run one model turn in the
//!   parent's full tool context. The action prompt is derived from the
//!   notification method and params.
//!
//! ## Raw payload privacy
//!
//! Raw notification payloads are **not** persisted as chat content or trigger
//! audit unless the source explicitly opts in via `persist_raw_payloads`.
//! The adapter only stores the normalized summary and action prompt in the
//! trigger envelope.
//!
//! ## Architecture
//!
//! The adapter is decoupled from the actual chat-injection mechanism through
//! the [`NotificationInjector`] trait. In production, an implementation
//! injects into the real session; in tests, [`RecordingNotificationInjector`]
//! captures injections for verification. This follows the standalone module
//! independence requirement (FR-001).

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use ragent_config::McpNotificationMode;
use ragent_types::trigger::{TriggerActionKind, TriggerEnvelope, TriggerFired, TriggerSourceKind};
use serde_json::Value;
use thiserror::Error;
use tracing::{debug, warn};

use super::runtime::TriggerRuntime;

/// Errors produced by the MCP notification adapter.
#[derive(Debug, Error)]
pub enum McpNotificationError {
    /// The server is not registered with the adapter.
    #[error("MCP server '{server_id}' is not registered")]
    ServerNotRegistered {
        /// The unregistered server's identifier.
        server_id: String,
    },
    /// The server's notification mode is `None` (notifications ignored).
    #[error("MCP server '{server_id}' has notification mode 'none'")]
    ModeNone {
        /// The server whose mode is `None`.
        server_id: String,
    },
    /// The notification payload could not be normalized.
    #[error("failed to normalize notification from '{server_id}': {reason}")]
    NormalizeFailed {
        /// The server that produced the un-normalizable notification.
        server_id: String,
        /// A human-readable description of why normalization failed.
        reason: String,
    },
}

/// A normalized MCP notification frame ready for adapter processing.
///
/// This is the input to [`McpNotificationAdapter::handle_notification`]. It
/// represents a JSON-RPC notification pushed by an MCP server, stripped of
/// transport-specific details.
#[derive(Debug, Clone)]
pub struct McpNotification {
    /// The ID of the MCP server that pushed this notification.
    pub server_id: String,
    /// The JSON-RPC method (e.g., `"notifications/message"`,
    /// `"notifications/progress"`).
    pub method: String,
    /// The notification parameters (JSON value).
    pub params: Value,
}

impl McpNotification {
    /// Creates a new MCP notification.
    pub fn new(server_id: impl Into<String>, method: impl Into<String>, params: Value) -> Self {
        Self {
            server_id: server_id.into(),
            method: method.into(),
            params,
        }
    }
}

/// Injects a notification-derived message into the parent session.
///
/// In production, an implementation writes into the real session's chat feed
/// or triggers a model turn. In tests, [`RecordingNotificationInjector`]
/// captures the injections for verification.
#[async_trait]
pub trait NotificationInjector: Send + Sync + 'static {
    /// Inject a bounded summary into the parent chat without a model call
    /// (FR-003 `inject_summary`).
    ///
    /// # Arguments
    ///
    /// * `server_id` - the MCP server that produced the notification
    /// * `summary` - the normalized, bounded summary text
    async fn inject_summary(&self, server_id: &str, summary: &str) -> anyhow::Result<()>;

    /// Inject a prompt and run one model turn in the parent's full tool
    /// context (FR-003 `inject_and_run`).
    ///
    /// # Arguments
    ///
    /// * `server_id` - the MCP server that produced the notification
    /// * `prompt` - the action prompt to submit as a user turn
    async fn inject_and_run(&self, server_id: &str, prompt: &str) -> anyhow::Result<()>;
}

/// A recording [`NotificationInjector`] for test verification.
///
/// Captures every injection call so tests can assert on the exact messages
/// and modes used.
pub struct RecordingNotificationInjector {
    /// Record of all injections: (server_id, mode, text).
    injections: Arc<parking_lot::Mutex<Vec<(String, String, String)>>>,
}

impl RecordingNotificationInjector {
    /// Creates a new recording injector.
    pub fn new() -> Self {
        Self {
            injections: Arc::new(parking_lot::Mutex::new(Vec::new())),
        }
    }

    /// Returns a snapshot of all recorded injections.
    ///
    /// Each entry is `(server_id, mode, text)` where `mode` is
    /// `"inject_summary"` or `"inject_and_run"`.
    pub fn injections(&self) -> Vec<(String, String, String)> {
        self.injections.lock().clone()
    }

    /// Returns the number of injections recorded.
    pub fn count(&self) -> usize {
        self.injections.lock().len()
    }

    /// Returns `true` if no injections have been recorded.
    pub fn is_empty(&self) -> bool {
        self.injections.lock().is_empty()
    }
}

impl Default for RecordingNotificationInjector {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl NotificationInjector for RecordingNotificationInjector {
    async fn inject_summary(&self, server_id: &str, summary: &str) -> anyhow::Result<()> {
        self.injections.lock().push((
            server_id.to_string(),
            "inject_summary".to_string(),
            summary.to_string(),
        ));
        Ok(())
    }

    async fn inject_and_run(&self, server_id: &str, prompt: &str) -> anyhow::Result<()> {
        self.injections.lock().push((
            server_id.to_string(),
            "inject_and_run".to_string(),
            prompt.to_string(),
        ));
        Ok(())
    }
}

/// Per-server notification configuration held by the adapter.
#[derive(Debug, Clone)]
struct ServerNotificationConfig {
    /// Injection mode for this server.
    mode: McpNotificationMode,
    /// Whether raw notification payloads may be persisted (explicit opt-in,
    /// FR-003). Currently informational - the adapter never includes raw
    /// JSON in the trigger envelope regardless; this flag is reserved for
    /// future persistence wiring.
    #[allow(dead_code)]
    persist_raw_payloads: bool,
}

/// The MCP notification push-event adapter.
///
/// Normalizes MCP server notification frames into trigger envelopes and
/// routes them through the [`TriggerRuntime`] for deduplication and cycle
/// suppression. When an envelope is dispatched, the configured
/// [`NotificationInjector`] performs the appropriate injection.
///
/// Thread-safe via internal `Mutex` on the per-server config map. The
/// `TriggerRuntime` is already thread-safe.
pub struct McpNotificationAdapter {
    /// The trigger runtime (shared with the session).
    runtime: TriggerRuntime,
    /// Per-server notification configs.
    servers: parking_lot::Mutex<HashMap<String, ServerNotificationConfig>>,
    /// The injector (production or recording).
    injector: Arc<dyn NotificationInjector>,
}

impl McpNotificationAdapter {
    /// Creates a new MCP notification adapter.
    ///
    /// # Arguments
    ///
    /// * `runtime` - the trigger runtime to route envelopes through
    /// * `injector` - the injector that performs chat injection
    pub fn new(runtime: TriggerRuntime, injector: Arc<dyn NotificationInjector>) -> Self {
        Self {
            runtime,
            servers: parking_lot::Mutex::new(HashMap::new()),
            injector,
        }
    }

    /// Registers an MCP server with the adapter.
    ///
    /// After registration, notifications from this server will be normalized
    /// and routed through the trigger runtime.
    ///
    /// # Arguments
    ///
    /// * `server_id` - the MCP server's unique identifier
    /// * `mode` - the injection mode (`InjectSummary` or `InjectAndRun`)
    /// * `persist_raw_payloads` - if `true`, raw notification payloads may
    ///   be persisted (explicit opt-in, FR-003)
    pub fn register_server(
        &self,
        server_id: &str,
        mode: McpNotificationMode,
        persist_raw_payloads: bool,
    ) {
        if mode.is_none() {
            debug!(
                server_id,
                "MCP server registered with notification mode 'none'"
            );
        } else {
            debug!(
                server_id,
                ?mode,
                persist_raw_payloads,
                "MCP server registered for notification push events"
            );
        }
        self.servers.lock().insert(
            server_id.to_string(),
            ServerNotificationConfig {
                mode,
                persist_raw_payloads,
            },
        );
    }

    /// Unregisters an MCP server. Subsequent notifications from this server
    /// will be dropped.
    pub fn unregister_server(&self, server_id: &str) {
        self.servers.lock().remove(server_id);
        debug!(
            server_id,
            "MCP server unregistered from notification adapter"
        );
    }

    /// Returns `true` if the given server is registered.
    pub fn is_registered(&self, server_id: &str) -> bool {
        self.servers.lock().contains_key(server_id)
    }

    /// Returns the number of registered servers.
    pub fn server_count(&self) -> usize {
        self.servers.lock().len()
    }

    /// Handles an MCP notification by normalizing it into a trigger envelope,
    /// routing it through the trigger runtime, and - if dispatched -
    /// performing the appropriate injection.
    ///
    /// Returns `Ok(Some(TriggerFired))` if the envelope was dispatched,
    /// `Ok(None)` if it was suppressed by dedup/cycle, or an error if the
    /// server is not registered or the notification cannot be normalized.
    ///
    /// # Errors
    ///
    /// - [`McpNotificationError::ServerNotRegistered`] if the server is not
    ///   registered.
    /// - [`McpNotificationError::ModeNone`] if the server's mode is `None`.
    /// - [`McpNotificationError::NormalizeFailed`] if the notification payload
    ///   cannot be normalized.
    pub async fn handle_notification(
        &self,
        notification: McpNotification,
    ) -> Result<Option<TriggerFired>, McpNotificationError> {
        let server_cfg = {
            let servers = self.servers.lock();
            servers
                .get(&notification.server_id)
                .cloned()
                .ok_or_else(|| McpNotificationError::ServerNotRegistered {
                    server_id: notification.server_id.clone(),
                })?
        };

        if server_cfg.mode.is_none() {
            return Err(McpNotificationError::ModeNone {
                server_id: notification.server_id,
            });
        }

        // Normalize the notification into summary + action_prompt.
        let (summary, action_prompt) = normalize_notification(&notification).map_err(|reason| {
            McpNotificationError::NormalizeFailed {
                server_id: notification.server_id.clone(),
                reason,
            }
        })?;

        // Determine the action kind from the injection mode.
        let action_kind = if server_cfg.mode.is_inject_summary() {
            TriggerActionKind::InjectSummary
        } else {
            TriggerActionKind::InjectAndRun
        };

        // Create the trigger envelope.
        let envelope = TriggerEnvelope::new(
            TriggerSourceKind::McpNotification,
            &notification.server_id,
            &summary,
            &action_prompt,
            action_kind,
            false, // MCP notifications don't promote to chat by default
        );

        // Route through the trigger runtime (dedup + cycle suppression).
        let fired = self.runtime.process(envelope);

        if let Some(ref fired) = fired {
            // Perform the injection.
            let inject_result = if server_cfg.mode.is_inject_summary() {
                self.injector
                    .inject_summary(&notification.server_id, &fired.envelope.summary)
                    .await
            } else {
                self.injector
                    .inject_and_run(&notification.server_id, &fired.envelope.action_prompt)
                    .await
            };

            if let Err(e) = inject_result {
                warn!(
                    server_id = %notification.server_id,
                    error = %e,
                    "Failed to inject MCP notification"
                );
            } else {
                debug!(
                    server_id = %notification.server_id,
                    method = %notification.method,
                    "MCP notification injected"
                );
            }
        }

        Ok(fired)
    }

    /// Returns a reference to the underlying trigger runtime.
    pub fn runtime(&self) -> &TriggerRuntime {
        &self.runtime
    }
}

/// Normalizes an MCP notification into a `(summary, action_prompt)` pair.
///
/// The summary is a bounded human-readable description of the notification.
/// The action prompt is the text to inject or run. Raw JSON is not included
/// unless the caller explicitly opts in to persistence (handled by the
/// adapter, not here).
///
/// This function extracts readable content from common MCP notification
/// methods:
///
/// - `notifications/message` - uses `data` or `message` field from params
/// - `notifications/progress` - uses `progress` and `message` fields
/// - Other methods - uses the method name and a truncated JSON of params
fn normalize_notification(notification: &McpNotification) -> Result<(String, String), String> {
    let method = notification.method.as_str();
    let params = &notification.params;

    // Handle known MCP notification methods.
    let (summary, action_prompt) = match method {
        "notifications/message" => {
            let data = params
                .get("data")
                .or_else(|| params.get("message"))
                .map(extract_text)
                .unwrap_or_else(|| "[MCP message notification]".to_string());
            let level = params
                .get("level")
                .and_then(|v| v.as_str())
                .unwrap_or("info");
            let summary = format!("[MCP {level}] {data}");
            let action_prompt = format!("MCP server sent a {level} message: {data}");
            (summary, action_prompt)
        }
        "notifications/progress" => {
            let progress = params
                .get("progress")
                .map(extract_text)
                .unwrap_or_else(|| "unknown".to_string());
            let message = params
                .get("message")
                .map(extract_text)
                .unwrap_or_else(|| "progress update".to_string());
            let summary = format!("[MCP progress] {progress} - {message}");
            let action_prompt = format!("MCP server reported progress: {progress} - {message}");
            (summary, action_prompt)
        }
        "notifications/cancelled" => {
            let request_id = params
                .get("requestId")
                .map(extract_text)
                .unwrap_or_else(|| "unknown".to_string());
            let reason = params
                .get("reason")
                .map(extract_text)
                .unwrap_or_else(|| "no reason given".to_string());
            let summary = format!("[MCP cancelled] request {request_id}: {reason}");
            let action_prompt = format!("MCP server cancelled request {request_id}: {reason}");
            (summary, action_prompt)
        }
        _ => {
            // Generic fallback: use the method name and a truncated JSON.
            let params_str =
                serde_json::to_string(params).unwrap_or_else(|_| "<unserializable>".to_string());
            // Char-boundary-safe truncation: byte slicing can panic on multibyte
            // JSON. Collect the 201-char head once, then branch on whether the
            // 201st char exists (equivalent to a chars-count > 200 check
            // without a second pass over the string).
            let head: String = params_str.chars().take(201).collect();
            let truncated = if head.chars().count() > 200 {
                format!("{}...", head.chars().take(198).collect::<String>())
            } else {
                params_str
            };
            let summary = format!("[MCP {method}] {truncated}");
            let action_prompt = format!("MCP server sent notification '{method}': {truncated}");
            (summary, action_prompt)
        }
    };

    if summary.is_empty() {
        return Err("normalized summary is empty".to_string());
    }

    Ok((summary, action_prompt))
}

/// Extracts a readable text representation from a JSON value.
fn extract_text(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Null => "null".to_string(),
        _ => serde_json::to_string(value).unwrap_or_else(|_| "<unserializable>".to_string()),
    }
}

#[cfg(test)]
#[path = "../../tests/inline/mcp_notification_tests.rs"]
mod tests;
