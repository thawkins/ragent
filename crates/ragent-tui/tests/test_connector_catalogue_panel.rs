//! Render tests for the connector-catalogue browse panel (`/connectors claude`).
//!
//! The panel is the interactive peer of `/plugins claude` for the connector
//! catalogue. These drive the real renderer and the real key routing through a
//! `TestBackend` and read the painted buffer back, so no catalogue endpoint is
//! contacted: the browser state is filled directly.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;

use ragent_connectors::{
    CategoryFilter, ConnectorAuthShape, ConnectorDescriptor, ConnectorId, ConnectorServer,
};
use ragent_tui::App;
use ragent_tui::layout;

#[path = "support/mod.rs"]
mod support;

fn server() -> ConnectorServer {
    ConnectorServer {
        id: "main".to_string(),
        transport: "stdio".to_string(),
        command: Some("echo".to_string()),
        args: Vec::new(),
        env: Default::default(),
        url: None,
        headers: Default::default(),
    }
}

fn descriptor(id: &str, category: &str) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("valid id"),
        name: id.to_string(),
        description: format!("{id} description"),
        category: category.to_string(),
        tags: Vec::new(),
        source: format!("https://example.org/{id}.json"),
        marketplace: String::new(),
        provenance: Default::default(),
        auth: ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers: vec![server()],
        unsupported: Vec::new(),
    }
}

fn sample() -> Vec<ConnectorDescriptor> {
    vec![
        descriptor("google-drive", "productivity"),
        descriptor("github", "developer"),
        descriptor("slack", "communication"),
    ]
}

/// An app with the panel open and the given entries/installed set loaded. The
/// fetch is never attempted (no async reactor), so the entries are installed
/// directly.
fn open(entries: Vec<ConnectorDescriptor>, installed: &[&str], skipped: usize) -> App {
    let mut app = support::make_app();
    app.open_connector_catalogue("", CategoryFilter::all(), false);
    let browser = app.connector_store.as_mut().expect("panel open");
    browser.set_installed(installed.iter().map(|s| (*s).to_string()).collect());
    browser.set_entries(entries, skipped);
    app
}

/// An app with the panel open on `category`, populated from `entries` exactly as
/// [`App::poll_connector_catalogue_result`] fills it after an off-loop fetch:
/// the entries land first and the launch category is applied against the real
/// category set.
fn open_with_category(entries: Vec<ConnectorDescriptor>, category: CategoryFilter) -> App {
    let mut app = support::make_app();
    app.open_connector_catalogue("", category, false);
    let browser = app.connector_store.as_mut().expect("panel open");
    browser.set_entries(entries, 0);
    browser.apply_pending_category();
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

/// Read one painted result-list row across the result column (the panel body is
/// split into the category navigator and the result column; `x0`/`x1` are the
/// result column bounds).
fn result_row_text(terminal: &Terminal<TestBackend>, app: &App, y: u16) -> String {
    let area = app.connector_store_result_area;
    row_text(terminal, y, area.x, area.x + area.width)
}

/// Every painted row of the panel's inner area (borders stripped).
fn panel_rows(terminal: &Terminal<TestBackend>, app: &App) -> Vec<String> {
    let area = app.connector_store_area;
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
    // The panel body is split into the category navigator and the result
    // column; scan the result column's first painted glyph so a navigator row's
    // highlight on the same buffer row cannot be mistaken for the result
    // cursor.
    let area = app.connector_store_result_area;
    let buffer = terminal.backend().buffer();
    let x0 = area.x;
    let x1 = area.x + area.width;
    (x0..x1)
        .find(|&x| buffer[(x, y)].symbol() != " ")
        .map(|x| buffer[(x, y)].bg)
        .unwrap_or(Color::Reset)
}

/// Find the painted panel row containing `needle`, returning its buffer row.
fn find_row(terminal: &Terminal<TestBackend>, app: &App, needle: &str) -> Option<u16> {
    let area = app.connector_store_area;
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (area.y + 1..area.y + area.height.saturating_sub(1))
        .find(|&y| row_text(terminal, y, x0, x1).contains(needle))
}

// ---------------------------------------------------------------------------
// The panel is a bordered, titled modal with Unicode box-drawing borders
// ---------------------------------------------------------------------------

#[test]
fn the_panel_is_a_bordered_modal_with_the_catalogue_name_and_counts() {
    let mut app = open(sample(), &[], 0);
    let terminal = render(&mut app, 100, 30);
    let area = app.connector_store_area;

    let title = full_row(&terminal, area.y);
    assert!(
        title.contains("Claude Connectors"),
        "title names the catalogue: {title:?}"
    );
    assert!(
        title.contains("3 of 3"),
        "title reports the visible/total count: {title:?}"
    );
    assert!(
        title.contains("search:"),
        "title carries the search field label: {title:?}"
    );
    assert!(
        title.contains("category ALL"),
        "title names the active category: {title:?}"
    );

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
    let mut app = open(sample(), &[], 0);
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app);
    let footer = rows.last().expect("footer row");
    assert!(
        footer.contains("Enter select/install"),
        "footer: {footer:?}"
    );
    assert!(footer.contains("Esc close"), "footer: {footer:?}");
    assert!(footer.contains("Up/Down"), "footer: {footer:?}");
    assert!(footer.contains("c clear"), "footer: {footer:?}");
    assert!(footer.contains("Tab categories"), "footer: {footer:?}");
}

#[test]
fn the_panel_is_framed_with_unicode_box_drawing_borders() {
    let mut app = open(sample(), &["github"], 0);
    let terminal = render(&mut app, 100, 30);
    let area = app.connector_store_area;
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
// Rows, the block cursor, and the installed marker
// ---------------------------------------------------------------------------

#[test]
fn the_result_rows_list_the_id_category_marketplace_and_description() {
    let mut app = open(sample(), &[], 0);
    // 160 columns: with the category navigator sharing the body, the result
    // column needs the extra width to paint all four row fields (id, category,
    // marketplace, description) in full.
    let terminal = render(&mut app, 160, 30);

    let row = find_row(&terminal, &app, "google-drive").expect("row painted");
    let text = result_row_text(&terminal, &app, row);
    assert!(text.contains("google-drive"), "row id: {text:?}");
    assert!(text.contains("productivity"), "row category: {text:?}");
    assert!(text.contains("description"), "row description: {text:?}");
}

#[test]
fn the_highlighted_row_carries_a_full_row_block_cursor() {
    let mut app = open(sample(), &[], 0);
    let terminal = render(&mut app, 100, 30);

    // The first row is highlighted; its leading marker cell is painted with the
    // block-cursor background. The catalogue sorts by id (FR-046), so the first row
    // is `github`.
    let row = find_row(&terminal, &app, "github").expect("row painted");
    assert_eq!(
        first_glyph_bg(&terminal, &app, row),
        Color::Magenta,
        "the highlighted row carries the cursor background"
    );
}

#[test]
fn an_installed_row_carries_an_installed_marker() {
    let mut app = open(sample(), &["github"], 0);
    let terminal = render(&mut app, 100, 30);

    let row = find_row(&terminal, &app, "github").expect("row painted");
    let text = result_row_text(&terminal, &app, row);
    assert!(
        text.contains("[installed]"),
        "installed row carries the marker: {text:?}"
    );

    // A row that is not installed carries no marker.
    let other = find_row(&terminal, &app, "slack").expect("row painted");
    let other_text = result_row_text(&terminal, &app, other);
    assert!(
        !other_text.contains("[installed]"),
        "uninstalled row carries no marker: {other_text:?}"
    );
}

// ---------------------------------------------------------------------------
// Explicit state lines instead of a blank list
// ---------------------------------------------------------------------------

#[test]
fn a_loading_browser_renders_a_loading_line() {
    let mut app = support::make_app();
    app.open_connector_catalogue("", CategoryFilter::all(), false);
    // Force the browser back to the loading state so the line is deterministic.
    if let Some(browser) = app.connector_store.as_mut() {
        browser.status = ragent_connectors::CatalogueBrowseStatus::Loading;
    }
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app).join("\n");
    assert!(
        rows.contains("loading connector catalogue..."),
        "loading line: {rows:?}"
    );
}

#[test]
fn a_failed_fetch_renders_the_cause_inline() {
    let mut app = support::make_app();
    app.open_connector_catalogue("", CategoryFilter::all(), false);
    if let Some(browser) = app.connector_store.as_mut() {
        browser.set_failed("catalogue unreachable".to_string());
    }
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app).join("\n");
    assert!(
        rows.contains("catalogue failed: catalogue unreachable"),
        "failure line: {rows:?}"
    );
}

#[test]
fn an_empty_catalogue_renders_an_explicit_empty_line() {
    let mut app = open(Vec::new(), &[], 0);
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app).join("\n");
    assert!(
        rows.contains("no connectors in this catalogue"),
        "empty line: {rows:?}"
    );
}

#[test]
fn a_query_that_matches_nothing_renders_a_no_match_line() {
    let mut app = open(sample(), &[], 0);
    if let Some(browser) = app.connector_store.as_mut() {
        browser.set_query("zzz-nothing".to_string());
    }
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app).join("\n");
    assert!(
        rows.contains("no matching connectors"),
        "no-match line: {rows:?}"
    );
}

// ---------------------------------------------------------------------------
// Key routing while the panel is open
// ---------------------------------------------------------------------------

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

#[tokio::test]
async fn down_moves_the_block_cursor_to_the_next_row() {
    let mut app = open(sample(), &[], 0);
    ragent_tui::input::handle_key(&mut app, key(KeyCode::Down)).await;
    let terminal = render(&mut app, 100, 30);

    // After one Down the cursor sits on the second row. The catalogue sorts by id
    // (FR-046): first `github`, second `google-drive`.
    let row = find_row(&terminal, &app, "google-drive").expect("row painted");
    assert_eq!(
        first_glyph_bg(&terminal, &app, row),
        Color::Magenta,
        "the second row carries the cursor after Down"
    );
}

#[tokio::test]
async fn typing_filters_the_result_rows() {
    let mut app = open(sample(), &[], 0);
    for c in "github".chars() {
        ragent_tui::input::handle_key(&mut app, key(KeyCode::Char(c))).await;
    }
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app).join("\n");
    assert!(rows.contains("github"), "matching row painted: {rows:?}");
    assert!(
        !rows.contains("google-drive") && !rows.contains("slack"),
        "non-matching rows are filtered out: {rows:?}"
    );
}

#[tokio::test]
async fn escape_on_an_empty_query_dismisses_the_panel() {
    let mut app = open(sample(), &[], 0);
    ragent_tui::input::handle_key(&mut app, key(KeyCode::Esc)).await;
    assert!(
        app.connector_store.is_none(),
        "Esc on an empty query closes the panel"
    );
}

#[tokio::test]
async fn enter_on_an_empty_result_set_records_a_notice_and_installs_nothing() {
    let mut app = open(Vec::new(), &[], 0);
    ragent_tui::input::handle_key(&mut app, key(KeyCode::Enter)).await;
    let browser = app.connector_store.as_ref().expect("panel still open");
    assert_eq!(
        browser.last_install.as_deref(),
        Some("no result highlighted")
    );
}

#[tokio::test]
async fn enter_on_an_installed_row_is_refused() {
    // The catalogue sorts by id (FR-046), so row 0 is `github`; mark it installed so
    // Enter on the highlighted row is refused.
    let mut app = open(sample(), &["github"], 0);
    ragent_tui::input::handle_key(&mut app, key(KeyCode::Enter)).await;
    let browser = app.connector_store.as_ref().expect("panel still open");
    assert_eq!(
        browser.last_install.as_deref(),
        Some("connector github is already installed")
    );
}

// ---------------------------------------------------------------------------
// The empty-query dismiss leaves the area cleared on the next frame
// ---------------------------------------------------------------------------

#[test]
fn the_panel_area_resets_when_the_panel_closes() {
    let mut app = open(sample(), &[], 0);
    let _ = render(&mut app, 100, 30);
    assert_ne!(app.connector_store_area, ratatui::layout::Rect::default());
    app.close_connector_catalogue();
    let _ = render(&mut app, 100, 30);
    assert_eq!(app.connector_store_area, ratatui::layout::Rect::default());
}

// ---------------------------------------------------------------------------
// Launch category filter (`/connectors claude --category <name>`)
// ---------------------------------------------------------------------------

#[test]
fn a_launch_category_filters_the_result_rows() {
    let mut app = open_with_category(sample(), CategoryFilter::parse("developer"));
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app).join("\n");
    assert!(
        rows.contains("github"),
        "the matching row is painted: {rows:?}"
    );
    assert!(
        !rows.contains("google-drive") && !rows.contains("slack"),
        "rows in other categories are filtered out: {rows:?}"
    );
    let title = full_row(&terminal, app.connector_store_area.y);
    assert!(
        title.contains("category developer"),
        "the title names the active launch category: {title:?}"
    );
}

#[test]
fn a_launch_category_the_catalogue_lacks_is_refused_instead_of_emptying_the_panel() {
    let mut app = open_with_category(sample(), CategoryFilter::parse("nosuch"));
    // The refusal is reported, and the panel keeps every row visible rather than
    // rendering an empty list that looks like a catalogue with no matches.
    let browser = app.connector_store.as_ref().expect("panel open");
    assert_eq!(browser.selected_category_label(), "ALL");
    assert_eq!(
        browser.last_install.as_deref(),
        Some("unknown category `nosuch`")
    );
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app).join("\n");
    assert!(rows.contains("google-drive"), "rows stay visible: {rows:?}");
}

#[test]
fn a_launch_category_all_leaves_every_row_visible() {
    let mut app = open_with_category(sample(), CategoryFilter::all());
    let terminal = render(&mut app, 100, 30);
    let title = full_row(&terminal, app.connector_store_area.y);
    assert!(
        title.contains("category ALL"),
        "an unfiltered launch names ALL: {title:?}"
    );
    let rows = panel_rows(&terminal, &app).join("\n");
    for id in ["google-drive", "github", "slack"] {
        assert!(rows.contains(id), "every row is visible: {rows:?}");
    }
}

#[tokio::test]
async fn c_clears_the_category_filter_back_to_all() {
    let mut app = open_with_category(sample(), CategoryFilter::parse("developer"));
    // With `developer` active the panel shows only the matching row; `c` is the
    // navigator's clear key: it restores the unfiltered `ALL` view rather than
    // stepping to the next category (the navigator rows do the stepping).
    {
        let browser = app.connector_store.as_ref().expect("panel open");
        assert_eq!(browser.selected_category_label(), "developer");
    }
    ragent_tui::input::handle_key(&mut app, key(KeyCode::Char('c'))).await;
    let browser = app.connector_store.as_ref().expect("panel open");
    assert_eq!(
        browser.selected_category_label(),
        "ALL",
        "`c` clears the active category back to ALL"
    );
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app).join("\n");
    for id in ["google-drive", "github", "slack"] {
        assert!(rows.contains(id), "every row is visible again: {rows:?}");
    }
    let title = full_row(&terminal, app.connector_store_area.y);
    assert!(
        title.contains("category ALL"),
        "the title names the cleared category: {title:?}"
    );
}

#[tokio::test]
async fn tab_routes_up_down_to_the_category_navigator() {
    let mut app = open(sample(), &[], 0);
    // Result rows focused by default: Down moves the result cursor and leaves
    // the category alone.
    ragent_tui::input::handle_key(&mut app, key(KeyCode::Down)).await;
    {
        let browser = app.connector_store.as_ref().expect("panel open");
        assert_eq!(browser.cursor, 1, "Down moved the result cursor");
        assert_eq!(browser.selected_category_label(), "ALL");
    }
    // Tab hands focus to the navigator: Down applies the first category row on
    // the same keypress.
    ragent_tui::input::handle_key(&mut app, key(KeyCode::Tab)).await;
    ragent_tui::input::handle_key(&mut app, key(KeyCode::Down)).await;
    {
        let browser = app.connector_store.as_ref().expect("panel open");
        assert_eq!(
            browser.selected_category_label(),
            "communication",
            "Down over the navigator applies the highlighted category"
        );
    }
    // Enter over the navigator applies the category and hands focus back to the
    // result list; the next Down moves the result cursor again.
    ragent_tui::input::handle_key(&mut app, key(KeyCode::Enter)).await;
    {
        let browser = app.connector_store.as_ref().expect("panel open");
        assert!(
            !browser.nav_focused,
            "Enter over the navigator returns focus to the results"
        );
        assert_eq!(
            browser.selected_category_label(),
            "communication",
            "the applied category sticks"
        );
    }
    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app).join("\n");
    assert!(rows.contains("slack"), "matching row painted: {rows:?}");
    assert!(
        !rows.contains("google-drive") && !rows.contains("github"),
        "non-matching rows are filtered out: {rows:?}"
    );
    let title = full_row(&terminal, app.connector_store_area.y);
    assert!(
        title.contains("category communication"),
        "the title names the navigator-applied category: {title:?}"
    );
}

// ---------------------------------------------------------------------------
// FR-044 - the marketplace column and the enrichment loading hint
// ---------------------------------------------------------------------------

/// A descriptor whose `marketplace` names the source it came from.
fn attributed(id: &str, marketplace: &str) -> ConnectorDescriptor {
    let mut entry = descriptor(id, "developer");
    entry.marketplace = marketplace.to_string();
    entry
}

#[test]
fn a_row_names_the_marketplace_it_came_from() {
    let mut app = open(vec![attributed("community-one", "acme")], &[], 0);
    let terminal = render(&mut app, 160, 30);

    let row = find_row(&terminal, &app, "community-one").expect("row painted");
    let text = result_row_text(&terminal, &app, row);
    assert!(
        text.contains("acme"),
        "the row names its marketplace: {text:?}"
    );
}

#[test]
fn a_row_with_no_marketplace_shows_a_dash_marker() {
    let mut app = open(sample(), &[], 0);
    let terminal = render(&mut app, 160, 30);

    let row = find_row(&terminal, &app, "google-drive").expect("row painted");
    let text = result_row_text(&terminal, &app, row);
    // The marketplace cell of a hand-built entry is blank, rendered as `-`.
    assert!(
        text.contains(" - "),
        "a blank marketplace renders as a dash: {text:?}"
    );
}

#[test]
fn the_title_shows_a_loading_details_hint_while_enrichment_runs() {
    let mut app = open(sample(), &[], 0);
    {
        let browser = app.connector_store.as_mut().expect("panel open");
        browser.enriching = true;
    }
    let terminal = render(&mut app, 120, 30);
    let title = full_row(&terminal, app.connector_store_area.y);
    assert!(
        title.contains("loading details..."),
        "the title shows the enrichment hint: {title:?}"
    );

    // Clearing the flag (as the terminal `Finished` message does) removes it.
    app.connector_store
        .as_mut()
        .expect("panel open")
        .finish_enrichment();
    let terminal = render(&mut app, 120, 30);
    let title = full_row(&terminal, app.connector_store_area.y);
    assert!(
        !title.contains("loading details..."),
        "the hint clears when enrichment finishes: {title:?}"
    );
}

// ---------------------------------------------------------------------------
// The pinned, dark-on-light column-heading row
// ---------------------------------------------------------------------------

#[test]
fn the_column_headings_render_above_the_rows_and_do_not_scroll() {
    // The headings sit one row above the first result row, aligned with their
    // columns, so each label reads as a pinned header rather than a result row.
    let mut app = open(sample(), &[], 0);
    let terminal = render(&mut app, 160, 30);

    let list = app.connector_store_result_area;
    // The catalogue sorts by id (FR-046), so the first result row is `github`.
    let first_row = find_row(&terminal, &app, "github").expect("result row painted");
    assert_eq!(
        first_row, list.y,
        "the first result row sits at the top of the scrolling list"
    );

    let heading_y = list.y - 1;
    let heading = row_text(&terminal, heading_y, list.x, list.x + list.width);
    assert!(
        heading.contains("Id")
            && heading.contains("Category")
            && heading.contains("Marketplace")
            && heading.contains("Description"),
        "the heading row names every column: {heading:?}"
    );

    // Each heading label starts exactly where its column's values start.
    let row = row_text(&terminal, first_row, list.x, list.x + list.width);
    let heading_id = heading.find("Id").expect("Id label painted");
    let row_id = row.find("github").expect("id value painted");
    assert_eq!(
        heading_id, row_id,
        "the Id label aligns with its column: {heading:?} vs {row:?}"
    );
}

#[test]
fn the_column_headings_are_dark_on_light_and_not_a_result_row() {
    // The heading cells are dark text on a light background so the pinned row is
    // visibly distinct from the normal and installed row colours.
    let mut app = open(sample(), &[], 0);
    let terminal = render(&mut app, 160, 30);

    let list = app.connector_store_result_area;
    let buffer = terminal.backend().buffer();
    let id_x = (list.x..list.x + list.width)
        .find(|&x| buffer[(x, list.y - 1)].symbol() == "I")
        .expect("Id heading painted");
    let heading_cell = &buffer[(id_x, list.y - 1)];
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

    // The first data row is the list's top row, below the heading, so finding a
    // row by id can never land on the heading itself. Sorted by id (FR-046) makes
    // `github` the top row.
    let first = find_row(&terminal, &app, "github").expect("result row painted");
    assert_eq!(first, list.y);
}

#[test]
fn the_column_headings_stay_pinned_when_the_rows_scroll() {
    // A catalogue too long for the panel forces the list to scroll; the heading
    // row must remain above the list, never scrolled away with the rows.
    let entries: Vec<_> = (0..40)
        .map(|i| descriptor(&format!("conn-{i:02}"), "developer"))
        .collect();
    let mut app = open(entries, &[], 0);
    app.connector_store.as_mut().expect("panel open").cursor = 39;
    let terminal = render(&mut app, 160, 30);

    let list = app.connector_store_result_area;
    let heading = row_text(&terminal, list.y - 1, list.x, list.x + list.width);
    assert!(
        heading.contains("Category") && heading.contains("Marketplace"),
        "the heading stays pinned above the scrolled list: {heading:?}"
    );

    // The list scrolled: its top row is a data row far from the first entry.
    let top = row_text(&terminal, list.y, list.x, list.x + list.width);
    assert!(
        top.contains("conn-") && !top.contains("conn-00"),
        "the list scrolled past the first entry: {top:?}"
    );
    assert!(
        !top.contains("Category"),
        "no column heading scrolled into the list body: {top:?}"
    );
}
