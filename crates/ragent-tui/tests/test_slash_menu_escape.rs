//! Regression tests for slash-menu escape handling.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_tui::input;

#[path = "support/mod.rs"]
mod support;

fn esc_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
}

#[test]
fn test_slash_menu_escape_closes_menu_and_preserves_input_cursor() {
    let mut app = support::make_app();
    app.input = "/mod".to_string();
    app.input_cursor = app.input.chars().count();
    app.update_slash_menu();

    assert!(app.slash_menu.is_some());

    let action = input::handle_key(&mut app, esc_key());

    assert!(action.is_none());
    assert!(app.slash_menu.is_none());
    assert_eq!(app.input, "/mod");
    assert_eq!(app.input_cursor, app.input_len_chars());
}

#[test]
fn test_slash_menu_escape_with_invalid_cursor_clamps_to_input_length() {
    let mut app = support::make_app();
    app.input = "/mod".to_string();
    app.input_cursor = 10;
    app.update_slash_menu();

    assert!(app.slash_menu.is_some());
    assert!(app.input_cursor > app.input_len_chars());

    let action = input::handle_key(&mut app, esc_key());

    assert!(action.is_none());
    assert!(app.slash_menu.is_none());
    assert_eq!(app.input, "/mod");
    assert_eq!(app.input_cursor, app.input_len_chars());
}
