//! Render tests for the plugin-store browse panel (spec `pluginstores` T-006).
//!
//! Covers FR-004 (bordered, titled modal with a search field, a result list, and
//! a footer of key hints, framed with Unicode box-drawing borders), FR-005 (the
//! highlighted row carries a
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

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};

use ragent_plugins::{StoreEntry, StoreKind};
use ragent_tui::App;
use ragent_tui::app::StorePane;
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

/// Background colour of the first non-space glyph on result row `y`.
///
/// The scan is scoped to the result column (`plugin_store_result_area`), not the
/// whole panel inner row: the left navigator column starts every row with a
/// category label, so a panel-wide scan would report that label instead of the
/// result row the caller is asking about.
fn first_glyph_bg(terminal: &Terminal<TestBackend>, app: &App, y: u16) -> Color {
    let area = app.plugin_store_result_area;
    let buffer = terminal.backend().buffer();
    (area.x..area.x + area.width)
        .find(|&x| buffer[(x, y)].symbol() != " ")
        .map(|x| buffer[(x, y)].bg)
        .unwrap_or(Color::Reset)
}

/// Foreground colour of the first non-space glyph on result row `y`, scoped to the
/// result column for the same reason as [`first_glyph_bg`].
fn first_glyph_fg(terminal: &Terminal<TestBackend>, app: &App, y: u16) -> Color {
    let area = app.plugin_store_result_area;
    let buffer = terminal.backend().buffer();
    (area.x..area.x + area.width)
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
// FR-004 - bordered, titled modal, Unicode box-drawing borders
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
    // A four-corner border frames the panel with the default ratatui
    // box-drawing glyphs (FR-004), like every other panel.
    let first = full_row(&terminal, area.y);
    let last = full_row(&terminal, area.y + area.height.saturating_sub(1));
    let left = area.x as usize;
    assert_eq!(
        first.chars().nth(left),
        Some('\u{250c}'),
        "top-left corner is drawn: {first:?}"
    );
    assert_eq!(
        last.chars().nth(left),
        Some('\u{2514}'),
        "bottom-left corner is drawn: {last:?}"
    );
}

#[test]
fn the_footer_lists_the_available_keys() {
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app);
    // The bottom inner row is now the search box, so the footer key hints sit one
    // row above it.
    let footer = rows
        .iter()
        .find(|r| r.contains("Tab focus"))
        .expect("footer row painted");
    assert!(footer.contains("Tab focus"), "footer: {footer:?}");
    assert!(
        footer.contains("Enter select/install"),
        "footer: {footer:?}"
    );
    assert!(footer.contains("c clear filter"), "footer: {footer:?}");
    assert!(footer.contains("Esc close"), "footer: {footer:?}");
    assert!(footer.contains("Up/Down"), "footer: {footer:?}");
}

#[test]
fn the_panel_is_framed_with_unicode_box_drawing_borders() {
    let mut app = open(StoreKind::Codex, sample(), &["codex-time"]);
    let terminal = render(&mut app, 100, 30);
    let area = app.plugin_store_area;
    let buffer = terminal.backend().buffer();
    let right = area.x + area.width.saturating_sub(1);
    let bottom = area.y + area.height.saturating_sub(1);

    // The four corners are ratatui's default box-drawing glyphs.
    assert_eq!(buffer[(area.x, area.y)].symbol(), "\u{250c}", "top-left");
    assert_eq!(buffer[(right, area.y)].symbol(), "\u{2510}", "top-right");
    assert_eq!(buffer[(area.x, bottom)].symbol(), "\u{2514}", "bottom-left");
    assert_eq!(buffer[(right, bottom)].symbol(), "\u{2518}", "bottom-right");

    // Both edges are vertical rules, never the ASCII `|`.
    for y in area.y + 1..bottom {
        assert_eq!(buffer[(area.x, y)].symbol(), "\u{2502}", "left edge y={y}");
        assert_eq!(buffer[(right, y)].symbol(), "\u{2502}", "right edge y={y}");
    }

    // The inner body keeps plain ASCII text and markers.
    for y in area.y + 1..bottom {
        for x in area.x + 1..right {
            let symbol = buffer[(x, y)].symbol();
            assert!(
                symbol.is_ascii(),
                "non-ASCII body glyph {symbol:?} at ({x}, {y})"
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

    // Focusing the category navigator fills its cursor cyan and dims the result
    // cursor instead (FR-009, FR-052).
    app.plugin_store.as_mut().expect("panel open").focus = StorePane::Categories;
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
fn the_navigator_rows_show_the_entry_count_beside_each_name() {
    // Three uncategorised entries (`sample()`), so the category navigator has only
    // the `ALL` row and it counts all three; each row renders the name with the
    // count appended (FR-053).
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let terminal = render(&mut app, 100, 30);
    let nav = app.plugin_store_nav_area;
    let painted: String = (nav.y..nav.y + nav.height)
        .map(|y| row_text(&terminal, y, nav.x, nav.x + nav.width))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        painted.contains("ALL (3)"),
        "the ALL row carries its entry count: {painted:?}"
    );

    // Give the entries distinct categories so a named row also shows a count.
    let categorised = |id: &str, description: &str, category: &str| StoreEntry {
        category: Some(category.to_string()),
        ..entry(id, description)
    };
    let mut app = open(
        StoreKind::Codex,
        vec![
            categorised("codex-weather", "Weather lookups", "Web"),
            categorised("codex-time", "Time helpers", "Web"),
            categorised("codex-shell", "Shell wrappers", "Utility"),
        ],
        &[],
    );
    let terminal = render(&mut app, 100, 30);
    let nav = app.plugin_store_nav_area;
    let painted: String = (nav.y..nav.y + nav.height)
        .map(|y| row_text(&terminal, y, nav.x, nav.x + nav.width))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        painted.contains("ALL (3)"),
        "ALL counts every entry: {painted:?}"
    );
    assert!(
        painted.contains("Utility (1)"),
        "a named category shows its count: {painted:?}"
    );
    assert!(
        painted.contains("Web (2)"),
        "a folded category shows its count: {painted:?}"
    );
}

// ---------------------------------------------------------------------------
// FR-004 - a reversed-video title sits above each navigator's ALL entry
// ---------------------------------------------------------------------------

#[test]
fn each_navigator_has_a_reversed_video_title_above_its_all_row() {
    // The category navigator's title names the navigation type one row above its
    // `ALL` entry, painted black-on-white (reversed video); likewise the marketplace
    // navigator (FR-004).
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let terminal = render(&mut app, 100, 30);
    let nav = app.plugin_store_nav_area;
    let mp = app.plugin_store_marketplace_area;

    // Each navigator's row area sits directly below its one-row title.
    let category_title = row_text(&terminal, nav.y - 1, nav.x, nav.x + nav.width);
    assert!(
        category_title.contains("Categories"),
        "the category title names the navigation type: {category_title:?}"
    );
    let marketplace_title = row_text(&terminal, mp.y - 1, mp.x, mp.x + mp.width);
    assert!(
        marketplace_title.contains("Marketplace"),
        "the marketplace title names the navigation type: {marketplace_title:?}"
    );

    // The title is reversed video: black foreground on a white background, so it
    // reads as a header band above the `ALL` row.
    let buffer = terminal.backend().buffer();
    assert_eq!(
        buffer[(nav.x, nav.y - 1)].fg,
        Color::Black,
        "the category title foreground is reversed"
    );
    assert_eq!(
        buffer[(nav.x, nav.y - 1)].bg,
        Color::White,
        "the category title background is white"
    );
    assert_eq!(
        buffer[(mp.x, mp.y - 1)].fg,
        Color::Black,
        "the marketplace title foreground is reversed"
    );
    assert_eq!(
        buffer[(mp.x, mp.y - 1)].bg,
        Color::White,
        "the marketplace title background is white"
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
    // The marker sits in the one-column lane between the `>` gutter and the id
    // column, so it never competes with the description for horizontal room
    // (FR-005).
    let result = app.plugin_store_result_area;
    let marker_x = result.x + 2;
    assert_eq!(
        row_text(&terminal, installed, marker_x, marker_x + 1),
        "*",
        "installed row carries the marker in its leading lane: {text:?}"
    );
    assert_eq!(
        first_glyph_fg(&terminal, &app, installed),
        Color::Green,
        "installed row is painted in the installed colour"
    );

    // A not-installed row away from the block cursor is painted in the normal
    // colour and carries a blank in the marker lane, so the id, version,
    // marketplace and description columns stay aligned with the installed rows.
    let plain = find_row(&terminal, &app, "codex-time").expect("plain row painted");
    assert_eq!(first_glyph_fg(&terminal, &app, plain), Color::White);
    assert_eq!(
        row_text(&terminal, plain, marker_x, marker_x + 1),
        " ",
        "a not-installed row leaves the marker lane blank"
    );
    // Both rows start their id column in the same cell.
    let installed_id = row_text(&terminal, installed, result.x, result.x + result.width)
        .find("codex-weather")
        .expect("installed id painted");
    let plain_id = row_text(&terminal, plain, result.x, result.x + result.width)
        .find("codex-time")
        .expect("plain id painted");
    assert_eq!(
        installed_id, plain_id,
        "the marker lane keeps the id column aligned"
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

#[test]
fn the_description_runs_to_the_panel_edge_and_is_clipped_there() {
    // The description is the row's last column: it renders right up to the result
    // column's right edge and a description longer than that room is clipped there,
    // instead of pushing the id, version or marketplace columns off the row
    // (FR-004, FR-018, FR-048).
    let mut entries = sample();
    entries[0].description = "d".repeat(400);
    entries[0].marketplace = "zeta".to_string();
    let mut app = open(StoreKind::Codex, entries, &[]);
    let terminal = render(&mut app, 140, 30);

    let row = find_row(&terminal, &app, "codex-weather").expect("row painted");
    let result = app.plugin_store_result_area;
    let text = row_text(&terminal, row, result.x, result.x + result.width);
    assert!(
        text.contains("codex-weather") && text.contains("1.0.0") && text.contains("zeta"),
        "the fixed columns and the marketplace still render: {text:?}"
    );
    assert!(
        text.ends_with('d'),
        "the description fills every cell up to the column edge: {text:?}"
    );
    // Everything after the marketplace column is description, so the description
    // owns the whole trailing remainder of the row - more columns than the previous
    // fixed ceiling allowed.
    let cells: Vec<char> = text.chars().collect();
    let desc_start = cells
        .iter()
        .rposition(|&c| c != 'd')
        .map(|i| i + 1)
        .expect("the row is not all description");
    assert!(
        cells[desc_start..].iter().all(|&c| c == 'd'),
        "the description owns the whole trailing remainder: {text:?}"
    );
    let marketplace_cell = text.find("zeta").expect("marketplace painted");
    assert!(
        desc_start > marketplace_cell,
        "the description follows the marketplace column: {text:?}"
    );
    assert!(
        cells.len() - desc_start >= 45,
        "the description is given the row's remaining columns: {text:?}"
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

// ---------------------------------------------------------------------------
// Search box - bottom-edge filter field with a click-to-clear button
// ---------------------------------------------------------------------------

/// A left-click mouse event at `(col, row)`.
fn left_click(col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

#[test]
fn the_search_box_sits_on_the_bottom_edge_with_a_clear_button() {
    // The panel's last inner row carries the live search box: the `search:` label,
    // the editable field, and the right-aligned `[x] clear` button. The footer moves
    // up one row to make space.
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let terminal = render(&mut app, 120, 30);
    let area = app.plugin_store_area;

    let search = app.plugin_store_search_area;
    assert!(search.width > 0, "the search-box area is recorded");
    assert_eq!(
        search.y,
        area.y + area.height - 2,
        "the search box is the last inner row, beneath the footer"
    );

    let row = row_text(&terminal, search.y, search.x, search.x + search.width);
    assert!(
        row.contains("search:"),
        "the box carries the search label: {row:?}"
    );
    assert!(
        row.contains("[x] clear"),
        "the box carries the clear button: {row:?}"
    );

    // The clear button occupies the box's rightmost columns.
    let clear = app.plugin_store_clear_area;
    assert!(clear.width > 0, "the clear-button area is recorded");
    assert_eq!(
        clear.x + clear.width,
        search.x + search.width,
        "the clear button is right-aligned in the box"
    );
}

#[test]
fn the_search_box_field_reflects_the_live_query() {
    // Filtering by typed text is reflected in the box: the field paints the same
    // characters the result filter uses.
    let mut app = open(StoreKind::Codex, sample(), &[]);
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_query("wea".to_string());
    let terminal = render(&mut app, 120, 30);

    let search = app.plugin_store_search_area;
    let row = row_text(&terminal, search.y, search.x, search.x + search.width);
    assert!(
        row.contains("search: wea"),
        "the box mirrors the query: {row:?}"
    );
    // The filter narrowed the list to the one matching row.
    let title = full_row(&terminal, app.plugin_store_area.y);
    assert!(title.contains("1 of 3"), "title: {title:?}");
}

#[test]
fn the_search_box_clear_button_clears_the_filter_on_click() {
    // A left-click inside the clear button wipes the live search filter and restores
    // the full result set through the real mouse-routing path.
    let mut app = open(StoreKind::Codex, sample(), &[]);
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_query("wea".to_string());
    let _ = render(&mut app, 120, 30);
    assert_eq!(
        app.plugin_store
            .as_ref()
            .expect("panel open")
            .filtered
            .len(),
        1,
        "the query narrowed the list before the click"
    );

    let clear = app.plugin_store_clear_area;
    app.handle_mouse_event(left_click(clear.x, clear.y));
    assert_eq!(
        app.plugin_store.as_ref().expect("panel open").query,
        "",
        "the click cleared the search query"
    );
    assert_eq!(
        app.plugin_store
            .as_ref()
            .expect("panel open")
            .filtered
            .len(),
        3,
        "the full result set is restored after clearing"
    );
}

// ---------------------------------------------------------------------------
// Readability - the cursor gutter and the zebra band
// ---------------------------------------------------------------------------

#[test]
fn the_cursor_row_carries_a_gutter_marker() {
    // A `>` in the two-column gutter marks the row under the cursor, so the cursor
    // stays visible even where a background-only highlight washes out (FR-005), and
    // every other row keeps the blank gutter so the columns stay aligned.
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let terminal = render(&mut app, 120, 30);
    let result = app.plugin_store_result_area;

    // The alphabetically first row (`codex-shell`) holds the cursor at open.
    let selected = find_row(&terminal, &app, "codex-shell").expect("cursor row painted");
    let gutter = row_text(&terminal, selected, result.x, result.x + 2);
    assert_eq!(gutter, "> ", "the cursor row carries the gutter marker");

    let other = find_row(&terminal, &app, "codex-time").expect("other row painted");
    let gutter = row_text(&terminal, other, result.x, result.x + 2);
    assert_eq!(gutter, "  ", "an unselected row leaves the gutter blank");
}

#[test]
fn alternate_result_rows_carry_a_band_background() {
    // Consecutive rows alternate a near-black band so a wide row can be followed
    // across its id, version, marketplace and description columns.
    let mut app = open(StoreKind::Codex, sample(), &[]);
    let terminal = render(&mut app, 120, 30);
    let result = app.plugin_store_result_area;
    let buffer = terminal.backend().buffer();

    // The cursor rests on the first row, so its own highlight masks the band there;
    // the banding alternates over the rows beneath it.
    let band = Color::Rgb(22, 22, 30);
    let second = result.y + 1;
    let third = result.y + 2;
    assert_eq!(
        buffer[(result.x + 3, second)].bg,
        band,
        "the second row carries the band"
    );
    assert_ne!(
        buffer[(result.x + 3, third)].bg,
        band,
        "the third row is unbanded"
    );

    // The band reaches the column's right edge rather than stopping after the last
    // glyph, so the row reads as one stripe.
    let right_edge = result.x + result.width - 1;
    assert_eq!(
        buffer[(right_edge, second)].bg,
        band,
        "the band spans the whole row"
    );
}
