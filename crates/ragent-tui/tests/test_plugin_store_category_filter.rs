//! Pure unit tests for the plugin-store category filter (spec `catnav` T-007;
//! FR-007, FR-008, FR-014, FR-015, FR-018).
//!
//! These drive [`PluginStoreBrowser`] directly with an in-memory entry list, no
//! renderer, and no I/O, so they are deterministic and contact no store endpoint.
//! They pin the category-filter model behind the navigator: the derived distinct
//! category set, the query-plus-category composition, the `Up`/`Down` cursor move
//! applying on the same call, clear-to-`ALL`, the result cursor reset on a category
//! change, the `ALL`-only navigator for a category-less index, and the drop of a
//! category the new index no longer declares.

use ragent_plugins::{StoreEntry, StoreKind};
use ragent_tui::app::{PluginStoreBrowser, PluginStoreStatus};

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

/// The derived category rows (without the implicit `ALL`), in navigator order.
fn category_rows(browser: &PluginStoreBrowser) -> Vec<&str> {
    browser
        .categories
        .categories()
        .iter()
        .map(String::as_str)
        .collect()
}

/// Four entries across two categories (`Web` and the two `Utility`/`utility`
/// spellings) plus one uncategorised entry. The distinct rows fold the two
/// `Utility` spellings into one and sort as `ALL`, `Utility`, `Web`.
fn sample() -> Vec<StoreEntry> {
    vec![
        entry("alpha", Some("Web")),
        entry("beta", Some("Utility")),
        entry("gamma", Some("utility")),
        entry("delta", None),
    ]
}

/// `count` entries each in its own distinct category, so the navigator has
/// `count + 1` rows and the wheel/keyboard has something to scroll.
fn many(count: usize) -> Vec<StoreEntry> {
    (0..count)
        .map(|i| {
            let id = format!("codex-{i:02}");
            let category = format!("Cat{i:02}");
            entry(&id, Some(&category))
        })
        .collect()
}

// -- Derived category set (FR-001, A2, A5) ------------------------------------

#[test]
fn the_category_set_is_distinct_case_insensitive_and_sorted_with_all_implied() {
    let browser = loaded(sample());

    // `Utility` and `utility` fold into one row keeping the first spelling; the
    // set is sorted case-insensitively; `ALL` is implicit rather than listed.
    assert_eq!(category_rows(&browser), ["Utility", "Web"]);
    assert_eq!(browser.category_row_count(), 3, "ALL + Utility + Web");
    assert_eq!(browser.category_row_label(0), "ALL");
    assert_eq!(browser.active_category(), "ALL");
}

// -- Query and category compose (FR-003, A3) ----------------------------------

#[test]
fn the_query_and_the_category_compose() {
    let mut browser = loaded(sample());
    assert_eq!(filtered_ids(&browser), ["alpha", "beta", "gamma", "delta"]);

    assert!(browser.category_move_down(), "move onto `Utility`");
    assert_eq!(browser.active_category(), "Utility");
    assert_eq!(filtered_ids(&browser), ["beta", "gamma"]);

    // The query narrows inside the active category, not across it.
    browser.push_char('b');
    assert_eq!(filtered_ids(&browser), ["beta"]);

    // Removing the query restores the whole category, still not the full index.
    assert!(browser.backspace());
    assert_eq!(filtered_ids(&browser), ["beta", "gamma"]);

    // Clearing the category restores the full index.
    browser.category_clear();
    assert_eq!(filtered_ids(&browser), ["alpha", "beta", "gamma", "delta"]);
}

#[test]
fn all_matches_every_entry_including_the_uncategorised_one() {
    let browser = loaded(sample());

    // The uncategorised `delta` is shown under `ALL` only (A5, FR-003).
    assert_eq!(browser.active_category(), "ALL");
    assert!(filtered_ids(&browser).contains(&"delta"));
}

#[test]
fn a_category_with_no_query_match_yields_an_empty_set() {
    let mut browser = loaded(sample());
    assert!(browser.category_move_down()); // `Utility`
    for c in "alpha".chars() {
        browser.push_char(c); // only `alpha` (in `Web`) matches `alpha`
    }

    assert!(
        filtered_ids(&browser).is_empty(),
        "the category still excludes the query's only match"
    );
    // The full index is untouched, so `ALL` would still surface it.
    assert_eq!(browser.all.len(), 4);
}

// -- Up/Down move the category cursor and apply on move (FR-007, FR-008) ----

#[test]
fn move_down_applies_the_new_category_on_the_same_call() {
    let mut browser = loaded(sample());

    assert!(browser.category_move_down());
    assert_eq!(browser.category_cursor_row(), 1);
    assert_eq!(browser.active_category(), "Utility");
    assert_eq!(filtered_ids(&browser), ["beta", "gamma"]);

    assert!(browser.category_move_down());
    assert_eq!(browser.category_cursor_row(), 2);
    assert_eq!(browser.active_category(), "Web");
    assert_eq!(filtered_ids(&browser), ["alpha"]);
}

#[test]
fn move_up_applies_the_new_category_on_the_same_call() {
    let mut browser = loaded(sample());
    assert!(browser.category_move_down());
    assert!(browser.category_move_down());
    assert_eq!(browser.active_category(), "Web");

    assert!(browser.category_move_up());
    assert_eq!(browser.category_cursor_row(), 1);
    assert_eq!(browser.active_category(), "Utility");
    assert_eq!(filtered_ids(&browser), ["beta", "gamma"]);
}

#[test]
fn the_category_cursor_clamps_at_both_ends_without_moving() {
    let mut browser = loaded(sample());

    // Up at the top is a no-op and reports it.
    assert!(!browser.category_move_up());
    assert_eq!(browser.category_cursor_row(), 0);
    assert_eq!(browser.active_category(), "ALL");

    // Down past the last row is a no-op and reports it.
    assert!(browser.category_move_down());
    assert!(browser.category_move_down());
    assert!(!browser.category_move_down());
    assert_eq!(browser.category_cursor_row(), 2);
    assert_eq!(browser.active_category(), "Web");
}

#[test]
fn moving_the_category_cursor_scrolls_the_navigator_to_keep_it_visible() {
    let mut browser = loaded(many(20));
    browser.category_viewport = 1;

    for _ in 0..5 {
        assert!(browser.category_move_down());
    }

    // With a one-row viewport the scroll tracks the cursor so the highlighted
    // category stays on screen (FR-007, FR-008).
    assert_eq!(browser.category_cursor_row(), 5);
    assert_eq!(browser.category_scroll, 5);
}

// -- Selecting a row out of range changes nothing (FR-027) -------------------

#[test]
fn selecting_an_out_of_range_row_changes_nothing() {
    let mut browser = loaded(sample());

    assert!(!browser.category_select_row(99));
    assert_eq!(browser.active_category(), "ALL");
    assert_eq!(browser.category_cursor_row(), 0);
    assert_eq!(browser.category_row_label(99), "ALL", "clamped to ALL");
}

// -- Clear to ALL (FR-015) ---------------------------------------------------

#[test]
fn clear_sets_all_and_restores_the_full_set() {
    let mut browser = loaded(sample());
    assert!(browser.category_move_down()); // `Utility`
    assert_eq!(filtered_ids(&browser), ["beta", "gamma"]);

    browser.category_clear();

    assert_eq!(browser.active_category(), "ALL");
    assert_eq!(browser.category_cursor_row(), 0);
    assert_eq!(filtered_ids(&browser), ["alpha", "beta", "gamma", "delta"]);
}

// -- Category change re-derives and resets the result cursor (FR-014) ------

#[test]
fn changing_the_category_resets_the_result_cursor_and_scroll() {
    let mut browser = loaded(sample());
    browser.move_down();
    browser.move_down();
    assert_eq!(browser.cursor, 2, "the result cursor started at row 2");

    assert!(browser.category_move_down()); // `Utility`

    // A category change re-derives from the full index and clamps the result
    // cursor to the first survivor without any re-fetch (FR-014).
    assert_eq!(browser.cursor, 0);
    assert_eq!(browser.scroll, 0);
    assert_eq!(browser.all.len(), 4, "the fetched index is untouched");
    assert!(
        browser.last_install.is_none(),
        "filtering never installs (FR-024)"
    );
}

#[test]
fn clear_resets_the_result_cursor_to_the_first_survivor() {
    let mut browser = loaded(sample());
    assert!(browser.category_move_down()); // `Utility`
    browser.move_down();
    assert_eq!(browser.cursor, 1);

    browser.category_clear();

    assert_eq!(browser.cursor, 0, "clear re-derives and resets the cursor");
    assert_eq!(browser.scroll, 0);
}

// -- No categories yields an ALL-only navigator (FR-018, A7) -----------------

#[test]
fn an_index_with_no_categories_yields_an_all_only_navigator() {
    let mut browser = loaded(vec![entry("alpha", None), entry("beta", None)]);

    assert_eq!(browser.category_row_count(), 1, "ALL only");
    assert_eq!(browser.category_row_label(0), "ALL");
    assert_eq!(category_rows(&browser).len(), 0);
    assert!(!browser.category_move_down(), "no row below ALL");

    // Browsing still works normally (A7).
    assert_eq!(filtered_ids(&browser), ["alpha", "beta"]);
}

#[test]
fn an_empty_index_yields_an_all_only_navigator() {
    let browser = loaded(Vec::new());

    assert_eq!(browser.status, PluginStoreStatus::Empty);
    assert_eq!(browser.category_row_count(), 1);
    assert_eq!(browser.category_row_label(0), "ALL");
    assert_eq!(browser.filtered.len(), 0);
}

// -- A vanished category is dropped on re-fetch (FR-027) --------------------

#[test]
fn a_category_the_new_index_no_longer_declares_is_dropped_to_all() {
    let mut browser = loaded(sample());
    assert!(browser.category_select_row(2)); // `Web`
    assert_eq!(browser.active_category(), "Web");

    // Re-fetch with an index that no longer declares `Web`.
    browser.set_entries(vec![entry("beta", Some("Utility"))]);

    assert_eq!(
        browser.active_category(),
        "ALL",
        "a vanished category falls back to ALL"
    );
    assert_eq!(browser.category_cursor_row(), 0);
    assert_eq!(filtered_ids(&browser), ["beta"]);
}

#[test]
fn a_category_the_new_index_still_declares_is_retained() {
    let mut browser = loaded(sample());
    assert!(browser.category_select_row(1)); // `Utility`
    assert_eq!(browser.active_category(), "Utility");

    // Re-fetch with an index that still declares `Utility`.
    browser.set_entries(vec![entry("gamma", Some("Utility"))]);

    assert_eq!(
        browser.active_category(),
        "Utility",
        "a surviving category is retained"
    );
    assert_eq!(filtered_ids(&browser), ["gamma"]);
}
