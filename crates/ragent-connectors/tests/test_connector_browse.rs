//! Tests for the connector category filter (spec `connectors` T-020; FR-039,
//! FR-040): the distinct category set is derived from the catalogue plus the
//! installed connectors, `ALL` is the default, and category comparison is exact
//! and case-insensitive.
//!
//! Every test is offline and pure: no filesystem, no network.

use ragent_connectors::{
    ALL_CATEGORY, CategoryFilter, CategoryFilterState, ConnectorAuthShape, ConnectorDescriptor,
    ConnectorId, ConnectorServer, build_categories,
};

// ── helpers ──────────────────────────────────────────────────────────────────

/// A minimal stdio server so a descriptor passes the "at least one server" shape.
fn server() -> ConnectorServer {
    ConnectorServer {
        id: "main".to_string(),
        transport: "stdio".to_string(),
        command: Some("echo".to_string()),
        args: Vec::new(),
        env: Default::default(),
        url: None,
        headers: Default::default(),
    }
}

/// A descriptor with the given id and declared category.
fn descriptor(id: &str, category: &str) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("valid id"),
        name: id.to_string(),
        description: String::new(),
        category: category.to_string(),
        tags: Vec::new(),
        source: String::new(),
        provenance: Default::default(),
        auth: ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers: vec![server()],
        unsupported: Vec::new(),
    }
}

// ── CategoryFilter: sentinel, parsing, labels (FR-040) ───────────────────────

#[test]
fn all_is_the_default_filter() {
    assert_eq!(CategoryFilter::default(), CategoryFilter::All);
    assert!(CategoryFilter::all().is_all());
    assert_eq!(CategoryFilter::all().label(), ALL_CATEGORY);
    assert_eq!(ALL_CATEGORY, "ALL");
}

#[test]
fn parsing_all_in_any_case_and_blank_yields_the_unfiltered_sentinel() {
    for raw in ["ALL", "all", "All", "  all  ", "", "   "] {
        assert_eq!(
            CategoryFilter::parse(raw),
            CategoryFilter::All,
            "input {raw:?} should mean no filter"
        );
    }
}

#[test]
fn parsing_a_name_trims_and_keeps_the_category() {
    let filter = CategoryFilter::parse("  developer  ");
    assert_eq!(filter, CategoryFilter::Category("developer".to_string()));
    assert_eq!(filter.label(), "developer");
    assert!(!filter.is_all());
}

#[test]
fn display_matches_label() {
    assert_eq!(CategoryFilter::All.to_string(), "ALL");
    assert_eq!(
        CategoryFilter::Category("data".to_string()).to_string(),
        "data"
    );
}

// ── CategoryFilter::matches: exact, case-insensitive, blank under ALL (FR-039) ─

#[test]
fn all_matches_every_descriptor_including_blank_category() {
    let filter = CategoryFilter::All;
    assert!(filter.matches(&descriptor("a", "developer")));
    assert!(filter.matches(&descriptor("b", "")));
    assert!(filter.matches(&descriptor("c", "data")));
}

#[test]
fn category_filter_matches_exactly_and_case_insensitively() {
    let filter = CategoryFilter::parse("developer");
    assert!(filter.matches(&descriptor("a", "developer")));
    assert!(filter.matches(&descriptor("b", "DEVELOPER")));
    assert!(filter.matches(&descriptor("c", "  Developer  ")));
    assert!(!filter.matches(&descriptor("d", "data")));
    assert!(!filter.matches(&descriptor("e", "data-productivity")));
}

#[test]
fn blank_category_is_visible_only_under_all() {
    let blank = descriptor("blank", "");
    assert!(CategoryFilter::All.matches(&blank));
    assert!(!CategoryFilter::parse("developer").matches(&blank));
    // The `ALL` sentinel parses to the unfiltered variant, so it matches.
    assert!(CategoryFilter::parse("ALL").matches(&blank));
}

// ── build_categories: distinct, sorted, case-insensitive, blank dropped (FR-040) ─

#[test]
fn build_categories_dedups_case_insensitively_and_sorts() {
    let installed = [descriptor("a", "Developer"), descriptor("b", "data")];
    let catalogue = [
        descriptor("c", "developer"), // dupe of "Developer" (different case)
        descriptor("d", "productivity"),
        descriptor("e", ""), // blank: dropped
    ];
    let categories = build_categories(&installed, &catalogue);
    assert_eq!(categories, vec!["data", "Developer", "productivity"]);
}

#[test]
fn build_categories_prefers_the_installed_spelling() {
    let installed = [descriptor("a", "DEV")];
    let catalogue = [descriptor("b", "dev")];
    let categories = build_categories(&installed, &catalogue);
    assert_eq!(categories, vec!["DEV"]);
}

#[test]
fn build_categories_is_empty_when_no_connector_declares_one() {
    let installed = [descriptor("a", "")];
    let catalogue = [descriptor("b", "   ")];
    assert!(build_categories(&installed, &catalogue).is_empty());
}

// ── CategoryFilterState: default ALL, selection, visible/count (FR-039, FR-040) ─

#[test]
fn a_fresh_state_is_on_all_with_no_categories() {
    let state = CategoryFilterState::new();
    assert!(state.is_all());
    assert_eq!(state.filter(), &CategoryFilter::All);
    assert!(state.categories().is_empty());
}

#[test]
fn from_descriptors_defaults_to_all_and_populates_categories() {
    let installed = [descriptor("a", "productivity")];
    let catalogue = [descriptor("b", "developer"), descriptor("c", "data")];
    let state = CategoryFilterState::from_descriptors(&installed, &catalogue);
    assert!(state.is_all());
    assert_eq!(state.categories(), &["data", "developer", "productivity"]);
}

#[test]
fn selecting_a_category_restricts_visible_and_counts_matches() {
    let all = [
        descriptor("echo", "productivity"),
        descriptor("two-server", "developer"),
        descriptor("needs-token", "data"),
    ];
    let mut state = CategoryFilterState::new();
    state.select_by_name("developer");

    let visible = state.visible(&all);
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id.as_str(), "two-server");
    assert_eq!(state.count_in(&all), 1);
    assert!(!state.is_all());

    state.select_by_name("data");
    let visible = state.visible(&all);
    assert_eq!(visible.len(), 1);
    assert_eq!(visible[0].id.as_str(), "needs-token");

    state.select_all();
    assert_eq!(state.visible(&all).len(), 3);
    assert_eq!(state.count_in(&all), 3);
    assert!(state.is_all());
}

#[test]
fn selecting_all_in_any_case_clears_the_filter() {
    let all = [descriptor("a", "developer"), descriptor("b", "data")];
    let mut state = CategoryFilterState::new();
    state.select_by_name("developer");
    assert_eq!(state.count_in(&all), 1);

    state.select_by_name("ALL");
    assert!(state.is_all());
    assert_eq!(state.count_in(&all), 2);
}

#[test]
fn a_unknown_category_matches_nothing_and_is_not_all() {
    let all = [descriptor("a", "developer"), descriptor("b", "data")];
    let mut state = CategoryFilterState::new();
    state.select_by_name("nosuch");
    assert!(!state.is_all());
    assert!(state.visible(&all).is_empty());
    assert_eq!(state.count_in(&all), 0);
}

#[test]
fn summary_reports_the_active_category_and_match_count() {
    let all = [
        descriptor("echo", "productivity"),
        descriptor("two-server", "developer"),
        descriptor("needs-token", "data"),
    ];
    let mut state = CategoryFilterState::new();
    assert_eq!(
        state.summary(state.count_in(&all), all.len()),
        "category ALL (3 of 3)"
    );

    state.select_by_name("developer");
    assert_eq!(
        state.summary(state.count_in(&all), all.len()),
        "category developer (1 of 3)"
    );
}

#[test]
fn selecting_by_filter_value_matches_selecting_by_name() {
    let all = [descriptor("a", "developer"), descriptor("b", "data")];
    let mut a = CategoryFilterState::new();
    a.select(CategoryFilter::parse("developer"));
    let mut b = CategoryFilterState::new();
    b.select_by_name("developer");
    assert_eq!(a, b);
    assert_eq!(a.visible(&all), b.visible(&all));
    assert_eq!(a.count_in(&all), 1);
}

// ── select_when_known (T-021; FR-039..FR-041) ────────────────────────────────

#[test]
fn select_when_known_accepts_all_and_any_known_category() {
    let known = [descriptor("a", "developer"), descriptor("b", "data")];
    let mut state = CategoryFilterState::from_descriptors(known.iter(), std::iter::empty());
    assert!(state.is_all());

    // A known category is accepted case-insensitively (FR-039).
    assert!(state.select_when_known("DEVELOPER"));
    assert_eq!(state.filter().label(), "DEVELOPER");
    assert_eq!(state.count_in(&known), 1);

    // `ALL` (any case) and a blank value clear the filter (FR-040).
    for sentinel in [ALL_CATEGORY, "all", "", "  "] {
        assert!(state.select_when_known("developer"));
        assert!(!state.is_all());
        assert!(state.select_when_known(sentinel), "`{sentinel}` clears");
        assert!(state.is_all());
    }
}

#[test]
fn select_when_known_refuses_an_unknown_category_and_changes_no_state() {
    let known = [descriptor("a", "developer"), descriptor("b", "data")];
    let mut state = CategoryFilterState::from_descriptors(known.iter(), std::iter::empty());
    state.select(CategoryFilter::parse("developer"));

    assert!(!state.select_when_known("nosuch"));
    // The refusal leaves the selection untouched (FR-041: change no state).
    assert_eq!(state.filter().label(), "developer");
    assert_eq!(state.count_in(&known), 1);

    // With no categories known, only `ALL` is accepted.
    let mut empty = CategoryFilterState::new();
    assert!(!empty.select_when_known("developer"));
    assert!(empty.is_all());
}

// ── CatalogueBrowser: the /connectors claude browse panel state ──────────────

use std::collections::BTreeSet;

use ragent_connectors::{CatalogueBrowseStatus, CatalogueBrowser, browse_install_report};

#[test]
fn a_new_browser_starts_loading_and_empty() {
    let browser = CatalogueBrowser::new("", false);
    assert_eq!(browser.status, CatalogueBrowseStatus::Loading);
    assert!(browser.all.is_empty());
    assert!(browser.filtered.is_empty());
    assert!(!browser.has_results());
    assert_eq!(browser.selected_category_label(), ALL_CATEGORY);
    assert!(!browser.refresh);
    assert!(browser.last_install.is_none());
}

#[test]
fn a_prefilled_launch_applies_the_query_on_open() {
    let browser = CatalogueBrowser::new("drive", true);
    assert_eq!(browser.query, "drive");
    assert!(browser.refresh);
}

#[test]
fn set_entries_derives_the_status_and_the_category_set() {
    let mut browser = CatalogueBrowser::new("", false);
    browser.set_entries(
        vec![descriptor("a", "developer"), descriptor("b", "data")],
        0,
    );
    assert_eq!(browser.status, CatalogueBrowseStatus::Ready);
    assert_eq!(browser.filtered.len(), 2);
    assert_eq!(
        browser.categories.categories(),
        &["data".to_string(), "developer".to_string()]
    );

    // An empty catalogue is the explicit empty state, still no rows.
    browser.set_entries(Vec::new(), 3);
    assert_eq!(browser.status, CatalogueBrowseStatus::Empty);
    assert!(!browser.has_results());
    assert_eq!(browser.skipped, 3);
}

#[test]
fn the_query_and_the_category_both_narrow_the_filtered_set() {
    let mut browser = CatalogueBrowser::new("", false);
    browser.set_entries(
        vec![
            descriptor("google-drive", "productivity"),
            descriptor("github", "developer"),
            descriptor("slack", "communication"),
        ],
        0,
    );
    assert_eq!(browser.filtered.len(), 3);

    // A category filter narrows to one row (FR-039).
    assert!(browser.select_category("developer"));
    assert_eq!(browser.filtered.len(), 1);
    assert_eq!(browser.selected().map(|e| e.id.as_str()), Some("github"));
    assert_eq!(browser.selected_category_label(), "developer");

    // A query further narrows within the category.
    browser.set_query("git".to_string());
    assert_eq!(browser.filtered.len(), 1);
    browser.set_query("drive".to_string());
    assert!(browser.filtered.is_empty());

    // Clearing the category restores the query-only view.
    assert!(browser.select_category(ALL_CATEGORY));
    assert_eq!(browser.filtered.len(), 1);
    assert_eq!(
        browser.selected().map(|e| e.id.as_str()),
        Some("google-drive")
    );
}

#[test]
fn an_unknown_category_is_refused_and_changes_no_state() {
    let mut browser = CatalogueBrowser::new("", false);
    browser.set_entries(vec![descriptor("a", "developer")], 0);
    assert!(browser.select_category("developer"));
    assert!(!browser.select_category("nosuch"));
    assert_eq!(browser.selected_category_label(), "developer");
    assert_eq!(browser.filtered.len(), 1);
}

#[test]
fn the_cursor_moves_within_the_filtered_set_only() {
    let mut browser = CatalogueBrowser::new("", false);
    browser.set_entries(
        vec![
            descriptor("a", ""),
            descriptor("b", ""),
            descriptor("c", ""),
        ],
        0,
    );
    assert_eq!(browser.selected().map(|e| e.id.as_str()), Some("a"));
    // Up at the top is a no-op.
    browser.move_up();
    assert_eq!(browser.selected().map(|e| e.id.as_str()), Some("a"));
    browser.move_down();
    browser.move_down();
    assert_eq!(browser.selected().map(|e| e.id.as_str()), Some("c"));
    // Down past the end is a no-op.
    browser.move_down();
    assert_eq!(browser.selected().map(|e| e.id.as_str()), Some("c"));
}

#[test]
fn backspace_reports_whether_it_removed_a_character() {
    let mut browser = CatalogueBrowser::new("", false);
    assert!(!browser.backspace(), "empty query reports nothing removed");
    browser.push_char('a');
    assert!(browser.backspace());
    assert_eq!(browser.query, "");
}

#[test]
fn the_installed_set_marks_a_row_and_is_replaceable() {
    let mut browser = CatalogueBrowser::new("", false);
    browser.set_entries(vec![descriptor("github", "developer")], 0);
    assert!(!browser.is_installed("github"));
    browser.set_installed(BTreeSet::from(["github".to_string()]));
    assert!(browser.is_installed("github"));
    assert!(!browser.is_installed("slack"));
}

#[test]
fn a_failed_fetch_renders_the_failure_state() {
    let mut browser = CatalogueBrowser::new("", false);
    browser.set_failed("catalogue unreachable".to_string());
    assert_eq!(
        browser.status,
        CatalogueBrowseStatus::Failed("catalogue unreachable".to_string())
    );
}

#[test]
fn entry_matches_covers_the_id_name_description_category_and_tags() {
    let entry = descriptor("google-drive", "productivity");
    assert!(ragent_connectors::entry_matches(&entry, ""));
    assert!(ragent_connectors::entry_matches(&entry, "google"));
    assert!(ragent_connectors::entry_matches(&entry, "productivity"));
    assert!(!ragent_connectors::entry_matches(&entry, "slack"));
}

#[test]
fn the_browse_install_report_names_the_install_and_its_enabled_posture() {
    let outcome = ragent_connectors::StagedConnector {
        descriptor: descriptor("github", "developer"),
        installed_dir: std::path::PathBuf::from("/store/github"),
    };
    let report = browse_install_report("github", &outcome);
    assert!(report.contains("Installed connector `github`"));
    assert!(report.contains("category: developer"));
    assert!(report.contains("/store/github"));
    assert!(report.contains("recorded **enabled**"));
    assert!(report.contains("/connectors enable github"));
}
