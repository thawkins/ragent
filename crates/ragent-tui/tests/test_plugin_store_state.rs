//! Unit tests for the plugin-store browser's filtering, cursor movement, query
//! editing, and fetch-status state transitions (spec `pluginstores` T-012;
//! FR-008, FR-009, FR-010, FR-012).
//!
//! These drive the pure `PluginStoreBrowser` logic and the `App` editing
//! wrapper directly with in-memory entries, so they are deterministic and issue
//! no network request. They complement `test_plugin_store_filter.rs` (T-005) and
//! `test_plugin_store_browser.rs` (T-004) by pinning the editing and
//! state-transition semantics those suites do not assert.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use ragent_plugins::{StoreEntry, StoreKind};
use ragent_tui::app::{PluginStoreBrowser, PluginStoreStatus, QueuedInput};
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

/// A browser with `entries` loaded and no prefill.
fn loaded(entries: Vec<StoreEntry>) -> PluginStoreBrowser {
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_entries(entries);
    browser
}

/// The ids behind the current filtered set, in order.
fn filtered_ids(browser: &PluginStoreBrowser) -> Vec<&str> {
    browser
        .filtered
        .iter()
        .map(|index| browser.all[*index].id.as_str())
        .collect()
}

// ── Filtering semantics (FR-008) ─────────────────────────────────────────────

#[test]
fn a_query_matches_a_substring_anywhere_in_a_field() {
    let mut browser = loaded(vec![entry("codex-weather", "Weather", "Sky", &[])]);

    // "eath" sits inside "weather"/"Weather": the match is a substring, not a
    // prefix or whole-word test (FR-008).
    browser.set_query("eath".to_string());

    assert_eq!(filtered_ids(&browser), ["codex-weather"]);
}

#[test]
fn one_query_term_can_match_different_fields_on_different_entries() {
    let mut browser = loaded(vec![
        entry("alpha", "Alpha", "plain description", &[]),
        entry("beta", "Beta", "", &["alpha-tag"]),
    ]);

    // "alpha" survives one entry via its id and another via a tag, so the union
    // (in document order) is both entries.
    browser.set_query("alpha".to_string());

    assert_eq!(filtered_ids(&browser), ["alpha", "beta"]);
}

#[test]
fn clearing_a_no_match_query_restores_the_full_set_at_the_top() {
    let mut browser = loaded(vec![entry("a", "A", "", &[]), entry("b", "B", "", &[])]);

    browser.set_query("zzz".to_string());
    assert!(browser.filtered.is_empty());

    browser.set_query(String::new());

    assert_eq!(filtered_ids(&browser), ["a", "b"]);
    assert_eq!(browser.cursor, 0, "the cursor resets to the first survivor");
    assert_eq!(browser.scroll, 0, "the viewport returns to the top");
}

// ── Cursor movement (FR-010) ─────────────────────────────────────────────────

#[test]
fn the_cursor_clamps_within_the_filtered_subset_not_the_full_list() {
    let mut browser = loaded(vec![
        entry("keep-one", "One", "", &[]),
        entry("drop", "Drop", "", &[]),
        entry("keep-two", "Two", "", &[]),
    ]);
    browser.set_query("keep".to_string());
    assert_eq!(browser.filtered.len(), 2);

    // Down clamps at the last *surviving* row, not the last row of `all`.
    for _ in 0..5 {
        browser.move_down();
    }
    assert_eq!(browser.cursor, 1);

    for _ in 0..5 {
        browser.move_up();
    }
    assert_eq!(browser.cursor, 0);
}

#[test]
fn a_single_surviving_result_pins_the_cursor_at_zero() {
    let mut browser = loaded(vec![
        entry("only", "Only", "", &[]),
        entry("other", "Other", "", &[]),
    ]);

    browser.set_query("only".to_string());
    browser.move_down();
    assert_eq!(browser.cursor, 0, "one result cannot move down");
    browser.move_up();
    assert_eq!(browser.cursor, 0, "one result cannot move up");
}

#[test]
fn moving_down_then_up_returns_to_the_original_row() {
    let mut browser = loaded(vec![
        entry("a", "A", "", &[]),
        entry("b", "B", "", &[]),
        entry("c", "C", "", &[]),
    ]);

    browser.move_down();
    browser.move_down();
    assert_eq!(browser.selected().expect("selected").id, "c");

    browser.move_up();
    assert_eq!(browser.selected().expect("selected").id, "b");
    browser.move_up();
    assert_eq!(browser.selected().expect("selected").id, "a");
}

// ── Query editing (FR-009) ───────────────────────────────────────────────────

#[test]
fn set_query_replaces_rather_than_appends_to_the_query() {
    let mut browser = loaded(vec![entry("alpha", "Alpha", "", &[])]);

    browser.set_query("alp".to_string());
    browser.set_query("pha".to_string());

    assert_eq!(browser.query, "pha", "each set replaces the whole query");
    assert_eq!(filtered_ids(&browser), ["alpha"]);
}

#[test]
fn push_char_appends_exactly_one_character_and_refilters() {
    let mut browser = loaded(vec![entry("ab", "AB", "", &[])]);

    browser.push_char('a');
    browser.push_char('b');

    assert_eq!(browser.query, "ab");
    assert_eq!(filtered_ids(&browser), ["ab"]);
    assert_eq!(browser.cursor, 0);
}

#[test]
fn backspace_reports_whether_a_character_was_removed() {
    let mut browser = loaded(vec![entry("ab", "AB", "", &[])]);

    browser.set_query("ab".to_string());
    assert!(browser.backspace(), "removing 'b' succeeds");
    assert_eq!(browser.query, "a");
    assert!(browser.backspace(), "removing 'a' succeeds");
    assert!(browser.query.is_empty());
    assert!(!browser.backspace(), "an empty query removes nothing");
    assert!(browser.query.is_empty());
}

#[test]
fn backspace_on_an_empty_query_does_not_disturb_the_result_set() {
    let mut browser = loaded(vec![entry("a", "A", "", &[]), entry("b", "B", "", &[])]);

    assert!(!browser.backspace());

    assert_eq!(filtered_ids(&browser), ["a", "b"]);
    assert_eq!(browser.cursor, 0);
}

#[test]
fn query_editing_removes_one_whole_unicode_character() {
    let mut browser = loaded(vec![entry("a", "A", "", &[])]);

    // Both characters are multi-byte, so this proves backspace trims a whole
    // scalar value rather than a byte (FR-009, no split code point).
    browser.push_char('ß');
    browser.push_char('é');
    assert_eq!(browser.query, "ßé");

    assert!(browser.backspace());
    assert_eq!(browser.query, "ß");
    assert!(browser.backspace());
    assert!(browser.query.is_empty());
}

#[test]
fn edit_or_close_edits_a_non_empty_query_and_only_dismisses_on_empty() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    handle_key(&mut app, key(KeyCode::Char('a')));

    assert!(
        app.plugin_store_edit_or_close(),
        "edits the non-empty query"
    );
    assert!(app.plugin_store.is_some(), "panel stays open (FR-009)");
    assert!(app.plugin_store.as_ref().expect("panel").query.is_empty());

    assert!(
        !app.plugin_store_edit_or_close(),
        "an empty query dismisses the panel"
    );
    assert!(app.plugin_store.is_none(), "panel dismissed (FR-012)");
}

#[test]
fn edit_or_close_is_a_noop_without_a_panel() {
    let mut app = support::make_app();

    assert!(!app.plugin_store_edit_or_close());
    assert!(app.plugin_store.is_none());
}

// ── State transitions (FR-008, FR-012) ───────────────────────────────────────

#[test]
fn a_refresh_replaces_the_entry_list_rather_than_appending() {
    let mut browser = loaded(vec![
        entry("old-1", "Old", "", &[]),
        entry("old-2", "Old", "", &[]),
    ]);

    browser.set_entries(vec![entry("new-1", "New", "", &[])]);

    assert_eq!(browser.all.len(), 1, "the previous list is replaced");
    assert_eq!(filtered_ids(&browser), ["new-1"]);
}

#[test]
fn a_successful_fetch_clears_a_prior_failure() {
    let mut browser = PluginStoreBrowser::new(StoreKind::Claude, "", false);
    browser.set_failed("HTTP 503".to_string());
    assert!(matches!(browser.status, PluginStoreStatus::Failed(_)));

    browser.set_entries(vec![entry("claude-x", "X", "", &[])]);

    assert_eq!(browser.status, PluginStoreStatus::Ready);
    assert_eq!(browser.filtered.len(), 1);
}

#[test]
fn a_fetch_failure_after_load_transitions_to_the_error_state() {
    let mut browser = loaded(vec![entry("a", "A", "", &[])]);
    assert_eq!(browser.status, PluginStoreStatus::Ready);

    browser.set_failed("store fetch timed out".to_string());

    assert_eq!(
        browser.status,
        PluginStoreStatus::Failed("store fetch timed out".to_string())
    );
}

#[test]
fn an_index_that_reloads_empty_moves_through_ready_to_empty() {
    let mut browser = loaded(vec![entry("a", "A", "", &[])]);
    assert_eq!(browser.status, PluginStoreStatus::Ready);

    browser.set_entries(Vec::new());

    assert_eq!(browser.status, PluginStoreStatus::Empty);
    assert!(!browser.has_results());
}

#[test]
fn has_results_tracks_rows_across_every_status() {
    // Loading: no rows have arrived yet.
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    assert_eq!(browser.status, PluginStoreStatus::Loading);
    assert!(!browser.has_results());

    // Ready with rows.
    browser.set_entries(vec![entry("a", "A", "", &[])]);
    assert!(browser.has_results());

    // Ready but filtered to nothing.
    browser.set_query("zzz".to_string());
    assert!(!browser.has_results());

    // Failed is reported through the status, not a result row.
    browser.set_failed("boom".to_string());
    assert!(matches!(browser.status, PluginStoreStatus::Failed(_)));
    assert!(!browser.has_results());
}

// ── Dismissal leaves no side effects (FR-012) ────────────────────────────────

#[test]
fn closing_a_loaded_panel_discards_the_browser_and_requests_a_repaint() {
    let mut app = support::make_app();
    app.input = "draft".to_string();
    app.input_cursor = 2;
    app.input_queue.push_back(QueuedInput {
        text: "queued".to_string(),
        image_paths: Vec::new(),
    });
    app.open_plugin_store(StoreKind::Codex, "wea", false);
    app.needs_redraw = false;

    app.close_plugin_store();

    assert!(
        app.plugin_store.is_none(),
        "the browser is dropped (FR-012)"
    );
    assert!(app.needs_redraw, "the restored screen repaints");
    // Nothing else changes (FR-012).
    assert_eq!(app.input, "draft");
    assert_eq!(app.input_cursor, 2);
    assert_eq!(app.input_queue.len(), 1);
}

#[test]
fn closing_twice_is_idempotent_and_leaves_nothing_behind() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    app.close_plugin_store();
    app.needs_redraw = false;

    app.close_plugin_store();

    assert!(app.plugin_store.is_none());
    assert!(!app.needs_redraw, "a second close has nothing to repaint");
}
