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
        marketplace: String::new(),
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
/// `Utility` spellings into one and sort as `ALL`, `Utility`, `Web`. The result
/// set is sorted alphabetically by id on load (`alpha`, `beta`, `delta`, `gamma`).
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

// -- Row counts appended to the navigator labels (spec `pluginstores` FR-053) ---

#[test]
fn each_navigator_row_shows_the_count_of_entries_under_it() {
    let browser = loaded(sample());

    // The count is over the full loaded index, independent of the active query or
    // filter: `ALL` counts every entry (four), `Utility` two (the folded spellings),
    // and `Web` one.
    assert_eq!(browser.category_entry_count("ALL"), 4);
    assert_eq!(browser.category_entry_count("Utility"), 2);
    assert_eq!(browser.category_entry_count("Web"), 1);
    assert_eq!(
        browser.category_entry_count("missing"),
        0,
        "unknown is zero"
    );

    // The rendered row display appends the count to the base label.
    assert_eq!(browser.category_row_display(0), "ALL (4)");
    assert_eq!(browser.category_row_display(1), "Utility (2)");
    assert_eq!(browser.category_row_display(2), "Web (1)");
}

#[test]
fn the_marketplace_row_shows_the_count_of_entries_under_it() {
    let browser = loaded(marketplace_sample());

    // `ALL` counts every entry (four, including the blank-marketplace `delta`);
    // `codex` two, `community` one.
    assert_eq!(browser.marketplace_entry_count("ALL"), 4);
    assert_eq!(browser.marketplace_entry_count("codex"), 2);
    assert_eq!(browser.marketplace_entry_count("community"), 1);
    // A blank label degrades to the `ALL` sentinel (`CategoryFilter::of`), so it
    // counts every entry; no blank marketplace is ever a navigator row (FR-052).
    assert_eq!(browser.marketplace_entry_count(""), 4, "blank reads as ALL");

    assert_eq!(browser.marketplace_row_display(0), "ALL (4)");
    assert_eq!(browser.marketplace_row_display(1), "codex (2)");
    assert_eq!(browser.marketplace_row_display(2), "community (1)");
}

#[test]
fn tags_do_not_form_navigator_rows() {
    // FR-021 (tag-derived rows) is withdrawn: only a declared `category` forms a
    // navigator row, so a tag cannot add one and the set stays the two declared
    // categories sorted case-insensitively.
    let mut tagged = entry("alpha", Some("Web"));
    tagged.tags = vec!["Analysis".to_string()];
    let browser = loaded(vec![tagged, entry("beta", Some("Utility"))]);

    assert_eq!(category_rows(&browser), ["Utility", "Web"]);
}

// -- Query and category compose (FR-003, A3) ----------------------------------

#[test]
fn the_query_and_the_category_compose() {
    let mut browser = loaded(sample());
    assert_eq!(filtered_ids(&browser), ["alpha", "beta", "delta", "gamma"]);

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
    assert_eq!(filtered_ids(&browser), ["alpha", "beta", "delta", "gamma"]);
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
    assert_eq!(filtered_ids(&browser), ["alpha", "beta", "delta", "gamma"]);
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
    let mut browser = loaded(vec![entry("beta", None), entry("alpha", None)]);

    assert_eq!(browser.category_row_count(), 1, "ALL only");
    assert_eq!(browser.category_row_label(0), "ALL");
    assert_eq!(category_rows(&browser).len(), 0);
    assert!(!browser.category_move_down(), "no row below ALL");

    // Browsing still works normally (A7) and the set is sorted by id (FR-050).
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

// -- Marketplace navigator (spec `pluginstores` FR-052) -----------------------

/// The same as [`entry`], but attributed to `marketplace` (FR-052).
fn mp_entry(id: &str, category: Option<&str>, marketplace: &str) -> StoreEntry {
    let mut e = entry(id, category);
    e.marketplace = marketplace.to_string();
    e
}

/// The derived marketplace rows (without the implicit `ALL`), in navigator order.
fn marketplace_rows(browser: &PluginStoreBrowser) -> Vec<&str> {
    browser
        .marketplaces
        .categories()
        .iter()
        .map(String::as_str)
        .collect()
}

/// Three entries across two marketplaces (`codex` and `community`) plus one row
/// with a blank marketplace (visible only under `ALL`). The rows sort as
/// `ALL`, `codex`, `community`.
fn marketplace_sample() -> Vec<StoreEntry> {
    vec![
        mp_entry("alpha", Some("Web"), "community"),
        mp_entry("beta", Some("Utility"), "codex"),
        mp_entry("gamma", Some("Web"), "codex"),
        mp_entry("delta", None, ""),
    ]
}

#[test]
fn the_marketplace_set_is_distinct_and_sorted_with_all_implied() {
    let browser = loaded(marketplace_sample());

    // The blank marketplace of `delta` forms no row, so the rows are the two
    // labelled marketplaces sorted case-insensitively with `ALL` implicit (FR-052).
    assert_eq!(marketplace_rows(&browser), ["codex", "community"]);
    assert_eq!(
        browser.marketplace_row_count(),
        3,
        "ALL + codex + community"
    );
    assert_eq!(browser.marketplace_row_label(0), "ALL");
    assert_eq!(browser.active_marketplace(), "ALL");
}

#[test]
fn the_category_marketplace_and_query_filters_compose() {
    let mut browser = loaded(marketplace_sample());
    assert_eq!(filtered_ids(&browser), ["alpha", "beta", "delta", "gamma"]);

    // Select `codex`: only its two entries survive (FR-052).
    assert!(browser.marketplace_move_down());
    assert_eq!(browser.active_marketplace(), "codex");
    assert_eq!(filtered_ids(&browser), ["beta", "gamma"]);

    // Narrow by `Web` within `codex`: only `gamma` is both `codex` and `Web`.
    assert!(browser.category_select_row(2)); // `Web`
    assert_eq!(browser.active_category(), "Web");
    assert_eq!(filtered_ids(&browser), ["gamma"]);

    // The query then narrows inside both filters; removing it restores the pair.
    for c in "gam".chars() {
        browser.push_char(c);
    }
    assert_eq!(filtered_ids(&browser), ["gamma"]);
    for _ in 0..3 {
        assert!(browser.backspace());
    }
    assert_eq!(filtered_ids(&browser), ["gamma"]);

    // Clearing the marketplace restores every `Web` entry across marketplaces.
    browser.marketplace_clear();
    assert_eq!(browser.active_marketplace(), "ALL");
    assert_eq!(filtered_ids(&browser), ["alpha", "gamma"]);
}

#[test]
fn moving_the_marketplace_cursor_applies_on_the_same_call_and_clears_to_all() {
    let mut browser = loaded(marketplace_sample());

    assert!(browser.marketplace_move_down());
    assert_eq!(browser.marketplace_cursor_row(), 1);
    assert_eq!(browser.active_marketplace(), "codex");
    assert_eq!(filtered_ids(&browser), ["beta", "gamma"]);

    assert!(browser.marketplace_move_down());
    assert_eq!(browser.active_marketplace(), "community");
    assert_eq!(filtered_ids(&browser), ["alpha"]);

    // Down past the last row is a no-op; Up steps back; clear restores `ALL`.
    assert!(!browser.marketplace_move_down());
    assert!(browser.marketplace_move_up());
    assert_eq!(browser.active_marketplace(), "codex");
    browser.marketplace_clear();
    assert_eq!(browser.active_marketplace(), "ALL");
    assert_eq!(browser.marketplace_cursor_row(), 0);
    assert_eq!(filtered_ids(&browser), ["alpha", "beta", "delta", "gamma"]);
}

#[test]
fn an_index_with_no_marketplace_yields_an_all_only_marketplace_navigator() {
    // `sample()` leaves every marketplace blank, so only the `ALL` row exists but
    // browsing still works (FR-052).
    let browser = loaded(sample());
    assert_eq!(browser.marketplace_row_count(), 1, "ALL only");
    assert_eq!(browser.marketplace_row_label(0), "ALL");
    assert_eq!(filtered_ids(&browser), ["alpha", "beta", "delta", "gamma"]);
}

#[test]
fn a_marketplace_the_new_index_no_longer_declares_is_dropped_to_all() {
    let mut browser = loaded(marketplace_sample());
    assert!(browser.marketplace_select_row(1)); // `codex`
    assert_eq!(browser.active_marketplace(), "codex");

    // Re-fetch with an index that only declares `community`.
    browser.set_entries(vec![mp_entry("alpha", Some("Web"), "community")]);

    assert_eq!(
        browser.active_marketplace(),
        "ALL",
        "a vanished marketplace falls back to ALL"
    );
    assert_eq!(browser.marketplace_cursor_row(), 0);
    assert_eq!(filtered_ids(&browser), ["alpha"]);
}

#[test]
fn a_marketplace_the_new_index_still_declares_is_retained() {
    let mut browser = loaded(marketplace_sample());
    assert!(browser.marketplace_select_row(2)); // `community`
    assert_eq!(browser.active_marketplace(), "community");

    browser.set_entries(vec![mp_entry("alpha", Some("Web"), "community")]);

    assert_eq!(
        browser.active_marketplace(),
        "community",
        "a surviving marketplace is retained"
    );
    assert_eq!(filtered_ids(&browser), ["alpha"]);
}
