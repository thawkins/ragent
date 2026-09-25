//! Tests for how the input field behaves while the primary agent is busy.
//!
//! Spec `inputqueue` (FR-002, FR-011, FR-012, FR-017) relaxes the old guard so a
//! plain message is still accepted while a turn is executing. FR-017 was amended
//! to also queue slash commands the same way, while bang commands and
//! teammate-targeted messages keep their busy behaviour and are never queued.

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

#[tokio::test]
async fn test_enter_submits_plain_message_while_processing() {
    let mut app = support::make_app();
    app.is_processing = true;
    app.input = "hello".to_string();

    let action = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).await;

    match action {
        Some(InputAction::SendMessage(text)) => assert_eq!(text, "hello"),
        _ => panic!("expected SendMessage action while busy"),
    }
    assert_ne!(
        app.status, "busy - wait for the current turn to finish",
        "a plain message must not be rejected while the agent executes"
    );
}

#[tokio::test]
async fn test_enter_still_submits_when_idle() {
    let mut app = support::make_app();
    app.input = "hello".to_string();

    let action = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).await;

    match action {
        Some(InputAction::SendMessage(text)) => assert_eq!(text, "hello"),
        _ => panic!("expected SendMessage action"),
    }
}

#[tokio::test]
async fn test_plain_char_is_accepted_while_processing() {
    let mut app = support::make_app();
    app.is_processing = true;
    app.input = "draft".to_string();

    let action = handle_key(
        &mut app,
        KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
    )
    .await;

    assert!(action.is_none(), "typing a character emits no action");
    assert_eq!(
        app.input, "sdraft",
        "the input field stays editable while the agent executes"
    );
    assert_ne!(
        app.status, "busy - wait for the current turn to finish",
        "typing must not be rejected while the agent executes"
    );
}

#[tokio::test]
async fn test_slash_command_is_accepted_while_processing() {
    let mut app = support::make_app();
    app.is_processing = true;
    app.input = "/status".to_string();

    let action = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).await;

    match action {
        Some(InputAction::SlashCommand(cmd)) => assert_eq!(cmd, "/status"),
        _ => panic!("FR-017 amendment: a slash command is queueable, not refused"),
    }
    assert_ne!(
        app.status, "busy - wait for the current turn to finish",
        "a slash command must not be rejected while the agent executes"
    );
}

#[tokio::test]
async fn test_bang_command_is_refused_while_processing() {
    let mut app = support::make_app();
    app.is_processing = true;
    app.input = "! ls -la".to_string();

    let action = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).await;

    assert!(action.is_none(), "bang commands keep their busy guard");
    assert_eq!(app.status, "busy - wait for the current turn to finish");
}

#[tokio::test]
async fn test_teammate_targeted_message_is_refused_while_processing() {
    let mut app = support::make_app();
    app.is_processing = true;
    app.focused_teammate = Some("teammate-1".to_string());
    app.input = "hello teammate".to_string();

    let action = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).await;

    assert!(
        action.is_none(),
        "teammate-targeted messages keep their busy guard"
    );
    assert_eq!(app.status, "busy - wait for the current turn to finish");
}

#[tokio::test]
async fn test_key_release_events_are_ignored() {
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
    )
    .await;

    assert!(action.is_none(), "release events should be ignored");
    assert_eq!(app.input, "draft");
}

#[tokio::test]
async fn test_agent_error_clears_processing_so_input_unblocks() {
    let mut app = support::make_app();
    app.session_id = Some("session-1".to_string());
    app.is_processing = true;
    app.cancel_flag = Some(Arc::new(std::sync::atomic::AtomicBool::new(false)));

    app.handle_event(Event::AgentError {
        session_id: "session-1".to_string(),
        error: "simulated failure".to_string(),
    })
    .await;

    assert!(
        !app.is_processing,
        "agent error should end busy input gating"
    );
    assert!(
        app.cancel_flag.is_none(),
        "cancel flag should be cleared on error"
    );

    app.input = "hello".to_string();
    let action = handle_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).await;
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
fn test_input_border_stays_white_while_processing() {
    let mut app = support::make_app();
    app.is_processing = true;

    let color = render_and_get_input_border_color(&mut app);

    assert_eq!(
        color,
        Color::White,
        "the input border stays unlocked while the agent executes (FR-011)"
    );
}

#[test]
fn test_input_border_is_red_when_a_modal_is_open() {
    let mut app = support::make_app();
    app.is_processing = true;
    // An overlay that swallows keystrokes genuinely locks the input field.
    app.pending_stop_confirm = true;

    let color = render_and_get_input_border_color(&mut app);

    assert_eq!(color, Color::Red);
}
