//! Tests for the dispatch overlap guard (spec `inputqueue` T-008, FR-016).
//!
//! FR-016: the TUI **shall not** dispatch a queued entry while the primary
//! agent is executing (`is_processing` is `true`) or while a compaction run is
//! in progress. [`App::advance_input_queue`] owns that single boundary check, so
//! every drain path (`MessageEnd`, `AgentError`, and the queue-control `Next`
//! selection) shares one guard.
//!
//! These tests drive [`App::advance_input_queue`] directly to pin each overlap
//! signal independently, and confirm the guard *defers* rather than *drops*:
//! once the overlapping state clears, the retained entry dispatches at the next
//! boundary (the same deferral contract the `MessageEnd`/`AgentError` suites
//! exercise end to end).

use ragent_agent::message::Role;

use ragent_tui::App;
use ragent_tui::app::QueuedInput;

#[path = "support/mod.rs"]
mod support;

/// Seed a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

/// Build an app with an active session so `advance_input_queue` is willing to
/// dispatch once the overlap guard clears.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app
}

/// Count user messages in the conversation.
fn user_message_count(app: &App) -> usize {
    app.messages.iter().filter(|m| m.role == Role::User).count()
}

// ---------------------------------------------------------------------------
// FR-016 — a running turn blocks the dispatch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_guard_defers_while_the_primary_agent_is_processing() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("deferred"));

    // A running turn cannot be dispatched over; the queue is untouched.
    app.advance_input_queue().await;
    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-016: no dispatch while is_processing is true"
    );
    assert_eq!(
        user_message_count(&app),
        0,
        "FR-016: nothing is dispatched over the running turn"
    );

    // The turn finishes: the retained entry runs at the next boundary.
    app.is_processing = false;
    app.advance_input_queue().await;
    assert_eq!(user_message_count(&app), 1);
    assert_eq!(app.last_prompt, "deferred");
    assert_eq!(app.input_queue_len(), 0);
}

// ---------------------------------------------------------------------------
// FR-016 — a compaction run blocks the dispatch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_guard_defers_while_compaction_is_in_progress() {
    let mut app = app_with_session();
    app.compact_in_progress = true;
    app.input_queue.push_back(entry("deferred"));

    app.advance_input_queue().await;
    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-016: no dispatch while compact_in_progress is true"
    );
    assert_eq!(user_message_count(&app), 0);

    app.compact_in_progress = false;
    app.advance_input_queue().await;
    assert_eq!(app.last_prompt, "deferred");
    assert_eq!(app.input_queue_len(), 0);
}

#[tokio::test]
async fn test_guard_defers_while_auto_compaction_is_in_progress() {
    let mut app = app_with_session();
    app.auto_compact_in_progress = true;
    app.input_queue.push_back(entry("deferred"));

    app.advance_input_queue().await;
    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-016: no dispatch while auto_compact_in_progress is true"
    );
    assert_eq!(user_message_count(&app), 0);

    app.auto_compact_in_progress = false;
    app.advance_input_queue().await;
    assert_eq!(app.last_prompt, "deferred");
    assert_eq!(app.input_queue_len(), 0);
}

#[tokio::test]
async fn test_guard_defers_when_a_post_compact_send_is_pending() {
    let mut app = app_with_session();
    app.pending_send_after_compact = Some(("in flight".to_string(), Vec::new()));
    app.input_queue.push_back(entry("deferred"));

    app.advance_input_queue().await;
    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-016: no dispatch while a post-compact send is pending"
    );
    assert_eq!(user_message_count(&app), 0);

    app.pending_send_after_compact = None;
    app.advance_input_queue().await;
    assert_eq!(app.last_prompt, "deferred");
    assert_eq!(app.input_queue_len(), 0);
}

// ---------------------------------------------------------------------------
// FR-016 — every overlap signal blocks; only a fully idle boundary dispatches
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_guard_holds_until_every_overlap_signal_clears() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.compact_in_progress = true;
    app.auto_compact_in_progress = true;
    app.pending_send_after_compact = Some(("in flight".to_string(), Vec::new()));
    app.input_queue.push_back(entry("deferred"));

    // Clear the signals one at a time; the guard holds while any remain set.
    app.advance_input_queue().await;
    assert_eq!(app.input_queue_len(), 1, "all four signals set");

    app.is_processing = false;
    app.advance_input_queue().await;
    assert_eq!(app.input_queue_len(), 1, "compaction signals remain");

    app.compact_in_progress = false;
    app.advance_input_queue().await;
    assert_eq!(app.input_queue_len(), 1, "auto-compaction remains");

    app.auto_compact_in_progress = false;
    app.advance_input_queue().await;
    assert_eq!(
        app.input_queue_len(),
        1,
        "pending post-compact send remains"
    );

    app.pending_send_after_compact = None;
    app.advance_input_queue().await;
    assert_eq!(
        app.input_queue_len(),
        0,
        "FR-016: the entry dispatches only once no overlap signal remains"
    );
    assert_eq!(app.last_prompt, "deferred");
}

// ---------------------------------------------------------------------------
// FR-016 — the guard also holds the `Next`-triggered dispatch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_guard_defers_pending_next_while_processing() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.queue_next_pending = true;
    app.input_queue.push_back(entry("next-entry"));

    app.advance_input_queue().await;
    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-016: a pending Next cannot dispatch over a running turn"
    );
    assert!(
        app.queue_next_pending,
        "FR-030: the pending Next survives the unavailable boundary"
    );
    assert_eq!(user_message_count(&app), 0);

    // Safe boundary: the pending dispatch fires exactly once.
    app.is_processing = false;
    app.advance_input_queue().await;
    assert_eq!(app.last_prompt, "next-entry");
    assert_eq!(app.input_queue_len(), 0);
    assert!(
        !app.queue_next_pending,
        "the pending flag is consumed at the safe boundary"
    );
}

#[tokio::test]
async fn test_guard_defers_pending_next_during_compaction() {
    let mut app = app_with_session();
    app.compact_in_progress = true;
    app.queue_next_pending = true;
    app.input_queue.push_back(entry("next-entry"));

    app.advance_input_queue().await;
    assert_eq!(app.input_queue_len(), 1);
    assert!(
        app.queue_next_pending,
        "FR-016/FR-030: a compaction window defers the pending Next"
    );
    assert_eq!(user_message_count(&app), 0);
}

// ---------------------------------------------------------------------------
// FR-016 — an idle boundary still dispatches (guard is not over-broad)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_guard_allows_dispatch_at_a_fully_idle_boundary() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("ready"));

    app.advance_input_queue().await;

    assert_eq!(
        app.input_queue_len(),
        0,
        "FR-016: with no overlap the guard lets the drain proceed"
    );
    assert_eq!(app.last_prompt, "ready");
}
