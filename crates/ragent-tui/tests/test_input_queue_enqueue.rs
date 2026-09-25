//! Tests for the enqueue-on-submit path of the message input queue
//! (spec `inputqueue` T-003).
//!
//! Covers FR-005 (Enter while executing appends text plus staged attachments,
//! clears the field, and records history), FR-003 (history is written at
//! submission time, not dispatch), FR-004 (the capacity bound rejects overflow
//! without losing the typed text), FR-014 (a successful enqueue echoes a
//! `queued (N in queue)` notice into the log panel), FR-018 (a rejected/retained
//! message is not dropped), and FR-019 (FIFO ordering).
//!
//! These exercise the full key-event path through [`App::handle_key_event`] so
//! the `InputAction::SendMessage` arm is tested as the user drives it.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ragent_tui::App;
use ragent_tui::app::{ConfiguredProvider, ProviderSource};

#[path = "support/mod.rs"]
mod support;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Build a `make_app` with a provider and model selected so the submit path
/// passes its configuration guards and reaches the queue branch.
fn app_ready_to_send() -> App {
    let mut app = support::make_app();
    app.configured_provider = Some(ConfiguredProvider {
        id: "ollama".to_string(),
        name: "Ollama".to_string(),
        source: ProviderSource::AutoDiscovered,
    });
    app.selected_model = Some("ollama/qwen3:latest".to_string());
    app
}

/// Simulate the user typing `text` at the end of the prompt and pressing Enter.
async fn type_and_submit(app: &mut App, text: &str) {
    app.input = text.to_string();
    app.input_cursor = app.input_len_chars();
    app.handle_key_event(key(KeyCode::Enter)).await;
}

#[tokio::test]
async fn test_enter_enqueues_plain_message_while_processing() {
    let mut app = app_ready_to_send();
    app.is_processing = true;

    type_and_submit(&mut app, "queued while busy").await;

    assert_eq!(app.input_queue_len(), 1, "FR-005: Enter must enqueue");
    assert_eq!(app.input_queue[0].text, "queued while busy");
    assert!(
        app.input.is_empty(),
        "FR-005: the input field is cleared after enqueueing"
    );
    assert_eq!(app.input_cursor, 0, "the cursor resets to the line start");
    assert!(
        app.messages.is_empty(),
        "FR-016: a queued entry must not dispatch while the turn runs"
    );
}

#[tokio::test]
async fn test_enter_enqueues_while_compaction_is_in_progress() {
    let mut app = app_ready_to_send();
    app.compact_in_progress = true;

    type_and_submit(&mut app, "queued during compaction").await;

    assert_eq!(app.input_queue_len(), 1);
    assert!(app.messages.is_empty(), "no dispatch while compacting");
}

#[tokio::test]
async fn test_enqueue_adds_to_history_at_submission_time() {
    let mut app = app_ready_to_send();
    app.is_processing = true;

    type_and_submit(&mut app, "HISTORY-PROBE").await;

    // FR-003: the entry is in history before it has been executed.
    assert_eq!(
        app.input_history.last().map(String::as_str),
        Some("HISTORY-PROBE")
    );
    assert!(app.history_index.is_none(), "history browsing resets");

    type_and_submit(&mut app, "HISTORY-PROBE-2").await;
    assert_eq!(
        app.input_history.last().map(String::as_str),
        Some("HISTORY-PROBE-2")
    );
}

#[tokio::test]
async fn test_enqueue_preserves_fifo_order_across_submissions() {
    let mut app = app_ready_to_send();
    app.is_processing = true;

    type_and_submit(&mut app, "first").await;
    type_and_submit(&mut app, "second").await;
    type_and_submit(&mut app, "third").await;

    let queued: Vec<&str> = app
        .input_queue
        .iter()
        .map(|entry| entry.text.as_str())
        .collect();
    assert_eq!(
        queued,
        vec!["first", "second", "third"],
        "FR-019: FIFO order"
    );
}

#[tokio::test]
async fn test_enqueue_keeps_staged_image_attachments() {
    let mut app = app_ready_to_send();
    app.is_processing = true;
    app.pending_attachments = vec![std::path::PathBuf::from("/tmp/shot.png")];

    type_and_submit(&mut app, "see attached").await;

    assert_eq!(app.input_queue_len(), 1);
    assert_eq!(
        app.input_queue[0].image_paths,
        vec![std::path::PathBuf::from("/tmp/shot.png")],
        "FR-005: staged attachments travel with the queued entry"
    );
    assert!(
        app.pending_attachments.is_empty(),
        "attachments are taken from the staging area when queued"
    );
}

#[tokio::test]
async fn test_enqueue_rejects_overflow_and_keeps_typed_text() {
    let mut app = app_ready_to_send();
    app.is_processing = true;
    app.input_queue_capacity = 1;

    type_and_submit(&mut app, "accepted").await;
    assert_eq!(app.input_queue_len(), 1);

    type_and_submit(&mut app, "rejected").await;

    assert_eq!(app.input_queue_len(), 1, "FR-004: the cap is enforced");
    assert_eq!(app.input_queue[0].text, "accepted", "FR-019: no reordering");
    assert_eq!(
        app.input, "rejected",
        "FR-004/FR-018: the rejected text is restored, not lost"
    );
    assert_eq!(app.input_cursor, app.input_len_chars());
    assert!(
        app.status.contains("queue full"),
        "a status message explains the rejection, got: {}",
        app.status
    );
}

#[tokio::test]
async fn test_enqueue_rejection_restores_staged_attachments() {
    let mut app = app_ready_to_send();
    app.is_processing = true;
    app.input_queue_capacity = 1;
    type_and_submit(&mut app, "accepted").await;

    app.pending_attachments = vec![std::path::PathBuf::from("/tmp/shot.png")];
    type_and_submit(&mut app, "rejected").await;

    assert_eq!(
        app.pending_attachments,
        vec![std::path::PathBuf::from("/tmp/shot.png")],
        "a rejected submission keeps its staged attachments"
    );
}

#[tokio::test]
async fn test_enqueue_respects_configured_capacity() {
    let mut app = app_ready_to_send();
    app.is_processing = true;
    app.input_queue_capacity = 2;

    type_and_submit(&mut app, "one").await;
    type_and_submit(&mut app, "two").await;
    type_and_submit(&mut app, "three").await;

    assert_eq!(
        app.input_queue_len(),
        2,
        "FR-015: enforcement uses the configured capacity"
    );
    assert_eq!(
        app.input, "three",
        "the overflow entry is kept in the field"
    );
}

#[tokio::test]
async fn test_enqueue_requests_redraw_for_the_counter() {
    let mut app = app_ready_to_send();
    app.is_processing = true;
    app.needs_redraw = false;

    type_and_submit(&mut app, "queue me").await;

    assert!(
        app.needs_redraw,
        "NFR-003: the new counter must be painted on the next frame"
    );
}

#[tokio::test]
async fn test_enqueue_echoes_queued_notice_with_post_append_depth() {
    let mut app = app_ready_to_send();
    app.is_processing = true;
    app.log_entries.clear();

    type_and_submit(&mut app, "first").await;

    assert_eq!(
        app.log_entries.len(),
        1,
        "FR-014: a successful enqueue echoes one notice"
    );
    assert_eq!(app.log_entries[0].message, "queued (1 in queue)");

    type_and_submit(&mut app, "second").await;

    assert_eq!(app.log_entries.len(), 2, "each enqueue echoes one notice");
    assert_eq!(
        app.log_entries[1].message, "queued (2 in queue)",
        "FR-014: N is the depth after the append"
    );
}

#[tokio::test]
async fn test_enqueue_overflow_does_not_echo_a_success_notice() {
    let mut app = app_ready_to_send();
    app.is_processing = true;
    app.input_queue_capacity = 1;
    app.log_entries.clear();

    type_and_submit(&mut app, "accepted").await;
    assert_eq!(app.log_entries.len(), 1);

    // FR-004: the second submission is rejected, so no queued notice is echoed.
    type_and_submit(&mut app, "rejected").await;

    assert_eq!(
        app.log_entries.len(),
        1,
        "FR-014: only successful enqueues are echoed"
    );
}
