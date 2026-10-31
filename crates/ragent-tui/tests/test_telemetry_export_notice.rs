//! TUI tests for the OTLP export-failure "Agent Notice" surface.
//!
//! The periodic OTLP reader never publishes an [`Event`], so an unreachable
//! collector or a non-2xx response reaches the UI only through
//! [`App::poll_telemetry_export_health`], which drains the process-wide
//! `ragent_telemetry::export_health` monitor. These tests drive that poll with
//! records made directly against the shared monitor and assert that:
//!
//! - one failed export produces exactly one "Agent Notice" bubble naming the
//!   consecutive-failure count, the reason, and the endpoint;
//! - the fifth consecutive failure disables telemetry.
//!
//! The monitor is process-wide, so the crate-wide `global_env_lock()` is held
//! for the duration of each test.

use ragent_agent::message::{MessagePart, Role};
use ragent_telemetry::export_health::{AUTO_DISABLE_THRESHOLD, ExportFailureKind, export_health};

#[path = "support/mod.rs"]
mod support;

/// Run `poll_telemetry_export_health` and return the notice bubbles it added
/// (every assistant message whose first text part starts with "Agent Notice").
fn poll_and_collect_notices(app: &mut ragent_tui::App) -> Vec<String> {
    let before = app.messages.len();
    app.poll_telemetry_export_health();
    app.messages[before..]
        .iter()
        .filter(|message| message.role == Role::Assistant)
        .filter_map(|message| match message.parts.first() {
            Some(MessagePart::Text { text }) if text.starts_with("Agent Notice") => {
                Some(text.clone())
            }
            _ => None,
        })
        .collect()
}

/// A non-2xx export response must surface as a single "Agent Notice" bubble
/// naming the status, the endpoint, and the failure count.
#[test]
fn test_export_failure_emits_agent_notice() {
    let _guard = crate::global_env_lock();
    let health = export_health();
    health.clear();
    health.record_failure(
        ExportFailureKind::HttpStatus,
        "http://127.0.0.1:4318/v1/metrics",
        "Operation failed: HTTP export failed with status code: 503",
    );

    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    let notices = poll_and_collect_notices(&mut app);

    assert_eq!(
        notices.len(),
        1,
        "one failure must produce one notice: {notices:?}"
    );
    let text = &notices[0];
    assert!(text.contains("telemetry export failed"), "got: {text}");
    assert!(text.contains("1 of 5 consecutive"), "got: {text}");
    assert!(
        text.contains("collector returned an error status"),
        "got: {text}"
    );
    assert!(text.contains("status code: 503"), "got: {text}");
    assert!(text.contains("127.0.0.1:4318"), "got: {text}");
    assert!(
        !text.contains("Telemetry disabled"),
        "a single failure must not disable telemetry: {text}"
    );

    // The monitor is drained: a second poll adds nothing.
    assert_eq!(poll_and_collect_notices(&mut app).len(), 0);
    health.clear();
}

/// A timeout is reported with its own reason wording.
#[test]
fn test_export_timeout_notice_names_timeout() {
    let _guard = crate::global_env_lock();
    let health = export_health();
    health.clear();
    health.record_failure(
        ExportFailureKind::Timeout,
        "http://127.0.0.1:4318",
        "Operation timed out: 1s",
    );

    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    let notices = poll_and_collect_notices(&mut app);

    assert_eq!(notices.len(), 1, "expected one notice: {notices:?}");
    assert!(
        notices[0].contains("export timed out"),
        "got: {:?}",
        notices[0]
    );
    health.clear();
}

/// The fifth consecutive failure turns telemetry off through the same path as
/// `/telemetry off`, and its notice says so.
#[test]
fn test_fifth_consecutive_failure_disables_telemetry() {
    let _guard = crate::global_env_lock();
    let health = export_health();
    health.clear();
    for _ in 0..AUTO_DISABLE_THRESHOLD {
        health.record_failure(
            ExportFailureKind::Transport,
            "http://127.0.0.1:4318",
            "refused",
        );
    }
    assert!(
        health.is_auto_disabled(),
        "the exporter must latch the auto-disable"
    );

    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    let notices = poll_and_collect_notices(&mut app);

    // Every queued failure is reported, and the last one announces the disable.
    assert_eq!(
        notices.len(),
        AUTO_DISABLE_THRESHOLD as usize,
        "expected one notice per failed export: {notices:?}"
    );
    let last = notices.last().expect("at least one notice");
    assert!(
        last.contains("5 of 5 consecutive") && last.contains("Telemetry disabled after 5"),
        "the final notice must announce the disable: {last}"
    );

    // The poll performed the `/telemetry off` side effect: the live subsystem
    // is disabled and the latch has been released so `/telemetry on` can retry.
    assert!(
        !app.session_processor.telemetry.is_enabled(),
        "telemetry must be disabled after the threshold"
    );
    assert!(
        !health.is_auto_disabled(),
        "the latch must be cleared once the UI has acted on it"
    );
    assert!(
        app.status
            .contains("disabled after repeated export failures")
    );
    health.clear();
}

/// With nothing recorded the poll is a no-op, so a healthy session never
/// gains a stray notice bubble.
#[test]
fn test_no_failures_emits_no_notice() {
    let _guard = crate::global_env_lock();
    let health = export_health();
    health.clear();

    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    let before = app.messages.len();
    app.poll_telemetry_export_health();
    assert_eq!(
        app.messages.len(),
        before,
        "a clean monitor must not post a notice"
    );
}
