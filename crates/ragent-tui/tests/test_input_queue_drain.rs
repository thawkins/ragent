//! Tests for the queue drain at the `MessageEnd` turn boundary
//! (spec `inputqueue` T-006).
//!
//! Covers FR-006 (a non-cancelled finish pops the oldest entry and dispatches
//! it), FR-016 (no dispatch while a compaction run or post-compact send owns
//! the turn), FR-018 (a cancelled turn retains the queue), FR-019 (FIFO — the
//! oldest entry is never overtaken), FR-008 (the counter decrements) and
//! NFR-004 (the drain reuses the asynchronous dispatch path instead of blocking
//! the UI thread).
//!
//! The drain is exercised through the public `handle_event` path so the
//! `MessageEnd` handler is tested as the event bus drives it.

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

/// Build an app with an active session so `advance_input_queue` is willing to
/// dispatch.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app
}

/// Deliver a `MessageEnd` for the current session with the given reason.
async fn message_end(app: &mut App, reason: FinishReason) {
    app.handle_event(Event::MessageEnd {
        session_id: "test-session".to_string(),
        message_id: "msg-1".to_string(),
        reason,
    })
    .await;
}

/// Count user messages in the conversation.
fn user_message_count(app: &App) -> usize {
    app.messages.iter().filter(|m| m.role == Role::User).count()
}

#[tokio::test]
async fn test_message_end_drains_oldest_entry() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("first queued"));

    message_end(&mut app, FinishReason::Stop).await;

    assert_eq!(
        app.input_queue_len(),
        0,
        "FR-006: the oldest entry is popped at the turn boundary"
    );
    assert_eq!(
        app.last_prompt, "first queued",
        "FR-006: the popped entry is dispatched as the next user turn"
    );
    assert!(
        app.messages
            .iter()
            .any(|m| m.role == Role::User && m.text_content() == "first queued"),
        "FR-006: the dispatched message is added to the conversation"
    );
}

#[tokio::test]
async fn test_message_end_drains_strict_fifo_order() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("A"));
    app.input_queue.push_back(entry("B"));
    app.input_queue.push_back(entry("C"));

    message_end(&mut app, FinishReason::Stop).await;
    assert_eq!(app.last_prompt, "A", "FR-019: oldest entry goes first");
    assert_eq!(app.input_queue_len(), 2);

    // The dispatched turn ends, opening the next boundary.
    message_end(&mut app, FinishReason::Stop).await;
    assert_eq!(app.last_prompt, "B");
    assert_eq!(app.input_queue_len(), 1);

    message_end(&mut app, FinishReason::Stop).await;
    assert_eq!(app.last_prompt, "C");
    assert_eq!(app.input_queue_len(), 0);
}

#[tokio::test]
async fn test_cancelled_turn_retains_the_queue() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("keep me"));

    message_end(&mut app, FinishReason::Cancelled).await;

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

#[tokio::test]
async fn test_empty_queue_is_a_noop_at_the_boundary() {
    let mut app = app_with_session();
    app.is_processing = true;

    message_end(&mut app, FinishReason::Stop).await;

    assert_eq!(app.input_queue_len(), 0);
    assert_eq!(user_message_count(&app), 0);
}

#[tokio::test]
async fn test_drain_is_skipped_while_compaction_owns_the_turn() {
    // The MessageEnd compaction path is already covered by the post-compact
    // send test; drive advance_input_queue directly to pin the compaction
    // guard independently (T-008 boundary check shared by every drain path).
    let mut app = app_with_session();
    app.compact_in_progress = true;
    app.input_queue.push_back(entry("deferred"));

    app.advance_input_queue().await;

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-016: the queue is not drained while compaction owns the turn"
    );
    assert_eq!(user_message_count(&app), 0);
}

#[tokio::test]
async fn test_drain_is_skipped_while_auto_compaction_owns_the_turn() {
    let mut app = app_with_session();
    app.auto_compact_in_progress = true;
    app.input_queue.push_back(entry("deferred"));

    app.advance_input_queue().await;

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-016: the queue is not drained while auto-compaction owns the turn"
    );
    assert_eq!(user_message_count(&app), 0);
}

#[tokio::test]
async fn test_drain_is_skipped_while_still_processing() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("deferred"));

    app.advance_input_queue().await;

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-016: a running turn defers the drain to the next boundary"
    );
    assert_eq!(user_message_count(&app), 0);
}

#[tokio::test]
async fn test_drain_is_skipped_without_an_active_session() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("no session"));

    app.advance_input_queue().await;

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-018: with no active session the entry is retained, never dropped"
    );
}

#[tokio::test]
async fn test_drain_is_skipped_when_a_post_compact_send_is_pending() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.pending_send_after_compact = Some(("in flight".to_string(), Vec::new()));
    app.input_queue.push_back(entry("deferred"));

    message_end(&mut app, FinishReason::Stop).await;

    assert_eq!(
        app.input_queue_len(),
        1,
        "FR-016: the queue is not drained while a post-compact send is pending"
    );
}

#[tokio::test]
async fn test_drain_requests_redraw_for_the_decremented_counter() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("one"));
    app.input_queue.push_back(entry("two"));
    app.needs_redraw = false;

    message_end(&mut app, FinishReason::Stop).await;

    assert!(
        app.needs_redraw,
        "FR-008: the decremented counter must be painted on the next frame"
    );
}

#[tokio::test]
async fn test_drain_uses_the_async_dispatch_path_without_blocking() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(entry("async"));

    message_end(&mut app, FinishReason::Stop).await;

    // NFR-004: dispatch_user_message spawns the turn on a tokio task and arms a
    // cancel flag; it never runs the turn inline, so the UI thread stays free.
    assert!(
        app.cancel_flag.is_some(),
        "NFR-004: the async dispatch armed a cancel flag for the new turn"
    );
}

#[tokio::test]
async fn test_multiple_boundaries_drain_the_queue_to_empty() {
    let mut app = app_with_session();
    let total = 5;
    for i in 0..total {
        app.input_queue.push_back(entry(&format!("entry-{i}")));
    }

    for _ in 0..total {
        app.is_processing = true;
        message_end(&mut app, FinishReason::Stop).await;
    }

    assert_eq!(
        app.input_queue_len(),
        0,
        "FR-006: repeated boundaries drain every entry"
    );
    assert_eq!(app.last_prompt, "entry-4", "FIFO order is preserved");
    assert_eq!(
        user_message_count(&app),
        total,
        "each drained entry becomes one user turn"
    );
}

#[tokio::test]
async fn test_drained_entry_with_attachments_is_dispatched() {
    let mut app = app_with_session();
    app.is_processing = true;
    app.input_queue.push_back(QueuedInput {
        text: "with image".to_string(),
        image_paths: vec![std::path::PathBuf::from("/tmp/shot.png")],
    });

    message_end(&mut app, FinishReason::Stop).await;

    assert_eq!(app.input_queue_len(), 0);
    assert!(
        app.messages
            .iter()
            .any(|m| m.role == Role::User && m.text_content().contains("with image")),
        "the drained entry's text is dispatched"
    );
}
