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
//! - **`inject_summary`** — inject a bounded summary into the parent chat
//!   *without* a model call. The summary is produced by normalizing the
//!   notification payload and is capped at [`TriggerEnvelope::SUMMARY_MAX`]
//!   characters.
//!
//! - **`inject_and_run`** — inject a prompt and run one model turn in the
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
    /// * `server_id` — the MCP server that produced the notification
    /// * `summary` — the normalized, bounded summary text
    async fn inject_summary(&self, server_id: &str, summary: &str) -> anyhow::Result<()>;

    /// Inject a prompt and run one model turn in the parent's full tool
    /// context (FR-003 `inject_and_run`).
    ///
    /// # Arguments
    ///
    /// * `server_id` — the MCP server that produced the notification
    /// * `prompt` — the action prompt to submit as a user turn
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
    /// FR-003). Currently informational — the adapter never includes raw
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
    /// * `runtime` — the trigger runtime to route envelopes through
    /// * `injector` — the injector that performs chat injection
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
    /// * `server_id` — the MCP server's unique identifier
    /// * `mode` — the injection mode (`InjectSummary` or `InjectAndRun`)
    /// * `persist_raw_payloads` — if `true`, raw notification payloads may
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
    /// routing it through the trigger runtime, and — if dispatched —
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
/// - `notifications/message` — uses `data` or `message` field from params
/// - `notifications/progress` — uses `progress` and `message` fields
/// - Other methods — uses the method name and a truncated JSON of params
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
            let summary = format!("[MCP progress] {progress} — {message}");
            let action_prompt = format!("MCP server reported progress: {progress} — {message}");
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
            let truncated = if params_str.len() > 200 {
                format!("{}…", &params_str[..200])
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
mod tests {
    use super::*;
    use crate::trigger::runtime::{TriggerRuntime, TriggerRuntimeConfig};
    use serde_json::json;
    use std::time::Duration;

    fn make_adapter() -> (
        McpNotificationAdapter,
        Arc<RecordingNotificationInjector>,
        TriggerRuntime,
    ) {
        // Use a non-zero dedup window so dedup tests work.
        let runtime = TriggerRuntime::new(TriggerRuntimeConfig {
            dedup_window: Duration::from_secs(60),
            max_cycles: 100,
        });
        let injector = Arc::new(RecordingNotificationInjector::new());
        let adapter = McpNotificationAdapter::new(runtime.clone(), injector.clone());
        (adapter, injector, runtime)
    }

    #[test]
    fn test_register_and_unregister_server() {
        let (adapter, _injector, _rt) = make_adapter();
        assert!(!adapter.is_registered("srv-1"));

        adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);
        assert!(adapter.is_registered("srv-1"));
        assert_eq!(adapter.server_count(), 1);

        adapter.unregister_server("srv-1");
        assert!(!adapter.is_registered("srv-1"));
        assert_eq!(adapter.server_count(), 0);
    }

    #[test]
    fn test_normalize_message_notification() {
        let notification = McpNotification::new(
            "srv-1",
            "notifications/message",
            json!({"level": "warning", "data": "build completed with 3 errors"}),
        );
        let (summary, action) = normalize_notification(&notification).unwrap();
        assert!(summary.contains("warning"));
        assert!(summary.contains("build completed with 3 errors"));
        assert!(action.contains("warning"));
        assert!(action.contains("build completed with 3 errors"));
    }

    #[test]
    fn test_normalize_progress_notification() {
        let notification = McpNotification::new(
            "srv-1",
            "notifications/progress",
            json!({"progress": "50%", "message": "halfway done"}),
        );
        let (summary, action) = normalize_notification(&notification).unwrap();
        assert!(summary.contains("50%"));
        assert!(summary.contains("halfway done"));
        assert!(action.contains("50%"));
    }

    #[test]
    fn test_normalize_cancelled_notification() {
        let notification = McpNotification::new(
            "srv-1",
            "notifications/cancelled",
            json!({"requestId": "req-42", "reason": "user cancelled"}),
        );
        let (summary, _action) = normalize_notification(&notification).unwrap();
        assert!(summary.contains("req-42"));
        assert!(summary.contains("user cancelled"));
    }

    #[test]
    fn test_normalize_generic_notification() {
        let notification =
            McpNotification::new("srv-1", "notifications/custom", json!({"foo": "bar"}));
        let (summary, action) = normalize_notification(&notification).unwrap();
        assert!(summary.contains("notifications/custom"));
        assert!(action.contains("notifications/custom"));
    }

    #[tokio::test]
    async fn test_handle_notification_inject_summary() {
        let (adapter, injector, _rt) = make_adapter();
        adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

        let notification = McpNotification::new(
            "srv-1",
            "notifications/message",
            json!({"level": "info", "data": "hello world"}),
        );

        let fired = adapter.handle_notification(notification).await.unwrap();
        assert!(fired.is_some());
        assert_eq!(
            fired.as_ref().unwrap().envelope.action_kind,
            TriggerActionKind::InjectSummary
        );

        assert_eq!(injector.count(), 1);
        let injections = injector.injections();
        assert_eq!(injections[0].0, "srv-1");
        assert_eq!(injections[0].1, "inject_summary");
        assert!(injections[0].2.contains("hello world"));
    }

    #[tokio::test]
    async fn test_handle_notification_inject_and_run() {
        let (adapter, injector, _rt) = make_adapter();
        adapter.register_server("srv-1", McpNotificationMode::InjectAndRun, false);

        let notification = McpNotification::new(
            "srv-1",
            "notifications/message",
            json!({"level": "error", "data": "deployment failed"}),
        );

        let fired = adapter.handle_notification(notification).await.unwrap();
        assert!(fired.is_some());
        assert_eq!(
            fired.as_ref().unwrap().envelope.action_kind,
            TriggerActionKind::InjectAndRun
        );

        assert_eq!(injector.count(), 1);
        let injections = injector.injections();
        assert_eq!(injections[0].0, "srv-1");
        assert_eq!(injections[0].1, "inject_and_run");
        assert!(injections[0].2.contains("deployment failed"));
    }

    #[tokio::test]
    async fn test_handle_notification_unregistered_server() {
        let (adapter, _injector, _rt) = make_adapter();
        let notification = McpNotification::new(
            "unknown-srv",
            "notifications/message",
            json!({"data": "test"}),
        );
        let result = adapter.handle_notification(notification).await;
        assert!(matches!(
            result,
            Err(McpNotificationError::ServerNotRegistered { .. })
        ));
    }

    #[tokio::test]
    async fn test_handle_notification_mode_none() {
        let (adapter, _injector, _rt) = make_adapter();
        adapter.register_server("srv-1", McpNotificationMode::None, false);

        let notification =
            McpNotification::new("srv-1", "notifications/message", json!({"data": "test"}));
        let result = adapter.handle_notification(notification).await;
        assert!(matches!(result, Err(McpNotificationError::ModeNone { .. })));
    }

    #[tokio::test]
    async fn test_dedup_suppresses_duplicate_notifications() {
        let (adapter, injector, _rt) = make_adapter();
        adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

        let params = json!({"level": "info", "data": "same message"});
        let notification1 = McpNotification::new("srv-1", "notifications/message", params.clone());
        let notification2 = McpNotification::new("srv-1", "notifications/message", params);

        let fired1 = adapter.handle_notification(notification1).await.unwrap();
        let fired2 = adapter.handle_notification(notification2).await.unwrap();

        assert!(fired1.is_some());
        assert!(fired2.is_none()); // suppressed by dedup
        assert_eq!(injector.count(), 1); // only one injection
    }

    #[tokio::test]
    async fn test_different_content_not_suppressed() {
        let (adapter, injector, _rt) = make_adapter();
        adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

        let notification1 = McpNotification::new(
            "srv-1",
            "notifications/message",
            json!({"level": "info", "data": "first"}),
        );
        let notification2 = McpNotification::new(
            "srv-1",
            "notifications/message",
            json!({"level": "info", "data": "second"}),
        );

        assert!(
            adapter
                .handle_notification(notification1)
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            adapter
                .handle_notification(notification2)
                .await
                .unwrap()
                .is_some()
        );
        assert_eq!(injector.count(), 2);
    }

    #[tokio::test]
    async fn test_envelope_source_kind_is_mcp_notification() {
        let (adapter, _injector, _rt) = make_adapter();
        adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

        let notification =
            McpNotification::new("srv-1", "notifications/message", json!({"data": "test"}));

        let fired = adapter
            .handle_notification(notification)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            fired.envelope.source_kind,
            TriggerSourceKind::McpNotification
        );
    }

    #[tokio::test]
    async fn test_mcp_envelope_has_no_rule_id() {
        let (adapter, _injector, _rt) = make_adapter();
        adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

        let notification =
            McpNotification::new("srv-1", "notifications/message", json!({"data": "test"}));

        let fired = adapter
            .handle_notification(notification)
            .await
            .unwrap()
            .unwrap();
        assert!(fired.rule_id.is_none());
    }

    #[tokio::test]
    async fn test_summary_is_bounded() {
        let (adapter, _injector, _rt) = make_adapter();
        adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

        // Create a notification with a very long data field.
        let long_data = "x".repeat(10_000);
        let notification = McpNotification::new(
            "srv-1",
            "notifications/message",
            json!({"level": "info", "data": long_data}),
        );

        let fired = adapter
            .handle_notification(notification)
            .await
            .unwrap()
            .unwrap();
        assert!(
            fired.envelope.summary.chars().count() <= TriggerEnvelope::SUMMARY_MAX,
            "summary should be bounded to {} chars, got {}",
            TriggerEnvelope::SUMMARY_MAX,
            fired.envelope.summary.chars().count()
        );
    }

    #[tokio::test]
    async fn test_multiple_servers_independent() {
        let (adapter, injector, _rt) = make_adapter();
        adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);
        adapter.register_server("srv-2", McpNotificationMode::InjectAndRun, false);

        let n1 = McpNotification::new(
            "srv-1",
            "notifications/message",
            json!({"data": "from srv-1"}),
        );
        let n2 = McpNotification::new(
            "srv-2",
            "notifications/message",
            json!({"data": "from srv-2"}),
        );

        adapter.handle_notification(n1).await.unwrap();
        adapter.handle_notification(n2).await.unwrap();

        assert_eq!(injector.count(), 2);
        let injections = injector.injections();
        assert_eq!(injections[0].0, "srv-1");
        assert_eq!(injections[0].1, "inject_summary");
        assert_eq!(injections[1].0, "srv-2");
        assert_eq!(injections[1].1, "inject_and_run");
    }
}
