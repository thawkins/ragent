//! Tests for the `Installed` pseudo-category of the plugin-store category
//! navigator (spec `pluginstores` FR-054).
//!
//! The pseudo-category is pinned directly after the `ALL` sentinel, so the
//! installed plugins are one keypress away in both `/plugins codex` and
//! `/plugins claude`. Unlike a store category it matches the plugin id against the
//! store scan ([`PluginStoreBrowser::installed`]) rather than an entry's declared
//! `category`, so an installed plugin with no category at all still appears under
//! it. These tests drive the browser and the real renderer only: no store endpoint
//! is contacted and nothing is installed.

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_plugins::{StoreEntry, StoreKind};
use ragent_tui::App;
use ragent_tui::app::{INSTALLED_CATEGORY, PluginStoreBrowser};
use ragent_tui::layout;

#[path = "support/mod.rs"]
mod support;

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

/// A browser with `entries` loaded and `installed` recorded as the store scan.
fn loaded(entries: Vec<StoreEntry>, installed: &[&str]) -> PluginStoreBrowser {
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_installed(installed.iter().map(|s| (*s).to_string()).collect());
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

/// The absolute navigator row carrying `label`.
fn row_of(browser: &PluginStoreBrowser, label: &str) -> usize {
    (0..browser.category_row_count())
        .find(|&row| browser.category_row_label(row).eq_ignore_ascii_case(label))
        .unwrap_or_else(|| panic!("no navigator row for `{label}`"))
}

/// Three entries: two installed (`alpha` in `Web`, `beta` uncategorised) and one
/// not installed (`gamma` in `Web`).
fn sample() -> Vec<StoreEntry> {
    vec![
        entry("alpha", Some("Web")),
        entry("beta", None),
        entry("gamma", Some("Web")),
    ]
}

// -- The row is pinned directly after ALL (FR-054) -----------------------------

#[test]
fn the_installed_row_is_pinned_directly_after_all() {
    let browser = loaded(sample(), &[]);

    assert_eq!(browser.category_row_label(0), "ALL");
    assert_eq!(
        browser.category_row_label(1),
        INSTALLED_CATEGORY,
        "the pseudo-category follows ALL before any declared category"
    );
    assert_eq!(browser.category_row_label(2), "Web");
    assert_eq!(browser.category_row_count(), 3, "ALL + Installed + Web");
}

#[test]
fn the_installed_row_exists_even_with_no_declared_category_or_no_entries() {
    // A category-less index still offers the pseudo-category (FR-018, FR-054).
    let browser = loaded(vec![entry("a", None)], &[]);
    assert_eq!(browser.category_row_count(), 2);
    assert_eq!(browser.category_row_label(1), "Installed");

    // So does a still-loading browser, before any index has landed.
    let loading = PluginStoreBrowser::new(StoreKind::Claude, "", false);
    assert_eq!(loading.category_row_count(), 2);
    assert_eq!(loading.category_row_label(0), "ALL");
    assert_eq!(loading.category_row_label(1), "Installed");
}

// -- Selecting it narrows to the store scan (FR-054) ---------------------------

#[test]
fn selecting_installed_narrows_the_list_to_installed_plugins_only() {
    let mut browser = loaded(sample(), &["alpha", "beta"]);

    let row = row_of(&browser, "Installed");
    assert!(browser.category_select_row(row));

    assert_eq!(browser.active_category(), "Installed");
    // `beta` declares no category at all and still appears: the pseudo-category
    // matches the store scan, not the `category` field.
    assert_eq!(filtered_ids(&browser), ["alpha", "beta"]);
}

#[test]
fn installed_composes_with_the_query_and_marketplace_filters() {
    let mut entries = sample();
    entries[0].marketplace = "codex".to_string();
    entries[1].marketplace = "community".to_string();
    entries[2].marketplace = "codex".to_string();
    let mut browser = loaded(entries, &["alpha", "beta", "gamma"]);

    let row = row_of(&browser, "Installed");
    assert!(browser.category_select_row(row));
    assert_eq!(filtered_ids(&browser), ["alpha", "beta", "gamma"]);

    // The free-text query still narrows inside the pseudo-category.
    browser.push_char('b');
    assert_eq!(filtered_ids(&browser), ["beta"]);
    assert!(browser.backspace());

    // So does the marketplace filter.
    assert!(browser.marketplace_select_row(1));
    assert_eq!(browser.active_marketplace(), "codex");
    assert_eq!(filtered_ids(&browser), ["alpha", "gamma"]);
}

#[test]
fn the_row_shows_the_count_of_installed_entries_in_the_index() {
    // `delta` is installed in the store scan but the fetched index lists no such
    // entry, so the count is over the loaded entries the scan matches: two, not
    // three.
    let browser = loaded(sample(), &["alpha", "beta", "delta"]);
    assert_eq!(browser.category_entry_count("Installed"), 2);
    assert_eq!(browser.category_row_display(1), "Installed (2)");

    // A scan entry the index does not list is never counted, and nothing installed
    // counts zero.
    let none = loaded(sample(), &[]);
    assert_eq!(none.category_entry_count("Installed"), 0);
    assert_eq!(none.category_row_display(1), "Installed (0)");
}

// -- The name is reserved: a declared `installed` category adds no second row ---

#[test]
fn a_declared_installed_category_does_not_form_a_second_row() {
    let mut browser = loaded(
        vec![entry("a", Some("Installed")), entry("b", Some("Web"))],
        &[],
    );

    // Only one `Installed` row exists (the pinned pseudo-category), and the declared
    // spelling is not offered as a store category alongside it.
    let installed_rows = (0..browser.category_row_count())
        .filter(|&row| {
            browser
                .category_row_label(row)
                .eq_ignore_ascii_case("Installed")
        })
        .count();
    assert_eq!(installed_rows, 1, "the name is reserved (FR-054)");
    assert!(
        !browser
            .categories
            .categories()
            .iter()
            .any(|c| c.eq_ignore_ascii_case("Installed")),
        "a declared `Installed` category never becomes a derived row"
    );
    // `a` declares that name but is not installed, so it is not in the pseudo-row.
    let row = row_of(&browser, "Installed");
    assert!(browser.category_select_row(row));
    assert_eq!(filtered_ids(&browser), Vec::<&str>::new());
}

// -- A refreshed scan re-derives an already-selected pseudo-category -----------

#[test]
fn a_newly_installed_plugin_appears_while_installed_is_selected() {
    let mut browser = loaded(sample(), &[]);
    let row = row_of(&browser, "Installed");
    assert!(browser.category_select_row(row));
    assert_eq!(filtered_ids(&browser), Vec::<&str>::new());

    // The scan refresh after an install lands (FR-011) must re-derive this row.
    browser.set_installed(std::iter::once("gamma".to_string()).collect());

    assert_eq!(
        browser.active_category(),
        "Installed",
        "the pseudo-category stays selected"
    );
    assert_eq!(filtered_ids(&browser), ["gamma"]);
}

#[test]
fn the_pseudo_category_survives_an_index_re_fetch() {
    let mut browser = loaded(sample(), &["alpha"]);
    let row = row_of(&browser, "Installed");
    assert!(browser.category_select_row(row));

    // A re-fetch rebuilds the derived category set; the pseudo-category is not part
    // of that set, so the selection must be restored explicitly.
    browser.set_entries(vec![entry("alpha", Some("Database"))]);

    assert_eq!(browser.active_category(), "Installed");
    assert_eq!(
        browser.category_cursor_row(),
        row_of(&browser, "Installed"),
        "the cursor stays on the pinned row"
    );
    assert_eq!(filtered_ids(&browser), ["alpha"]);
}

// -- Render (FR-054) -----------------------------------------------------------

/// Render one frame so the navigator rows are painted.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");
    terminal
}

#[test]
fn the_panel_paints_the_installed_row_under_all() {
    let mut app = support::make_app();
    app.open_plugin_store(StoreKind::Codex, "", false);
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_installed(std::iter::once("alpha".to_string()).collect());
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_entries(sample());

    let terminal = render(&mut app, 120, 40);
    let nav = app.plugin_store_nav_area;
    let buffer = terminal.backend().buffer();
    let painted: Vec<String> = (nav.y..nav.y + nav.height)
        .map(|y| {
            (nav.x..nav.x + nav.width)
                .map(|x| buffer[(x, y)].symbol().to_string())
                .collect()
        })
        .collect();

    let all_row = painted
        .iter()
        .position(|row| row.contains("ALL"))
        .expect("ALL row painted");
    let installed_row = painted
        .iter()
        .position(|row| row.contains("Installed"))
        .expect("Installed row painted");
    assert_eq!(
        installed_row,
        all_row + 1,
        "Installed is painted directly beneath ALL: {painted:?}"
    );
    assert!(
        painted[installed_row].contains("Installed (1)"),
        "the row carries its installed count: {:?}",
        painted[installed_row]
    );
}
