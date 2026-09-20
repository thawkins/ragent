//! Tests for the queue-control menu's `Next` action (spec `inputqueue` T-015).
//!
//! Covers FR-024 (selecting `Next` stops the running turn and dispatches the
//! oldest queued entry as the next user turn), FR-029 (a stop never advances the
//! queue on its own — the dispatch is deferred to the turn boundary), FR-030 (the
//! action is deferred or refused while a compaction run owns the turn) and
//! NFR-006 (the action never blocks the UI thread).
//!
//! The row is driven through the public [`App::queue_menu_select_next`] entry
//! point, mirroring how the T-016 tests drive [`App::queue_menu_select_halt`] and
//! the T-006 drain tests drive [`App::advance_input_queue`]. The Up/Down/Enter
//! key handling that routes a keystroke to this method is covered by the menu
//! key-handling tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use ragent_agent::event::{Event, FinishReason};
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

/// Build an app with an active session so the dispatch path has a session.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app
}

/// Deliver a `MessageEnd` for the current session with the given reason.
fn message_end(app: &mut App, reason: FinishReason) {
    app.handle_event(Event::MessageEnd {
        session_id: "test-session".to_string(),
        message_id: "msg-1".to_string(),
        reason,
    });
}

/// Count user messages in the conversation.
fn user_message_count(app: &App) -> usize {
    app.messages.iter().filter(|m| m.role == Role::User).count()
}

// ---------------------------------------------------------------------------
// FR-024 — Next dispatches the oldest entry (immediate when idle)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_next_row_dispatches_oldest_entry_when_not_processing() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("NEXT-QUEUE-1"));
    app.input_queue.push_back(entry("NEXT-QUEUE-2"));

    app.queue_menu_select_next();

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-024: Next pops exactly one entry (the oldest)"
    );
    assert_eq!(
        app.last_prompt, "NEXT-QUEUE-1",
        "FR-024: the oldest entry is dispatched as the next user turn"
    );
    assert!(
        app.messages
            .iter()
            .any(|m| m.role == Role::User && m.text_content() == "NEXT-QUEUE-1"),
        "FR-024: the dispatched entry is added to the conversation"
    );
}

#[tokio::test]
async fn test_next_row_preserves_fifo_order_across_selections() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("A"));
    app.input_queue.push_back(entry("B"));

    app.queue_menu_select_next();
    assert_eq!(app.last_prompt, "A", "FR-019: oldest entry goes first");
    assert_eq!(app.input_queue_len(), 1);

    app.queue_menu_select_next();
    assert_eq!(app.last_prompt, "B", "FR-019: the next entry follows");
    assert_eq!(app.input_queue_len(), 0);
}

#[tokio::test]
async fn test_next_row_arms_the_async_dispatch_path() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("async"));

    app.queue_menu_select_next();

    // NFR-006: dispatch_user_message spawns the turn on a tokio task and arms a
    // cancel flag; it never runs the turn inline, so the UI thread stays free.
    assert!(
        app.cancel_flag.is_some(),
        "NFR-006: the dispatch armed a cancel flag off the UI thread"
    );
}

// ---------------------------------------------------------------------------
// FR-024 / FR-029 — Next stops the running turn, deferring the dispatch
// ---------------------------------------------------------------------------

#[test]
fn test_next_row_halts_the_running_turn() {
    let mut app = app_with_session();
    app.is_processing = true;
    let flag = Arc::new(AtomicBool::new(false));
    app.cancel_flag = Some(flag.clone());
    app.input_queue.push_back(entry("first"));

    app.queue_menu_select_next();

    assert!(
        flag.load(Ordering::Relaxed),
        "FR-024: selecting Next must set the running turn's cancel flag"
    );
}

#[test]
fn test_next_row_does_not_advance_the_queue_while_processing() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
    app.input_queue.push_back(entry("first"));
    app.input_queue.push_back(entry("second"));
    let before = user_message_count(&app);

    app.queue_menu_select_next();

    assert_eq!(
        app.input_queue_len(),
        2,
        "FR-029: a running turn is not dispatched over; the queue is untouched"
    );
    assert_eq!(
        user_message_count(&app),
        before,
        "FR-029: no entry is dispatched while the turn is still executing"
    );
    assert!(
        app.queue_next_pending,
        "FR-030: the dispatch is deferred to the turn boundary the cancel opens"
    );
}

#[tokio::test]
async fn test_next_row_defers_dispatch_to_the_turn_boundary() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
    app.input_queue.push_back(entry("deferred-entry"));

    app.queue_menu_select_next();
    assert_eq!(app.input_queue_len(), 1, "precondition: deferred");
    assert_eq!(
        user_message_count(&app),
        0,
        "precondition: nothing dispatched"
    );

    // The cancel settles: a cancelled turn boundary forces the deferred dispatch.
    message_end(&mut app, FinishReason::Cancelled);

    assert_eq!(
        app.last_prompt, "deferred-entry",
        "FR-024: the deferred entry runs at the boundary the cancel opened"
    );
    assert_eq!(app.input_queue_len(), 0);
    assert!(
        !app.queue_next_pending,
        "the pending flag is consumed exactly once"
    );
}

#[tokio::test]
async fn test_pending_next_fires_only_once_at_later_boundaries() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.cancel_flag = Some(Arc::new(AtomicBool::new(false)));
    app.input_queue.push_back(entry("one"));
    app.input_queue.push_back(entry("two"));

    app.queue_menu_select_next();
    message_end(&mut app, FinishReason::Cancelled);
    assert_eq!(app.last_prompt, "one");
    assert_eq!(app.input_queue_len(), 1);

    // A normal completion now drains only because the queue is non-empty; the
    // spent pending flag does not force a second dispatch of its own.
    message_end(&mut app, FinishReason::Stop);
    assert_eq!(app.last_prompt, "two");
    assert_eq!(app.input_queue_len(), 0);
}

// ---------------------------------------------------------------------------
// FR-018 — the new cancelled-path condition does not discard entries
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cancelled_turn_without_pending_next_still_retains_the_queue() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("keep me"));

    // A plain Escape cancel (not a Next) leaves the queue alone (FR-018).
    message_end(&mut app, FinishReason::Cancelled);

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-018: a cancelled turn must not discard queued entries"
    );
    assert_eq!(
        user_message_count(&app),
        0,
        "FR-018: nothing is dispatched when the turn is cancelled"
    );
}

// ---------------------------------------------------------------------------
// FR-030 — deferral / refusal while compaction owns the turn
// ---------------------------------------------------------------------------

#[test]
fn test_next_row_defers_while_compaction_owns_the_turn() {
    let mut app = app_with_session();
    app.compact_in_progress = true;
    app.input_queue.push_back(entry("deferred"));

    app.queue_menu_select_next();

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-030: no entry is dispatched while compaction runs"
    );
    assert_eq!(user_message_count(&app), 0);
    assert!(
        !app.queue_next_pending,
        "FR-030: a compaction window refuses (no boundary to defer to)"
    );
    assert_eq!(app.status, "queue: next deferred — compaction in progress");
}

#[test]
fn test_next_row_defers_while_auto_compaction_owns_the_turn() {
    let mut app = app_with_session();
    app.auto_compact_in_progress = true;
    app.input_queue.push_back(entry("deferred"));

    app.queue_menu_select_next();

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-030: auto-compaction defers too"
    );
    assert_eq!(user_message_count(&app), 0);
}

#[test]
fn test_next_row_defers_when_a_post_compact_send_is_pending() {
    let mut app = app_with_session();
    app.pending_send_after_compact = Some(("in flight".to_string(), Vec::new()));
    app.input_queue.push_back(entry("deferred"));

    app.queue_menu_select_next();

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-030: a pending post-compact send defers the dispatch"
    );
    assert_eq!(user_message_count(&app), 0);
}

#[tokio::test]
async fn test_pending_next_is_retried_at_the_next_safe_boundary() {
    let mut app = app_with_session();
    app.queue_next_pending = true;
    app.compact_in_progress = true;
    app.input_queue.push_back(entry("retry me"));

    // While compaction blocks the boundary the pending flag is retained.
    app.advance_input_queue();
    assert_eq!(app.input_queue_len(), 1, "FR-030: deferred, not lost");
    assert!(
        app.queue_next_pending,
        "FR-030: the pending dispatch survives an unavailable boundary"
    );

    // Once the boundary is safe the pending dispatch fires.
    app.compact_in_progress = false;
    app.advance_input_queue();
    assert_eq!(app.last_prompt, "retry me");
    assert_eq!(app.input_queue_len(), 0);
}

// ---------------------------------------------------------------------------
// Guard rails — empty queue, menu close, input untouched, redraw
// ---------------------------------------------------------------------------

#[test]
fn test_next_row_with_empty_queue_reports_nothing_to_run() {
    let mut app = app_with_session();

    app.queue_menu_select_next();

    assert_eq!(app.input_queue_len(), 0);
    assert_eq!(
        user_message_count(&app),
        0,
        "an empty queue has nothing to dispatch"
    );
    assert_eq!(app.status, "queue: nothing to run — the queue is empty");
}

#[tokio::test]
async fn test_next_row_closes_the_menu_and_resets_selection() {
    let mut app = app_with_session();
    app.queue_menu_open = true;
    app.queue_menu_selected = 0;
    app.input_queue.push_back(entry("run me"));

    app.queue_menu_select_next();

    assert!(!app.queue_menu_open, "the menu closes after the action");
    assert_eq!(
        app.queue_menu_selected, 0,
        "the selection resets for the next open"
    );
}

#[tokio::test]
async fn test_next_row_leaves_input_buffer_and_attachments_untouched() {
    let mut app = app_with_session();
    app.input = "unfinished draft".to_string();
    app.input_cursor = 5;
    app.pending_attachments
        .push(std::path::PathBuf::from("/tmp/diagram.png"));
    app.input_queue.push_back(entry("run me"));

    app.queue_menu_select_next();

    assert_eq!(
        app.input, "unfinished draft",
        "FR-031: Next must not mutate the editable input buffer"
    );
    assert_eq!(app.input_cursor, 5, "the cursor position is unchanged");
    assert_eq!(
        app.pending_attachments.len(),
        1,
        "the staged attachments are untouched"
    );
}

#[tokio::test]
async fn test_next_row_sets_the_redraw_flag() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("run me"));
    app.needs_redraw = false;

    app.queue_menu_select_next();

    assert!(
        app.needs_redraw,
        "the decremented counter must repaint on the next frame"
    );
}
