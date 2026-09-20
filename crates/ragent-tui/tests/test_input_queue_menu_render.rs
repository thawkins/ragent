//! Render tests for the ALT-Q queue-control menu overlay (spec `inputqueue`
//! T-014).
//!
//! Covers FR-021 (the menu presents exactly the four options `Next`, `Stop`,
//! `Clear`, and `Show`), FR-023 (the `Next` option is visible/selectable only
//! while the input queue is non-empty, otherwise hidden/non-selectable), and
//! NFR-007 (the menu reuses the shared overlay/modal rendering machinery rather
//! than a parallel path). The action semantics for the rows are covered by the
//! T-015..T-018 tests and the `Show`-panel suites.

#[path = "support/mod.rs"]
mod support;

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;

use ragent_tui::App;
use ragent_tui::app::QueuedInput;
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

/// Read one painted row between `x0` (inclusive) and `x1` (exclusive).
fn row_text(terminal: &Terminal<TestBackend>, y: u16, x0: u16, x1: u16) -> String {
    let buffer = terminal.backend().buffer();
    (x0..x1)
        .map(|x| buffer[(x, y)].symbol().to_string())
        .collect()
}

/// Every painted row of the queue menu's inner area, with the leading/trailing
/// border columns stripped.
fn menu_rows(terminal: &Terminal<TestBackend>, app: &App) -> Vec<String> {
    let area = app.queue_menu_area;
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (area.y + 1..area.y + area.height.saturating_sub(1))
        .map(|y| row_text(terminal, y, x0, x1))
        .collect()
}

/// Find the painted menu row containing `needle`, returning its row text.
fn find_row<'a>(rows: &'a [String], needle: &str) -> Option<&'a String> {
    rows.iter().find(|r| r.contains(needle))
}

/// Foreground colour of the first non-space glyph on a row (the selection
/// marker column, or the first label character when unselected).
fn first_glyph_fg(terminal: &Terminal<TestBackend>, app: &App, y: u16) -> Color {
    let area = app.queue_menu_area;
    let buffer = terminal.backend().buffer();
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (x0..x1)
        .find(|&x| buffer[(x, y)].symbol() != " ")
        .map(|x| buffer[(x, y)].fg)
        .unwrap_or(Color::Reset)
}

// ---------------------------------------------------------------------------
// FR-021 — exactly four options, in fixed order
// ---------------------------------------------------------------------------

#[test]
fn test_menu_labels_are_next_halt_clear_and_show_in_order() {
    let app = support::make_app();

    let labels = app.queue_menu_labels();

    assert_eq!(labels.len(), 4, "FR-021: the menu has exactly four options");
    assert_eq!(labels[0], "Next", "FR-021: the first option is `Next`");
    assert_eq!(labels[2], "Clear", "FR-021: the third option is `Clear`");
    assert_eq!(labels[3], "Show", "FR-021: the fourth option is `Show`");
}

#[test]
fn test_open_menu_paints_all_four_option_rows() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("queued"));
    app.is_processing = true;
    app.queue_menu_open = true;

    let terminal = render(&mut app, 120, 40);
    let rows = menu_rows(&terminal, &app);

    for label in ["Next", "Stop", "Clear", "Show"] {
        assert!(
            find_row(&rows, label).is_some(),
            "FR-021: the open menu must paint the `{label}` row; painted rows: {rows:?}"
        );
    }
}

#[test]
fn test_closed_menu_is_not_painted() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("queued"));
    app.queue_menu_open = false;

    let terminal = render(&mut app, 120, 40);

    let painted = terminal
        .backend()
        .buffer()
        .content()
        .iter()
        .map(|c| c.symbol().to_string())
        .collect::<String>();
    assert!(
        !painted.contains("Queue control"),
        "FR-021: the menu must not paint while closed"
    );
}

// ---------------------------------------------------------------------------
// FR-023 — `Next` visible/selectable only with a non-empty queue
// ---------------------------------------------------------------------------

#[test]
fn test_next_row_is_selectable_when_queue_is_non_empty() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("queued"));
    app.queue_menu_open = true;
    app.queue_menu_selected = 0;

    let terminal = render(&mut app, 120, 40);
    let rows = menu_rows(&terminal, &app);

    let next_row = find_row(&rows, "Next").expect("FR-023: `Next` row must be painted");
    assert!(
        next_row.starts_with("> "),
        "FR-023: with a non-empty queue `Next` is selectable and carries the marker, got {next_row:?}"
    );
    assert!(
        !next_row.contains("Resume") && !next_row.contains("Clear"),
        "FR-021: the `Next` row holds exactly one option"
    );
}

#[test]
fn test_next_row_is_non_selectable_when_queue_is_empty() {
    let mut app = support::make_app();
    assert_eq!(app.input_queue_len(), 0, "precondition: empty queue");
    app.queue_menu_open = true;
    app.queue_menu_selected = 0;

    let terminal = render(&mut app, 120, 40);
    let rows = menu_rows(&terminal, &app);

    let next_row = find_row(&rows, "Next").expect("FR-023: `Next` row is still painted (dimmed)");
    assert!(
        !next_row.starts_with("> "),
        "FR-023: with an empty queue `Next` must not carry the selection marker, got {next_row:?}"
    );
}

#[test]
fn test_empty_queue_next_is_dimmed_while_other_rows_are_not() {
    let mut app = support::make_app();
    app.queue_menu_open = true;
    app.queue_menu_selected = 0;

    let terminal = render(&mut app, 120, 40);
    let rows = menu_rows(&terminal, &app);
    let inner_y0 = app.queue_menu_area.y + 1;

    let next_y = inner_y0
        + rows
            .iter()
            .position(|r| r.contains("Next"))
            .expect("Next row") as u16;
    let clear_y = inner_y0
        + rows
            .iter()
            .position(|r| r.contains("Clear"))
            .expect("Clear row") as u16;

    assert_eq!(
        first_glyph_fg(&terminal, &app, next_y),
        Color::DarkGray,
        "FR-023: an empty queue renders `Next` dimmed (non-selectable)"
    );
    assert_ne!(
        first_glyph_fg(&terminal, &app, clear_y),
        Color::DarkGray,
        "FR-023: `Clear` stays selectable while the queue is empty"
    );
}

#[test]
fn test_next_marker_returns_when_an_entry_is_queued_after_an_empty_render() {
    let mut app = support::make_app();
    app.queue_menu_open = true;

    let empty = render(&mut app, 120, 40);
    let empty_rows = menu_rows(&empty, &app);
    assert!(
        !find_row(&empty_rows, "Next")
            .expect("Next row")
            .starts_with("> "),
        "FR-023: empty queue hides the selection marker"
    );

    app.input_queue.push_back(entry("queued"));
    let filled = render(&mut app, 120, 40);
    let filled_rows = menu_rows(&filled, &app);
    assert!(
        find_row(&filled_rows, "Next")
            .expect("Next row")
            .starts_with("> "),
        "FR-023: enqueuing an entry makes `Next` selectable again"
    );
}

// ---------------------------------------------------------------------------
// NFR-007 — reuses the shared overlay/modal rendering path
// ---------------------------------------------------------------------------

#[test]
fn test_open_menu_records_its_overlay_area_and_clears_it_when_closed() {
    let mut app = support::make_app();
    app.queue_menu_open = true;

    let _ = render(&mut app, 120, 40);
    let open_area = app.queue_menu_area;
    assert!(
        open_area.width > 0 && open_area.height > 0,
        "NFR-007: the open menu records its overlay area for the shared modal path"
    );

    app.queue_menu_open = false;
    let _ = render(&mut app, 120, 40);
    assert_eq!(
        app.queue_menu_area,
        ratatui::layout::Rect::default(),
        "NFR-007: the overlay area is cleared once the menu closes"
    );
}

#[test]
fn test_open_menu_overlays_the_chat_beneath_it() {
    let mut app = support::make_app();
    app.queue_menu_open = true;

    let terminal = render(&mut app, 120, 40);
    let area = app.queue_menu_area;
    let buffer = terminal.backend().buffer();

    // The menu owns its whole rectangle: the shared `Clear` + border chrome
    // means the top-left corner is the border glyph, not chat content.
    let corner = buffer[(area.x, area.y)].symbol().to_string();
    assert!(
        matches!(corner.as_str(), "┌" | "┏" | "+"),
        "NFR-007: the overlay paints its own border at the recorded area, got {corner:?}"
    );
}

#[test]
fn test_menu_title_reports_the_pending_count() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("a"));
    app.input_queue.push_back(entry("b"));
    app.queue_menu_open = true;

    let terminal = render(&mut app, 120, 40);
    let area = app.queue_menu_area;
    let buffer = terminal.backend().buffer();
    let title_row: String = (area.x..area.x + area.width)
        .map(|x| buffer[(x, area.y)].symbol().to_string())
        .collect();

    assert!(
        title_row.contains("2 pending"),
        "FR-021/NFR-007: the menu title reflects the live queue length, got {title_row:?}"
    );
}

// ---------------------------------------------------------------------------
// FR-026 — halt label reflects the running-turn state
// ---------------------------------------------------------------------------

#[test]
fn test_halt_label_is_stop_while_processing_and_resume_when_idle() {
    let mut app = support::make_app();

    app.is_processing = true;
    assert_eq!(
        app.queue_menu_halt_label(),
        "Stop",
        "FR-026: while a turn executes the halt option reads `Stop`"
    );
    assert_eq!(app.queue_menu_labels()[1], "Stop");

    app.is_processing = false;
    assert_eq!(
        app.queue_menu_halt_label(),
        "Resume",
        "FR-026: once the agent has stopped the option reads `Resume`"
    );
    assert_eq!(app.queue_menu_labels()[1], "Resume");
}

#[test]
fn test_menu_renders_resume_row_when_agent_is_not_processing() {
    let mut app = support::make_app();
    app.input_queue.push_back(entry("queued"));
    app.queue_menu_open = true;
    app.is_processing = false;

    let terminal = render(&mut app, 120, 40);
    let rows = menu_rows(&terminal, &app);

    assert!(
        find_row(&rows, "Resume").is_some(),
        "FR-026: an idle agent renders the `Resume` row; painted rows: {rows:?}"
    );
    assert!(
        find_row(&rows, "Clear").is_some(),
        "FR-021: `Clear` remains rendered alongside `Resume`"
    );
}
