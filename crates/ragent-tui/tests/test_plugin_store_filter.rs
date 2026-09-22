//! Tests for the pure plugin-store browser logic: search filtering, cursor
//! clamp, scrolling, and the empty-result predicate (spec `pluginstores` T-005;
//! FR-008, FR-010, FR-017, FR-022).
//!
//! These drive `PluginStoreBrowser` directly with an in-memory entry list and no
//! renderer, so they are deterministic and touch no store endpoint (FR-022).

use ragent_plugins::{StoreEntry, StoreKind};
use ragent_tui::app::{PluginStoreBrowser, PluginStoreStatus};

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

fn sample() -> Vec<StoreEntry> {
    vec![
        entry(
            "codex-weather",
            "Weather",
            "Live weather lookups",
            &["weather", "http"],
        ),
        entry(
            "codex-todo",
            "Todo List",
            "Manage a todo list",
            &["productivity"],
        ),
        entry(
            "codex-notes",
            "Notes",
            "Plain note taking",
            &["productivity", "TEXT"],
        ),
    ]
}

// ── Search filtering (FR-008) ───────────────────────────────────────────────

#[test]
fn an_empty_query_matches_every_entry() {
    let browser = loaded(sample());
    assert_eq!(browser.filtered.len(), 3);
    assert_eq!(browser.status, PluginStoreStatus::Ready);
    assert!(browser.has_results());
}

#[test]
fn the_match_is_case_insensitive_over_every_field() {
    let mut browser = loaded(sample());

    // id
    browser.set_query("WEATHER".to_string());
    assert_eq!(filtered_ids(&browser), ["codex-weather"]);
    // name
    browser.set_query("todo".to_string());
    assert_eq!(filtered_ids(&browser), ["codex-todo"]);
    // description
    browser.set_query("NOTE".to_string());
    assert_eq!(filtered_ids(&browser), ["codex-notes"]);
    // tag, with the tag stored upper-cased
    browser.set_query("text".to_string());
    assert_eq!(filtered_ids(&browser), ["codex-notes"]);
}

#[test]
fn a_shared_tag_returns_every_matching_entry_in_document_order() {
    let mut browser = loaded(sample());
    browser.set_query("productivity".to_string());
    assert_eq!(filtered_ids(&browser), ["codex-todo", "codex-notes"]);
}

#[test]
fn a_query_that_matches_nothing_yields_no_results_and_keeps_the_entries() {
    let mut browser = loaded(sample());
    browser.set_query("zzz-no-match".to_string());
    assert!(browser.filtered.is_empty());
    assert!(!browser.has_results());
    assert!(browser.selected().is_none());
    // The full entry list is retained so clearing the query restores the set.
    assert_eq!(browser.all.len(), 3);
}

#[test]
fn each_keystroke_re_filters_and_resets_the_cursor_to_the_first_match() {
    let mut browser = loaded(sample());
    browser.set_query("productivity".to_string());
    browser.move_down();
    assert_eq!(browser.cursor, 1);

    // A new character re-derives the filtered set and resets the cursor.
    browser.push_char('x');
    assert_eq!(browser.query, "productivityx");
    assert!(browser.filtered.is_empty());
    assert_eq!(browser.cursor, 0);

    // Removing it restores the survivors and still resets the cursor.
    assert!(browser.backspace());
    assert_eq!(browser.query, "productivity");
    assert_eq!(filtered_ids(&browser), ["codex-todo", "codex-notes"]);
    assert_eq!(browser.cursor, 0);
}

#[test]
fn filtering_never_touches_the_store_and_needs_no_network() {
    // FR-022: only state changes; there is no I/O surface on the browser, so a
    // query cannot install, download, or write anything.
    let mut browser = loaded(sample());
    browser.set_query("todo".to_string());
    browser.push_char('x');
    browser.backspace();
    let _ = browser.selected();
    // Reaching here without a fetch is the assertion: filtering is pure.
    assert_eq!(filtered_ids(&browser), ["codex-todo"]);
}

// ── Cursor clamp (FR-010) ───────────────────────────────────────────────────

#[test]
fn move_up_and_down_clamp_at_both_ends() {
    let mut browser = loaded(sample());
    browser.move_up();
    assert_eq!(browser.cursor, 0, "up at the top clamps");

    for _ in 0..10 {
        browser.move_down();
    }
    assert_eq!(browser.cursor, 2, "down at the bottom clamps");

    for _ in 0..10 {
        browser.move_up();
    }
    assert_eq!(browser.cursor, 0);
}

#[test]
fn move_leaves_the_cursor_at_zero_on_an_empty_set() {
    let mut browser = loaded(Vec::new());
    assert_eq!(browser.status, PluginStoreStatus::Empty);
    browser.move_down();
    browser.move_up();
    assert_eq!(browser.cursor, 0);
    assert!(!browser.has_results());
}

// ── Scrolling (FR-010) ──────────────────────────────────────────────────────

#[test]
fn ensure_visible_keeps_the_cursor_in_a_scrolling_window() {
    let entries: Vec<StoreEntry> = (0..10)
        .map(|i| entry(&format!("p{i}"), &format!("P{i}"), "", &[]))
        .collect();
    let mut browser = loaded(entries);
    let viewport = 3;

    // Walking to the bottom scrolls the window down to keep the cursor visible.
    for _ in 0..9 {
        browser.move_down();
        browser.ensure_visible(viewport);
    }
    assert_eq!(browser.cursor, 9);
    assert_eq!(browser.scroll, 7);
    assert!(browser.cursor >= browser.scroll);
    assert!(browser.cursor < browser.scroll + viewport);

    // Walking back to the top scrolls it up again.
    for _ in 0..9 {
        browser.move_up();
        browser.ensure_visible(viewport);
    }
    assert_eq!(browser.cursor, 0);
    assert_eq!(browser.scroll, 0);
}

#[test]
fn ensure_visible_does_not_scroll_a_window_larger_than_the_set() {
    let mut browser = loaded(sample());
    browser.move_down();
    browser.move_down();
    browser.ensure_visible(50);
    assert_eq!(browser.cursor, 2);
    assert_eq!(browser.scroll, 0, "the whole set fits the viewport");
}

#[test]
fn ensure_visible_parks_the_scroll_for_a_zero_viewport_or_empty_set() {
    let mut browser = loaded(sample());
    browser.move_down();
    browser.ensure_visible(0);
    assert_eq!(browser.scroll, 0, "a zero-row viewport anchors at the top");

    let mut empty = loaded(Vec::new());
    empty.ensure_visible(4);
    assert_eq!(empty.scroll, 0);
}

#[test]
fn ensure_visible_is_idempotent() {
    let entries: Vec<StoreEntry> = (0..8)
        .map(|i| entry(&format!("p{i}"), &format!("P{i}"), "", &[]))
        .collect();
    let mut browser = loaded(entries);
    for _ in 0..8 {
        browser.move_down();
    }
    browser.ensure_visible(3);
    let scroll = browser.scroll;
    browser.ensure_visible(3);
    assert_eq!(browser.scroll, scroll);
}

#[test]
fn a_re_filter_resets_the_scroll() {
    let entries: Vec<StoreEntry> = (0..8)
        .map(|i| entry(&format!("p{i}"), &format!("P{i}"), "", &[]))
        .collect();
    let mut browser = loaded(entries);
    for _ in 0..8 {
        browser.move_down();
    }
    browser.ensure_visible(3);
    assert!(browser.scroll > 0);

    browser.set_query("p1".to_string());
    assert_eq!(browser.cursor, 0);
    assert_eq!(
        browser.scroll, 0,
        "a new filter resets the viewport to the top"
    );
}
