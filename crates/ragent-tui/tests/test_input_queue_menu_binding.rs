//! Tests for the ALT-Q binding that opens the queue-control menu overlay
//! (spec `inputqueue` T-013).
//!
//! Covers FR-021 (ALT-Q opens the queue-control menu), FR-022 (opening the menu
//! leaves the input buffer, staged attachments, and running turn untouched),
//! and NFR-009 (ALT-Q does not shadow or consume any other keystroke). The
//! three-option rendering and the `Next`/`Stop`/`Clear` actions are covered by
//! the T-014..T-019 tests.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ragent_tui::input::{InputAction, handle_key};

#[path = "support/mod.rs"]
mod support;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn alt(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::ALT)
}

#[tokio::test]
async fn test_alt_q_returns_open_queue_menu() {
    let mut app = support::make_app();
    let action = handle_key(&mut app, alt(KeyCode::Char('q'))).await;
    assert!(
        matches!(action, Some(InputAction::OpenQueueMenu)),
        "FR-021: ALT-Q must produce InputAction::OpenQueueMenu, got {action:?}"
    );
}

#[tokio::test]
async fn test_alt_q_opens_menu_without_mutating_input() {
    let mut app = support::make_app();
    app.input = "MENU-OPEN-PROBE".to_string();
    app.input_cursor = app.input_len_chars();
    app.pending_attachments = vec![std::path::PathBuf::from("/tmp/probe.png")];

    app.handle_key_event(alt(KeyCode::Char('q'))).await;

    assert!(app.queue_menu_open, "FR-021: the menu must be marked open");
    assert_eq!(
        app.input, "MENU-OPEN-PROBE",
        "FR-022/FR-031: opening the menu must not touch the input buffer"
    );
    assert_eq!(
        app.input_cursor,
        app.input_len_chars(),
        "FR-022: the cursor must not move"
    );
    assert_eq!(
        app.pending_attachments,
        vec![std::path::PathBuf::from("/tmp/probe.png")],
        "FR-022: staged attachments must be preserved"
    );
    assert_eq!(
        app.input_queue_len(),
        0,
        "FR-022: opening the menu must not enqueue anything"
    );
}

#[tokio::test]
async fn test_alt_q_opens_menu_while_processing() {
    let mut app = support::make_app();
    app.is_processing = true;

    app.handle_key_event(alt(KeyCode::Char('q'))).await;

    assert!(
        app.queue_menu_open,
        "FR-021: ALT-Q opens the menu while a turn is executing"
    );
    assert!(
        !app.pending_stop_confirm,
        "FR-022: opening the menu must not arm the stop-confirmation dialog"
    );
    assert!(app.is_processing, "FR-022: the running turn is untouched");
}

#[tokio::test]
async fn test_alt_q_does_not_insert_q_into_input() {
    let mut app = support::make_app();

    app.handle_key_event(alt(KeyCode::Char('q'))).await;

    assert!(
        app.input.is_empty(),
        "FR-022: ALT-Q must not leak the 'q' into the input buffer"
    );
}

#[tokio::test]
async fn test_alt_q_resets_menu_selection_to_first_row() {
    let mut app = support::make_app();
    app.queue_menu_selected = 2;

    app.handle_key_event(alt(KeyCode::Char('q'))).await;

    assert_eq!(
        app.queue_menu_selected, 0,
        "FR-021: reopening the menu resets the selection to the first row"
    );
}

#[tokio::test]
async fn test_alt_q_sets_redraw_flag() {
    let mut app = support::make_app();
    app.needs_redraw = false;

    app.handle_key_event(alt(KeyCode::Char('q'))).await;

    assert!(
        app.needs_redraw,
        "NFR-008/NFR-009: opening the menu must request a repaint"
    );
}

#[tokio::test]
async fn test_alt_q_does_not_dispatch_or_enqueue_a_message() {
    let mut app = support::make_app();
    app.is_processing = true;
    app.input = "typed but not sent".to_string();
    app.input_cursor = app.input_len_chars();

    app.handle_key_event(alt(KeyCode::Char('q'))).await;

    assert_eq!(
        app.input_queue_len(),
        0,
        "FR-022: ALT-Q must not enqueue the in-progress message"
    );
    assert!(
        app.messages.is_empty(),
        "FR-022: ALT-Q must not dispatch a turn"
    );
}

#[tokio::test]
async fn test_plain_q_still_types_into_input() {
    let mut app = support::make_app();

    app.handle_key_event(key(KeyCode::Char('q'))).await;

    assert_eq!(
        app.input, "q",
        "NFR-009: without ALT, 'q' must still type a normal character"
    );
    assert!(
        !app.queue_menu_open,
        "NFR-009: a plain 'q' must not open the menu"
    );
}

#[tokio::test]
async fn test_other_alt_keys_are_unaffected() {
    let mut app = support::make_app();

    let action = handle_key(&mut app, alt(KeyCode::Char('l'))).await;
    assert!(
        matches!(action, Some(InputAction::ToggleLog)),
        "NFR-009: ALT-L must still toggle the log panel, got {action:?}"
    );

    let action = handle_key(&mut app, alt(KeyCode::Char('t'))).await;
    assert!(
        matches!(action, Some(InputAction::ToggleTasksPanel)),
        "NFR-009: ALT-T must still toggle the tasks panel, got {action:?}"
    );
}
