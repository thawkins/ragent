//! Tests for the queue-control menu's `Clear` action (spec `inputqueue` T-017).
//!
//! Covers FR-028 (selecting `Clear` must not empty the queue on its own),
//! FR-033 (selecting `Clear` opens the `Clear the input queue?` confirmation
//! dialog) and NFR-008 (the action paints on the next frame).
//!
//! The row is driven through the public [`App::queue_menu_select_clear`] entry
//! point, mirroring how the T-016 tests drive [`App::queue_menu_select_halt`] and
//! the T-006 drain tests drive [`App::advance_input_queue`]. The Up/Down/Enter key
//! handling that routes a keystroke to this method is covered by the menu
//! key-handling tests.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

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

/// Build an app with an active session.
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
// FR-033 — selecting Clear opens the confirmation dialog and closes the menu
// ---------------------------------------------------------------------------

#[test]
fn test_clear_row_opens_the_confirmation_dialog() {
    let mut app = app_with_session();
    app.queue_menu_open = true;

    app.queue_menu_select_clear();

    assert!(
        app.queue_clear_confirm_open,
        "FR-033: selecting Clear must open the confirmation dialog"
    );
}

#[test]
fn test_clear_row_closes_the_menu_and_resets_selection() {
    let mut app = app_with_session();
    app.queue_menu_open = true;
    app.queue_menu_selected = 2;

    app.queue_menu_select_clear();

    assert!(
        !app.queue_menu_open,
        "the queue-control menu must close when the dialog opens"
    );
    assert_eq!(
        app.queue_menu_selected, 0,
        "the selection must reset for the next open"
    );
}

#[test]
fn test_dialog_opens_even_with_an_empty_queue() {
    let mut app = app_with_session();
    app.queue_menu_open = true;

    assert_eq!(app.input_queue_len(), 0, "precondition: the queue is empty");

    app.queue_menu_select_clear();

    assert!(
        app.queue_clear_confirm_open,
        "FR-033: the Clear row is selectable regardless of queue depth, so the \
         dialog opens even for an empty queue"
    );
}

// ---------------------------------------------------------------------------
// FR-028 / FR-037 — the queue is untouched until the user confirms
// ---------------------------------------------------------------------------

#[test]
fn test_clear_row_does_not_empty_the_queue() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("first"));
    app.input_queue.push_back(entry("second"));

    app.queue_menu_select_clear();

    assert_eq!(
        app.input_queue_len(),
        2,
        "FR-028/FR-037: the Clear row must not empty the queue before confirmation"
    );
}

#[test]
fn test_clear_row_does_not_dispatch_any_entry() {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("first"));
    let before = user_message_count(&app);

    app.queue_menu_select_clear();

    assert_eq!(
        user_message_count(&app),
        before,
        "FR-028: selecting Clear must not dispatch any queued entry"
    );
}

// ---------------------------------------------------------------------------
// Non-mutation — input buffer, attachments, and the running turn are untouched
// ---------------------------------------------------------------------------

#[test]
fn test_clear_row_leaves_input_buffer_and_attachments_untouched() {
    let mut app = app_with_session();
    app.input = "DRAFT".to_string();
    app.input_cursor = app.input_len_chars();
    app.pending_attachments = vec![std::path::PathBuf::from("/tmp/a.png")];

    app.queue_menu_select_clear();

    assert_eq!(
        app.input, "DRAFT",
        "the Clear row must not mutate the input buffer"
    );
    assert_eq!(
        app.pending_attachments,
        vec![std::path::PathBuf::from("/tmp/a.png")],
        "the Clear row must not touch staged attachments"
    );
}

#[test]
fn test_clear_row_leaves_the_running_turn_untouched() {
    let mut app = app_with_session();
    app.is_processing = true;
    let flag = Arc::new(AtomicBool::new(false));
    app.cancel_flag = Some(flag.clone());
    let status_before = app.status.clone();

    app.queue_menu_select_clear();

    assert!(
        app.is_processing,
        "the currently executing turn must be unaffected by selecting Clear"
    );
    assert!(
        !flag.load(Ordering::Relaxed),
        "selecting Clear must not cancel the running turn"
    );
    assert_eq!(
        app.status, status_before,
        "selecting Clear must not overwrite the status line"
    );
}

// ---------------------------------------------------------------------------
// NFR-008 — the action repaints on the next frame
// ---------------------------------------------------------------------------

#[test]
fn test_clear_row_sets_the_redraw_flag() {
    let mut app = app_with_session();
    app.needs_redraw = false;

    app.queue_menu_select_clear();

    assert!(
        app.needs_redraw,
        "NFR-008: the Clear action must repaint on the next frame"
    );
}
