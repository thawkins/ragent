//! Tests for the slash-command status auto-expiry mechanism.
//!
//! After a slash command completes, the status indicator shows the command's
//! status (e.g. "help", "tools: teams on") for a short grace period and then
//! auto-transitions to "ready" so the user can see the system is ready for the
//! next interaction. These tests verify the arming, polling, and guard logic.

#[path = "support/mod.rs"]
mod support;

use ragent_tui::App;

/// A test [`App`] built on the shared crate-wide fixture.
fn make_app() -> App {
    support::make_app()
}

/// A slash command that sets a specific status should keep it immediately after
/// execution (the grace period hasn't elapsed yet).
#[tokio::test]
async fn test_slash_status_preserved_immediately_after_command() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/help").await;

    // Right after the command, the status is the command-specific value.
    assert_eq!(app.status, "help");
    // The expiry timer should be armed.
    assert!(app.status_set_at.is_some());
}

/// After the grace period elapses (and the status hasn't changed), the status
/// should transition to "ready".
#[tokio::test]
async fn test_slash_status_expires_to_ready() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/help").await;
    assert_eq!(app.status, "help");

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

/// If something else changes the status during the grace period, the timer
/// should NOT overwrite the new status.
#[tokio::test]
async fn test_slash_status_expiry_skips_if_status_changed() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/help").await;
    assert_eq!(app.status, "help");

    // Simulate the agent starting to process and setting a "busy" status.
    app.status = "busy - wait for the current turn to finish".to_string();
    // Backdate the timer so the grace period has elapsed.
    app.status_set_at = Some(
        std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(5))
            .expect("now - 5s is representable"),
    );

    app.poll_status_expiry();

    // The timer is cleared but the busy status is preserved.
    assert_eq!(app.status, "busy - wait for the current turn to finish");
    assert!(app.status_set_at.is_none());
}

/// The expiry timer should not be armed for async-in-progress () statuses.
#[test]
fn test_slash_status_expiry_not_armed_for_async() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // Simulate a slash command that sets an async status.
    app.status = "[wait] opt/co_star: optimizing…".to_string();
    app.arm_status_expiry();

    assert!(app.status_set_at.is_none());
    assert_eq!(app.status, "[wait] opt/co_star: optimizing…");
}

/// The expiry timer should not be armed for error () statuses.
#[test]
fn test_slash_status_expiry_not_armed_for_error() {
    let mut app = make_app();

    app.status = "[warn] Please provide a task ID prefix: /cancel <id>".to_string();
    app.arm_status_expiry();

    assert!(app.status_set_at.is_none());
}

/// The expiry timer should not be armed when the status is already "ready".
#[test]
fn test_slash_status_expiry_not_armed_for_ready() {
    let mut app = make_app();

    app.status = "ready".to_string();
    app.arm_status_expiry();

    assert!(app.status_set_at.is_none());
}

/// Before the grace period elapses, polling should keep the timer armed and
/// the status unchanged.
#[tokio::test]
async fn test_slash_status_expiry_keeps_timer_before_grace_period() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.execute_slash_command("/help").await;
    assert_eq!(app.status, "help");

    // Poll immediately - the grace period hasn't elapsed.
    app.poll_status_expiry();

    assert_eq!(app.status, "help");
    assert!(app.status_set_at.is_some());
}

/// A slash command that errors ( status) should not arm the expiry timer.
#[tokio::test]
async fn test_slash_error_status_not_armed() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // /cancel with no argument produces an error status.
    app.execute_slash_command("/cancel").await;

    assert!(app.status.starts_with("[warn]"));
    assert!(
        app.status_set_at.is_none(),
        "error statuses should not arm the expiry timer"
    );
}

/// `/research create` runs its analysis in a background tokio task. Its status
/// must use the `` async-in-progress prefix so [`App::arm_status_expiry`]
/// (called at the end of [`App::execute_slash_command`]) does NOT auto-clear it
/// to "ready" while the final analysis is still running.
///
/// This test reproduces the regression where the status flipped to "ready"
/// before the background research completed.
#[test]
fn test_research_create_status_is_async_in_progress() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    // Simulate the status the `/research create` handler sets.
    app.status = "[wait] research: my-topic…".to_string();
    // execute_slash_command calls arm_status_expiry after the inner handler
    // returns; replicate that here.
    app.arm_status_expiry();

    // The expiry timer must NOT be armed for an async-in-progress status.
    assert!(
        app.status_set_at.is_none(),
        "research create status should be async-in-progress (⏳) and not armed for auto-expiry"
    );
    assert_eq!(app.status, "[wait] research: my-topic…");

    // Even after the grace period elapses, the status must remain unchanged.
    app.status_set_at = Some(
        std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(5))
            .expect("now - 5s is representable"),
    );
    app.poll_status_expiry();
    assert_eq!(
        app.status, "[wait] research: my-topic…",
        "async-in-progress research status must not auto-expire to ready"
    );
}

/// The live per-phase status updated by the research progress event handler
/// also uses the `` prefix, so it stays visible (and is not auto-cleared)
/// while the background research is still running.
#[test]
fn test_research_progress_status_is_async_in_progress() {
    let mut app = make_app();

    // Simulate the per-phase running status set by the AgentNotice progress
    // handler while the research session is mid-run.
    app.status = "[wait] research: my-topic — web (▶) — running".to_string();
    app.arm_status_expiry();
    assert!(
        app.status_set_at.is_none(),
        "running research phase status should not arm the auto-expiry timer"
    );

    // Once the final progress event arrives, the handler sets a terminal
    // (non-) completion status and arms expiry - that one SHOULD expire.
    app.status = "research: my-topic complete — 7 sources".to_string();
    app.arm_status_expiry();
    assert!(
        app.status_set_at.is_some(),
        "terminal research completion status should arm the auto-expiry timer"
    );
    app.status_set_at = Some(
        std::time::Instant::now()
            .checked_sub(std::time::Duration::from_secs(5))
            .expect("now - 5s is representable"),
    );
    app.poll_status_expiry();
    assert_eq!(app.status, "ready");
}
