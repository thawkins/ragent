//! Pure unit tests for the plugin-store category-launch paths (spec `catnav`
//! T-008; FR-021 withdrawn, FR-022).
//!
//! These drive [`PluginStoreBrowser`] directly with an in-memory entry list, no
//! renderer, and no I/O, so they are deterministic and contact no store endpoint.
//! They pin the `--category <name>` launch filter, held until the fetched index is
//! known and then validated against the categories it actually declares; and they
//! pin that an entry's `tags` never form a navigator row (FR-021 withdrawn).

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

/// The absolute navigator row carrying `label` (row `0` is `ALL`, row `1` is the
/// `Installed` pseudo-category; the derived categories start at row `2`).
fn row_of(browser: &PluginStoreBrowser, label: &str) -> usize {
    (0..browser.category_row_count())
        .find(|&row| browser.category_row_label(row).eq_ignore_ascii_case(label))
        .unwrap_or_else(|| panic!("no navigator row for `{label}`"))
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
        row_of(&browser, "Web"),
        "the cursor sits on the pre-selected row (ALL, Installed, Database, Web)"
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

#[test]
fn an_installed_launch_category_applies_the_installed_pseudo_filter() {
    // `--category installed` names the pinned pseudo-category (FR-054) instead of a
    // store category, so it is accepted as soon as the index lands.
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_pending_category(CategoryFilter::of("installed"));
    browser.set_entries(vec![
        entry("a", Some("Web"), &[]),
        entry("b", Some("Database"), &[]),
    ]);
    browser.set_installed(std::iter::once("b".to_string()).collect());
    browser.apply_pending_category();

    assert_eq!(browser.active_category(), "Installed");
    assert_eq!(browser.category_cursor_row(), 1, "the pinned pseudo row");
    assert_eq!(filtered_ids(&browser), ["b"], "only installed plugins show");
    assert!(browser.last_install.is_none(), "no refusal is reported");
}

// -- A withdrawn FR-021: tags never form navigator rows ----------------------

#[test]
fn tags_of_categorised_entries_do_not_add_navigator_rows() {
    let browser = loaded(vec![
        entry("a", Some("Web"), &["http", "api"]),
        entry("b", Some("Database"), &["sql"]),
        entry("c", Some("Web"), &["http"]),
    ]);

    // FR-021 is withdrawn: only the declared categories form rows, so the tags add
    // nothing and `ALL` stays implicit and first.
    assert_eq!(category_rows(&browser), ["Database", "Web"]);
    assert_eq!(
        browser.category_row_count(),
        4,
        "ALL, Installed, plus the two declared categories"
    );
}

#[test]
fn a_tag_never_selects_a_category() {
    let mut browser = loaded(vec![
        entry("a", Some("Web"), &["http"]),
        entry("b", Some("Database"), &["sql"]),
        entry("c", Some("Web"), &["http", "api"]),
    ]);

    // No `http` row exists, and a category row is matched by `category` alone: under
    // `Database` the `sql` tag of `b` is irrelevant, and the `Web` entries stay out
    // (FR-021 withdrawn, FR-003).
    assert!(
        !browser.categories.categories().iter().any(|c| c == "http"),
        "a tag is not a navigator row"
    );
    let database = row_of(&browser, "Database");
    assert!(browser.category_select_row(database));
    assert_eq!(browser.active_category(), "Database");
    assert_eq!(
        filtered_ids(&browser),
        ["b"],
        "matching is on the declared category only"
    );
}

#[test]
fn an_uncategorised_index_with_tags_browses_normally() {
    let mut browser = loaded(vec![
        entry("b", None, &["beta"]),
        entry("a", None, &["alpha"]),
    ]);

    // With no declared category the navigator carries only `ALL` and the
    // `Installed` pseudo-category, so it still browses normally rather than
    // becoming a tag menu (FR-018, A7, FR-054) and sorted by id (FR-050).
    assert_eq!(browser.category_row_count(), 2);
    assert_eq!(browser.category_row_label(0), "ALL");
    assert_eq!(browser.category_row_label(1), "Installed");
    assert_eq!(category_rows(&browser).len(), 0);
    assert!(!browser.category_select_row(2), "no row below Installed");
    assert_eq!(filtered_ids(&browser), ["a", "b"]);
}

#[test]
fn a_category_survives_a_re_fetch_that_keeps_it() {
    let mut browser = loaded(vec![entry("a", Some("Web"), &["http"])]);
    // `Web` is the only derived row: the `http` tag adds none.
    assert_eq!(category_rows(&browser), ["Web"]);
    let web = row_of(&browser, "Web");
    assert!(browser.category_select_row(web));
    assert_eq!(browser.active_category(), "Web");

    // Re-fetch with the same category: the selection survives (FR-003, FR-027).
    browser.set_entries(vec![
        entry("a", Some("Web"), &["http"]),
        entry("b", Some("Web"), &[]),
    ]);

    assert_eq!(browser.active_category(), "Web");
    assert_eq!(category_rows(&browser), ["Web"]);
    assert_eq!(
        filtered_ids(&browser),
        ["a", "b"],
        "both Web entries survive"
    );
}
