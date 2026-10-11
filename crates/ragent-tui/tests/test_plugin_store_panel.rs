//! Render tests for the plugin-store browse panel (spec `pluginstores` T-006).
//!
//! Covers FR-004 (bordered, titled modal with a search field, a result list, and
//! a footer of key hints, ASCII-only), FR-005 (the highlighted row carries a
//! full-row block cursor via `List::highlight_style`, and installed rows use the
//! installed colour with an `[installed]` marker), FR-011/FR-017 (loading, error,
//! and empty bodies render an explicit state line rather than a blank list), and
//! FR-018 (a terminal resize re-derives a centred, fully visible panel).
//!
//! These drive the real renderer through a `TestBackend` and read the painted
//! buffer back, so no store endpoint is contacted.

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;

use ragent_plugins::{StoreEntry, StoreKind};
use ragent_tui::App;
use ragent_tui::layout;

#[path = "support/mod.rs"]
mod support;

fn entry(id: &str, description: &str) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: "1.0.0".to_string(),
        source: format!("https://example.org/{id}.zip"),
        description: description.to_string(),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
        marketplace: String::new(),
        category: None,
    }
}

fn sample() -> Vec<StoreEntry> {
    vec![
        entry("codex-weather", "Weather lookups"),
        entry("codex-time", "Time helpers"),
        entry("codex-shell", "Shell wrappers"),
    ]
}

/// An app with the panel open for `kind` and the given entries/installed set
/// loaded (no fetch is attempted without an async reactor).
fn open(kind: StoreKind, entries: Vec<StoreEntry>, installed: &[&str]) -> App {
    let mut app = support::make_app();
    app.open_plugin_store(kind, "", false);
    let browser = app.plugin_store.as_mut().expect("panel open");
    browser.set_installed(installed.iter().map(|s| (*s).to_string()).collect());
    browser.set_entries(entries);
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

/// Every painted row of the panel's inner area (borders stripped).
fn panel_rows(terminal: &Terminal<TestBackend>, app: &App) -> Vec<String> {
    let area = app.plugin_store_area;
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (area.y + 1..area.y + area.height.saturating_sub(1))
        .map(|y| row_text(terminal, y, x0, x1))
        .collect()
}

/// The whole painted panel row `y`, borders included.
fn full_row(terminal: &Terminal<TestBackend>, y: u16) -> String {
    let width = terminal.backend().buffer().area.width;
    row_text(terminal, y, 0, width)
}

/// Background colour of the first non-space glyph on painted panel row `y`.
fn first_glyph_bg(terminal: &Terminal<TestBackend>, app: &App, y: u16) -> Color {
    let area = app.plugin_store_area;
    let buffer = terminal.backend().buffer();
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (x0..x1)
        .find(|&x| buffer[(x, y)].symbol() != " ")
        .map(|x| buffer[(x, y)].bg)
        .unwrap_or(Color::Reset)
}

/// Foreground colour of the first non-space glyph on painted panel row `y`.
fn first_glyph_fg(terminal: &Terminal<TestBackend>, app: &App, y: u16) -> Color {
    let area = app.plugin_store_area;
    let buffer = terminal.backend().buffer();
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (x0..x1)
        .find(|&x| buffer[(x, y)].symbol() != " ")
        .map(|x| buffer[(x, y)].fg)
        .unwrap_or(Color::Reset)
}

/// Find the painted panel row containing `needle`, returning its buffer row.
fn find_row(terminal: &Terminal<TestBackend>, app: &App, needle: &str) -> Option<u16> {
    let area = app.plugin_store_area;
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (area.y + 1..area.y + area.height.saturating_sub(1))
        .find(|&y| row_text(terminal, y, x0, x1).contains(needle))
}

// ---------------------------------------------------------------------------
// FR-004 - bordered, titled modal, ASCII-only
// ---------------------------------------------------------------------------

#[test]
fn the_panel_is_a_bordered_modal_with_the_store_name_and_counts_in_the_title() {
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let terminal = render(&mut app, 100, 30);
    let area = app.plugin_store_area;

    // The title line sits on the top border row and names the store plus the
    // visible/total counts (`n of m`).
    let title = full_row(&terminal, area.y);
    assert!(title.contains("Codex"), "title names the store: {title:?}");
    assert!(
        title.contains("3 of 3"),
        "title reports the visible/total count: {title:?}"
    );
    assert!(
        title.contains("search:"),
        "title carries the search field label: {title:?}"
    );
    // A four-corner border frames the panel (ASCII `+` corners, FR-004).
    let first = full_row(&terminal, area.y);
    let last = full_row(&terminal, area.y + area.height.saturating_sub(1));
    let left = area.x as usize;
    assert_eq!(
        first.chars().nth(left),
        Some('+'),
        "top-left corner is drawn: {first:?}"
    );
    assert_eq!(
        last.chars().nth(left),
        Some('+'),
        "bottom-left corner is drawn: {last:?}"
    );
}

#[test]
fn the_footer_lists_the_available_keys() {
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app);
    let footer = rows.last().expect("footer row");
    assert!(footer.contains("Tab focus"), "footer: {footer:?}");
    assert!(
        footer.contains("Enter select/install"),
        "footer: {footer:?}"
    );
    assert!(footer.contains("c clear category"), "footer: {footer:?}");
    assert!(footer.contains("Esc close"), "footer: {footer:?}");
    assert!(footer.contains("Up/Down"), "footer: {footer:?}");
}

#[test]
fn every_painted_glyph_in_the_panel_is_ascii() {
    let mut app = open(StoreKind::Codex, sample(), &["codex-time"]);
    let terminal = render(&mut app, 100, 30);
    let area = app.plugin_store_area;
    let buffer = terminal.backend().buffer();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            let symbol = buffer[(x, y)].symbol();
            assert!(
                symbol.is_ascii(),
                "non-ASCII glyph {symbol:?} at ({x}, {y})"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// FR-005 - block cursor and installed colour
// ---------------------------------------------------------------------------

#[test]
fn the_highlighted_row_carries_a_full_row_block_cursor() {
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let terminal = render(&mut app, 100, 30);

    // The result set is sorted alphabetically (FR-050), so the cursor starts on
    // the first row, `codex-shell`.
    let target = find_row(&terminal, &app, "codex-shell").expect("first row painted");
    let area = app.plugin_store_area;
    // The result rows now sit in the right-hand column, to the right of the
    // category navigator (spec `catnav` T-004).
    let result = app.plugin_store_result_area;
    let buffer = terminal.backend().buffer();
    // The block cursor background spans the whole row, not just the text: it is
    // present at the first painted column of the result column and at the far
    // edge of the inner area.
    assert_eq!(
        buffer[(result.x, target)].bg,
        Color::Magenta,
        "cursor spans the leading edge of the row"
    );
    assert_eq!(
        buffer[(area.x + area.width - 2, target)].bg,
        Color::Magenta,
        "cursor spans the trailing edge of the row"
    );

    // A non-highlighted row does not carry the cursor background.
    let other = find_row(&terminal, &app, "codex-weather").expect("non-cursor row painted");
    assert_ne!(
        first_glyph_bg(&terminal, &app, other),
        Color::Magenta,
        "un-highlighted rows do not carry the block cursor"
    );
}

#[test]
fn the_cursor_follows_the_block_cursor_position() {
    let mut app = open(StoreKind::Codex, sample(), &[]);
    app.plugin_store.as_mut().expect("panel open").move_down();
    let terminal = render(&mut app, 100, 30);

    let second = find_row(&terminal, &app, "codex-time").expect("row painted");
    assert_eq!(
        first_glyph_bg(&terminal, &app, second),
        Color::Magenta,
        "the down-moved cursor highlights the second row"
    );
    let first = find_row(&terminal, &app, "codex-weather").expect("row painted");
    assert_ne!(
        first_glyph_bg(&terminal, &app, first),
        Color::Magenta,
        "the first row is no longer highlighted"
    );
}

// ---------------------------------------------------------------------------
// FR-009 - the focused pane's cursor is rendered distinctly
// ---------------------------------------------------------------------------

#[test]
fn the_focused_navigator_cursor_differs_from_the_unfocused_result_cursor() {
    // With the result list focused (the default), the navigator's `ALL` row cursor
    // is dimmed rather than filled cyan (FR-009).
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let terminal = render(&mut app, 100, 30);
    let nav = app.plugin_store_nav_area;
    let buffer = terminal.backend().buffer();
    let all_row = (0..nav.height)
        .find(|&dy| {
            let y = nav.y + dy;
            (nav.x..nav.x + nav.width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect::<String>()
                .contains("ALL")
        })
        .map(|dy| nav.y + dy)
        .expect("the ALL row is painted in the navigator");
    assert_ne!(
        buffer[(nav.x, all_row)].bg,
        Color::Cyan,
        "the unfocused navigator cursor is not the filled focus colour"
    );
    drop(terminal);

    // Focusing the navigator fills its cursor cyan and dims the result cursor
    // instead (FR-009).
    app.plugin_store.as_mut().expect("panel open").nav_focused = true;
    let terminal = render(&mut app, 100, 30);
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(nav.x, all_row)].bg,
        Color::Cyan,
        "the focused navigator cursor is filled cyan (FR-009)"
    );
    let result = app.plugin_store_result_area;
    let first = find_row(&terminal, &app, "codex-weather").expect("row painted");
    assert_ne!(
        buffer[(result.x, first)].bg,
        Color::Magenta,
        "the unfocused result cursor is not the filled focus colour"
    );
}

#[test]
fn an_installed_row_uses_the_installed_colour_and_marker() {
    // `codex-weather` is installed and sits away from the block cursor (which
    // rests on the alphabetically first row, `codex-shell`).
    let mut app = open(StoreKind::Codex, sample(), &["codex-weather"]);
    let terminal = render(&mut app, 100, 30);

    let installed = find_row(&terminal, &app, "codex-weather").expect("installed row painted");
    let area = app.plugin_store_area;
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    let text = row_text(&terminal, installed, x0, x1);
    assert!(
        text.contains("[installed]"),
        "installed row carries the marker: {text:?}"
    );
    assert_eq!(
        first_glyph_fg(&terminal, &app, installed),
        Color::Green,
        "installed row is painted in the installed colour"
    );

    // A not-installed row away from the block cursor is painted in the normal
    // colour and has no marker.
    let plain = find_row(&terminal, &app, "codex-time").expect("plain row painted");
    assert_eq!(first_glyph_fg(&terminal, &app, plain), Color::White);
    assert!(
        !row_text(&terminal, plain, x0, x1).contains("[installed]"),
        "a not-installed row omits the marker"
    );
}

// ---------------------------------------------------------------------------
// FR-048 - the row names the marketplace the plugin came from
// ---------------------------------------------------------------------------

#[test]
fn each_row_shows_the_marketplace_it_came_from() {
    // A row carries the display name of the marketplace it was fetched from, so a
    // merged multi-marketplace panel names each source at a glance (FR-048).
    let mut entries = sample();
    entries[0].marketplace = "codex".to_string();
    entries[1].marketplace = "community".to_string();
    let mut app = open(StoreKind::Codex, entries, &[]);
    let terminal = render(&mut app, 160, 30);

    let first = find_row(&terminal, &app, "codex-weather").expect("first row painted");
    let area = app.plugin_store_area;
    let text = row_text(&terminal, first, area.x + 1, area.x + area.width - 1);
    assert!(
        text.contains("codex"),
        "the row names its marketplace: {text:?}"
    );
    let second = find_row(&terminal, &app, "codex-time").expect("second row painted");
    let text = row_text(&terminal, second, area.x + 1, area.x + area.width - 1);
    assert!(
        text.contains("community"),
        "a different marketplace is named on its own row: {text:?}"
    );
}

// ---------------------------------------------------------------------------
// FR-004 - the pinned column-heading row
// ---------------------------------------------------------------------------

#[test]
fn the_column_headings_render_above_the_rows_and_do_not_scroll() {
    // The headings sit one row above the first result row, aligned with their
    // columns, and carry a dark-on-light style so they read as pinned labels
    // rather than a result row.
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let terminal = render(&mut app, 140, 30);

    // The alphabetically first row (`codex-shell`) sits directly below the heading.
    let first_row = find_row(&terminal, &app, "codex-shell").expect("result row painted");
    let area = app.plugin_store_area;
    let heading_y = first_row - 1;
    let heading = row_text(&terminal, heading_y, area.x + 1, area.x + area.width - 1);
    assert!(
        heading.contains("Id")
            && heading.contains("Marketplace")
            && heading.contains("Description"),
        "the heading row names every column: {heading:?}"
    );

    // Each heading label starts exactly where its column's values start.
    let row = row_text(&terminal, first_row, area.x + 1, area.x + area.width - 1);
    let heading_id = heading.find("Id").expect("Id label painted");
    let row_id = row.find("codex-shell").expect("id value painted");
    assert_eq!(
        heading_id, row_id,
        "the Id label aligns with its column: {heading:?} vs {row:?}"
    );
}

#[test]
fn the_column_headings_are_dark_on_light_and_not_a_result_row() {
    // The heading cells are dark text on a light background so the pinned row is
    // visibly distinct from the normal and installed row colours.
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let terminal = render(&mut app, 140, 30);

    let area = app.plugin_store_area;
    let result_area = app.plugin_store_result_area;
    assert_eq!(
        result_area.y,
        area.y + 2,
        "the row list starts one row below the heading, inside the border"
    );

    let buffer = terminal.backend().buffer();
    let id_x = (result_area.x..result_area.x + result_area.width)
        .find(|&x| buffer[(x, result_area.y - 1)].symbol() == "I")
        .expect("Id heading painted");
    let heading_cell = &buffer[(id_x, result_area.y - 1)];
    assert_eq!(
        heading_cell.fg,
        Color::Black,
        "the heading text is dark (fg)"
    );
    assert_eq!(
        heading_cell.bg,
        Color::Gray,
        "the heading background is light (bg)"
    );

    // The first data row (the alphabetically first id, `codex-shell`) is one row
    // below the heading, so finding a row by id can never land on the heading itself.
    let first = find_row(&terminal, &app, "codex-shell").expect("result row painted");
    assert_eq!(first, result_area.y);
}

#[test]
fn the_version_column_shows_the_entry_version() {
    // The version column renders the entry's own version (FR-004); the sample
    // entries declare `1.0.0`.
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let terminal = render(&mut app, 120, 30);

    let row = find_row(&terminal, &app, "codex-weather").expect("row painted");
    let area = app.plugin_store_area;
    let text = row_text(&terminal, row, area.x + 1, area.x + area.width - 1);
    assert!(
        text.contains("1.0.0"),
        "the row carries the plugin version: {text:?}"
    );
}

#[test]
fn a_blank_version_renders_an_explicit_unknown_marker() {
    // A vendor index that omits the version leaves the field blank; the panel shows
    // a `-` so the unknown state is visible rather than an empty cell (FR-004).
    let mut entries = sample();
    entries[0].version = String::new();
    let mut app = open(StoreKind::Codex, entries, &[]);
    let terminal = render(&mut app, 120, 30);

    let row = find_row(&terminal, &app, "codex-weather").expect("row painted");
    let area = app.plugin_store_area;
    let text = row_text(&terminal, row, area.x + 1, area.x + area.width - 1);
    // The first non-space token after the id is the version column, which reads `-`.
    let after = text.split("codex-weather").nth(1).unwrap_or_default();
    assert_eq!(
        after.trim_start().chars().next(),
        Some('-'),
        "a blank version reads as an unknown marker: {text:?}"
    );
}

#[test]
fn the_title_hints_while_manifest_enrichment_is_running() {
    // While the off-loop manifest enrichment is still running, the title carries a
    // `loading details...` hint so a still-blank description reads as pending
    // (FR-048).
    let mut app = open(StoreKind::Codex, sample(), &[]);
    app.plugin_store.as_mut().expect("panel open").enriching = true;
    let terminal = render(&mut app, 120, 30);
    let area = app.plugin_store_area;
    let title = full_row(&terminal, area.y);
    assert!(
        title.contains("loading details..."),
        "the title hints while enrichment runs: {title:?}"
    );
}

// ---------------------------------------------------------------------------
// FR-011 / FR-013 / FR-016 / FR-017 - loading, error, empty states
// ---------------------------------------------------------------------------

#[test]
fn the_loading_state_renders_an_explicit_line() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    // No `set_entries`, so the browser stays in `Loading`.
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app);
    assert!(
        rows.iter().any(|r| r.contains("loading")),
        "loading row present: {rows:?}"
    );
}

#[test]
fn a_failed_fetch_renders_the_cause_inline() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_failed("connection refused".to_string());
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app);
    assert!(
        rows.iter()
            .any(|r| r.contains("failed") && r.contains("connection refused")),
        "error row names the cause: {rows:?}"
    );
}

#[test]
fn an_empty_index_renders_an_explicit_empty_line() {
    let mut app = open(StoreKind::Codex, Vec::new(), &[]);
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app);
    assert!(
        rows.iter().any(|r| r.contains("no plugins")),
        "empty-index row present: {rows:?}"
    );
}

#[test]
fn a_query_that_matches_nothing_renders_the_no_match_line() {
    let mut app = open(StoreKind::Codex, sample(), &[]);
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_query("zzzzzz".to_string());
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app);
    assert!(
        rows.iter().any(|r| r.contains("no matching")),
        "no-match row present: {rows:?}"
    );
    // The title still reports the visible/total counts (0 of 3).
    let title = full_row(&terminal, app.plugin_store_area.y);
    assert!(title.contains("0 of 3"), "title: {title:?}");
}

// ---------------------------------------------------------------------------
// FR-018 - resize re-derives a centred, fully visible panel
// ---------------------------------------------------------------------------

#[test]
fn the_panel_re_derives_a_centred_area_on_resize() {
    let mut app = open(StoreKind::Codex, sample(), &[]);

    let wide = render(&mut app, 120, 40);
    let wide_area = app.plugin_store_area;
    assert!(wide_area.width <= 120 && wide_area.height <= 40);
    assert!(
        wide_area.x + wide_area.width <= 120 && wide_area.y + wide_area.height <= 40,
        "panel fits within the screen"
    );
    drop(wide);

    let narrow = render(&mut app, 60, 20);
    let narrow_area = app.plugin_store_area;
    assert!(
        narrow_area.width <= 60 && narrow_area.height <= 20,
        "panel fits within the smaller screen: {narrow_area:?}"
    );
    assert_ne!(
        wide_area, narrow_area,
        "the panel area re-derives on a terminal resize"
    );

    // Still centred after the resize.
    let expected_x = (60 - narrow_area.width) / 2;
    let expected_y = (20 - narrow_area.height) / 2;
    assert_eq!(narrow_area.x, expected_x);
    assert_eq!(narrow_area.y, expected_y);
    drop(narrow);
}

#[test]
fn the_panel_area_is_cleared_when_the_panel_closes() {
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let _ = render(&mut app, 100, 30);
    assert!(app.plugin_store_area.width > 0, "area recorded while open");

    app.close_plugin_store();
    let _ = render(&mut app, 100, 30);
    assert_eq!(
        app.plugin_store_area,
        ratatui::layout::Rect::default(),
        "area cleared when the panel is closed"
    );
}
