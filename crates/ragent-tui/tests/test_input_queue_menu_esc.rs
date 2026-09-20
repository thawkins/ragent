//! Tests for dismissing the queue-control menu with `Esc` (spec `inputqueue`
//! T-018).
//!
//! Covers FR-032 (`Esc` closes the queue-control menu taking no action) and
//! FR-031 (the menu must not insert characters into, or otherwise mutate, the
//! editable input buffer). The action rows themselves (`Next`, `Stop`/`Resume`,
//! `Clear`) are driven through their own public entry points and covered by the
//! T-015/T-016/T-017 suites; this suite exercises the key path that opens and
//! dismisses the overlay.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ragent_tui::App;
use ragent_tui::app::QueuedInput;

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

/// Open the menu the way the user would, then return the app.
fn app_with_menu_open() -> App {
    let mut app = support::make_app();
    app.handle_key_event(alt(KeyCode::Char('q')));
    assert!(app.queue_menu_open, "precondition: ALT-Q opened the menu");
    app
}

// ---------------------------------------------------------------------------
// FR-032 — Esc closes the menu taking no action
// ---------------------------------------------------------------------------

#[test]
fn test_esc_closes_the_menu() {
    let mut app = app_with_menu_open();

    app.handle_key_event(key(KeyCode::Esc));

    assert!(
        !app.queue_menu_open,
        "FR-032: Esc must dismiss the queue-control menu"
    );
}

#[test]
fn test_esc_resets_the_selection_for_the_next_open() {
    let mut app = app_with_menu_open();
    app.queue_menu_selected = 2;

    app.handle_key_event(key(KeyCode::Esc));

    assert_eq!(
        app.queue_menu_selected, 0,
        "dismissing the menu must reset the selection"
    );
}

#[test]
fn test_esc_sets_the_redraw_flag() {
    let mut app = app_with_menu_open();
    app.needs_redraw = false;

    app.handle_key_event(key(KeyCode::Esc));

    assert!(
        app.needs_redraw,
        "NFR-008: dismissing the menu must repaint on the next frame"
    );
}

// ---------------------------------------------------------------------------
// FR-031 — the menu never mutates the editable input buffer
// ---------------------------------------------------------------------------

#[test]
fn test_esc_leaves_input_buffer_and_attachments_untouched() {
    let mut app = app_with_menu_open();
    app.input = "KEEP-ME".to_string();
    app.input_cursor = app.input_len_chars();
    app.pending_attachments = vec![std::path::PathBuf::from("/tmp/keep.png")];

    app.handle_key_event(key(KeyCode::Esc));

    assert_eq!(
        app.input, "KEEP-ME",
        "FR-031: Esc must not mutate the input buffer"
    );
    assert_eq!(
        app.input_cursor,
        app.input_len_chars(),
        "FR-031: Esc must not move the cursor"
    );
    assert_eq!(
        app.pending_attachments,
        vec![std::path::PathBuf::from("/tmp/keep.png")],
        "FR-031: Esc must not touch staged attachments"
    );
}

#[test]
fn test_esc_leaves_the_queue_untouched() {
    let mut app = app_with_menu_open();
    app.input_queue.push_back(entry("first"));
    app.input_queue.push_back(entry("second"));

    app.handle_key_event(key(KeyCode::Esc));

    assert_eq!(
        app.input_queue_len(),
        2,
        "FR-032: dismissing the menu must leave the queue unchanged"
    );
}

#[test]
fn test_esc_leaves_the_running_turn_untouched() {
    let mut app = support::make_app();
    app.is_processing = true;
    let flag = Arc::new(AtomicBool::new(false));
    app.cancel_flag = Some(flag.clone());
    app.handle_key_event(alt(KeyCode::Char('q')));
    let status_before = app.status.clone();

    app.handle_key_event(key(KeyCode::Esc));

    assert!(
        app.is_processing,
        "FR-032: dismissing the menu must not stop the running turn"
    );
    assert!(
        !flag.load(Ordering::Relaxed),
        "FR-032: dismissing the menu must not cancel the running turn"
    );
    assert_eq!(
        app.status, status_before,
        "FR-032: dismissing the menu must not overwrite the status line"
    );
}

// ---------------------------------------------------------------------------
// FR-031 — every other key is swallowed while the menu is open
// ---------------------------------------------------------------------------

#[test]
fn test_plain_character_is_swallowed_while_menu_is_open() {
    let mut app = app_with_menu_open();
    app.input = "PROBE".to_string();
    app.input_cursor = app.input_len_chars();

    app.handle_key_event(key(KeyCode::Char('z')));

    assert_eq!(
        app.input, "PROBE",
        "FR-031: a printable character must not reach the input buffer while the menu is open"
    );
    assert!(
        app.queue_menu_open,
        "FR-031: a non-Esc key must not dismiss the menu"
    );
}

#[test]
fn test_backspace_is_swallowed_while_menu_is_open() {
    let mut app = app_with_menu_open();
    app.input = "PROBE".to_string();
    app.input_cursor = app.input_len_chars();

    app.handle_key_event(key(KeyCode::Backspace));

    assert_eq!(
        app.input, "PROBE",
        "FR-031: Backspace must not edit the input buffer while the menu is open"
    );
}

#[test]
fn test_enter_is_swallowed_while_menu_is_open() {
    let mut app = app_with_menu_open();
    app.input = "typed but not sent".to_string();
    app.input_cursor = app.input_len_chars();

    app.handle_key_event(key(KeyCode::Enter));

    assert!(
        app.messages.is_empty(),
        "FR-031: Enter must not dispatch a turn while the menu is open"
    );
    assert_eq!(
        app.input_queue_len(),
        0,
        "FR-031: Enter must not enqueue a message while the menu is open"
    );
    assert_eq!(
        app.input, "typed but not sent",
        "FR-031: the pending text must survive the swallowed Enter"
    );
}

// ---------------------------------------------------------------------------
// Esc keeps its normal behaviour when the menu is not open
// ---------------------------------------------------------------------------

#[test]
fn test_esc_still_cancels_a_running_turn_when_menu_is_closed() {
    let mut app = support::make_app();
    app.is_processing = true;
    let flag = Arc::new(AtomicBool::new(false));
    app.cancel_flag = Some(flag.clone());
    assert!(!app.queue_menu_open, "precondition: the menu is closed");

    app.handle_key_event(key(KeyCode::Esc));

    assert!(
        flag.load(Ordering::Relaxed),
        "NFR-009: with the menu closed, Esc must still cancel the running turn"
    );
}
