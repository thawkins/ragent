//! Tests for the plugin-store browser state, launch/close API, key routing,
//! and input lock (spec `pluginstores` T-004; FR-002, FR-007, FR-009, FR-012,
//! FR-015, FR-020).
//!
//! These tests are deterministic and offline: they construct the browser
//! directly and feed it an in-memory entry list, so no store endpoint is
//! contacted (the off-loop fetch and its fixture seam are later tasks).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ragent_plugins::{StoreEntry, StoreKind};
use ragent_tui::App;
use ragent_tui::app::{PluginStoreBrowser, PluginStoreStatus};
use ragent_tui::input::handle_key;

#[path = "support/mod.rs"]
mod support;

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn entry(id: &str, name: &str, description: &str, tags: &[&str]) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: name.to_string(),
        version: "1.0.0".to_string(),
        source: format!("https://example.org/{id}.zip"),
        description: description.to_string(),
        dialect: None,
        tags: tags.iter().map(|t| (*t).to_string()).collect(),
        homepage: None,
    }
}

/// An app with the panel open for `kind` and the given entries loaded.
fn open_with_entries(kind: StoreKind, entries: Vec<StoreEntry>) -> App {
    let mut app = support::make_app();
    app.open_plugin_store(kind, "", false);
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_entries(entries);
    app
}

fn query(app: &App) -> &str {
    app.plugin_store
        .as_ref()
        .expect("panel open")
        .query
        .as_str()
}

/// An installed-id set from a slice of ids.
fn installed(ids: &[&str]) -> std::collections::BTreeSet<String> {
    ids.iter().map(|id| (*id).to_string()).collect()
}

// ── Launch and close API (FR-007, FR-012, FR-020) ───────────────────────────

#[test]
fn open_sets_a_loading_browser_for_the_requested_store() {
    let mut app = support::make_app();
    assert!(app.plugin_store.is_none(), "no panel before open");

    app.open_plugin_store(StoreKind::Codex, "", false);

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.kind, StoreKind::Codex);
    assert_eq!(browser.status, PluginStoreStatus::Loading);
    assert!(browser.query.is_empty());
    assert!(browser.all.is_empty());
    assert_eq!(browser.cursor, 0);
    assert!(!browser.refresh);
    assert!(app.needs_redraw, "opening the panel requests a repaint");
}

#[test]
fn both_stores_share_one_browser_parameterised_by_kind() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    assert_eq!(
        app.plugin_store.as_ref().expect("panel").kind,
        StoreKind::Codex
    );
    app.open_plugin_store(StoreKind::Claude, "", false);
    assert_eq!(
        app.plugin_store.as_ref().expect("panel").kind,
        StoreKind::Claude
    );
}

#[test]
fn a_prefilled_query_is_applied_on_open() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "weather", false);

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.query, "weather");
    // No entries have arrived yet, so the filter set is empty; the query is
    // already recorded and will constrain the set once the fetch lands (FR-020).
    assert!(browser.filtered.is_empty());
}

#[test]
fn a_refresh_launch_records_the_request() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Claude, "", true);
    assert!(app.plugin_store.as_ref().expect("panel open").refresh);
}

#[test]
fn close_clears_the_browser_and_is_idempotent() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "x", false);
    app.needs_redraw = false;

    app.close_plugin_store();
    assert!(app.plugin_store.is_none());
    assert!(app.needs_redraw, "closing the panel requests a repaint");

    // A second close is a harmless no-op (and no spurious repaint).
    app.needs_redraw = false;
    app.close_plugin_store();
    assert!(app.plugin_store.is_none());
    assert!(!app.needs_redraw);
}

#[test]
fn opening_the_panel_leaves_a_running_turn_and_the_queue_untouched() {
    let mut app = support::make_app();
    app.is_processing = true;
    app.input = "a live draft".to_string();
    app.input_queue.push_back(ragent_tui::app::QueuedInput {
        text: "queued one".to_string(),
        image_paths: Vec::new(),
    });

    app.open_plugin_store(StoreKind::Codex, "", false);

    assert!(app.is_processing, "a running turn is undisturbed (FR-007)");
    assert_eq!(app.input, "a live draft");
    assert_eq!(app.input_queue.len(), 1);
}

// ── Key routing while the panel is open (FR-008, FR-009, FR-010, FR-015) ────

#[tokio::test]
async fn a_printable_key_types_into_the_panel_query_not_the_input_buffer() {
    let mut app = support::make_app();
    app.input = "draft".to_string();
    app.open_plugin_store(StoreKind::Codex, "", false);

    let action = handle_key(&mut app, key(KeyCode::Char('w'))).await;

    assert!(action.is_none(), "the panel consumes the key");
    assert_eq!(query(&app), "w");
    // The message input buffer is untouched while the panel owns the keyboard.
    assert_eq!(app.input, "draft");
}

#[tokio::test]
async fn typing_filters_the_loaded_entries_case_insensitively() {
    let app_entries = vec![
        entry("codex-weather", "Weather", "Weather lookups", &["http"]),
        entry("codex-time", "Time", "Clock tools", &["clock"]),
    ];
    let mut app = open_with_entries(StoreKind::Codex, app_entries);

    for c in "WEATH".chars() {
        handle_key(&mut app, key(KeyCode::Char(c))).await;
    }

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.query, "WEATH");
    assert_eq!(browser.filtered.len(), 1, "only the weather entry survives");
    assert_eq!(browser.all[browser.filtered[0]].id, "codex-weather");
    assert_eq!(browser.cursor, 0, "the cursor resets to the first survivor");
}

#[tokio::test]
async fn backspace_edits_the_query_and_keeps_the_panel_open() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    handle_key(&mut app, key(KeyCode::Char('a'))).await;
    handle_key(&mut app, key(KeyCode::Char('b'))).await;

    let action = handle_key(&mut app, key(KeyCode::Backspace)).await;

    assert!(action.is_none());
    assert!(app.plugin_store.is_some(), "the panel stays open");
    assert_eq!(query(&app), "a");
}

#[tokio::test]
async fn esc_on_a_non_empty_query_edits_rather_than_closing() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    handle_key(&mut app, key(KeyCode::Char('x'))).await;

    let action = handle_key(&mut app, key(KeyCode::Esc)).await;

    assert!(action.is_none());
    assert!(app.plugin_store.is_some(), "the panel stays open (FR-009)");
    assert!(query(&app).is_empty());
}

#[tokio::test]
async fn esc_on_an_empty_query_dismisses_the_panel_without_side_effects() {
    let mut app = support::make_app();
    app.input = "draft".to_string();
    app.input_cursor = 5;
    app.input_queue.push_back(ragent_tui::app::QueuedInput {
        text: "queued".to_string(),
        image_paths: Vec::new(),
    });
    app.open_plugin_store(StoreKind::Codex, "", false);

    let action = handle_key(&mut app, key(KeyCode::Esc)).await;

    assert!(action.is_none());
    assert!(
        app.plugin_store.is_none(),
        "the panel is dismissed (FR-012)"
    );
    // Dismissal disturbs nothing else (FR-012).
    assert_eq!(app.input, "draft");
    assert_eq!(app.input_cursor, 5);
    assert_eq!(app.input_queue.len(), 1);
}

#[tokio::test]
async fn backspace_on_an_empty_query_dismisses_the_panel() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);

    handle_key(&mut app, key(KeyCode::Backspace)).await;

    assert!(
        app.plugin_store.is_none(),
        "the panel is dismissed (FR-009)"
    );
}

#[tokio::test]
async fn up_and_down_move_the_block_cursor_within_the_filtered_set() {
    let entries = vec![
        entry("a", "A", "", &[]),
        entry("b", "B", "", &[]),
        entry("c", "C", "", &[]),
    ];
    let mut app = open_with_entries(StoreKind::Codex, entries);

    handle_key(&mut app, key(KeyCode::Down)).await;
    handle_key(&mut app, key(KeyCode::Down)).await;
    assert_eq!(app.plugin_store.as_ref().expect("panel").cursor, 2);

    // Down at the bottom clamps.
    handle_key(&mut app, key(KeyCode::Down)).await;
    assert_eq!(app.plugin_store.as_ref().expect("panel").cursor, 2);

    handle_key(&mut app, key(KeyCode::Up)).await;
    assert_eq!(app.plugin_store.as_ref().expect("panel").cursor, 1);

    // Up at the top clamps.
    handle_key(&mut app, key(KeyCode::Up)).await;
    handle_key(&mut app, key(KeyCode::Up)).await;
    assert_eq!(app.plugin_store.as_ref().expect("panel").cursor, 0);
}

#[tokio::test]
async fn an_unhandled_key_is_swallowed_and_changes_nothing() {
    let entries = vec![entry("a", "A", "", &[])];
    let mut app = open_with_entries(StoreKind::Codex, entries);
    app.input = "draft".to_string();

    let action = handle_key(&mut app, key(KeyCode::Tab)).await;

    assert!(action.is_none(), "the panel swallows every other key");
    assert!(app.plugin_store.is_some());
    assert!(query(&app).is_empty());
    assert_eq!(app.input, "draft");
}

// ── Status transitions (FR-013, FR-016, FR-017) ─────────────────────────────

#[test]
fn a_loaded_entry_list_moves_the_browser_to_ready() {
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    assert_eq!(browser.status, PluginStoreStatus::Loading);

    browser.set_entries(vec![entry("a", "A", "", &[])]);

    assert_eq!(browser.status, PluginStoreStatus::Ready);
    assert_eq!(browser.filtered.len(), 1);
}

#[test]
fn an_empty_entry_list_moves_the_browser_to_empty() {
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_entries(Vec::new());
    assert_eq!(browser.status, PluginStoreStatus::Empty);
}

#[test]
fn a_failed_fetch_records_the_detail_for_inline_rendering() {
    let mut browser = PluginStoreBrowser::new(StoreKind::Claude, "", false);
    browser.set_failed("store fetch returned HTTP 503".to_string());
    assert_eq!(
        browser.status,
        PluginStoreStatus::Failed("store fetch returned HTTP 503".to_string())
    );
}

#[test]
fn a_prefilled_query_constrains_entries_arriving_after_open() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "time", false);
    app.plugin_store.as_mut().expect("panel").set_entries(vec![
        entry("codex-weather", "Weather", "Sky", &[]),
        entry("codex-time", "Time", "Clock", &[]),
    ]);

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.filtered.len(), 1);
    assert_eq!(browser.all[browser.filtered[0]].id, "codex-time");
    assert_eq!(browser.status, PluginStoreStatus::Ready);
}

#[test]
fn selected_returns_the_highlighted_entry_and_none_when_empty() {
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    assert!(browser.selected().is_none());

    browser.set_entries(vec![entry("a", "A", "", &[]), entry("b", "B", "", &[])]);
    assert_eq!(browser.selected().expect("selected").id, "a");

    browser.move_down();
    assert_eq!(browser.selected().expect("selected").id, "b");
}

// ── Installed set and ENTER routing (T-007; FR-005, FR-014, FR-015) ───────────

#[test]
fn a_new_browser_starts_with_an_empty_installed_set() {
    let browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    assert!(browser.installed.is_empty());
    assert!(!browser.is_installed("codex-weather"));
}

#[test]
fn set_installed_replaces_the_set_and_drives_membership() {
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_installed(installed(&["codex-weather"]));

    assert!(browser.is_installed("codex-weather"));
    assert!(!browser.is_installed("codex-time"));
}

#[test]
fn opening_the_panel_derives_the_installed_set_from_the_store() {
    // T-007/FR-005: opening derives the set from the store scan (A5). This repo
    // ships no `codex-*` plugin in the project store, so a store entry carrying
    // that id is reported not-installed.
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    let browser = app.plugin_store.as_ref().expect("panel open");
    assert!(!browser.is_installed("codex-weather"));
}

#[test]
fn refresh_installed_set_is_a_noop_without_a_panel() {
    let mut app = support::make_app();
    assert!(app.plugin_store.is_none());
    app.refresh_plugin_store_installed();
    assert!(app.plugin_store.is_none());
}

#[tokio::test]
async fn enter_on_a_not_installed_result_starts_an_install() {
    // T-009/FR-011: ENTER spawns the off-loop install; the panel shows a
    // progress notice until the worker's result is polled. The install pipeline
    // itself is covered by `test_plugin_store_install.rs`.
    let entries = vec![entry("codex-weather", "Weather", "", &[])];
    let mut app = open_with_entries(StoreKind::Codex, entries);

    let action = handle_key(&mut app, key(KeyCode::Enter)).await;

    assert!(action.is_none(), "ENTER is routed to the panel");
    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(
        browser.last_install.as_deref(),
        Some("installing codex-weather..."),
        "progress is shown while the install runs (FR-011)"
    );
}

#[tokio::test]
async fn enter_on_an_installed_result_reports_already_installed() {
    // FR-014: a re-install on an installed id is refused with a notice.
    let entries = vec![entry("codex-weather", "Weather", "", &[])];
    let mut app = open_with_entries(StoreKind::Codex, entries);
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_installed(installed(&["codex-weather"]));

    handle_key(&mut app, key(KeyCode::Enter)).await;

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(
        browser.last_install.as_deref(),
        Some("plugin codex-weather is already installed")
    );
}

#[tokio::test]
async fn enter_with_no_result_highlighted_records_a_neutral_notice() {
    // An empty (still-loading or filtered-to-nothing) result set must not panic.
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);

    handle_key(&mut app, key(KeyCode::Enter)).await;

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(
        browser.last_install.as_deref(),
        Some("no result highlighted")
    );
}

#[tokio::test]
async fn enter_on_the_panel_never_reaches_the_message_input() {
    // FR-015: the panel owns the keyboard, so ENTER cannot submit the draft.
    let entries = vec![entry("a", "A", "", &[])];
    let mut app = open_with_entries(StoreKind::Codex, entries);
    app.input = "draft".to_string();

    let action = handle_key(&mut app, key(KeyCode::Enter)).await;

    assert!(action.is_none());
    assert_eq!(app.input, "draft", "the draft is untouched");
}
