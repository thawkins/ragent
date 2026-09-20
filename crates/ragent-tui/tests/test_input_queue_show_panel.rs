//! Tests for the scrollable queue-entry panel opened by the queue-control
//! menu's `Show` row (spec `inputqueue` `Show` row).
//!
//! Covers the panel listing every queued entry oldest-first, `Up`/`Down`
//! scrolling the block-cursor highlight, `Enter` moving the highlighted entry
//! one step toward the front of the queue, `Del` removing it, and `Esc` being
//! the only key that dismisses the panel. The panel must never mutate the
//! editable input buffer, the staged attachments, or the running turn.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;

use ragent_tui::App;
use ragent_tui::app::QueuedInput;
use ragent_tui::layout;

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

/// An app with the given entries queued and the `Show` panel open.
fn panel_with(entries: &[&str]) -> App {
    let mut app = support::make_app();
    for text in entries {
        app.input_queue.push_back(entry(text));
    }
    app.queue_show_open_panel();
    assert!(app.queue_show_open, "precondition: the panel opened");
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

/// Every painted row of the panel's inner area.
fn panel_rows(terminal: &Terminal<TestBackend>, app: &App) -> Vec<String> {
    let area = app.queue_show_area;
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (area.y + 1..area.y + area.height.saturating_sub(1))
        .map(|y| row_text(terminal, y, x0, x1))
        .collect()
}

/// The background colour of the first glyph on a painted row.
fn first_glyph_bg(terminal: &Terminal<TestBackend>, app: &App, y: u16) -> Color {
    let area = app.queue_show_area;
    let buffer = terminal.backend().buffer();
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (x0..x1)
        .find(|&x| buffer[(x, y)].symbol() != " ")
        .map(|x| buffer[(x, y)].bg)
        .unwrap_or(Color::Reset)
}

/// The queue texts, oldest first, as a plain vector.
fn queue_texts(app: &App) -> Vec<String> {
    app.input_queue.iter().map(|e| e.text.clone()).collect()
}

// ---------------------------------------------------------------------------
// Opening the panel
// ---------------------------------------------------------------------------

#[test]
fn test_show_row_opens_the_panel_and_closes_the_menu() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("a"));
    app.queue_menu_open = true;
    app.queue_menu_selected = ragent_tui::app::QUEUE_MENU_ROW_SHOW;

    app.queue_menu_activate_selected();

    assert!(app.queue_show_open, "the `Show` row opens the panel");
    assert!(!app.queue_menu_open, "the menu closes when the panel opens");
}

#[test]
fn test_opening_the_panel_resets_the_highlight_to_the_oldest_entry() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("a"));
    app.input_queue.push_back(entry("b"));
    app.queue_show_selected = 5;

    app.queue_show_open_panel();

    assert_eq!(
        app.queue_show_selected, 0,
        "the panel opens on the oldest entry"
    );
}

#[test]
fn test_opening_the_panel_sets_the_redraw_flag() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("a"));
    app.needs_redraw = false;

    app.queue_show_open_panel();

    assert!(app.needs_redraw, "the panel must paint on the next frame");
}

// ---------------------------------------------------------------------------
// Rendering: entries oldest-first with a block cursor on the highlight
// ---------------------------------------------------------------------------

#[test]
fn test_panel_lists_every_entry_oldest_first() {
    let mut app = panel_with(&["first", "second", "third"]);

    let terminal = render(&mut app, 120, 40);
    let rows = panel_rows(&terminal, &app);

    let first = rows.iter().position(|r| r.contains("first"));
    let second = rows.iter().position(|r| r.contains("second"));
    let third = rows.iter().position(|r| r.contains("third"));
    assert!(
        first.is_some() && second.is_some() && third.is_some(),
        "the panel must list every entry; painted rows: {rows:?}"
    );
    assert!(
        first < second && second < third,
        "the entries must be listed oldest-first; painted rows: {rows:?}"
    );
}

#[test]
fn test_panel_records_its_overlay_area_and_clears_it_when_closed() {
    let mut app = panel_with(&["a"]);

    let _ = render(&mut app, 120, 40);
    let open_area = app.queue_show_area;
    assert!(
        open_area.width > 0 && open_area.height > 0,
        "the open panel records its overlay area for the shared modal path"
    );

    app.queue_show_close();
    let _ = render(&mut app, 120, 40);
    assert_eq!(
        app.queue_show_area,
        ratatui::layout::Rect::default(),
        "the overlay area is cleared once the panel closes"
    );
}

#[test]
fn test_panel_cursor_block_marks_only_the_highlighted_row() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 1;

    let terminal = render(&mut app, 120, 40);
    let inner_y0 = app.queue_show_area.y + 1;
    let rows = panel_rows(&terminal, &app);
    let y_of = |needle: &str| {
        inner_y0
            + rows
                .iter()
                .position(|r| r.contains(needle))
                .unwrap_or_else(|| panic!("row {needle:?} must be painted")) as u16
    };

    assert_eq!(
        first_glyph_bg(&terminal, &app, y_of("b")),
        Color::Magenta,
        "the highlighted row carries the block cursor"
    );
    assert_ne!(
        first_glyph_bg(&terminal, &app, y_of("a")),
        Color::Magenta,
        "an unhighlighted row must not carry the block cursor"
    );
    assert_ne!(
        first_glyph_bg(&terminal, &app, y_of("c")),
        Color::Magenta,
        "an unhighlighted row must not carry the block cursor"
    );
}

#[test]
fn test_panel_cursor_follows_the_scroll() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 2;

    let terminal = render(&mut app, 120, 40);
    let inner_y0 = app.queue_show_area.y + 1;
    let rows = panel_rows(&terminal, &app);
    let c_y = inner_y0 + rows.iter().position(|r| r.contains('c')).expect("c row") as u16;

    assert_eq!(
        first_glyph_bg(&terminal, &app, c_y),
        Color::Magenta,
        "scrolling down moves the block cursor to the newly highlighted row"
    );
}

#[test]
fn test_panel_title_reports_the_pending_count() {
    let mut app = panel_with(&["a", "b"]);

    let terminal = render(&mut app, 120, 40);
    let area = app.queue_show_area;
    let buffer = terminal.backend().buffer();
    let title_row: String = (area.x..area.x + area.width)
        .map(|x| buffer[(x, area.y)].symbol().to_string())
        .collect();

    assert!(
        title_row.contains('2'),
        "the panel title reflects the live queue length, got {title_row:?}"
    );
}

// ---------------------------------------------------------------------------
// Up/Down scroll the highlight
// ---------------------------------------------------------------------------

#[test]
fn test_down_moves_the_highlight_to_the_next_entry() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.needs_redraw = false;

    app.handle_key_event(key(KeyCode::Down));

    assert_eq!(app.queue_show_selected, 1, "Down highlights the next entry");
    assert!(app.needs_redraw, "the move must repaint on the next frame");
}

#[test]
fn test_up_moves_the_highlight_to_the_previous_entry() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 2;

    app.handle_key_event(key(KeyCode::Up));

    assert_eq!(
        app.queue_show_selected, 1,
        "Up highlights the previous entry"
    );
}

#[test]
fn test_down_stops_at_the_last_entry() {
    let mut app = panel_with(&["a", "b"]);
    app.queue_show_selected = 1;

    app.handle_key_event(key(KeyCode::Down));

    assert_eq!(
        app.queue_show_selected, 1,
        "Down is a no-op at the last entry"
    );
}

#[test]
fn test_up_stops_at_the_first_entry() {
    let mut app = panel_with(&["a", "b"]);
    assert_eq!(app.queue_show_selected, 0);

    app.handle_key_event(key(KeyCode::Up));

    assert_eq!(
        app.queue_show_selected, 0,
        "Up is a no-op at the first entry"
    );
}

// ---------------------------------------------------------------------------
// Enter moves the highlighted entry one step toward the front
// ---------------------------------------------------------------------------

#[test]
fn test_enter_moves_the_highlighted_entry_one_step_toward_the_front() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 2;

    app.handle_key_event(key(KeyCode::Enter));

    assert_eq!(
        queue_texts(&app),
        vec!["a", "c", "b"],
        "Enter swaps the highlighted entry with the one before it"
    );
    assert_eq!(
        app.queue_show_selected, 1,
        "the highlight follows the moved entry so a second Enter advances it again"
    );
}

#[test]
fn test_enter_at_the_front_is_a_noop() {
    let mut app = panel_with(&["a", "b", "c"]);
    assert_eq!(app.queue_show_selected, 0);

    app.handle_key_event(key(KeyCode::Enter));

    assert_eq!(
        queue_texts(&app),
        vec!["a", "b", "c"],
        "the oldest entry cannot move any further forward"
    );
}

#[test]
fn test_two_enters_walk_an_entry_to_the_front() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 2;

    app.handle_key_event(key(KeyCode::Enter));
    app.handle_key_event(key(KeyCode::Enter));

    assert_eq!(
        queue_texts(&app),
        vec!["c", "a", "b"],
        "repeated Enter walks the entry to the front one step at a time"
    );
    assert_eq!(app.queue_show_selected, 0);
}

#[test]
fn test_enter_preserves_the_order_of_the_other_entries() {
    let mut app = panel_with(&["a", "b", "c", "d"]);
    app.queue_show_selected = 3;

    app.handle_key_event(key(KeyCode::Enter));

    assert_eq!(
        queue_texts(&app),
        vec!["a", "b", "d", "c"],
        "only the highlighted entry moves; the rest keep their order"
    );
}

#[test]
fn test_enter_sets_the_redraw_flag() {
    let mut app = panel_with(&["a", "b"]);
    app.queue_show_selected = 1;
    app.needs_redraw = false;

    app.handle_key_event(key(KeyCode::Enter));

    assert!(app.needs_redraw, "the reorder must repaint the panel");
    assert!(app.queue_show_open, "Enter must not dismiss the panel");
}

// ---------------------------------------------------------------------------
// Del removes the highlighted entry
// ---------------------------------------------------------------------------

#[test]
fn test_del_removes_the_highlighted_entry() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 1;

    app.handle_key_event(key(KeyCode::Delete));

    assert_eq!(
        queue_texts(&app),
        vec!["a", "c"],
        "Del removes only the highlighted entry"
    );
    assert_eq!(app.input_queue_len(), 2);
    assert!(app.needs_redraw, "the removal must repaint the panel");
    assert!(app.queue_show_open, "Del must not dismiss the panel");
}

#[test]
fn test_del_keeps_the_highlight_in_range() {
    let mut app = panel_with(&["a", "b", "c"]);
    app.queue_show_selected = 2;

    app.handle_key_event(key(KeyCode::Delete));

    assert_eq!(
        queue_texts(&app),
        vec!["a", "b"],
        "the last entry is removed"
    );
    assert_eq!(
        app.queue_show_selected, 1,
        "the highlight clamps to the new last entry"
    );
}

#[test]
fn test_del_closes_the_panel_when_the_queue_becomes_empty() {
    let mut app = panel_with(&["only"]);

    app.handle_key_event(key(KeyCode::Delete));

    assert_eq!(app.input_queue_len(), 0, "the entry is removed");
    assert!(
        !app.queue_show_open,
        "an empty queue leaves nothing to show, so the panel closes"
    );
}

#[test]
fn test_del_on_an_empty_queue_is_a_noop() {
    let mut app = panel_with(&[]);
    app.queue_show_selected = 3;

    app.handle_key_event(key(KeyCode::Delete));

    assert_eq!(app.input_queue_len(), 0, "nothing is removed");
    assert!(app.queue_show_open, "the panel stays open");
}

// ---------------------------------------------------------------------------
// Esc is the only key that dismisses the panel
// ---------------------------------------------------------------------------

#[test]
fn test_esc_dismisses_the_panel() {
    let mut app = panel_with(&["a", "b"]);

    app.handle_key_event(key(KeyCode::Esc));

    assert!(!app.queue_show_open, "Esc dismisses the panel");
    assert_eq!(
        queue_texts(&app),
        vec!["a", "b"],
        "dismissing leaves the queue unchanged"
    );
}

#[test]
fn test_esc_resets_the_highlight_for_the_next_open() {
    let mut app = panel_with(&["a", "b"]);
    app.queue_show_selected = 1;

    app.handle_key_event(key(KeyCode::Esc));

    assert_eq!(
        app.queue_show_selected, 0,
        "dismissing resets the highlight"
    );
}

#[test]
fn test_change_keys_keep_the_panel_open() {
    let mut app = panel_with(&["a", "b"]);
    app.queue_show_selected = 1;

    app.handle_key_event(key(KeyCode::Enter));
    app.handle_key_event(key(KeyCode::Down));
    app.handle_key_event(key(KeyCode::Up));

    assert!(
        app.queue_show_open,
        "only Esc may dismiss the panel; Ent, Down, and Up keep it open"
    );
}

#[test]
fn test_plain_character_is_swallowed_without_closing_the_panel() {
    let mut app = panel_with(&["a", "b"]);
    app.input = "PROBE".to_string();
    app.input_cursor = app.input_len_chars();

    app.handle_key_event(key(KeyCode::Char('z')));

    assert!(
        app.queue_show_open,
        "a printable character must not dismiss the panel"
    );
    assert_eq!(
        app.input, "PROBE",
        "the character must not reach the input buffer"
    );
}

// ---------------------------------------------------------------------------
// The panel never touches the draft, attachments, or the running turn
// ---------------------------------------------------------------------------

#[test]
fn test_panel_never_mutates_the_draft_or_attachments() {
    let mut app = panel_with(&["a", "b"]);
    app.input = "KEEP".to_string();
    app.input_cursor = app.input_len_chars();
    app.pending_attachments
        .push(std::path::PathBuf::from("/tmp/keep.png"));

    app.handle_key_event(key(KeyCode::Down));
    app.handle_key_event(key(KeyCode::Enter));
    app.handle_key_event(key(KeyCode::Delete));
    app.handle_key_event(key(KeyCode::Up));

    assert_eq!(app.input, "KEEP", "the draft must be preserved");
    assert_eq!(app.input_cursor, app.input_len_chars());
    assert_eq!(
        app.pending_attachments,
        vec![std::path::PathBuf::from("/tmp/keep.png")],
        "staged attachments must be preserved"
    );
}

#[test]
fn test_panel_leaves_the_running_turn_untouched() {
    let mut app = panel_with(&["a", "b"]);
    app.is_processing = true;
    let flag = Arc::new(AtomicBool::new(false));
    app.cancel_flag = Some(flag.clone());

    app.handle_key_event(key(KeyCode::Down));
    app.handle_key_event(key(KeyCode::Enter));

    assert!(app.is_processing, "the running turn must not stop");
    assert!(
        !flag.load(Ordering::Relaxed),
        "the panel must not cancel the running turn"
    );
}

#[test]
fn test_menu_esc_does_not_open_the_panel() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("a"));
    app.handle_key_event(alt(KeyCode::Char('q')));
    assert!(app.queue_menu_open, "precondition: the menu is open");

    app.handle_key_event(key(KeyCode::Esc));

    assert!(!app.queue_menu_open, "Esc dismisses the menu");
    assert!(
        !app.queue_show_open,
        "dismissing the menu must not open the panel"
    );
}
