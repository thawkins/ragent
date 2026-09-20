//! Geometry tests for the message input queue counter prefix
//! (spec `inputqueue` T-005, NFR-002 / FR-020).
//!
//! The two-digit queue counter widens the chat input's prompt prefix from two
//! columns (`"> "`) to four (`"NN> "`). Every geometry consumer - wrapped rows,
//! the cursor position, and selection-to-character mapping - must derive its
//! prefix width from that counter state, otherwise the cursor and the wrapped
//! rows drift away from the painted frame.
//!
//! FR-020 additionally requires the counter digits to never be selectable or
//! copyable as message content; the selection mapping clamps anything inside
//! the counter columns to the input text only.

#[path = "support/mod.rs"]
mod support;

use ratatui::Terminal;
use ratatui::backend::{Backend, TestBackend};
use ratatui::layout::Rect;

use ragent_tui::App;
use ragent_tui::app::{ContextAction, ContextMenuState, QueuedInput, SelectionPane, TextSelection};
use ragent_tui::layout;

/// Build a queued entry with no attachments.
fn entry(text: &str) -> QueuedInput {
    QueuedInput {
        text: text.to_string(),
        image_paths: Vec::new(),
    }
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

/// Collect the text painted on the input area's first content row.
fn input_row_text(terminal: &Terminal<TestBackend>, app: &App) -> String {
    let area = app.input_area;
    let buffer = terminal.backend().buffer();
    (area.x + 1..area.x + area.width.saturating_sub(1))
        .map(|x| buffer[(x, area.y + 1)].symbol().to_string())
        .collect()
}

// ---------------------------------------------------------------------------
// Cursor column follows the counter prefix (NFR-002)
// ---------------------------------------------------------------------------

#[test]
fn test_cursor_column_includes_prompt_prefix_without_counter() {
    let mut app = support::make_app();
    app.input = "hi".to_string();
    app.input_cursor = 2;

    let _ = render(&mut app, 120, 40);

    assert_eq!(
        app.input_render_cache.cursor_pos,
        (0, 4),
        "`> ` (2) + `hi` (2) puts the cursor in column 4 (NFR-002)"
    );
}

#[test]
fn test_cursor_column_shifts_when_counter_is_shown() {
    let mut app = support::make_app();
    app.input = "hi".to_string();
    app.input_cursor = 2;
    app.input_queue.push_back(entry("queued"));

    let terminal = render(&mut app, 120, 40);

    assert_eq!(
        app.input_render_cache.cursor_pos,
        (0, 6),
        "`01> ` (4) + `hi` (2) puts the cursor in column 6 (NFR-002)"
    );
    assert_eq!(
        input_row_text(&terminal, &app).trim_end(),
        "01> hi",
        "the counter renders immediately before `>` and before the typed text"
    );
}

#[test]
fn test_cursor_pos_matches_the_painted_counter_prefix() {
    let mut app = support::make_app();
    app.input = "hi".to_string();
    app.input_cursor = 2;
    app.input_queue.push_back(entry("queued"));

    let terminal = render(&mut app, 120, 40);

    // The cursor column must land immediately after the last painted text
    // cell, so the character before it is the final input character.
    let area = app.input_area;
    let (row, col) = app.input_render_cache.cursor_pos;
    assert_eq!(row, 0);
    let buffer = terminal.backend().buffer();
    let cursor_screen_col = area.x + 1 + col as u16;
    assert_eq!(
        buffer[(cursor_screen_col - 1, area.y + 1)].symbol(),
        "i",
        "the cursor sits directly after the typed text, not on a counter digit (NFR-002)"
    );
}

#[test]
fn test_empty_input_cursor_sits_after_the_counter_prefix() {
    let mut app = support::make_app();
    app.input.clear();
    app.input_cursor = 0;
    app.input_queue.push_back(entry("queued"));

    let mut terminal = render(&mut app, 120, 40);

    let area = app.input_area;
    let cursor = terminal
        .backend_mut()
        .get_cursor_position()
        .expect("cursor position");
    assert_eq!(cursor.x, area.x + 1 + 4, "cursor after `01> ` (NFR-002)");
    assert_eq!(cursor.y, area.y + 1);
}

// ---------------------------------------------------------------------------
// Wrapped-row height follows the counter prefix (NFR-002)
// ---------------------------------------------------------------------------

#[test]
fn test_wrapped_height_accounts_for_the_wider_counter_prefix() {
    let mut app = support::make_app();
    // Render once with a placeholder to learn the resolved inner width.
    app.input = "x".to_string();
    let _ = render(&mut app, 60, 40);

    let area = app.input_area;
    let inner = area.width.saturating_sub(2) as usize;
    assert!(inner >= 4, "test terminal too narrow: inner width {inner}");

    // Pick a content length that fills one row with the bare two-column
    // prefix but spills onto a second row once the four-column counter
    // prefix is present.
    let content_len = inner - 3;
    app.input = "a".repeat(content_len);
    app.input_cursor = content_len;

    let _ = render(&mut app, 60, 40);
    let height_without_counter = app.input_render_cache.height;

    app.input_queue.push_back(entry("queued"));
    let _ = render(&mut app, 60, 40);
    let height_with_counter = app.input_render_cache.height;

    assert_eq!(
        height_with_counter,
        height_without_counter + 1,
        "the wider counter prefix wraps the input onto one more row (NFR-002)"
    );
}

#[test]
fn test_render_cache_rebuilds_when_counter_appears() {
    let mut app = support::make_app();
    app.input = "hello".to_string();
    app.input_cursor = 5;

    let _ = render(&mut app, 120, 40);
    let pos_without_counter = app.input_render_cache.cursor_pos;

    app.input_queue.push_back(entry("queued"));
    let _ = render(&mut app, 120, 40);
    let pos_with_counter = app.input_render_cache.cursor_pos;

    assert_ne!(
        pos_without_counter, pos_with_counter,
        "the cache must rebuild the cursor position when the counter appears (NFR-002)"
    );
    assert_eq!(pos_with_counter.1, pos_without_counter.1 + 2);
}

// ---------------------------------------------------------------------------
// Selection mapping follows the counter prefix and excludes counter digits
// (NFR-002, FR-020)
// ---------------------------------------------------------------------------

/// Build a Cut context menu over the Input pane so `execute_context_action`
/// routes the selection through `input_selection_char_range`.
fn cut_menu() -> ContextMenuState {
    ContextMenuState {
        x: 1,
        y: 1,
        pane: SelectionPane::Input,
        selected: 0,
        items: vec![(ContextAction::Cut, true)],
    }
}

#[test]
fn test_selection_maps_through_the_counter_prefix() {
    let mut app = support::make_app();
    app.input_area = Rect::new(0, 22, 40, 3);
    app.input = "abcdef".to_string();
    app.input_cursor = 6;
    app.input_queue.push_back(entry("queued"));
    // Painted first row is `01> abcdef`, so inner columns 6..8 are `c` and `d`.
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Input,
        anchor: (1 + 6, 23),
        endpoint: (1 + 7, 23),
    });
    app.context_menu = Some(cut_menu());

    app.execute_context_action(ContextAction::Cut);

    assert_eq!(
        app.input, "abef",
        "cutting the counter-prefixed row must remove only `cd` (NFR-002)"
    );
}

#[test]
fn test_selection_over_counter_digits_is_not_message_content() {
    let mut app = support::make_app();
    app.input_area = Rect::new(0, 22, 40, 3);
    app.input = "abcdef".to_string();
    app.input_cursor = 6;
    app.input_queue.push_back(entry("queued"));
    // Select the entire `01> ` counter prefix (inner columns 0..4).
    app.text_selection = Some(TextSelection {
        pane: SelectionPane::Input,
        anchor: (1, 23),
        endpoint: (1 + 3, 23),
    });
    app.context_menu = Some(cut_menu());

    app.execute_context_action(ContextAction::Cut);

    assert_eq!(
        app.input, "abcdef",
        "the queue counter is never selectable or cut as message content (FR-020)"
    );
}
