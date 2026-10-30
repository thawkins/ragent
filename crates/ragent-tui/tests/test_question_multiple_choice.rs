//! Tests for multiple-choice question dialog support.

#[path = "support/mod.rs"]
mod support;

use ragent_agent::event::Event;
use ragent_tui::{App, layout};
use ratatui::{Terminal, backend::TestBackend};

/// A test [`App`] built on the shared fixture, with the log panel open.
fn make_app() -> App {
    let mut app = support::make_app();
    app.show_log = true;
    app
}

fn render_app_to_string(app: &mut App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("render dialog");

    let backend = terminal.backend();
    let buffer = backend.buffer();
    let mut text = String::new();
    let area = buffer.area();
    for y in 0..area.height {
        for x in 0..area.width {
            text.push_str(buffer[(x, y)].symbol());
        }
        text.push('\n');
    }
    text
}

#[tokio::test]
async fn test_multiple_choice_question_renders_options() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::QuestionRequested {
        session_id: "s1".to_string(),
        request_id: "r1".to_string(),
        question: "Which provider?".to_string(),
        options: vec![
            "anthropic".to_string(),
            "openai".to_string(),
            "ollama".to_string(),
        ],
        default_index: Some(0),
        secret: false,
    })
    .await;

    let text = render_app_to_string(&mut app);

    // The question text should be visible.
    assert!(text.contains("Which provider?"));

    // All three options should be rendered.
    assert!(text.contains("anthropic"));
    assert!(text.contains("openai"));
    assert!(text.contains("ollama"));

    // Navigation hint should be visible.
    assert!(text.contains("to navigate"));
}

#[tokio::test]
async fn test_multiple_choice_question_shows_selected_option() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::QuestionRequested {
        session_id: "s1".to_string(),
        request_id: "r1".to_string(),
        question: "Pick one".to_string(),
        options: vec!["A".to_string(), "B".to_string()],
        default_index: Some(0),
        secret: false,
    })
    .await;

    let text = render_app_to_string(&mut app);

    // First option should have the selection indicator (>).
    assert!(text.contains("> A"));
}

#[tokio::test]
async fn test_multiple_choice_selection_navigates_down() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::QuestionRequested {
        session_id: "s1".to_string(),
        request_id: "r1".to_string(),
        question: "Pick".to_string(),
        options: vec!["first".to_string(), "second".to_string()],
        default_index: Some(0),
        secret: false,
    })
    .await;

    // Simulate pressing Down (Char('j')).
    let _ = ragent_tui::input::handle_key(
        &mut app,
        crossterm::event::KeyEvent::from(crossterm::event::KeyCode::Char('j')),
    )
    .await;

    let text = render_app_to_string(&mut app);

    // Second option should now be selected.
    assert!(text.contains("> second"));
}

#[tokio::test]
async fn test_free_text_question_does_not_show_options() {
    let mut app = make_app();
    app.session_id = Some("s1".to_string());

    app.handle_event(Event::QuestionRequested {
        session_id: "s1".to_string(),
        request_id: "r1".to_string(),
        question: "Type your answer".to_string(),
        options: vec![],
        default_index: None,
        secret: false,
    })
    .await;

    let text = render_app_to_string(&mut app);

    // Should show the free-text input hint.
    assert!(text.contains("Enter to submit"));
    assert!(!text.contains("↑/↓"));
}
