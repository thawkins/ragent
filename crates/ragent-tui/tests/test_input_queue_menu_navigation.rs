//! Keyboard-navigation tests for the ALT-Q queue-control menu (spec
//! `inputqueue` `Show` row work).
//!
//! Before this change the menu rows were reachable only through their public
//! test entry points; this suite covers the real key path: `Up`/`Down` move the
//! highlight (skipping the non-selectable empty-queue `Next` row, FR-023),
//! `Enter` activates the highlighted row, and `Esc` dismisses the menu
//! (FR-032). The row-specific effects themselves are covered by the
//! T-015..T-018 suites and the `Show`-panel suite.

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ragent_tui::App;
use ragent_tui::app::{
    QUEUE_MENU_ROW_CLEAR, QUEUE_MENU_ROW_HALT, QUEUE_MENU_ROW_NEXT, QUEUE_MENU_ROW_SHOW,
    QueuedInput,
};

#[path = "support/mod.rs"]
mod support;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn alt(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::ALT)
}

/// Seed a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

/// Open the menu the way the user would (`ALT-Q`).
async fn menu_open(queue_len: usize) -> App {
    let mut app = support::make_app();
    for i in 0..queue_len {
        app.input_queue.push_back(entry(&format!("entry-{i}")));
    }
    app.handle_key_event(alt(KeyCode::Char('q'))).await;
    assert!(app.queue_menu_open, "precondition: ALT-Q opened the menu");
    app
}

// ---------------------------------------------------------------------------
// Up/Down move the highlight
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_down_moves_the_highlight_to_the_next_row() {
    let mut app = menu_open(2).await;

    app.handle_key_event(key(KeyCode::Down)).await;

    assert_eq!(
        app.queue_menu_selected, QUEUE_MENU_ROW_HALT,
        "Down moves the highlight down one row"
    );
    assert!(app.needs_redraw, "the move must repaint on the next frame");
}

#[tokio::test]
async fn test_up_moves_the_highlight_back_to_the_first_row() {
    let mut app = menu_open(2).await;
    app.handle_key_event(key(KeyCode::Down)).await;
    app.queue_menu_selected = QUEUE_MENU_ROW_CLEAR;

    app.handle_key_event(key(KeyCode::Up)).await;

    assert_eq!(
        app.queue_menu_selected, QUEUE_MENU_ROW_HALT,
        "Up moves the highlight up one row"
    );
}

#[tokio::test]
async fn test_down_stops_at_the_last_row() {
    let mut app = menu_open(2).await;
    app.queue_menu_selected = QUEUE_MENU_ROW_SHOW;

    app.handle_key_event(key(KeyCode::Down)).await;

    assert_eq!(
        app.queue_menu_selected, QUEUE_MENU_ROW_SHOW,
        "Down is a no-op on the last row"
    );
}

#[tokio::test]
async fn test_up_stops_at_the_first_row() {
    let mut app = menu_open(2).await;
    assert_eq!(app.queue_menu_selected, QUEUE_MENU_ROW_NEXT);

    app.handle_key_event(key(KeyCode::Up)).await;

    assert_eq!(
        app.queue_menu_selected, QUEUE_MENU_ROW_NEXT,
        "Up is a no-op on the first row"
    );
}

// ---------------------------------------------------------------------------
// Navigation skips the non-selectable empty-queue `Next` row (FR-023)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_down_skips_the_non_selectable_next_row_when_the_queue_is_empty() {
    let mut app = menu_open(0).await;
    assert_eq!(app.queue_menu_selected, QUEUE_MENU_ROW_NEXT);
    assert!(
        !app.queue_menu_row_selectable(QUEUE_MENU_ROW_NEXT),
        "precondition: `Next` is non-selectable on an empty queue"
    );

    app.handle_key_event(key(KeyCode::Down)).await;

    assert_eq!(
        app.queue_menu_selected, QUEUE_MENU_ROW_HALT,
        "FR-023: Down skips the dead `Next` row"
    );
}

#[tokio::test]
async fn test_up_skips_the_non_selectable_next_row_when_the_queue_is_empty() {
    let mut app = menu_open(0).await;
    app.queue_menu_selected = QUEUE_MENU_ROW_HALT;

    app.handle_key_event(key(KeyCode::Up)).await;

    assert_eq!(
        app.queue_menu_selected, QUEUE_MENU_ROW_HALT,
        "FR-023: Up cannot land on the dead `Next` row"
    );
}

#[tokio::test]
async fn test_up_reaches_next_once_an_entry_is_queued() {
    let mut app = menu_open(1).await;
    app.queue_menu_selected = QUEUE_MENU_ROW_HALT;

    app.handle_key_event(key(KeyCode::Up)).await;

    assert_eq!(
        app.queue_menu_selected, QUEUE_MENU_ROW_NEXT,
        "FR-023: with a queued entry `Next` becomes selectable again"
    );
}

// ---------------------------------------------------------------------------
// Enter activates the highlighted row
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_enter_on_the_halt_row_halts_the_running_turn() {
    let mut app = menu_open(1).await;
    app.is_processing = true;
    let flag = Arc::new(AtomicBool::new(false));
    app.cancel_flag = Some(flag.clone());
    app.queue_menu_selected = QUEUE_MENU_ROW_HALT;

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(
        flag.load(std::sync::atomic::Ordering::Relaxed),
        "Enter on `Stop` must halt the running turn"
    );
    assert!(!app.queue_menu_open, "activating a row closes the menu");
}

#[tokio::test]
async fn test_enter_on_the_clear_row_opens_the_confirmation_dialog() {
    let mut app = menu_open(1).await;
    app.queue_menu_selected = QUEUE_MENU_ROW_CLEAR;

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(
        app.queue_clear_confirm_open,
        "Enter on `Clear` opens the confirmation dialog"
    );
    assert!(
        !app.queue_menu_open,
        "the menu closes when the dialog opens"
    );
}

#[tokio::test]
async fn test_enter_on_the_show_row_opens_the_queue_entry_panel() {
    let mut app = menu_open(2).await;
    app.queue_menu_selected = QUEUE_MENU_ROW_SHOW;

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(
        app.queue_show_open,
        "Enter on `Show` opens the queue-entry panel"
    );
    assert!(!app.queue_menu_open, "the menu closes when the panel opens");
}

#[tokio::test]
async fn test_enter_on_the_non_selectable_next_row_is_a_noop() {
    let mut app = menu_open(0).await;
    app.queue_menu_selected = QUEUE_MENU_ROW_NEXT;
    app.is_processing = true;

    app.handle_key_event(key(KeyCode::Enter)).await;

    assert!(
        app.queue_menu_open,
        "FR-023: Enter on the dead `Next` row must not act"
    );
    assert!(app.is_processing, "the running turn must be untouched");
    assert_eq!(app.input_queue_len(), 0, "no entry may be enqueued");
    assert!(
        app.messages.is_empty(),
        "no turn may be dispatched from the dead row"
    );
}

// ---------------------------------------------------------------------------
// Navigation never mutates the editable input buffer (FR-031)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_navigation_keys_do_not_mutate_the_input_buffer() {
    let mut app = menu_open(2).await;
    app.input = "KEEP".to_string();
    app.input_cursor = app.input_len_chars();
    app.pending_attachments
        .push(std::path::PathBuf::from("/tmp/keep.png"));

    app.handle_key_event(key(KeyCode::Down)).await;
    app.handle_key_event(key(KeyCode::Up)).await;
    app.handle_key_event(key(KeyCode::Down)).await;

    assert_eq!(app.input, "KEEP", "FR-031: navigation must not edit input");
    assert_eq!(app.input_cursor, app.input_len_chars());
    assert_eq!(
        app.pending_attachments,
        vec![std::path::PathBuf::from("/tmp/keep.png")],
        "FR-031: navigation must not touch staged attachments"
    );
}

#[tokio::test]
async fn test_esc_still_closes_the_menu_after_navigating() {
    let mut app = menu_open(2).await;
    app.handle_key_event(key(KeyCode::Down)).await;
    app.queue_menu_selected = QUEUE_MENU_ROW_SHOW;

    app.handle_key_event(key(KeyCode::Esc)).await;

    assert!(!app.queue_menu_open, "FR-032: Esc dismisses the menu");
    assert_eq!(
        app.queue_menu_selected, QUEUE_MENU_ROW_NEXT,
        "dismissing resets the selection for the next open"
    );
    assert_eq!(app.input_queue_len(), 2, "FR-032: the queue is unchanged");
}
