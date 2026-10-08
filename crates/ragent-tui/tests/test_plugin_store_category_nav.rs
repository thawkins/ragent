//! Keyboard-routing tests for the plugin-store category navigator (spec
//! `catnav` T-005; FR-007, FR-008, FR-009, FR-010, FR-015, FR-017, FR-024).
//!
//! These drive the real `handle_key` path with an in-memory entry list, so they
//! are deterministic and contact no store endpoint. They pin the two-pane focus
//! model: `Tab` transfers focus, `Up`/`Down` route to the focused pane (applying
//! the highlighted category over the navigator), `ENTER` over the navigator
//! applies the category and focuses the result list without installing, and `c`
//! clears the active category to `ALL`.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ragent_plugins::{StoreEntry, StoreKind};
use ragent_tui::App;
use ragent_tui::input::handle_key;

#[path = "support/mod.rs"]
mod support;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

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
        category: category.map(str::to_string),
    }
}

/// An app with the panel open and the fixture entries loaded. The distinct
/// categories are `Utility` and `Web` (the two `utility` spellings fold into one
/// row through the case-insensitive match), sorted as `ALL`, `Utility`, `Web`.
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

// -- Initial focus is the result list (FR-009) --------------------------------

#[tokio::test]
async fn before_any_focus_change_up_and_down_move_the_result_cursor() {
    let mut app = open();

    handle_key(&mut app, key(KeyCode::Down)).await;

    // The result list holds focus by default, so `Down` moves its cursor and
    // leaves the category untouched (FR-009).
    assert_eq!(browser(&app).cursor, 1);
    assert_eq!(browser(&app).category_cursor_row(), 0, "category unchanged");
    assert_eq!(browser(&app).active_category(), "ALL");
    assert!(
        !app.plugin_store_nav_focused(),
        "the result list holds focus"
    );
}

// -- Focus transfer routes Up/Down (FR-009, FR-017) ----------------------------

#[tokio::test]
async fn tab_transfers_focus_to_the_navigator_and_back() {
    let mut app = open();
    assert!(
        !app.plugin_store_nav_focused(),
        "the result list starts focused"
    );

    handle_key(&mut app, key(KeyCode::Tab)).await;
    assert!(app.plugin_store_nav_focused(), "Tab focuses the navigator");

    handle_key(&mut app, key(KeyCode::Tab)).await;
    assert!(!app.plugin_store_nav_focused(), "Tab toggles back");

    // BackTab is the reverse cycler and toggles the same two-pane focus.
    handle_key(&mut app, key(KeyCode::BackTab)).await;
    assert!(
        app.plugin_store_nav_focused(),
        "BackTab also transfers focus"
    );
}

#[tokio::test]
async fn while_the_navigator_is_focused_up_and_down_move_the_category_cursor() {
    let mut app = open();
    handle_key(&mut app, key(KeyCode::Tab)).await;

    handle_key(&mut app, key(KeyCode::Down)).await;

    // `Down` over the navigator moves the category cursor, not the result cursor,
    // and applies the highlighted category on the same keypress (FR-007, FR-017).
    assert_eq!(browser(&app).category_cursor_row(), 1);
    assert_eq!(browser(&app).active_category(), "Utility");
    assert_eq!(browser(&app).cursor, 0, "the result cursor is not moved");

    handle_key(&mut app, key(KeyCode::Down)).await;
    assert_eq!(browser(&app).category_cursor_row(), 2);
    assert_eq!(browser(&app).active_category(), "Web");

    handle_key(&mut app, key(KeyCode::Up)).await;
    assert_eq!(
        browser(&app).category_cursor_row(),
        1,
        "Up steps back (FR-008)"
    );
    assert_eq!(browser(&app).active_category(), "Utility");
}

// -- Applying a category narrows the result set (FR-007, FR-008) --------------

#[tokio::test]
async fn moving_the_category_cursor_applies_the_category_and_narrows_results() {
    let mut app = open();
    handle_key(&mut app, key(KeyCode::Tab)).await;

    handle_key(&mut app, key(KeyCode::Down)).await;
    // `Utility` folds the `Utility`/`utility` spellings into one exact,
    // case-insensitive match, so both survive (FR-007).
    assert_eq!(filtered_ids(&app), ["codex-time", "codex-shell"]);

    handle_key(&mut app, key(KeyCode::Down)).await;
    assert_eq!(
        filtered_ids(&app),
        ["codex-weather"],
        "`Web` selects one row"
    );

    handle_key(&mut app, key(KeyCode::Up)).await;
    handle_key(&mut app, key(KeyCode::Up)).await;
    // Back to `ALL`: every entry, including the uncategorised one, returns.
    assert_eq!(browser(&app).active_category(), "ALL");
    assert_eq!(
        filtered_ids(&app),
        ["codex-weather", "codex-time", "codex-shell", "codex-uncat"]
    );
}

#[tokio::test]
async fn the_category_cursor_clamps_at_both_ends_without_wrapping() {
    let mut app = open();
    handle_key(&mut app, key(KeyCode::Tab)).await;

    // Up at the top stays on `ALL`.
    handle_key(&mut app, key(KeyCode::Up)).await;
    assert_eq!(browser(&app).category_cursor_row(), 0);

    // Down past the last row stays on the last category.
    for _ in 0..5 {
        handle_key(&mut app, key(KeyCode::Down)).await;
    }
    assert_eq!(browser(&app).category_cursor_row(), 2);
    assert_eq!(browser(&app).active_category(), "Web");
}

// -- ENTER over the navigator applies and focuses the result list (FR-010, FR-024) --

#[tokio::test]
async fn enter_over_the_navigator_applies_the_category_and_focuses_the_result_list() {
    let mut app = open();
    handle_key(&mut app, key(KeyCode::Tab)).await;
    handle_key(&mut app, key(KeyCode::Down)).await;

    handle_key(&mut app, key(KeyCode::Enter)).await;

    // Focus moves to the result list and the highlighted category stays applied
    // (FR-010).
    assert!(
        !app.plugin_store_nav_focused(),
        "focus transfers to results"
    );
    assert_eq!(browser(&app).active_category(), "Utility");
    assert!(
        browser(&app).last_install.is_none(),
        "ENTER over the navigator installs nothing (FR-024)"
    );
}

#[tokio::test]
async fn after_enter_over_the_navigator_up_and_down_route_to_the_result_list() {
    let mut app = open();
    handle_key(&mut app, key(KeyCode::Tab)).await;
    handle_key(&mut app, key(KeyCode::Down)).await;
    handle_key(&mut app, key(KeyCode::Enter)).await;

    // Focus is now the result list, so `Down` moves the result cursor and leaves
    // the category (FR-009, FR-010).
    handle_key(&mut app, key(KeyCode::Down)).await;

    assert_eq!(browser(&app).category_cursor_row(), 1, "category unchanged");
    assert_eq!(browser(&app).active_category(), "Utility");
    assert_eq!(browser(&app).cursor, 1, "the result cursor advanced");
}

#[tokio::test]
async fn enter_on_the_result_list_still_starts_an_install() {
    let mut app = open();
    // No `Tab`: the result list holds focus, so ENTER keeps its install meaning
    // (FR-010 keeps the existing behaviour on the result pane).
    handle_key(&mut app, key(KeyCode::Enter)).await;

    assert_eq!(
        browser(&app).last_install.as_deref(),
        Some("installing codex-weather..."),
        "ENTER over the result list runs the normal install"
    );
}

// -- Clear-category key (FR-015) ----------------------------------------------

#[tokio::test]
async fn the_clear_key_sets_the_category_to_all_and_restores_the_full_set() {
    let mut app = open();
    handle_key(&mut app, key(KeyCode::Tab)).await;
    handle_key(&mut app, key(KeyCode::Down)).await;
    assert_eq!(filtered_ids(&app), ["codex-time", "codex-shell"]);

    handle_key(&mut app, key(KeyCode::Char('c'))).await;

    // `c` clears the active category to `ALL` and re-derives the visible set; it
    // is consumed as a command and never typed into the query (FR-015, FR-017).
    assert_eq!(browser(&app).active_category(), "ALL");
    assert_eq!(browser(&app).category_cursor_row(), 0);
    assert_eq!(
        filtered_ids(&app),
        ["codex-weather", "codex-time", "codex-shell", "codex-uncat"]
    );
    assert!(
        browser(&app).query.is_empty(),
        "`c` is not a query character"
    );
    assert!(
        browser(&app).last_install.is_none(),
        "clearing installs nothing (FR-024)"
    );
}

#[tokio::test]
async fn the_clear_key_also_works_while_the_result_list_is_focused() {
    let mut app = open();
    handle_key(&mut app, key(KeyCode::Tab)).await;
    handle_key(&mut app, key(KeyCode::Down)).await;
    handle_key(&mut app, key(KeyCode::Tab)).await; // focus back to the results
    assert!(!app.plugin_store_nav_focused());

    handle_key(&mut app, key(KeyCode::Char('c'))).await;

    assert_eq!(browser(&app).active_category(), "ALL");
}

// -- Routing: printable keys still edit the query while navigator-focused (FR-017) --

#[tokio::test]
async fn printable_keys_edit_the_query_while_the_navigator_is_focused() {
    let mut app = open();
    handle_key(&mut app, key(KeyCode::Tab)).await;

    for c in "weath".chars() {
        handle_key(&mut app, key(KeyCode::Char(c))).await;
    }

    // The navigator consumes only Up/Down/Enter/`c`; every other printable
    // character still reaches the search field (FR-017).
    assert_eq!(browser(&app).query, "weath");
    assert_eq!(filtered_ids(&app), ["codex-weather"]);
}

#[tokio::test]
async fn the_panel_keys_never_touch_the_message_input_or_the_queue() {
    let mut app = open();
    app.input = "draft".to_string();

    for code in [
        KeyCode::Tab,
        KeyCode::Down,
        KeyCode::Up,
        KeyCode::Char('c'),
        KeyCode::Enter,
    ] {
        handle_key(&mut app, key(code)).await;
    }

    // The input buffer and the panel state are disjoint: no panel key reaches the
    // message input (FR-007, FR-024).
    assert_eq!(app.input, "draft", "the draft is untouched");
}
