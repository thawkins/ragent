//! TUI event-handler tests for hook warning and flagged tool-result events.
//!
//! Covers T-023: `Event::HookWarning` renders a transient status toast and a
//! log entry; `Event::ToolResultFlagged` renders a flagged marker in the log
//! panel.

use ragent_agent::event::Event;
use ragent_tui::app::LogLevel;

#[path = "support/mod.rs"]
mod support;

#[test]
fn test_hook_warning_current_session_sets_transient_status_and_logs() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::HookWarning {
        session_id: "s1".to_string(),
        hook_command: "hooks/warn.sh".to_string(),
        tool: "bash".to_string(),
        stderr: "suspicious network call".to_string(),
    });

    assert!(
        app.status.contains("hook warning"),
        "status should show hook warning toast, got: {}",
        app.status
    );
    assert!(
        app.status.contains("bash"),
        "status should name the tool, got: {}",
        app.status
    );
    assert!(
        app.status.contains("suspicious network call"),
        "status should contain the short reason, got: {}",
        app.status
    );
    assert!(
        app.status_set_at.is_some(),
        "transient status should arm the expiry timer"
    );

    let entry = app.log_entries.last().expect("log entry should be added");
    assert_eq!(entry.level, LogLevel::Warn);
    assert!(
        entry.message.contains("hook warning on bash"),
        "log should identify the tool, got: {}",
        entry.message
    );
    assert!(
        entry.message.contains("hooks/warn.sh"),
        "log should identify the hook command, got: {}",
        entry.message
    );
    assert!(
        entry.message.contains("suspicious network call"),
        "log should contain the full stderr reason, got: {}",
        entry.message
    );
}

#[test]
fn test_hook_warning_long_stderr_is_truncated_safely_in_status() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    let long_reason = "a".repeat(200);

    app.handle_event(Event::HookWarning {
        session_id: "s1".to_string(),
        hook_command: "hooks/warn.sh".to_string(),
        tool: "write".to_string(),
        stderr: long_reason.clone(),
    });

    // The status toast must be truncated to keep the status bar readable.
    assert!(app.status.len() < long_reason.len() + 20);
    assert!(
        app.status.ends_with('…'),
        "status should truncate long reason with ellipsis, got: {}",
        app.status
    );

    // The log panel keeps the full reason.
    let entry = app.log_entries.last().expect("log entry should be added");
    assert!(entry.message.contains(&long_reason));
}

#[test]
fn test_hook_warning_other_session_is_ignored() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.status = "ready".to_string();

    app.handle_event(Event::HookWarning {
        session_id: "s2".to_string(),
        hook_command: "hooks/warn.sh".to_string(),
        tool: "bash".to_string(),
        stderr: "suspicious".to_string(),
    });

    assert!(app.log_entries.is_empty(), "no log entry for other session");
    assert_eq!(app.status, "ready", "status should remain unchanged");
    assert!(
        app.status_set_at.is_none(),
        "expiry timer should not be armed"
    );
}

#[test]
fn test_hook_warning_transient_status_expires_to_ready() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::HookWarning {
        session_id: "s1".to_string(),
        hook_command: "hooks/warn.sh".to_string(),
        tool: "bash".to_string(),
        stderr: "suspicious".to_string(),
    });
    assert!(app.status_set_at.is_some());

    // Simulate the grace period having elapsed by backdating the armed instant.
    app.status_set_at = Some(
        std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(5))
            .expect("now - 5s is representable"),
    );

    app.poll_status_expiry();

    assert_eq!(app.status, "ready");
    assert!(app.status_set_at.is_none());
}

#[test]
fn test_tool_result_flagged_current_session_logs_marker_and_sets_status() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::ToolResultFlagged {
        session_id: "s1".to_string(),
        tool: "bash".to_string(),
        hook_command: "hooks/flag.sh".to_string(),
        reason: "policy violation: rm -rf /".to_string(),
    });

    assert!(
        app.status.contains("bash"),
        "status should name the flagged tool, got: {}",
        app.status
    );
    assert!(
        app.status.contains("flagged"),
        "status should show flagged marker, got: {}",
        app.status
    );

    let entry = app.log_entries.last().expect("log entry should be added");
    assert_eq!(entry.level, LogLevel::Error);
    assert!(
        entry.message.contains("[flag]"),
        "log should contain the flagged marker, got: {}",
        entry.message
    );
    assert!(
        entry.message.contains("bash"),
        "log should name the tool, got: {}",
        entry.message
    );
    assert!(
        entry.message.contains("hooks/flag.sh"),
        "log should name the hook command, got: {}",
        entry.message
    );
    assert!(
        entry.message.contains("policy violation: rm -rf /"),
        "log should contain the reason, got: {}",
        entry.message
    );
}

#[test]
fn test_tool_result_flagged_other_session_is_ignored() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.status = "ready".to_string();

    app.handle_event(Event::ToolResultFlagged {
        session_id: "s2".to_string(),
        tool: "bash".to_string(),
        hook_command: "hooks/flag.sh".to_string(),
        reason: "policy violation".to_string(),
    });

    assert!(app.log_entries.is_empty(), "no log entry for other session");
    assert_eq!(app.status, "ready", "status should remain unchanged");
}
