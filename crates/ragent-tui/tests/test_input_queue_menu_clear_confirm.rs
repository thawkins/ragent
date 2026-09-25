//! Tests for the `Clear the input queue?` confirmation dialog (spec
//! `inputqueue` T-021).
//!
//! Covers FR-033 (the dialog presents the message `Clear the input queue?` with
//! exactly two options, `Yes` and `No`), FR-034 (`No` is the default selection
//! so a stray `Enter` cannot empty the queue), NFR-010 (the dialog reuses the
//! shared overlay/modal rendering and key-dispatch machinery) and NFR-011
//! (presenting, selecting, or dismissing the dialog sets the redraw flag).
//!
//! The `Clear` row that opens the dialog is [`App::queue_menu_select_clear`]
//! (T-017). This suite asserts the dialog's presentation and key mapping; the
//! `Yes`-empties / `No`-leaves-unchanged gating (T-022) is covered by
//! `test_input_queue_menu_clear_gating.rs`.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_tui::App;
use ragent_tui::app::{QUEUE_CLEAR_CONFIRM_NO, QUEUE_CLEAR_CONFIRM_YES, QueuedInput};
use ragent_tui::input::{InputAction, handle_key};
use ragent_tui::layout;

#[path = "support/mod.rs"]
mod support;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Seed a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
}

/// Build an app with an active session and a running turn.
fn app_with_session() -> App {
    let mut app = support::make_app();
    app.session_id = Some("test-session".to_string());
    app
}

/// Open the dialog the way the user would: ALT-Q, select `Clear`, `Enter`.
fn app_with_dialog_open() -> App {
    let mut app = app_with_session();
    app.input_queue.push_back(entry("CONFIRM-QUEUE-1"));
    app.input_queue.push_back(entry("CONFIRM-QUEUE-2"));
    app.queue_menu_open = true;
    app.queue_menu_select_clear();
    assert!(
        app.queue_clear_confirm_open,
        "precondition: selecting Clear opened the confirmation dialog"
    );
    app
}

/// Render one frame and return the terminal so the painted buffer can be read.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");
    terminal
}

/// Read one painted row between `x0` (inclusive) and `x1` (exclusive).
fn row_text(terminal: &Terminal<TestBackend>, y: u16, x0: u16, x1: u16) -> String {
    let buffer = terminal.backend().buffer();
    (x0..x1)
        .map(|x| buffer[(x, y)].symbol().to_string())
        .collect()
}

/// Every painted row of the dialog's inner area, with the border columns
/// stripped.
fn dialog_rows(terminal: &Terminal<TestBackend>, app: &App) -> Vec<String> {
    let area = app.queue_clear_confirm_area;
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (area.y + 1..area.y + area.height.saturating_sub(1))
        .map(|y| row_text(terminal, y, x0, x1))
        .collect()
}

// ---------------------------------------------------------------------------
// FR-033 — the dialog presents the message and exactly Yes / No
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_clear_row_opens_the_dialog_with_yes_and_no() {
    let mut app = app_with_session();
    app.queue_menu_open = true;

    app.queue_menu_select_clear();

    assert!(
        app.queue_clear_confirm_open,
        "FR-033: selecting Clear presents the confirmation dialog"
    );
    assert_eq!(
        app.queue_clear_confirm_selected, QUEUE_CLEAR_CONFIRM_NO,
        "FR-034: the dialog opens with `No` selected by default"
    );
}

#[test]
fn test_dialog_renders_the_message_and_two_options() {
    let mut app = app_with_dialog_open();

    let terminal = render(&mut app, 100, 30);
    let rows = dialog_rows(&terminal, &app);
    let joined = rows.join("\n");

    // The message is the dialog's title, painted on the border row.
    let area = app.queue_clear_confirm_area;
    let title_row = row_text(&terminal, area.y, area.x, area.x + area.width);
    assert!(
        title_row.contains("Clear the input queue?"),
        "FR-033: the dialog must present the message `Clear the input queue?`, \
         painted title was {title_row:?}"
    );
    assert!(
        joined.contains("Yes"),
        "FR-033: the dialog must offer the `Yes` option, rows were {rows:?}"
    );
    assert!(
        joined.contains("No"),
        "FR-033: the dialog must offer the `No` option, rows were {rows:?}"
    );
}

#[test]
fn test_dialog_defaults_to_no_and_highlights_it() {
    let mut app = app_with_dialog_open();

    let terminal = render(&mut app, 100, 30);
    let rows = dialog_rows(&terminal, &app);

    // The `>` selection marker is the on-screen expression of the default
    // selection: `No` carries it, `Yes` does not (FR-034).
    let yes_row = rows
        .iter()
        .find(|r| r.contains("Yes"))
        .expect("Yes row painted");
    let no_row = rows
        .iter()
        .find(|r| r.contains("No"))
        .expect("No row painted");
    assert!(
        !yes_row.contains('>'),
        "FR-034: `Yes` must not be the default selection, row was {yes_row:?}"
    );
    assert!(
        no_row.contains('>'),
        "FR-034: `No` must be the default selection, row was {no_row:?}"
    );
}

#[test]
fn test_closed_dialog_is_not_painted_and_area_is_cleared() {
    let mut app = app_with_session();

    let terminal = render(&mut app, 100, 30);

    assert_eq!(
        app.queue_clear_confirm_area,
        ratatui::layout::Rect::default(),
        "a closed dialog must leave no cached overlay area"
    );
    let buffer = terminal.backend().buffer();
    let painted: String = (0..buffer.area.height)
        .flat_map(|y| (0..buffer.area.width).map(move |x| (x, y)))
        .map(|(x, y)| buffer[(x, y)].symbol().to_string())
        .collect();
    assert!(
        !painted.contains("Clear the input queue?"),
        "a closed dialog must not be painted"
    );
}

// ---------------------------------------------------------------------------
// NFR-010 — the dialog is a modal reusing the shared key dispatch
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_left_right_toggle_the_selection() {
    let mut app = app_with_dialog_open();
    assert_eq!(app.queue_clear_confirm_selected, QUEUE_CLEAR_CONFIRM_NO);

    app.handle_key_event(key(KeyCode::Left)).await;
    assert_eq!(
        app.queue_clear_confirm_selected, QUEUE_CLEAR_CONFIRM_YES,
        "Left moves the selection to the other option"
    );

    app.handle_key_event(key(KeyCode::Right)).await;
    assert_eq!(
        app.queue_clear_confirm_selected, QUEUE_CLEAR_CONFIRM_NO,
        "Right moves the selection back"
    );
}

#[tokio::test]
async fn test_selection_change_sets_the_redraw_flag() {
    let mut app = app_with_dialog_open();
    app.needs_redraw = false;

    app.handle_key_event(key(KeyCode::Right)).await;

    assert!(
        app.needs_redraw,
        "NFR-011: moving the dialog selection must repaint on the next frame"
    );
}

// ---------------------------------------------------------------------------
// FR-034 — Enter with the default selection does not confirm `Yes`
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_enter_on_default_returns_cancel_not_confirm() {
    let mut app = app_with_dialog_open();

    // The default selection is `No`, so Enter must produce the cancel action,
    // never the confirm action (FR-034).
    let action = handle_key(&mut app, key(KeyCode::Enter)).await;

    assert!(
        matches!(action, Some(InputAction::CancelQueueClear)),
        "FR-034: Enter on the default `No` must map to CancelQueueClear, got {action:?}"
    );
}

#[tokio::test]
async fn test_enter_after_selecting_yes_returns_confirm() {
    let mut app = app_with_dialog_open();
    app.queue_clear_confirm_selected = QUEUE_CLEAR_CONFIRM_YES;

    let action = handle_key(&mut app, key(KeyCode::Enter)).await;

    assert!(
        matches!(action, Some(InputAction::ConfirmQueueClear)),
        "selecting `Yes` then Enter must map to ConfirmQueueClear, got {action:?}"
    );
}

#[tokio::test]
async fn test_esc_returns_cancel() {
    let mut app = app_with_dialog_open();

    let action = handle_key(&mut app, key(KeyCode::Esc)).await;

    assert!(
        matches!(action, Some(InputAction::CancelQueueClear)),
        "FR-035: Esc must map to CancelQueueClear, got {action:?}"
    );
}

// ---------------------------------------------------------------------------
// FR-035 / FR-037 — dismissing closes the dialog and leaves the queue
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_cancel_closes_the_dialog_and_leaves_the_queue_unchanged() {
    let mut app = app_with_dialog_open();
    let before = app.input_queue_len();

    app.handle_key_event(key(KeyCode::Esc)).await;

    assert!(
        !app.queue_clear_confirm_open,
        "FR-035: Esc closes the dialog"
    );
    assert_eq!(
        app.input_queue_len(),
        before,
        "FR-035/FR-037: dismissing the dialog must not remove any entry"
    );
}

#[tokio::test]
async fn test_dismiss_sets_the_redraw_flag_and_resets_the_default() {
    let mut app = app_with_dialog_open();
    app.queue_clear_confirm_selected = QUEUE_CLEAR_CONFIRM_YES;
    app.needs_redraw = false;

    app.handle_key_event(key(KeyCode::Esc)).await;

    assert!(
        app.needs_redraw,
        "NFR-011: dismissing the dialog must repaint on the next frame"
    );
    assert_eq!(
        app.queue_clear_confirm_selected, QUEUE_CLEAR_CONFIRM_NO,
        "FR-034: the selection must reset to `No` for the next open"
    );
}

#[tokio::test]
async fn test_plain_character_is_swallowed_while_dialog_is_open() {
    let mut app = app_with_dialog_open();
    app.input = "KEEP-ME".to_string();
    app.input_cursor = app.input_len_chars();
    app.pending_attachments = vec![std::path::PathBuf::from("/tmp/keep.png")];

    app.handle_key_event(key(KeyCode::Char('z'))).await;

    assert_eq!(
        app.input, "KEEP-ME",
        "FR-035: the modal must swallow characters so the input buffer is untouched"
    );
    assert_eq!(
        app.pending_attachments,
        vec![std::path::PathBuf::from("/tmp/keep.png")],
        "FR-035: staged attachments must be preserved"
    );
    assert!(
        app.queue_clear_confirm_open,
        "a stray character must not dismiss the dialog"
    );
}

#[tokio::test]
async fn test_dismiss_leaves_the_running_turn_untouched() {
    let mut app = app_with_dialog_open();
    app.is_processing = true;
    let flag = Arc::new(AtomicBool::new(false));
    app.cancel_flag = Some(flag.clone());

    app.handle_key_event(key(KeyCode::Esc)).await;

    assert!(
        app.is_processing,
        "the confirmation dialog must not affect the running turn"
    );
    assert!(
        !flag.load(Ordering::Relaxed),
        "dismissing the dialog must not cancel the running turn"
    );
}
