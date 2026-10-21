//! Keyboard-routing tests for the plugin-store category navigator (spec
//! `catnav` T-005; FR-007, FR-008, FR-009, FR-010, FR-015, FR-017, FR-024; spec
//! `pluginstores` FR-052).
//!
//! These drive the real `handle_key` path with an in-memory entry list, so they
//! are deterministic and contact no store endpoint. They pin the three-pane focus
//! model (FR-052): `Tab`/`BackTab` cycle focus forward/backward through the result
//! list, the category navigator, and the marketplace navigator, `Up`/`Down` route to
//! the focused pane (applying the highlighted value over a navigator), `ENTER` over a
//! navigator applies the value and focuses the result list without installing, and `c`
//! clears the focused navigator's filter.

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
        marketplace: String::new(),
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

/// Press `Down` over the focused category navigator until `label` is active.
///
/// Row `0` is `ALL`, row `1` is the `Installed` pseudo-category (`pluginstores`
/// FR-054), and the derived categories start at row `2`, so a test that wants a
/// named category presses Down to it rather than counting the fixed offset.
async fn down_to(app: &mut App, label: &str) {
    for _ in 0..app
        .plugin_store
        .as_ref()
        .expect("panel open")
        .category_row_count()
    {
        if browser(app).active_category().eq_ignore_ascii_case(label) {
            return;
        }
        handle_key(app, key(KeyCode::Down)).await;
    }
    panic!("never reached category `{label}`");
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
async fn tab_cycles_focus_forward_through_the_three_panes() {
    let mut app = open();
    assert!(
        !app.plugin_store_nav_focused() && !app.plugin_store_marketplace_focused(),
        "the result list starts focused"
    );

    handle_key(&mut app, key(KeyCode::Tab)).await;
    assert!(
        app.plugin_store_nav_focused(),
        "Tab focuses the category pane"
    );
    assert!(!app.plugin_store_marketplace_focused());

    handle_key(&mut app, key(KeyCode::Tab)).await;
    assert!(
        app.plugin_store_marketplace_focused(),
        "Tab advances to the marketplace pane (FR-052)"
    );
    assert!(!app.plugin_store_nav_focused());

    handle_key(&mut app, key(KeyCode::Tab)).await;
    assert!(
        !app.plugin_store_nav_focused() && !app.plugin_store_marketplace_focused(),
        "Tab wraps back to the result list"
    );

    // BackTab is the reverse cycler: from the result list it lands on the
    // marketplace pane (FR-052).
    handle_key(&mut app, key(KeyCode::BackTab)).await;
    assert!(
        app.plugin_store_marketplace_focused(),
        "BackTab cycles backwards to the marketplace pane"
    );
}

#[tokio::test]
async fn while_the_navigator_is_focused_up_and_down_move_the_category_cursor() {
    let mut app = open();
    handle_key(&mut app, key(KeyCode::Tab)).await;

    handle_key(&mut app, key(KeyCode::Down)).await;

    // `Down` over the navigator moves the category cursor, not the result cursor,
    // and applies the highlighted row on the same keypress (FR-007, FR-017). The
    // first row below `ALL` is the `Installed` pseudo-category (FR-054).
    assert_eq!(browser(&app).category_cursor_row(), 1);
    assert_eq!(browser(&app).active_category(), "Installed");
    assert_eq!(browser(&app).cursor, 0, "the result cursor is not moved");

    down_to(&mut app, "Web").await;
    assert_eq!(browser(&app).active_category(), "Web");

    handle_key(&mut app, key(KeyCode::Up)).await;
    assert_eq!(
        browser(&app).active_category(),
        "Utility",
        "Up steps back (FR-008)"
    );
}

// -- Applying a category narrows the result set (FR-007, FR-008) --------------

#[tokio::test]
async fn moving_the_category_cursor_applies_the_category_and_narrows_results() {
    let mut app = open();
    handle_key(&mut app, key(KeyCode::Tab)).await;

    down_to(&mut app, "Utility").await;
    // `Utility` folds the `Utility`/`utility` spellings into one exact,
    // case-insensitive match, so both survive (FR-007) in alphabetical order.
    assert_eq!(filtered_ids(&app), ["codex-shell", "codex-time"]);

    down_to(&mut app, "Web").await;
    assert_eq!(
        filtered_ids(&app),
        ["codex-weather"],
        "`Web` selects one row"
    );

    // Up twice steps back through `Utility` to the `Installed` pseudo-category,
    // which lists only the plugins already in the store scan (none here), and then
    // `c` restores `ALL` (FR-054, FR-015).
    handle_key(&mut app, key(KeyCode::Up)).await;
    handle_key(&mut app, key(KeyCode::Up)).await;
    assert_eq!(browser(&app).active_category(), "Installed");
    assert!(filtered_ids(&app).is_empty(), "nothing is installed here");
    handle_key(&mut app, key(KeyCode::Char('c'))).await;
    // Back to `ALL`: every entry, including the uncategorised one, returns
    // alphabetically by id (FR-050).
    assert_eq!(browser(&app).active_category(), "ALL");
    assert_eq!(
        filtered_ids(&app),
        ["codex-shell", "codex-time", "codex-uncat", "codex-weather"]
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
    for _ in 0..8 {
        handle_key(&mut app, key(KeyCode::Down)).await;
    }
    let last = browser(&app).category_row_count() - 1;
    assert_eq!(browser(&app).category_cursor_row(), last);
    assert_eq!(browser(&app).active_category(), "Web");
}

// -- ENTER over the navigator applies and focuses the result list (FR-010, FR-024) --

#[tokio::test]
async fn enter_over_the_navigator_applies_the_category_and_focuses_the_result_list() {
    let mut app = open();
    handle_key(&mut app, key(KeyCode::Tab)).await;
    down_to(&mut app, "Utility").await;

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
    down_to(&mut app, "Utility").await;
    let row = browser(&app).category_cursor_row();
    handle_key(&mut app, key(KeyCode::Enter)).await;

    // Focus is now the result list, so `Down` moves the result cursor and leaves
    // the category (FR-009, FR-010).
    handle_key(&mut app, key(KeyCode::Down)).await;

    assert_eq!(
        browser(&app).category_cursor_row(),
        row,
        "category unchanged"
    );
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
        Some("installing codex-shell..."),
        "ENTER over the result list runs the normal install on the first row"
    );
}

// -- Clear-category key (FR-015) ----------------------------------------------

#[tokio::test]
async fn the_clear_key_sets_the_category_to_all_and_restores_the_full_set() {
    let mut app = open();
    handle_key(&mut app, key(KeyCode::Tab)).await;
    down_to(&mut app, "Utility").await;
    assert_eq!(filtered_ids(&app), ["codex-shell", "codex-time"]);

    handle_key(&mut app, key(KeyCode::Char('c'))).await;

    // `c` clears the active category to `ALL` and re-derives the visible set; it
    // is consumed as a command and never typed into the query (FR-015, FR-017).
    assert_eq!(browser(&app).active_category(), "ALL");
    assert_eq!(browser(&app).category_cursor_row(), 0);
    assert_eq!(
        filtered_ids(&app),
        ["codex-shell", "codex-time", "codex-uncat", "codex-weather"]
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
async fn the_clear_key_clears_the_category_while_the_result_list_is_focused() {
    let mut app = open();
    handle_key(&mut app, key(KeyCode::Tab)).await;
    down_to(&mut app, "Utility").await;
    // Two more Tabs return focus to the result list (category -> marketplace ->
    // results), so the clear key must fall back to clearing the category (FR-015).
    handle_key(&mut app, key(KeyCode::Tab)).await;
    handle_key(&mut app, key(KeyCode::Tab)).await;
    assert!(
        !app.plugin_store_nav_focused() && !app.plugin_store_marketplace_focused(),
        "focus is on the result list"
    );

    handle_key(&mut app, key(KeyCode::Char('c'))).await;

    assert_eq!(browser(&app).active_category(), "ALL");
}

#[tokio::test]
async fn the_clear_key_clears_the_marketplace_when_that_pane_is_focused() {
    let mut app = open();
    // Give the entries marketplaces so the marketplace navigator has a real filter
    // to clear (`open()` leaves them blank, an ALL-only navigator).
    let mut entries = vec![
        entry("codex-weather", Some("Web")),
        entry("codex-time", Some("Utility")),
    ];
    entries[0].marketplace = "codex".to_string();
    entries[1].marketplace = "community".to_string();
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_entries(entries);

    // Two Tabs move focus onto the marketplace pane (Results -> Categories ->
    // Marketplaces) (FR-052).
    handle_key(&mut app, key(KeyCode::Tab)).await;
    handle_key(&mut app, key(KeyCode::Tab)).await;
    assert!(app.plugin_store_marketplace_focused());

    // Down applies the first marketplace row on the move.
    handle_key(&mut app, key(KeyCode::Down)).await;
    assert_eq!(browser(&app).active_marketplace(), "codex");
    assert_eq!(filtered_ids(&app), ["codex-weather"]);

    handle_key(&mut app, key(KeyCode::Char('c'))).await;

    // `c` clears the focused navigator's filter, so the marketplace (not the
    // category) goes back to `ALL` (FR-052).
    assert_eq!(browser(&app).active_marketplace(), "ALL");
    assert_eq!(browser(&app).marketplace_cursor_row(), 0);
    assert_eq!(
        browser(&app).active_category(),
        "ALL",
        "the category filter is untouched"
    );
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
