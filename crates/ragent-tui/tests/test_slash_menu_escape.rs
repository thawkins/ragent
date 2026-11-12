//! Regression tests for slash-menu escape handling.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ragent_tui::input;

#[path = "support/mod.rs"]
mod support;

const fn esc_key() -> KeyEvent {
    KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE)
}

#[tokio::test]
async fn test_slash_menu_escape_closes_menu_and_preserves_input_cursor() {
    let mut app = support::make_app();
    app.session.input = "/mod".to_string();
    app.session.input_cursor = app.session.input.chars().count();
    app.update_slash_menu();

    assert!(app.slash_menu.is_some());

    let action = input::handle_key(&mut app, esc_key()).await;

    assert!(action.is_none());
    assert!(app.slash_menu.is_none());
    assert_eq!(app.session.input, "/mod");
    assert_eq!(app.session.input_cursor, app.input_len_chars());
}

#[tokio::test]
async fn test_slash_menu_escape_with_invalid_cursor_clamps_to_input_length() {
    let mut app = support::make_app();
    app.session.input = "/mod".to_string();
    app.session.input_cursor = 10;
    app.update_slash_menu();

    assert!(app.slash_menu.is_some());
    assert!(app.session.input_cursor > app.input_len_chars());

    let action = input::handle_key(&mut app, esc_key()).await;

    assert!(action.is_none());
    assert!(app.slash_menu.is_none());
    assert_eq!(app.session.input, "/mod");
    assert_eq!(app.session.input_cursor, app.input_len_chars());
}
