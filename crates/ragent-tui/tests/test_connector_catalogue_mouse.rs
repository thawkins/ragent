//! Mouse-routing tests for the connector-catalogue category navigator
//! (`/connectors claude`).
//!
//! These drive the real `handle_mouse_event` path with an in-memory catalogue
//! after a real render has populated the navigator and result column areas, so
//! they are deterministic and contact no catalogue endpoint. They pin the same
//! mouse convenience layer the plugin-store navigator has: a left-click on a
//! category row selects and applies it and focuses the navigator, a wheel over
//! the navigator scrolls its viewport without changing the active category, a
//! left-click on a result row moves the result cursor and leaves the category
//! alone, and every mouse path installs nothing.

use crossterm::event::{KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_connectors::{
    CatalogueBrowser, CategoryFilter, ConnectorAuthShape, ConnectorDescriptor, ConnectorId,
    ConnectorServer,
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

/// A catalogue descriptor with `id` and `category`.
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

/// An app with the panel open and the fixture entries loaded. The distinct
/// categories are `developer`, `productivity`, and `communication` (sorted
/// case-insensitively), so the navigator rows are `ALL` (0), `communication`
/// (1), `developer` (2), `productivity` (3).
fn open() -> App {
    let mut app = support::make_app();
    app.open_connector_catalogue("", CategoryFilter::all(), false);
    let browser = app.connector_store.as_mut().expect("panel open");
    browser.set_entries(
        vec![
            descriptor("google-drive", "productivity"),
            descriptor("github", "developer"),
            descriptor("slack", "communication"),
        ],
        0,
    );
    app
}

/// The browser behind the open panel.
fn browser(app: &App) -> &CatalogueBrowser {
    app.connector_store.as_ref().expect("panel open")
}

/// Render one frame so the navigator and result column areas are populated.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");
    terminal
}

// -- Left-click a category row selects, applies, and focuses it ---------------

#[tokio::test]
async fn left_click_on_a_category_row_selects_applies_and_focuses_it() {
    let mut app = open();
    render(&mut app, 110, 30);
    let nav = app.connector_store_nav_area;
    assert!(
        !app.connector_catalogue_nav_focused(),
        "the result list starts focused"
    );

    // Row 2 in the navigator is `developer` (row 0 is `ALL`).
    app.handle_mouse_event(left_click(nav.x + 1, nav.y + 2));

    let browser = browser(&app);
    assert_eq!(
        browser.category_cursor_row(),
        2,
        "the cursor moved to the row"
    );
    assert_eq!(
        browser.active_category(),
        "developer",
        "the category applied"
    );
    assert_eq!(browser.filtered.len(), 1, "the result set narrowed");
    assert!(
        app.connector_catalogue_nav_focused(),
        "the click focuses the navigator"
    );
}

#[tokio::test]
async fn left_click_on_the_all_row_restores_the_full_set() {
    let mut app = open();
    render(&mut app, 110, 30);
    let nav = app.connector_store_nav_area;

    app.handle_mouse_event(left_click(nav.x + 1, nav.y + 2)); // developer
    assert_eq!(browser(&app).filtered.len(), 1);

    app.handle_mouse_event(left_click(nav.x + 1, nav.y)); // ALL
    assert_eq!(browser(&app).active_category(), "ALL");
    assert_eq!(browser(&app).filtered.len(), 3);
}

#[tokio::test]
async fn left_click_outside_the_navigator_rows_leaves_the_selection_unchanged() {
    let mut app = open();
    render(&mut app, 110, 30);
    let nav = app.connector_store_nav_area;

    // A click far below the last category row maps past the row count, so the
    // selection cannot change.
    app.handle_mouse_event(left_click(nav.x + 1, nav.y + 12));

    assert_eq!(browser(&app).active_category(), "ALL");
    assert_eq!(browser(&app).category_cursor_row(), 0);
}

// -- Left-click a result row moves the result cursor, keeps the category -----

#[tokio::test]
async fn left_click_on_a_result_row_moves_the_cursor_and_keeps_the_category() {
    let mut app = open();
    render(&mut app, 110, 30);
    let result = app.connector_store_result_area;

    // Row 1 in the result column is the second filtered entry; the active
    // category stays `ALL`.
    app.handle_mouse_event(left_click(result.x + 1, result.y + 1));

    let browser = browser(&app);
    assert_eq!(browser.cursor, 1, "the result cursor moved");
    assert_eq!(browser.active_category(), "ALL");
    assert_eq!(browser.category_cursor_row(), 0);
}

#[tokio::test]
async fn a_wheel_over_the_navigator_scrolls_without_changing_the_category() {
    let mut app = open();
    render(&mut app, 110, 30);
    let nav = app.connector_store_nav_area;

    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, nav.x + 1, nav.y + 1));

    let browser = browser(&app);
    assert_eq!(
        browser.active_category(),
        "ALL",
        "the wheel selects nothing"
    );
    assert_eq!(browser.category_cursor_row(), 0, "the cursor did not move");
    assert_eq!(
        browser.category_scroll, 3,
        "the viewport scrolled by the wheel step"
    );
}

// -- A click or wheel on the pinned heading row is inert ----------------------

#[tokio::test]
async fn a_click_on_the_pinned_heading_row_is_inert() {
    // The cached result area is anchored on the scrolling list, below the pinned
    // heading, so a click on the heading row maps to no row and moves nothing.
    let mut app = open();
    render(&mut app, 110, 30);
    let list = app.connector_store_result_area;

    app.handle_mouse_event(left_click(list.x + 2, list.y - 1));

    assert_eq!(browser(&app).cursor, 0, "the result cursor did not move");
    assert!(
        !app.connector_catalogue_nav_focused(),
        "the heading click does not focus the navigator"
    );
}

#[tokio::test]
async fn a_wheel_on_the_pinned_heading_row_is_inert() {
    let mut app = open();
    render(&mut app, 110, 30);
    let list = app.connector_store_result_area;

    app.handle_mouse_event(mouse(MouseEventKind::ScrollDown, list.x + 2, list.y - 1));

    assert_eq!(
        browser(&app).cursor,
        0,
        "the heading wheel does not move the result cursor"
    );
}
