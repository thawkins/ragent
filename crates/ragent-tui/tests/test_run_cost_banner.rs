//! TUI tests for the `Event::RunCostSummary` handler (T-013, FR-012).
//!
//! Verifies that receiving a `RunCostSummary` event for the current session:
//!  - populates the transient `run_cost_banner` with the one-line summary,
//!  - logs the full summary (with model + ms) to the log panel,
//!  - is ignored for other sessions,
//!  - and that any keypress dismisses the banner.

use ragent_agent::event::Event;
use ragent_tui::app::LogLevel;

#[path = "support/mod.rs"]
mod support;

/// A `RunCostSummary` for the current session should set the banner and log.
#[test]
fn test_run_cost_summary_sets_banner_and_logs() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::RunCostSummary {
        session_id: "s1".to_string(),
        model_id: "gpt-4o".to_string(),
        input_tokens: 1234,
        output_tokens: 567,
        total_cost_usd: 0.012_345_6,
        duration_ms: 4_250,
    });

    let banner = app
        .run_cost_banner
        .as_ref()
        .expect("banner should be populated");
    assert!(
        banner.contains("⟡ run complete"),
        "banner should start with the run-complete marker, got: {banner}"
    );
    assert!(
        banner.contains("1234+567 tokens"),
        "banner should show in+out token counts, got: {banner}"
    );
    assert!(
        banner.contains("$0.0123"),
        "banner should show the cost, got: {banner}"
    );
    assert!(
        banner.contains("4.2s") || banner.contains("4.3s"),
        "banner should show duration in seconds (1 decimal), got: {banner}"
    );

    let entry = app.log_entries.last().expect("log entry should be added");
    assert_eq!(entry.level, LogLevel::Info);
    assert!(
        entry.message.contains("gpt-4o"),
        "log should record the model id, got: {}",
        entry.message
    );
    assert!(
        entry.message.contains("4250ms"),
        "log should record the millisecond duration, got: {}",
        entry.message
    );
    assert!(
        entry.message.contains("1234in / 567out"),
        "log should record token split, got: {}",
        entry.message
    );
}

/// A `RunCostSummary` for a different session should be ignored entirely.
#[test]
fn test_run_cost_summary_other_session_is_ignored() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::RunCostSummary {
        session_id: "s2".to_string(),
        model_id: "gpt-4o".to_string(),
        input_tokens: 100,
        output_tokens: 50,
        total_cost_usd: 0.01,
        duration_ms: 1_000,
    });

    assert!(
        app.run_cost_banner.is_none(),
        "banner should not be set for another session"
    );
    assert!(
        app.log_entries.is_empty(),
        "no log entry should be added for another session"
    );
}

/// A keypress should dismiss the transient run-cost banner.
#[test]
fn test_run_cost_banner_dismissed_on_keypress() {
    let mut app = support::make_app();
    app.session_id = Some("s1".to_string());
    app.run_cost_banner = Some("⟡ run complete · 1+2 tokens · $0.01 · 1.0s".to_string());

    // Drive the real key-dispatch path; the first keypress is consumed to
    // dismiss the banner.
    app.handle_key_event(crossterm::event::KeyEvent::from(
        crossterm::event::KeyCode::Esc,
    ));

    assert!(
        app.run_cost_banner.is_none(),
        "banner should be cleared after a keypress"
    );
}

/// The banner field defaults to `None` on a freshly constructed `App`.
#[test]
fn test_run_cost_banner_defaults_none() {
    let app = support::make_app();
    assert!(app.run_cost_banner.is_none());
}
