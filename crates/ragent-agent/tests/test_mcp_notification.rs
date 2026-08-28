//! Integration tests for the MCP notification push-event adapter
//! (spec `piegap` FR-003 / T-003).
//!
//! These tests verify the adapter's public API: server registration,
//! notification normalization, inject_summary / inject_and_run modes,
//! deduplication, cycle suppression, raw payload privacy, and
//! multi-server independence. No real MCP server or LLM is required —
//! `RecordingNotificationInjector` stands in for the injection mechanism.

use std::sync::Arc;
use std::time::Duration;

use ragent_agent::trigger::mcp_notification::{
    McpNotification, McpNotificationAdapter, McpNotificationError, RecordingNotificationInjector,
};
use ragent_agent::trigger::runtime::{TriggerRuntime, TriggerRuntimeConfig};
use ragent_config::McpNotificationMode;
use ragent_types::trigger::{TriggerActionKind, TriggerEnvelope, TriggerSourceKind};
use serde_json::json;

// ── Helpers ────────────────────────────────────────────────────────────────

fn make_adapter() -> (
    McpNotificationAdapter,
    Arc<RecordingNotificationInjector>,
    TriggerRuntime,
) {
    // Use a non-zero dedup window so dedup tests work.
    // max_cycles is high so cycle suppression doesn't interfere with dedup tests.
    let runtime = TriggerRuntime::new(TriggerRuntimeConfig {
        dedup_window: Duration::from_secs(60),
        max_cycles: 100,
    });
    let injector = Arc::new(RecordingNotificationInjector::new());
    let adapter = McpNotificationAdapter::new(runtime.clone(), injector.clone());
    (adapter, injector, runtime)
}

fn make_message_notification(server: &str, data: &str) -> McpNotification {
    McpNotification::new(
        server,
        "notifications/message",
        json!({"level": "info", "data": data}),
    )
}

// ── Server registration tests ──────────────────────────────────────────────

#[test]
fn test_register_server() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);
    assert!(adapter.is_registered("srv-1"));
    assert_eq!(adapter.server_count(), 1);
}

#[test]
fn test_register_multiple_servers() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);
    adapter.register_server("srv-2", McpNotificationMode::InjectAndRun, false);
    adapter.register_server("srv-3", McpNotificationMode::None, false);
    assert_eq!(adapter.server_count(), 3);
}

#[test]
fn test_unregister_server() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);
    assert!(adapter.is_registered("srv-1"));

    adapter.unregister_server("srv-1");
    assert!(!adapter.is_registered("srv-1"));
    assert_eq!(adapter.server_count(), 0);
}

#[test]
fn test_unregister_unknown_server_is_noop() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.unregister_server("nonexistent");
    assert_eq!(adapter.server_count(), 0);
}

// ── Notification normalization tests ───────────────────────────────────────

#[tokio::test]
async fn test_message_notification_summary_contains_data() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    let notification = make_message_notification("srv-1", "build completed");
    let fired = adapter
        .handle_notification(notification)
        .await
        .unwrap()
        .unwrap();

    assert!(fired.envelope.summary.contains("build completed"));
}

#[tokio::test]
async fn test_progress_notification_summary() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    let notification = McpNotification::new(
        "srv-1",
        "notifications/progress",
        json!({"progress": "75%", "message": "almost done"}),
    );
    let fired = adapter
        .handle_notification(notification)
        .await
        .unwrap()
        .unwrap();

    assert!(fired.envelope.summary.contains("75%"));
    assert!(fired.envelope.summary.contains("almost done"));
}

#[tokio::test]
async fn test_cancelled_notification_summary() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    let notification = McpNotification::new(
        "srv-1",
        "notifications/cancelled",
        json!({"requestId": "req-99", "reason": "timeout"}),
    );
    let fired = adapter
        .handle_notification(notification)
        .await
        .unwrap()
        .unwrap();

    assert!(fired.envelope.summary.contains("req-99"));
    assert!(fired.envelope.summary.contains("timeout"));
}

#[tokio::test]
async fn test_generic_notification_summary() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    let notification = McpNotification::new(
        "srv-1",
        "notifications/custom-event",
        json!({"key": "value"}),
    );
    let fired = adapter
        .handle_notification(notification)
        .await
        .unwrap()
        .unwrap();

    assert!(
        fired
            .envelope
            .summary
            .contains("notifications/custom-event")
    );
}

// ── Injection mode tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_inject_summary_calls_injector() {
    let (adapter, injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    let notification = make_message_notification("srv-1", "hello");
    adapter.handle_notification(notification).await.unwrap();

    assert_eq!(injector.count(), 1);
    let injections = injector.injections();
    assert_eq!(injections[0].0, "srv-1");
    assert_eq!(injections[0].1, "inject_summary");
    assert!(injections[0].2.contains("hello"));
}

#[tokio::test]
async fn test_inject_and_run_calls_injector() {
    let (adapter, injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectAndRun, false);

    let notification = make_message_notification("srv-1", "deploy now");
    adapter.handle_notification(notification).await.unwrap();

    assert_eq!(injector.count(), 1);
    let injections = injector.injections();
    assert_eq!(injections[0].0, "srv-1");
    assert_eq!(injections[0].1, "inject_and_run");
    assert!(injections[0].2.contains("deploy now"));
}

#[tokio::test]
async fn test_inject_summary_uses_correct_action_kind() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    let notification = make_message_notification("srv-1", "test");
    let fired = adapter
        .handle_notification(notification)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(fired.envelope.action_kind, TriggerActionKind::InjectSummary);
}

#[tokio::test]
async fn test_inject_and_run_uses_correct_action_kind() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectAndRun, false);

    let notification = make_message_notification("srv-1", "test");
    let fired = adapter
        .handle_notification(notification)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(fired.envelope.action_kind, TriggerActionKind::InjectAndRun);
}

// ── Error handling tests ────────────────────────────────────────────────────

#[tokio::test]
async fn test_unregistered_server_returns_error() {
    let (adapter, _injector, _rt) = make_adapter();
    let notification = make_message_notification("unknown", "test");
    let result = adapter.handle_notification(notification).await;
    assert!(matches!(
        result,
        Err(McpNotificationError::ServerNotRegistered { .. })
    ));
}

#[tokio::test]
async fn test_mode_none_returns_error() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::None, false);

    let notification = make_message_notification("srv-1", "test");
    let result = adapter.handle_notification(notification).await;
    assert!(matches!(result, Err(McpNotificationError::ModeNone { .. })));
}

// ── Deduplication tests ──────────────────────���──────────────────────────────

#[tokio::test]
async fn test_duplicate_notification_suppressed() {
    // Use default adapter (60s dedup window) so duplicates are suppressed.
    let (adapter, injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    let n1 = make_message_notification("srv-1", "same content");
    let n2 = make_message_notification("srv-1", "same content");

    let f1 = adapter.handle_notification(n1).await.unwrap();
    let f2 = adapter.handle_notification(n2).await.unwrap();

    assert!(f1.is_some());
    assert!(f2.is_none());
    assert_eq!(injector.count(), 1);
}

#[tokio::test]
async fn test_different_content_not_suppressed() {
    let (adapter, injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    let n1 = make_message_notification("srv-1", "first");
    let n2 = make_message_notification("srv-1", "second");

    assert!(adapter.handle_notification(n1).await.unwrap().is_some());
    assert!(adapter.handle_notification(n2).await.unwrap().is_some());
    assert_eq!(injector.count(), 2);
}

#[tokio::test]
async fn test_different_servers_not_suppressed() {
    let (adapter, injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);
    adapter.register_server("srv-2", McpNotificationMode::InjectSummary, false);

    let n1 = make_message_notification("srv-1", "same content");
    let n2 = make_message_notification("srv-2", "same content");

    assert!(adapter.handle_notification(n1).await.unwrap().is_some());
    assert!(adapter.handle_notification(n2).await.unwrap().is_some());
    assert_eq!(injector.count(), 2);
}

// ── Cycle suppression tests ─────────────────────────────────────────────────

#[tokio::test]
async fn test_cycle_suppression_kicks_in() {
    let runtime = TriggerRuntime::new(TriggerRuntimeConfig {
        dedup_window: Duration::from_secs(0),
        max_cycles: 3,
    });
    let injector = Arc::new(RecordingNotificationInjector::new());
    let adapter = McpNotificationAdapter::new(runtime, injector.clone());
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    // Fire 3 identical notifications (should pass).
    for _ in 0..3 {
        let n = make_message_notification("srv-1", "repeating");
        assert!(adapter.handle_notification(n).await.unwrap().is_some());
    }

    // 4th identical notification should be suppressed.
    let n = make_message_notification("srv-1", "repeating");
    assert!(adapter.handle_notification(n).await.unwrap().is_none());

    // Only 3 injections should have been recorded.
    assert_eq!(injector.count(), 3);
}

#[tokio::test]
async fn test_cycle_resets_on_content_change() {
    let runtime = TriggerRuntime::new(TriggerRuntimeConfig {
        dedup_window: Duration::from_secs(0),
        max_cycles: 2,
    });
    let injector = Arc::new(RecordingNotificationInjector::new());
    let adapter = McpNotificationAdapter::new(runtime, injector.clone());
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    // Fire 2 identical (max_cycles = 2, should pass).
    for _ in 0..2 {
        let n = make_message_notification("srv-1", "A");
        assert!(adapter.handle_notification(n).await.unwrap().is_some());
    }

    // Different content resets cycle.
    let n = make_message_notification("srv-1", "B");
    assert!(adapter.handle_notification(n).await.unwrap().is_some());

    // Original content should pass again after reset.
    let n = make_message_notification("srv-1", "A");
    assert!(adapter.handle_notification(n).await.unwrap().is_some());

    assert_eq!(injector.count(), 4);
}

// ── Envelope property tests ─────────────────────────────────────────────────

#[tokio::test]
async fn test_envelope_source_kind_is_mcp_notification() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    let notification = make_message_notification("srv-1", "test");
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
async fn test_envelope_has_no_rule_id() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    let notification = make_message_notification("srv-1", "test");
    let fired = adapter
        .handle_notification(notification)
        .await
        .unwrap()
        .unwrap();

    assert!(fired.rule_id.is_none());
}

#[tokio::test]
async fn test_summary_is_bounded_to_max() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

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

    // Summary is bounded to SUMMARY_MAX *characters* (not bytes).
    // The ellipsis may add extra bytes but char count stays within the limit.
    assert!(
        fired.envelope.summary.chars().count() <= TriggerEnvelope::SUMMARY_MAX,
        "summary should be bounded to {} chars, got {}",
        TriggerEnvelope::SUMMARY_MAX,
        fired.envelope.summary.chars().count()
    );
}

#[tokio::test]
async fn test_raw_payload_not_in_envelope() {
    let (adapter, _injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    // The raw notification params JSON should not appear verbatim in the
    // envelope. The adapter normalizes the notification by extracting only
    // the human-readable `data` field, not the full JSON structure.
    let notification = McpNotification::new(
        "srv-1",
        "notifications/message",
        json!({"level": "info", "data": "visible message", "internal_id": "secret_token_123"}),
    );

    let fired = adapter
        .handle_notification(notification)
        .await
        .unwrap()
        .unwrap();

    // The envelope summary should contain the extracted data text.
    assert!(fired.envelope.summary.contains("visible message"));
    // The raw JSON structure keys should not appear in the normalized summary.
    assert!(!fired.envelope.summary.contains("internal_id"));
    assert!(!fired.envelope.summary.contains("secret_token_123"));
}

// ── Multi-server independence tests ─────────────────────────────────────────

#[tokio::test]
async fn test_multiple_servers_independent_injection() {
    let (adapter, injector, _rt) = make_adapter();
    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);
    adapter.register_server("srv-2", McpNotificationMode::InjectAndRun, false);

    let n1 = make_message_notification("srv-1", "from 1");
    let n2 = make_message_notification("srv-2", "from 2");

    adapter.handle_notification(n1).await.unwrap();
    adapter.handle_notification(n2).await.unwrap();

    assert_eq!(injector.count(), 2);
    let injections = injector.injections();
    assert_eq!(injections[0].0, "srv-1");
    assert_eq!(injections[0].1, "inject_summary");
    assert_eq!(injections[1].0, "srv-2");
    assert_eq!(injections[1].1, "inject_and_run");
}

#[tokio::test]
async fn test_same_content_different_servers_both_pass() {
    let (adapter, injector, _rt) = make_adapter();
    adapter.register_server("srv-A", McpNotificationMode::InjectSummary, false);
    adapter.register_server("srv-B", McpNotificationMode::InjectSummary, false);

    let n1 = make_message_notification("srv-A", "identical");
    let n2 = make_message_notification("srv-B", "identical");

    assert!(adapter.handle_notification(n1).await.unwrap().is_some());
    assert!(adapter.handle_notification(n2).await.unwrap().is_some());
    assert_eq!(injector.count(), 2);
}

// ── Shared runtime tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_adapter_shares_runtime_with_dynamic_rules() {
    let runtime = TriggerRuntime::new(TriggerRuntimeConfig {
        dedup_window: Duration::from_secs(0),
        max_cycles: 100,
    });
    let injector = Arc::new(RecordingNotificationInjector::new());
    let adapter = McpNotificationAdapter::new(runtime.clone(), injector);

    adapter.register_server("srv-1", McpNotificationMode::InjectSummary, false);

    // The adapter's runtime should be the same one we passed in.
    assert_eq!(adapter.runtime().rule_count(), 0);

    // Process a notification.
    let notification = make_message_notification("srv-1", "test");
    adapter.handle_notification(notification).await.unwrap();

    // The dedup cache should have an entry from the MCP notification.
    assert_eq!(adapter.runtime().dedup_cache_size(), 1);

    // The shared runtime should also see the dedup entry.
    assert_eq!(runtime.dedup_cache_size(), 1);
}
