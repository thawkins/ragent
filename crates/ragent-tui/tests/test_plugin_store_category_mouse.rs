//! Mouse-routing tests for the plugin-store category navigator (spec `catnav`
//! T-006; FR-011, FR-012, FR-013, FR-023, FR-024, FR-028).
//!
//! These drive the real `handle_mouse_event` path with an in-memory entry list
//! after a real render has populated the navigator and result column areas, so
//! they are deterministic and contact no store endpoint. They pin the mouse
//! convenience layer over the keyboard path: a left-click on a category row
//! selects and applies it and focuses the navigator, a wheel over the navigator
//! scrolls its viewport without changing the active category, a left-click on a
//! result row moves the result cursor and leaves the category alone, and every
//! mouse path installs nothing and never touches the message input.

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_plugins::{StoreEntry, StoreKind};
use ragent_tui::App;
use ragent_tui::layout;

#[path = "support/mod.rs"]
mod support;

/// A store entry with `id` and an optional `category`.
fn entry(id: &str, category: Option<&str>) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: "1.0.0".to_string(),
        source: format!("https://example.org/{id}.zip"),
        description: format!("{id} description"),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
        marketplace: String::new(),
        category: category.map(str::to_string),
    }
}

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

fn left_click(col: u16, row: u16) -> MouseEvent {
    mouse(MouseEventKind::Down(MouseButton::Left), col, row)
}

fn wheel_down(col: u16, row: u16) -> MouseEvent {
    mouse(MouseEventKind::ScrollDown, col, row)
}

fn wheel_up(col: u16, row: u16) -> MouseEvent {
    mouse(MouseEventKind::ScrollUp, col, row)
}

/// An app with the panel open and the fixture entries loaded. The distinct
/// categories are `Utility` and `Web` (the two `utility` spellings fold into one
/// row through the case-insensitive match), so the navigator rows are
/// `ALL` (0), `Utility` (1), `Web` (2).
fn open() -> App {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_entries(vec![
            entry("codex-weather", Some("Web")),
            entry("codex-time", Some("Utility")),
            entry("codex-shell", Some("utility")),
            entry("codex-uncat", None),
        ]);
    app
}

/// An app whose panel declares `count` distinct categories, so the navigator
/// overflows a short column and the wheel has something to scroll.
fn open_many(count: usize) -> App {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    let entries: Vec<StoreEntry> = (0..count)
        .map(|i| {
            let id = format!("codex-{i:02}");
            let category = format!("Cat{i:02}");
            entry(&id, Some(&category))
        })
        .collect();
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_entries(entries);
    app
}

/// The browser behind the open panel.
fn browser(app: &App) -> &ragent_tui::app::PluginStoreBrowser {
    app.plugin_store.as_ref().expect("panel open")
}

/// The filtered ids, in order.
fn filtered_ids(app: &App) -> Vec<String> {
    let browser = browser(app);
    browser
        .filtered
        .iter()
        .map(|index| browser.all[*index].id.clone())
        .collect()
}

/// The absolute navigator row carrying `label` (row `0` is `ALL`, row `1` is the
/// `Installed` pseudo-category; the derived categories start at row `2`).
fn row_of(app: &App, label: &str) -> usize {
    let browser = browser(app);
    (0..browser.category_row_count())
        .find(|&row| browser.category_row_label(row).eq_ignore_ascii_case(label))
        .unwrap_or_else(|| panic!("no navigator row for `{label}`"))
}

/// Render one frame so the navigator and result column areas are populated, and
/// return the terminal for its geometry.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");
    terminal
}

// -- Left-click a category row selects, applies, and focuses it (FR-011) -------

#[tokio::test]
async fn left_click_on_a_category_row_selects_applies_and_focuses_it() {
    let mut app = open();
    render(&mut app, 100, 30);
    let nav = app.plugin_store_nav_area;
    assert!(
        !app.plugin_store_nav_focused(),
        "the result list starts focused"
    );

    // Click the `Web` row (row 0 is `ALL`, row 1 is `Installed`).
    let web = row_of(&app, "Web");
    app.handle_mouse_event(left_click(nav.x + 1, nav.y + web as u16));

    // The click moves the cursor to `Web`, applies it, and focuses the navigator
    // (FR-011).
    assert_eq!(browser(&app).category_cursor_row(), web);
    assert_eq!(browser(&app).active_category(), "Web");
    assert!(
        app.plugin_store_nav_focused(),
        "the click focuses navigator"
    );
    assert_eq!(filtered_ids(&app), ["codex-weather"]);
}

#[tokio::test]
async fn left_click_on_the_all_row_restores_the_full_set() {
    let mut app = open();
    render(&mut app, 100, 30);
    let nav = app.plugin_store_nav_area;

    let web = row_of(&app, "Web");
    app.handle_mouse_event(left_click(nav.x + 1, nav.y + web as u16));
    assert_eq!(filtered_ids(&app), ["codex-weather"]);

    app.handle_mouse_event(left_click(nav.x + 1, nav.y)); // ALL
    assert_eq!(browser(&app).active_category(), "ALL");
    assert_eq!(
        filtered_ids(&app),
        ["codex-shell", "codex-time", "codex-uncat", "codex-weather"]
    );
}

#[tokio::test]
async fn left_click_outside_the_navigator_rows_leaves_the_selection_unchanged() {
    let mut app = open();
    render(&mut app, 100, 30);
    let nav = app.plugin_store_nav_area;

    // A click far below the last category row maps past the row count, so the
    // selection cannot change (FR-011, FR-027).
    app.handle_mouse_event(left_click(nav.x + 1, nav.y + 10));

    assert_eq!(browser(&app).active_category(), "ALL");
    assert_eq!(browser(&app).category_cursor_row(), 0);
}

// -- Left-click a result row moves the result cursor, keeps the category (FR-013) --

#[tokio::test]
async fn left_click_on_a_result_row_moves_the_cursor_and_keeps_the_category() {
    let mut app = open();
    render(&mut app, 100, 30);
    let result = app.plugin_store_result_area;

    // Row 1 in the result column is the second filtered entry; the active category
    // stays `ALL` (FR-013).
    app.handle_mouse_event(left_click(result.x + 1, result.y + 1));

    assert_eq!(browser(&app).cursor, 1, "the result cursor moved");
    assert_eq!(browser(&app).active_category(), "ALL");
    assert_eq!(browser(&app).category_cursor_row(), 0);
}

#[tokio::test]
async fn left_click_on_a_result_row_under_a_category_keeps_that_category() {
    let mut app = open();
    render(&mut app, 100, 30);
    let nav = app.plugin_store_nav_area;
    let result = app.plugin_store_result_area;

    // Select `Utility` first, so two rows survive (`codex-time`, `codex-shell`).
    let utility = row_of(&app, "Utility");
    app.handle_mouse_event(left_click(nav.x + 1, nav.y + utility as u16));
    assert_eq!(browser(&app).active_category(), "Utility");

    app.handle_mouse_event(left_click(result.x + 1, result.y + 1));

    assert_eq!(browser(&app).cursor, 1, "the cursor moved within Utility");
    assert_eq!(
        browser(&app).active_category(),
        "Utility",
        "the result click leaves the category unchanged (FR-013)"
    );
}

// -- Wheel over the navigator scrolls it without changing the category (FR-012) --

#[tokio::test]
async fn wheel_over_the_navigator_scrolls_the_viewport_without_changing_the_category() {
    // 30 categories overflow the navigator, so the wheel has room to move.
    let mut app = open_many(30);
    render(&mut app, 100, 30);
    let nav = app.plugin_store_nav_area;
    assert_eq!(browser(&app).category_scroll, 0);

    app.handle_mouse_event(wheel_down(nav.x + 1, nav.y + 1));

    // The wheel advances the viewport by the shared scroll step and leaves the
    // active category (and the result list) exactly as it was (FR-012).
    assert_eq!(browser(&app).category_scroll, 3);
    assert_eq!(browser(&app).active_category(), "ALL");

    app.handle_mouse_event(wheel_up(nav.x + 1, nav.y + 1));
    assert_eq!(browser(&app).category_scroll, 0);
    assert_eq!(browser(&app).active_category(), "ALL");
}

#[tokio::test]
async fn left_click_on_a_scrolled_navigator_selects_the_visible_row() {
    let mut app = open_many(30);
    render(&mut app, 100, 30);
    let nav = app.plugin_store_nav_area;

    // Scroll the viewport down six rows, then click its top row. The click maps
    // through the scroll offset, so it selects absolute row 6, whose label is read
    // back from the navigator rather than hard-coded (FR-011, FR-012).
    app.handle_mouse_event(wheel_down(nav.x + 1, nav.y + 1)); // scroll -> 3
    app.handle_mouse_event(wheel_down(nav.x + 1, nav.y + 1)); // scroll -> 6
    let expected = browser(&app).category_row_label(6).to_string();
    app.handle_mouse_event(left_click(nav.x + 1, nav.y));

    assert_eq!(browser(&app).category_cursor_row(), 6);
    assert_eq!(browser(&app).active_category(), expected);
    assert!(app.plugin_store_nav_focused());
}

// -- Wheel over the result column moves the result cursor (FR-013) ------------

#[tokio::test]
async fn wheel_over_the_result_column_moves_the_result_cursor_not_the_navigator() {
    let mut app = open_many(30);
    render(&mut app, 100, 30);
    let result = app.plugin_store_result_area;

    app.handle_mouse_event(wheel_down(result.x + 1, result.y + 1));

    // The result column scrolls (its cursor-driven window follows the cursor) while
    // the navigator viewport and its active category are untouched (FR-013).
    assert_eq!(browser(&app).cursor, 3, "the result cursor advanced");
    assert_eq!(browser(&app).category_scroll, 0);
    assert_eq!(browser(&app).active_category(), "ALL");

    app.handle_mouse_event(wheel_up(result.x + 1, result.y + 1));
    assert_eq!(browser(&app).cursor, 0, "the result cursor returned");
}

// -- No mouse path installs or touches the input (FR-024) ----------------------

#[tokio::test]
async fn mouse_paths_never_install_or_start_a_text_selection() {
    let mut app = open();
    app.input = "draft".to_string();
    render(&mut app, 100, 30);
    let nav = app.plugin_store_nav_area;
    let result = app.plugin_store_result_area;

    app.handle_mouse_event(left_click(nav.x + 1, nav.y + 2));
    app.handle_mouse_event(left_click(result.x + 1, result.y + 1));
    app.handle_mouse_event(wheel_down(nav.x + 1, nav.y + 1));

    assert!(
        browser(&app).last_install.is_none(),
        "no navigator or result click installs anything (FR-024)"
    );
    assert!(
        app.text_selection.is_none(),
        "the panel swallows the click instead of starting a selection"
    );
    assert_eq!(app.input, "draft", "the message draft is untouched");
}

#[tokio::test]
async fn a_click_on_the_panel_outside_both_columns_is_swallowed() {
    let mut app = open();
    render(&mut app, 100, 30);
    let area = app.plugin_store_area;

    // The footer row is inside the modal but outside both columns; the click is
    // swallowed, so it cannot fall through to a surface behind the panel (FR-023).
    app.handle_mouse_event(left_click(area.x + 1, area.y + area.height - 2));

    assert!(app.text_selection.is_none());
    assert_eq!(browser(&app).active_category(), "ALL");
}

// -- Marketplace navigator mouse routing (spec `pluginstores` FR-052) ----------

/// An app whose entries carry the given marketplaces, so the marketplace navigator
/// has rows to click and scroll.
fn open_marketplaces(entries: Vec<StoreEntry>) -> App {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_entries(entries);
    app
}

/// An entry attributed to `marketplace`.
fn mp_entry(id: &str, marketplace: &str) -> StoreEntry {
    let mut e = entry(id, Some("Web"));
    e.marketplace = marketplace.to_string();
    e
}

#[tokio::test]
async fn left_click_on_a_marketplace_row_selects_applies_and_focuses_it() {
    let mut app = open_marketplaces(vec![
        mp_entry("codex-a", "codex"),
        mp_entry("community-a", "community"),
    ]);
    render(&mut app, 100, 30);
    let mp = app.plugin_store_marketplace_area;
    assert!(
        !app.plugin_store_nav_focused() && !app.plugin_store_marketplace_focused(),
        "the result list starts focused"
    );

    // Row 1 in the marketplace pane is `codex` (row 0 is `ALL`).
    app.handle_mouse_event(left_click(mp.x + 1, mp.y + 1));

    assert_eq!(browser(&app).active_marketplace(), "codex");
    assert!(
        app.plugin_store_marketplace_focused(),
        "the click focuses the marketplace pane (FR-052)"
    );
    assert_eq!(filtered_ids(&app), ["codex-a"]);
}

#[tokio::test]
async fn wheel_over_the_marketplace_navigator_scrolls_it_without_changing_the_filter() {
    let entries: Vec<StoreEntry> = (0..30)
        .map(|i| mp_entry(&format!("codex-{i:02}"), &format!("Mkt{i:02}")))
        .collect();
    let mut app = open_marketplaces(entries);
    render(&mut app, 100, 30);
    let mp = app.plugin_store_marketplace_area;
    assert_eq!(browser(&app).marketplace_scroll, 0);

    app.handle_mouse_event(wheel_down(mp.x + 1, mp.y + 1));

    // The wheel advances the marketplace viewport and leaves the active marketplace
    // (and the result list) exactly as it was (FR-052).
    assert_eq!(browser(&app).marketplace_scroll, 3);
    assert_eq!(browser(&app).active_marketplace(), "ALL");
    assert_eq!(browser(&app).category_scroll, 0);
}
