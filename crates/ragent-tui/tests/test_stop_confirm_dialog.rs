//! Tests for the Alt+X stop-agent confirmation dialog: the dialog only opens
//! while a turn is running, Enter confirms the halt (turn cancel flag set,
//! loop interrupt raised) and Esc keeps the agent running.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{Terminal, backend::TestBackend};

#[path = "support/mod.rs"]
mod support;

fn key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

/// Render the app into a `TestBackend` and return the buffer content of the
/// centred 60x12 region where the stop dialog appears.
fn render_dialog_area(app: &mut ragent_tui::App) -> String {
    let backend = TestBackend::new(120, 40);
    let mut terminal = Terminal::new(backend).expect("create test terminal");
    terminal
        .draw(|frame| ragent_tui::layout::render(frame, app))
        .expect("render frame");

    let buffer = terminal.backend().buffer().clone();
    let mut text = String::new();
    for y in 10..22u16 {
        for x in 20..100u16 {
            if let Some(cell) = buffer.cell((x, y)) {
                text.push_str(cell.symbol());
            }
        }
        text.push('\n');
    }
    text
}

#[test]
fn test_alt_x_opens_stop_confirm_dialog_while_processing() {
    let mut app = support::make_app();
    app.is_processing = true;

    app.handle_key_event(key(KeyCode::Char('x'), KeyModifiers::ALT));

    assert!(
        app.pending_stop_confirm,
        "Alt+X while processing should open the stop confirmation dialog"
    );

    let rendered = render_dialog_area(&mut app);
    assert!(
        rendered.contains("Are you sure?"),
        "the dialog should ask Are you sure?: {rendered}"
    );
    assert!(
        rendered.contains("Stop Agent"),
        "the dialog should be titled Stop Agent: {rendered}"
    );
}

#[test]
fn test_alt_x_ignored_when_not_processing() {
    let mut app = support::make_app();
    app.is_processing = false;

    app.handle_key_event(key(KeyCode::Char('x'), KeyModifiers::ALT));

    assert!(
        !app.pending_stop_confirm,
        "Alt+X with no running turn should not open the dialog"
    );
    assert!(
        !app.input.contains('x'),
        "Alt+X should never type an x into the input buffer"
    );
}

#[test]
fn test_stop_confirm_yes_halts_agent() {
    let mut app = support::make_app();
    app.is_processing = true;
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    app.cancel_flag = Some(std::sync::Arc::clone(&cancel_flag));

    app.handle_key_event(key(KeyCode::Char('x'), KeyModifiers::ALT));
    assert!(app.pending_stop_confirm, "dialog open before Yes");

    app.handle_key_event(key(KeyCode::Enter, KeyModifiers::NONE));

    assert!(
        !app.pending_stop_confirm,
        "the dialog should close after Yes"
    );
    assert!(
        cancel_flag.load(std::sync::atomic::Ordering::Relaxed),
        "Yes should set the turn cancel flag"
    );
    assert!(
        app.status.contains("halting agent"),
        "Yes should report the halt: {}",
        app.status
    );
}

#[test]
fn test_stop_confirm_cancel_keeps_agent_running() {
    let mut app = support::make_app();
    app.is_processing = true;
    let cancel_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    app.cancel_flag = Some(std::sync::Arc::clone(&cancel_flag));

    app.handle_key_event(key(KeyCode::Char('x'), KeyModifiers::ALT));
    app.handle_key_event(key(KeyCode::Esc, KeyModifiers::NONE));

    assert!(
        !app.pending_stop_confirm,
        "the dialog should close after Cancel"
    );
    assert!(
        !cancel_flag.load(std::sync::atomic::Ordering::Relaxed),
        "Cancel must not set the turn cancel flag"
    );
    assert!(
        app.status.contains("still running"),
        "Cancel should report the agent keeps running: {}",
        app.status
    );
}

#[test]
fn test_stop_confirm_swallows_other_keys_until_resolved() {
    let mut app = support::make_app();
    app.is_processing = true;

    app.handle_key_event(key(KeyCode::Char('x'), KeyModifiers::ALT));
    // Any other key is swallowed by the modal.
    app.handle_key_event(key(KeyCode::Char('q'), KeyModifiers::NONE));
    assert!(
        app.pending_stop_confirm,
        "an unrelated key must not dismiss or confirm the dialog"
    );
    assert!(
        !app.input.contains('q'),
        "modal keys must not leak into the input buffer"
    );

    // Cancel closes it again.
    app.handle_key_event(key(KeyCode::Esc, KeyModifiers::NONE));
    assert!(!app.pending_stop_confirm, "Esc dismisses the dialog");
}

#[test]
fn test_stop_confirm_not_opened_while_another_modal_is_up() {
    let mut app = support::make_app();
    app.is_processing = true;
    // The memory-delete modal outranks the stop dialog inside `handle_key`:
    // with the memory modal open, Alt+X must not clobber it.
    app.pending_memory_delete = Some(ragent_tui::app::PendingMemoryDelete {
        id: 1,
        preview: "preview".to_string(),
    });

    app.handle_key_event(key(KeyCode::Char('x'), KeyModifiers::ALT));

    assert!(
        !app.pending_stop_confirm,
        "the stop dialog must not open on top of the memory delete modal"
    );
    assert!(
        app.pending_memory_delete.is_some(),
        "the memory delete modal should remain untouched"
    );
}
