//! Inline tests for `mcp_notification.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

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
    let notification = McpNotification::new("srv-1", "notifications/custom", json!({"foo": "bar"}));
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
