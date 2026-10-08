//! Pure unit tests for the plugin-store category-launch paths (spec `catnav`
//! T-008; FR-021, FR-022).
//!
//! These drive [`PluginStoreBrowser`] directly with an in-memory entry list, no
//! renderer, and no I/O, so they are deterministic and contact no store endpoint.
//! They pin the two optional category sources: the `--category <name>` launch
//! filter, held until the fetched index is known and then validated against the
//! categories it actually declares; and the extra navigator rows derived from the
//! entries' `tags`.

use ragent_connectors::CategoryFilter;
use ragent_plugins::{StoreEntry, StoreKind};
use ragent_tui::app::PluginStoreBrowser;

/// A store entry with `id`, an optional `category`, and `tags`.
fn entry(id: &str, category: Option<&str>, tags: &[&str]) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: "1.0.0".to_string(),
        source: format!("https://example.org/{id}.zip"),
        description: format!("{id} description"),
        dialect: None,
        tags: tags.iter().map(|t| (*t).to_string()).collect(),
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

/// The derived navigator rows (without the implicit `ALL`), in navigator order.
fn category_rows(browser: &PluginStoreBrowser) -> Vec<&str> {
    browser
        .categories
        .categories()
        .iter()
        .map(String::as_str)
        .collect()
}

// -- `--category` pre-selection (FR-022) --------------------------------------

#[test]
fn a_known_launch_category_is_applied_once_the_index_lands() {
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_pending_category(CategoryFilter::of("Web"));
    assert_eq!(
        browser.pending_category,
        Some(CategoryFilter::Category("Web".to_string())),
        "the launch filter is held, not applied, before the fetch"
    );
    assert_eq!(
        browser.active_category(),
        "ALL",
        "still unfiltered while loading"
    );

    browser.set_entries(vec![
        entry("a", Some("Web"), &[]),
        entry("b", Some("Database"), &[]),
    ]);
    browser.apply_pending_category();

    assert_eq!(browser.active_category(), "Web");
    assert_eq!(
        browser.category_cursor_row(),
        2,
        "the cursor sits on the pre-selected row (ALL, Database, Web)"
    );
    assert_eq!(filtered_ids(&browser), ["a"]);
    assert!(browser.pending_category.is_none(), "the filter is consumed");
}

#[test]
fn an_unknown_launch_category_is_refused_and_reported() {
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_pending_category(CategoryFilter::of("Nonexistent"));
    browser.set_entries(vec![
        entry("a", Some("Web"), &[]),
        entry("b", Some("Web"), &[]),
    ]);

    browser.apply_pending_category();

    assert_eq!(
        browser.active_category(),
        "ALL",
        "an unknown category falls back to ALL, not an empty list"
    );
    assert_eq!(browser.category_cursor_row(), 0, "the cursor stays on ALL");
    assert_eq!(
        filtered_ids(&browser),
        ["a", "b"],
        "every entry stays visible"
    );
    assert_eq!(
        browser.last_install.as_deref(),
        Some("unknown category `Nonexistent`"),
        "the refusal is reported in the footer"
    );
}

#[test]
fn an_all_launch_category_holds_no_filter_and_is_a_noop() {
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_pending_category(CategoryFilter::all());
    assert_eq!(browser.pending_category, None, "ALL drops any held filter");

    browser.set_entries(vec![entry("a", Some("Web"), &[])]);

    assert_eq!(browser.active_category(), "ALL");
    assert!(
        browser.last_install.is_none(),
        "an ALL launch reports nothing"
    );
}

// -- Tag-derived navigator rows (FR-021) -------------------------------------

#[test]
fn distinct_tags_of_categorised_entries_follow_the_declared_categories() {
    let browser = loaded(vec![
        entry("a", Some("Web"), &["http", "api"]),
        entry("b", Some("Database"), &["sql"]),
        entry("c", Some("Web"), &["http"]), // duplicate tag, folded
    ]);

    // Declared categories sort first, then the tags in first-seen order (FR-021).
    assert_eq!(
        category_rows(&browser),
        ["Database", "Web", "http", "api", "sql"]
    );
    assert_eq!(
        browser.category_row_count(),
        6,
        "ALL plus two categories plus three tags"
    );
}

#[test]
fn a_tag_row_selects_only_the_entries_carrying_that_tag() {
    let mut browser = loaded(vec![
        entry("a", Some("Web"), &["http"]),
        entry("b", Some("Database"), &["sql"]),
        entry("c", Some("Web"), &["http", "api"]),
    ]);
    let http_row = browser
        .categories
        .categories()
        .iter()
        .position(|c| c == "http")
        .expect("the http tag is a row")
        + 1;

    assert!(browser.category_select_row(http_row));

    assert_eq!(browser.active_category(), "http");
    assert_eq!(
        filtered_ids(&browser),
        ["a", "c"],
        "the tag predicate matches the entries that carry it"
    );
}

#[test]
fn a_tag_colliding_with_a_declared_category_is_not_duplicated() {
    let browser = loaded(vec![
        entry("a", Some("Web"), &["web"]),
        entry("b", Some("Web"), &[]),
    ]);

    // The tag folds case-insensitively into the declared `Web` row (FR-021).
    assert_eq!(category_rows(&browser), ["Web"]);
    assert_eq!(
        browser.category_row_count(),
        2,
        "ALL plus the single Web row"
    );
}

#[test]
fn an_uncategorised_index_with_tags_stays_all_only() {
    let mut browser = loaded(vec![
        entry("a", None, &["alpha"]),
        entry("b", None, &["beta"]),
    ]);

    // With no declared category the navigator is ALL only, so it still browses
    // normally rather than becoming a tag menu (FR-018, A7).
    assert_eq!(browser.category_row_count(), 1);
    assert_eq!(browser.category_row_label(0), "ALL");
    assert_eq!(category_rows(&browser).len(), 0);
    assert!(!browser.category_move_down(), "no row below ALL");
    assert_eq!(filtered_ids(&browser), ["a", "b"]);
}

#[test]
fn tag_rows_survive_a_re_fetch_that_keeps_the_category() {
    let mut browser = loaded(vec![entry("a", Some("Web"), &["http"])]);
    assert!(browser.category_select_row(1)); // `Web`
    assert_eq!(browser.active_category(), "Web");

    // Re-fetch with the same category: the selection survives and the tag row is
    // rederived (FR-021, FR-027).
    browser.set_entries(vec![
        entry("a", Some("Web"), &["http"]),
        entry("b", Some("Web"), &[]),
    ]);

    assert_eq!(browser.active_category(), "Web");
    assert_eq!(category_rows(&browser), ["Web", "http"]);
    assert_eq!(
        filtered_ids(&browser),
        ["a", "b"],
        "both Web entries survive"
    );
}
