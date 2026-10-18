//! `lib.rs` scheduling and `TerminalGuard` guard coverage.
//!
//! `run_tui` itself needs a real terminal, so it is not reachable from an
//! integration test. Its two pure helpers are: `compute_next_deadline_test`
//! (which the production loop calls each wake) and `should_render` (the
//! PERF-044 paint decision). This file drives both across every deadline
//! contributor and the `IDLE_REDRAW_INTERVAL_MS` boundary, so the policy is
//! pinned without a TTY.
//!
//! Tests only - no production logic is touched.

use std::time::{Duration, Instant};

use ragent_agent::permission::PermissionRequest;
use ragent_tui::{App, compute_next_deadline_test, should_render};

#[path = "support/mod.rs"]
mod support;

/// An app with a live session and no pending timers.
fn app() -> App {
    let mut app = support::make_app();
    app.session_id = Some("lib-timers".to_string());
    app
}

/// A pending file-read permission request.
fn permission_request(id: &str) -> PermissionRequest {
    PermissionRequest {
        id: id.to_string(),
        session_id: "lib-timers".to_string(),
        permission: "read".to_string(),
        patterns: vec!["src/lib.rs".to_string()],
        metadata: serde_json::Value::Null,
        tool_call_id: None,
    }
}

/// An `Instant` `ms` milliseconds in the past (saturating, so it can never
/// panic on a short monotonic clock).
fn stale(ms: u64) -> Instant {
    Instant::now()
        .checked_sub(Duration::from_millis(ms))
        .unwrap_or_else(Instant::now)
}

/// Bound the deadline to at most `at` (the helpers only ever shorten it).
fn deadline_is_at_most(deadline: Instant, at: Instant) -> bool {
    deadline <= at + Duration::from_millis(50)
}

/// With nothing animate, the wake is bounded by the idle redraw interval.
#[test]
fn deadline_idles_at_the_redraw_interval() {
    let app = app();
    let last_draw = Instant::now();
    let deadline = compute_next_deadline_test(&app, last_draw);
    assert!(
        deadline_is_at_most(deadline, last_draw + Duration::from_millis(2000)),
        "idle deadline must be bounded by IDLE_REDRAW_INTERVAL_MS"
    );
    assert!(deadline > Instant::now(), "deadline must be in the future");
}

/// A queued permission request wakes the loop within one second (countdown).
#[test]
fn deadline_wakes_for_permission_countdown() {
    let mut app = app();
    app.permission_queue.push_back(permission_request("req-1"));
    let deadline = compute_next_deadline_test(&app, Instant::now());
    assert!(
        deadline_is_at_most(deadline, Instant::now() + Duration::from_secs(1)),
        "a pending permission must wake within a second"
    );
}

/// An armed status expiry shortens the wake to the expiry instant.
#[test]
fn deadline_honours_status_expiry() {
    let mut app = app();
    let armed = Instant::now();
    app.status_set_at = Some(armed);
    let deadline = compute_next_deadline_test(&app, Instant::now());
    let expected = armed + Duration::from_millis(ragent_tui::app::STATUS_EXPIRY_MS);
    assert!(
        deadline_is_at_most(deadline, expected),
        "status expiry must bound the deadline"
    );
}

/// A showing run-cost banner shortens the wake to its own expiry.
#[test]
fn deadline_honours_run_cost_banner_expiry() {
    let mut app = app();
    let shown = Instant::now();
    app.run_cost_banner_at = Some(shown);
    let deadline = compute_next_deadline_test(&app, Instant::now());
    let expected = shown + Duration::from_secs(ragent_tui::app::RUN_COST_BANNER_EXPIRY_SECS);
    assert!(
        deadline_is_at_most(deadline, expected),
        "run-cost banner expiry must bound the deadline"
    );
}

/// A pending history flush pins the wake to the save deadline.
#[test]
fn deadline_honours_history_save_deadline() {
    let mut app = app();
    let save_at = Instant::now() + Duration::from_millis(750);
    app.history_save_deadline = Some(save_at);
    let deadline = compute_next_deadline_test(&app, Instant::now());
    assert!(
        deadline_is_at_most(deadline, save_at),
        "pending history flush must bound the deadline"
    );
}

/// An active swarm polls every two seconds.
#[test]
fn deadline_polls_active_swarm() {
    let mut app = app();
    app.swarm_state = Some(ragent_agent::team::SwarmState {
        team_name: "swarm-timers".to_string(),
        prompt: "do the thing".to_string(),
        decomposition: ragent_agent::team::SwarmDecomposition { tasks: Vec::new() },
        spawned: false,
        completed: false,
        default_agent_type: None,
    });
    let deadline = compute_next_deadline_test(&app, Instant::now());
    assert!(
        deadline_is_at_most(deadline, Instant::now() + Duration::from_secs(2)),
        "an active swarm must wake within two seconds"
    );
}

/// `should_render` paints when the UI is dirty.
#[test]
fn should_render_when_dirty() {
    let mut app = app();
    app.needs_redraw = true;
    assert!(should_render(&app, Instant::now()));
}

/// `should_render` stays quiet at rest (no dirty flag, fresh draw).
#[test]
fn should_not_render_when_idle_and_recently_drawn() {
    let mut app = app();
    app.needs_redraw = false;
    assert!(
        !should_render(&app, Instant::now()),
        "an idle app must not repaint on a fresh frame"
    );
}

/// A pending streamed message group forces a paint at the safety interval.
#[test]
fn should_render_for_pending_stream_group_at_the_safety_interval() {
    let mut app = app();
    app.needs_redraw = false;
    app.messages.push(ragent_types::message::Message::user_text(
        "lib-timers",
        "hello",
    ));
    app.message_cache_dirty_from = 0;
    let stale_draw = stale(2500);
    assert!(
        should_render(&app, stale_draw),
        "a pending cache group must force a paint at the safety interval"
    );
}

/// After the safety interval a periodic display repaints.
#[test]
fn should_render_for_periodic_display_at_the_safety_interval() {
    let mut app = app();
    app.needs_redraw = false;
    app.permission_queue.push_back(permission_request("req-2"));
    let stale_draw = stale(2500);
    assert!(
        should_render(&app, stale_draw),
        "a live countdown must repaint at the safety interval"
    );
}
