//! Regression tests for blocking new prompt submission while the app is busy.

use std::sync::Arc;

use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend, style::Color};

use ragent_agent::event::Event;
use ragent_tui::{
    App,
    input::{InputAction, handle_key},
    layout,
};

#[path = "support/mod.rs"]
mod support;

#[test]
fn test_enter_is_ignored_while_processing() {
    let mut app = support::make_app();
    app.is_processing = true;
    app.input = "hello".to_string();

    let action = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    assert!(action.is_none(), "busy app should not emit a send action");
    assert_eq!(app.input, "hello");
    assert_eq!(app.status, "busy - wait for the current turn to finish");
}

#[test]
fn test_enter_still_submits_when_idle() {
    let mut app = support::make_app();
    app.input = "hello".to_string();

    let action = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    match action {
        Some(InputAction::SendMessage(text)) => assert_eq!(text, "hello"),
        _ => panic!("expected SendMessage action"),
    }
}

#[test]
fn test_plain_char_is_ignored_while_processing() {
    let mut app = support::make_app();
    app.is_processing = true;
    app.input = "draft".to_string();

    let action = handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
    );

    assert!(
        action.is_none(),
        "busy app should not accept plain text input"
    );
    assert_eq!(app.input, "draft");
    assert_eq!(app.status, "busy - wait for the current turn to finish");
}

#[test]
fn test_key_release_events_are_ignored() {
    let mut app = support::make_app();
    app.input = "draft".to_string();

    let action = handle_key(
        &mut app,
        KeyEvent {
            code: KeyCode::Char('s'),
            modifiers: KeyModifiers::NONE,
            kind: KeyEventKind::Release,
            state: KeyEventState::NONE,
        },
    );

    assert!(action.is_none(), "release events should be ignored");
    assert_eq!(app.input, "draft");
}

#[test]
fn test_agent_error_clears_processing_so_input_unblocks() {
    let mut app = support::make_app();
    app.session_id = Some("session-1".to_string());
    app.is_processing = true;
    app.cancel_flag = Some(Arc::new(std::sync::atomic::AtomicBool::new(false)));

    app.handle_event(Event::AgentError {
        session_id: "session-1".to_string(),
        error: "simulated failure".to_string(),
    });

    assert!(
        !app.is_processing,
        "agent error should end busy input gating"
    );
    assert!(
        app.cancel_flag.is_none(),
        "cancel flag should be cleared on error"
    );

    app.input = "hello".to_string();
    let action = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
    match action {
        Some(InputAction::SendMessage(text)) => assert_eq!(text, "hello"),
        _ => panic!("expected SendMessage action after agent error"),
    }
}

fn render_and_get_input_border_color(app: &mut App) -> Color {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");

    let input_area = app.input_area;
    let buffer = terminal.backend().buffer();
    buffer[(input_area.x, input_area.y)].fg
}

#[test]
fn test_input_border_is_white_when_idle() {
    let mut app = support::make_app();

    let color = render_and_get_input_border_color(&mut app);

    assert_eq!(color, Color::White);
}

#[test]
fn test_input_border_is_red_when_busy() {
    let mut app = support::make_app();
    app.is_processing = true;

    let color = render_and_get_input_border_color(&mut app);

    assert_eq!(color, Color::Red);
}
