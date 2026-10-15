//! Connector category filter for the connector browser (spec `connectors`
//! T-020; FR-039, FR-040).
//!
//! The browser and the textual `/connectors list` report must never disagree
//! about which connectors a category shows, so the filter is one shared
//! predicate ([`CategoryFilter::matches`]) plus one shared enumeration
//! ([`build_categories`]) that both call.
//!
//! - [`CategoryFilter`] is the filter value: the sentinel [`CategoryFilter::All`]
//!   or a single category name. `ALL` is the default, so a browser opened with no
//!   selection shows every connector (FR-040).
//! - [`CategoryFilterState`] is the browser's filter state: the selected filter
//!   together with the distinct category set derived from the connector
//!   catalogue plus the installed connectors (FR-040). Its
//!   [`CategoryFilterState::select_when_known`] guard is the one place an
//!   unknown category is refused, and the `list`/`search` glue calls it so a
//!   malformed `--category` value renders an `[err]` row and changes no state,
//!   exactly as the browser leaves its selection alone (T-021).
//! - Category comparison is exact and case-insensitive against a descriptor's
//!   `category` field; a connector with an empty category is visible only under
//!   `ALL` (FR-039).
//!
//! The filter narrows what is *shown*, never what is discovered, enabled, or
//! connected, so filtering can never start or stop an MCP server.
//!
//! [`CatalogueBrowser`] is the pure state machine behind the browse panel
//! itself: the fetched entry list, the search query, the category filter, the
//! filtered index set, the block cursor, and the load status. It is I/O-free, so
//! the TUI renderer, the key routing, and the tests all drive the same value the
//! off-loop catalogue fetch fills.

use std::collections::{BTreeMap, BTreeSet};

use crate::descriptor::ConnectorDescriptor;

/// The `ALL` category sentinel: "no filter" (FR-040).
///
/// Selecting `ALL` clears any active filter and shows every connector; it is the
/// default filter whenever no specific category has been selected.
pub const ALL_CATEGORY: &str = "ALL";

/// A connector-browser category filter (FR-039, FR-040).
///
/// Either the [`ALL_CATEGORY`] sentinel (no filter) or one category name. The
/// comparison is exact and case-insensitive (see [`CategoryFilter::matches`]).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CategoryFilter {
    /// No filter: every connector is shown (FR-040).
    #[default]
    All,
    /// One category name; only connectors declaring it are shown (FR-039).
    Category(String),
}

impl CategoryFilter {
    /// The `ALL` sentinel: the default, unfiltered selection (FR-040).
    #[must_use]
    pub const fn all() -> Self {
        Self::All
    }

    /// A single-category filter for `name` (FR-039).
    ///
    /// The name is trimmed. A blank name, or the `ALL` sentinel in any case,
    /// degrades to [`CategoryFilter::All`] rather than a filter that can never
    /// match, so an empty selection is always the unfiltered view (FR-040).
    #[must_use]
    pub fn of(name: &str) -> Self {
        let trimmed = name.trim();
        if trimmed.is_empty() || trimmed.eq_ignore_ascii_case(ALL_CATEGORY) {
            Self::All
        } else {
            Self::Category(trimmed.to_string())
        }
    }

    /// Parse a filter from a command argument or a browser selection (FR-040,
    /// FR-041).
    ///
    /// Identical to [`CategoryFilter::of`]: `ALL` (any case) and a blank value
    /// both mean "no filter".
    #[must_use]
    pub fn parse(raw: &str) -> Self {
        Self::of(raw)
    }

    /// The filter's display label: `ALL` when unfiltered, else the category name
    /// (FR-039, FR-040).
    #[must_use]
    pub fn label(&self) -> &str {
        match self {
            Self::All => ALL_CATEGORY,
            Self::Category(name) => name,
        }
    }

    /// Whether this filter is the `ALL` sentinel, i.e. clears the filter (FR-040).
    #[must_use]
    pub const fn is_all(&self) -> bool {
        matches!(self, Self::All)
    }

    /// Whether a descriptor is shown under this filter (FR-039).
    ///
    /// The comparison is exact and case-insensitive against the descriptor's
    /// `category` field. `ALL` shows everything; a connector with an empty
    /// category is visible only under `ALL`.
    #[must_use]
    pub fn matches(&self, descriptor: &ConnectorDescriptor) -> bool {
        self.matches_category(Some(descriptor.category.as_str()))
    }

    /// Whether a raw category string is shown under this filter (FR-039).
    ///
    /// The comparison is exact and case-insensitive. `ALL` shows everything; a
    /// blank or absent category is visible only under `ALL`. This is the one
    /// shared predicate: [`CategoryFilter::matches`] delegates here, and a
    /// caller whose rows are not [`ConnectorDescriptor`]s (the plugin-store
    /// navigator) calls it directly so both surfaces agree on what a category
    /// is.
    #[must_use]
    pub fn matches_category(&self, category: Option<&str>) -> bool {
        match self {
            Self::All => true,
            Self::Category(name) => match category {
                Some(category) => {
                    let category = category.trim();
                    !category.is_empty() && category.eq_ignore_ascii_case(name)
                }
                None => false,
            },
        }
    }
}

impl std::fmt::Display for CategoryFilter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

/// The connector browser's category-filter state (FR-039, FR-040).
///
/// Holds the selected filter (default [`CategoryFilter::All`]) and the distinct
/// category set derived from the loaded catalogue plus the installed connectors,
/// so a browser opened with no selection shows every connector and the category
/// list is ready to render.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CategoryFilterState {
    selected: CategoryFilter,
    available: Vec<String>,
}

impl CategoryFilterState {
    /// An unfiltered browser on `ALL` with no categories yet known (FR-040).
    #[must_use]
    pub fn new() -> Self {
        Self {
            selected: CategoryFilter::All,
            available: Vec::new(),
        }
    }

    /// Build the state from the distinct category set for a loaded catalogue plus
    /// the installed connectors; the selection defaults to `ALL` (FR-040).
    #[must_use]
    pub fn from_descriptors<'a>(
        installed: impl IntoIterator<Item = &'a ConnectorDescriptor>,
        catalogue: impl IntoIterator<Item = &'a ConnectorDescriptor>,
    ) -> Self {
        Self {
            selected: CategoryFilter::All,
            available: build_categories(installed, catalogue),
        }
    }

    /// The current filter (FR-039).
    #[must_use]
    pub const fn filter(&self) -> &CategoryFilter {
        &self.selected
    }

    /// Build the state from an iterator of raw category strings (FR-040).
    ///
    /// Blank categories are dropped (a row with no category belongs to `ALL`
    /// only), duplicates are folded case-insensitively keeping the first
    /// spelling seen, and the set is sorted case-insensitively so the menu order
    /// does not depend on the input order. The selection starts on `ALL`.
    #[must_use]
    pub fn from_categories<'a>(categories: impl IntoIterator<Item = &'a str>) -> Self {
        Self {
            selected: CategoryFilter::All,
            available: dedup_sorted_categories(categories),
        }
    }

    /// The distinct category set, sorted, with `ALL` implied rather than listed
    /// (FR-040).
    #[must_use]
    pub fn categories(&self) -> &[String] {
        &self.available
    }

    /// Whether the current filter is `ALL` (FR-040).
    #[must_use]
    pub const fn is_all(&self) -> bool {
        self.selected.is_all()
    }

    /// Select a filter directly (FR-039, FR-040).
    pub fn select(&mut self, filter: CategoryFilter) {
        self.selected = filter;
    }

    /// Select a filter by name, parsing the `ALL` sentinel (FR-040).
    pub fn select_by_name(&mut self, name: &str) {
        self.selected = CategoryFilter::of(name);
    }

    /// Select `ALL`, clearing any active filter (FR-040).
    pub fn select_all(&mut self) {
        self.selected = CategoryFilter::All;
    }

    /// Add one category to the distinct set a selection is validated against
    /// (FR-039, FR-041).
    ///
    /// A blank category is ignored (a connector with no category belongs to
    /// `ALL` only); a category already present case-insensitively is not
    /// duplicated, so the set keeps the first spelling seen. The set is left
    /// unsorted: call `sort_categories` once after the last insert, so a batch
    /// of inserts does not re-sort on every call.
    pub fn add_category(&mut self, category: &str) {
        let category = category.trim();
        if category.is_empty()
            || self
                .available
                .iter()
                .any(|known| known.eq_ignore_ascii_case(category))
        {
            return;
        }
        self.available.push(category.to_string());
    }

    /// Sort the distinct category set case-insensitively, matching
    /// [`build_categories`]. Call once after populating via
    /// [`CategoryFilterState::add_category`].
    pub fn sort_categories(&mut self) {
        self.available.sort_by_key(|known| known.to_lowercase());
    }

    /// Clear the active filter only when the selected value is a known available
    /// category or the `ALL` sentinel; a filter naming no known category is
    /// refused and the selection is left unchanged.
    ///
    /// Returns `true` when the value was accepted (the filter is now `ALL` or the
    /// named category). The `list`/`search` glue uses this so an unknown
    /// `--category <name>` value renders an `[err]` row and changes no state
    /// (FR-041), while `ALL` and a known category always apply (FR-039, FR-040).
    #[must_use]
    pub fn select_when_known(&mut self, name: &str) -> bool {
        let filter = CategoryFilter::of(name);
        match &filter {
            CategoryFilter::All => {
                self.selected = CategoryFilter::All;
                true
            }
            CategoryFilter::Category(category) => {
                if self
                    .available
                    .iter()
                    .any(|known| known.eq_ignore_ascii_case(category))
                {
                    self.selected = filter;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Whether a descriptor is shown under the current filter (FR-039).
    #[must_use]
    pub fn matches(&self, descriptor: &ConnectorDescriptor) -> bool {
        self.selected.matches(descriptor)
    }

    /// The connectors visible under the current filter, in input order (FR-039).
    pub fn visible<'a>(
        &self,
        descriptors: impl IntoIterator<Item = &'a ConnectorDescriptor>,
    ) -> Vec<&'a ConnectorDescriptor> {
        descriptors
            .into_iter()
            .filter(|descriptor| self.matches(descriptor))
            .collect()
    }

    /// How many of `descriptors` the current filter shows (FR-039).
    #[must_use]
    pub fn count_in<'a>(
        &self,
        descriptors: impl IntoIterator<Item = &'a ConnectorDescriptor>,
    ) -> usize {
        descriptors
            .into_iter()
            .filter(|descriptor| self.matches(descriptor))
            .count()
    }

    /// The active category together with the match count, for a report header
    /// (FR-039, FR-040).
    ///
    /// Renders `category ALL (3 of 3)` when unfiltered and
    /// `category developer (1 of 3)` when a category is selected.
    #[must_use]
    pub fn summary(&self, matched: usize, total: usize) -> String {
        crate::management::category_header(&self.selected, matched, total)
    }
}

/// The distinct category set for a loaded catalogue plus the installed
/// connectors, sorted for a stable menu (FR-040).
///
/// Installed connectors are read first, so on a case-insensitive collision the
/// installed spelling wins. Blank categories are dropped (a connector with no
/// category belongs to `ALL` only); the remainder are deduplicated
/// case-insensitively and sorted by a case-insensitive key so the menu order does
/// not depend on the input order.
#[must_use]
pub fn build_categories<'a>(
    installed: impl IntoIterator<Item = &'a ConnectorDescriptor>,
    catalogue: impl IntoIterator<Item = &'a ConnectorDescriptor>,
) -> Vec<String> {
    dedup_sorted_categories(
        installed
            .into_iter()
            .chain(catalogue)
            .map(|descriptor| descriptor.category.as_str()),
    )
}

/// Deduplicate category names case-insensitively (the first spelling seen wins),
/// drop blank ones, and sort by a case-insensitive key so the menu order does not
/// depend on the input order.
///
/// The one shared implementation of the dedup-and-sort rule: [`build_categories`]
/// and [`CategoryFilterState::from_categories`] both delegate here, so the browse
/// panel's menu order cannot drift between the two construction paths.
fn dedup_sorted_categories<'a>(categories: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut by_key: BTreeMap<String, String> = BTreeMap::new();
    for category in categories {
        let category = category.trim();
        if category.is_empty() {
            continue;
        }
        by_key
            .entry(category.to_lowercase())
            .or_insert_with(|| category.to_string());
    }
    by_key.into_values().collect()
}

/// The load state of the connector catalogue browser (spec `connectors`
/// acceptance criterion 11).
///
/// Mirrors `ragent_plugins`'s `PluginStoreStatus`: the TUI keeps the load state
/// on this value so the panel can render an explicit line for each case rather
/// than a blank list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CatalogueBrowseStatus {
    /// The catalogue is being fetched; the panel renders a loading row.
    Loading,
    /// The catalogue loaded and carries at least one entry.
    Ready,
    /// The catalogue loaded but carries no entries.
    Empty,
    /// The fetch failed; the detail is rendered inline.
    Failed(String),
}

/// The catalogue-browser state for the connector catalogue (spec `connectors`).
///
/// The state machine behind the `/connectors claude` browse panel, mirroring
/// `ragent_plugins`'s `PluginStoreBrowser` so the two panels read the same way.
/// This type owns the pure, I/O-free half of the panel - the fetched entry list,
/// the current query, the filtered index set, the block-cursor position, the
/// load status, the malformed-entry count, and the derived installed-id set -
/// so the fetch, the renderer, and the key routing can each be exercised
/// without a terminal. It performs no I/O itself: the off-loop fetch fills it
/// through [`CatalogueBrowser::set_entries`] / [`CatalogueBrowser::set_failed`],
/// and the store scan fills the installed set through
/// [`CatalogueBrowser::set_installed`].
///
/// The category filter ([`CategoryFilterState`]) narrows what is *shown*, never
/// what is discovered, enabled, or connected, so browsing and filtering can
/// never start or stop an MCP server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CatalogueBrowser {
    /// Every entry fetched from the catalogue, in document order.
    pub all: Vec<ConnectorDescriptor>,
    /// Installed connector ids in the store when this browser last refreshed.
    /// Derived from the store scan, never persisted here; the renderer paints a
    /// row in the installed colour, with an `[installed]` marker, when its id is
    /// in this set.
    pub installed: BTreeSet<String>,
    /// The current search query.
    pub query: String,
    /// The category filter and its derived category set.
    pub categories: CategoryFilterState,
    /// Indices into [`Self::all`] that match [`Self::query`] and the active
    /// category, in order.
    pub filtered: Vec<usize>,
    /// Block-cursor position within [`Self::filtered`].
    pub cursor: usize,
    /// How many catalogue entries the parser skipped as malformed or
    /// unexpressible ([`crate::provider::ParsedCatalogue::skipped`]). Zero for a
    /// hand-built entry list.
    pub skipped: usize,
    /// The fetch/parse status of this browser.
    pub status: CatalogueBrowseStatus,
    /// Whether the launch requested a cache-bypassing re-fetch.
    pub refresh: bool,
    /// The most recent install result shown in the panel footer.
    pub last_install: Option<String>,
    /// Block-cursor position within the navigator's rows. Row `0` is the `ALL`
    /// sentinel; rows `1..=categories().len()` are the derived categories in
    /// sorted order. Kept in sync with the active category by
    /// [`Self::category_select_row`] and [`Self::apply_category_cursor`].
    pub category_cursor: usize,
    /// Index of the first navigator row shown in the navigator viewport.
    ///
    /// The wheel over the navigator moves this directly, so the viewport can
    /// scroll away from the category cursor without changing the active
    /// category; a keyboard move pulls it back so the cursor stays visible.
    pub category_scroll: usize,
    /// Height in rows of the navigator viewport, recorded by the renderer on
    /// every frame. The key and mouse handlers read it back to keep the
    /// category cursor visible after a move and to clamp the wheel scroll; a
    /// `0` (before the first render) parks the scroll at the top.
    pub category_viewport: usize,
    /// Whether the category navigator holds keyboard focus.
    ///
    /// `false` - the result list focused - is the initial state; the focus key
    /// (`Tab`) toggles it. `Up`/`Down` route to whichever pane holds focus and
    /// the focused pane's cursor is rendered distinctly; the search query is
    /// edited by printable characters in either state.
    pub nav_focused: bool,
    /// A category filter requested when the panel was opened, held until the
    /// catalogue lands and its real category set is known (FR-041).
    ///
    /// A launch (`/connectors claude --category <name>`) fixes the filter before
    /// any entry has been fetched, so the name cannot be validated yet. Holding
    /// it here lets [`CatalogueBrowser::apply_pending_category`] validate it
    /// against the fetched categories once, exactly as an in-panel selection is
    /// validated, and refuse an unknown name in the footer instead of leaving the
    /// panel silently empty.
    pending_category: Option<CategoryFilter>,
    /// Whether the off-loop manifest-metadata enrichment (FR-044) has finished
    /// for the current catalogue.
    ///
    /// The catalogue lands first, then each entry's own manifest is fetched
    /// off-loop to fill the description the catalogue omitted; while this is
    /// `false` the panel renders a small `loading details...` hint in its title so
    /// a still-blank description reads as pending rather than missing. It is set
    /// `true` once the enrichment for the currently loaded catalogue has drained.
    pub enriching: bool,
}

impl Default for CatalogueBrowser {
    fn default() -> Self {
        Self::new("", false)
    }
}

impl CatalogueBrowser {
    /// Build a browser, applying the optional `prefill` query immediately so the
    /// filter is already applied on open, and recording whether the launch asked
    /// for a re-fetch.
    ///
    /// The status starts [`CatalogueBrowseStatus::Loading`]: the entry list is
    /// empty until the off-loop fetch calls [`Self::set_entries`].
    #[must_use]
    pub fn new(prefill: &str, refresh: bool) -> Self {
        let mut browser = Self {
            all: Vec::new(),
            installed: BTreeSet::new(),
            query: String::new(),
            categories: CategoryFilterState::new(),
            filtered: Vec::new(),
            cursor: 0,
            skipped: 0,
            status: CatalogueBrowseStatus::Loading,
            refresh,
            last_install: None,
            category_cursor: 0,
            category_scroll: 0,
            category_viewport: 0,
            nav_focused: false,
            pending_category: None,
            enriching: false,
        };
        if !prefill.is_empty() {
            browser.set_query(prefill.to_string());
        }
        browser
    }

    /// Replace the query and re-apply the filter.
    pub fn set_query(&mut self, query: String) {
        self.query = query;
        self.apply_filter();
    }

    /// Append one character to the query and re-apply the filter.
    pub fn push_char(&mut self, c: char) {
        self.query.push(c);
        self.apply_filter();
    }

    /// Remove the last query character and re-apply the filter.
    ///
    /// Returns `true` when a character was removed and `false` when the query
    /// was already empty, so the caller can dismiss the panel on an empty `Esc`.
    pub fn backspace(&mut self) -> bool {
        if self.query.pop().is_some() {
            self.apply_filter();
            true
        } else {
            false
        }
    }

    /// Move the block cursor up one result within the filtered set.
    pub fn move_up(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
        }
    }

    /// Move the block cursor down one result within the filtered set.
    pub fn move_down(&mut self) {
        if self.cursor + 1 < self.filtered.len() {
            self.cursor += 1;
        }
    }

    /// Whether the panel currently has a result row to show.
    ///
    /// `false` covers every empty-result case the panel must call out with an
    /// explicit empty/error line rather than a blank list: a still-loading
    /// catalogue, an empty catalogue, and a query or category that matches
    /// nothing. A failed fetch is reported separately through [`Self::status`].
    #[must_use]
    pub fn has_results(&self) -> bool {
        !self.filtered.is_empty()
    }

    /// The entry under the block cursor, when the filtered set is non-empty.
    #[must_use]
    pub fn selected(&self) -> Option<&ConnectorDescriptor> {
        self.filtered
            .get(self.cursor)
            .and_then(|index| self.all.get(*index))
    }

    /// Install a fetched catalogue and derive the resulting status.
    ///
    /// Carries both the accepted entries and the count of entries the parser
    /// skipped as malformed or unexpressible, so the panel can report a short
    /// list as a partial result rather than a silent one. The status becomes
    /// [`CatalogueBrowseStatus::Ready`] when at least one entry loaded and
    /// [`CatalogueBrowseStatus::Empty`] otherwise; the category set and the
    /// filter are re-derived so any prefilled query still constrains the result
    /// set.
    ///
    /// The entries are sorted alphabetically by connector id (FR-046) before the
    /// filter is derived, so the displayed order is independent of the fetch order
    /// and a category filter narrows the already-sorted set.
    pub fn set_entries(&mut self, entries: Vec<ConnectorDescriptor>, skipped: usize) {
        self.all = entries;
        crate::descriptor::sort_connectors(&mut self.all);
        self.skipped = skipped;
        self.status = if self.all.is_empty() {
            CatalogueBrowseStatus::Empty
        } else {
            CatalogueBrowseStatus::Ready
        };
        let previous = self.categories.filter().label().to_string();
        self.categories =
            CategoryFilterState::from_descriptors(std::iter::empty(), self.all.iter());
        // Keep a still-declared category selected across the refresh; a category
        // no entry declares falls back to `ALL` so the navigator never shows a
        // row that selects nothing, and the cursor is placed on the surviving
        // selection's row.
        // INTENTIONAL: a category that no entry declares falls back to `ALL`; the miss is benign.
        let _ = self.categories.select_when_known(&previous);
        self.sync_category_cursor_from_filter();
        self.apply_filter();
    }

    /// Place the category cursor on the row of the currently selected filter,
    /// resetting the navigator scroll to the top.
    ///
    /// Called after the filter is changed outside the navigator's own row
    /// bookkeeping (a fresh catalogue landing, or the report-style category
    /// cycling helper), so the navigator cursor and scroll always spell the
    /// active category.
    pub fn sync_category_cursor_from_filter(&mut self) {
        self.category_cursor = self.category_row_of(self.categories.filter().label());
        self.category_scroll = 0;
    }

    /// The active category label (`ALL` when unfiltered).
    #[must_use]
    pub fn active_category(&self) -> &str {
        self.categories.filter().label()
    }

    /// The number of navigator rows: the `ALL` sentinel plus each derived
    /// category.
    #[must_use]
    pub fn category_row_count(&self) -> usize {
        1 + self.categories.categories().len()
    }

    /// The label of navigator row `row`: `ALL` for row `0`, else the sorted
    /// category at that row; an out-of-range row reads as `ALL` so a renderer or
    /// a mouse hit test can never index past the set.
    #[must_use]
    pub fn category_row_label(&self, row: usize) -> &str {
        match row
            .checked_sub(1)
            .and_then(|index| self.categories.categories().get(index))
        {
            Some(label) => label.as_str(),
            None => ALL_CATEGORY,
        }
    }

    /// The navigator row currently under the category cursor, clamped to the row
    /// count.
    #[must_use]
    pub fn category_cursor_row(&self) -> usize {
        self.category_cursor
            .min(self.category_row_count().saturating_sub(1))
    }

    /// Move the category cursor up one row and apply the newly highlighted
    /// category on the same keypress. Returns whether the cursor moved.
    pub fn category_move_up(&mut self) -> bool {
        if self.category_cursor == 0 {
            return false;
        }
        self.category_cursor -= 1;
        self.apply_category_cursor();
        self.ensure_category_visible(self.category_viewport);
        true
    }

    /// Move the category cursor down one row and apply the newly highlighted
    /// category on the same keypress. Returns whether the cursor moved.
    pub fn category_move_down(&mut self) -> bool {
        if self.category_cursor + 1 >= self.category_row_count() {
            return false;
        }
        self.category_cursor += 1;
        self.apply_category_cursor();
        self.ensure_category_visible(self.category_viewport);
        true
    }

    /// Select navigator row `row` and apply it. Returns `false` when `row` is
    /// out of range, leaving the selection unchanged, so a stale mouse hit test
    /// cannot select a row that does not exist.
    pub fn category_select_row(&mut self, row: usize) -> bool {
        if row >= self.category_row_count() {
            return false;
        }
        self.category_cursor = row;
        self.apply_category_cursor();
        self.ensure_category_visible(self.category_viewport);
        true
    }

    /// Move the navigator viewport by `delta` rows without touching the category
    /// cursor or the active category.
    ///
    /// The wheel over the navigator calls this. `delta` is positive to reveal
    /// rows further down and negative to scroll back up. It is deliberately
    /// independent of [`Self::category_cursor`], so the active category (and the
    /// result list) stays exactly as it was; the scroll is clamped to
    /// `0..=rows - 1`, so it never points past the last row.
    pub fn category_scroll_by(&mut self, delta: i16) {
        let rows = self.category_row_count();
        if rows == 0 {
            self.category_scroll = 0;
            return;
        }
        let max = rows - 1;
        let next = if delta.is_negative() {
            self.category_scroll
                .saturating_sub(delta.unsigned_abs() as usize)
        } else {
            self.category_scroll.saturating_add(delta as usize)
        };
        self.category_scroll = next.min(max);
    }

    /// Scroll the result viewport by moving the result cursor `delta` rows
    /// (mirroring the result list's wheel behaviour).
    ///
    /// The result list is cursor-driven: the renderer draws the block cursor and
    /// lets the list keep that row visible, so moving the cursor by a small step
    /// is exactly how the wheel scrolls the result column. The cursor is clamped
    /// to the filtered set, and the active category is left untouched.
    pub fn scroll_results_by(&mut self, delta: i16) {
        let rows = self.filtered.len();
        if rows == 0 {
            return;
        }
        self.cursor = if delta.is_negative() {
            self.cursor.saturating_sub(delta.unsigned_abs() as usize)
        } else {
            self.cursor.saturating_add(delta as usize).min(rows - 1)
        };
    }

    /// Clear the category filter to `ALL` and re-derive the results.
    pub fn category_clear(&mut self) {
        self.category_cursor = 0;
        self.apply_category_cursor();
        self.ensure_category_visible(self.category_viewport);
    }

    /// Apply the category under the cursor and hand focus back to the result
    /// list.
    ///
    /// This is the navigator's `ENTER`: it re-applies the highlighted category
    /// (idempotent when the cursor did not move) and clears
    /// [`Self::nav_focused`], so the next `Up`/`Down` routes to the result list.
    /// It installs nothing, so `ENTER` over the navigator can never write to the
    /// store.
    pub fn category_confirm(&mut self) {
        self.apply_category_cursor();
        self.nav_focused = false;
    }

    /// Select a category by name without validating it against the known set.
    ///
    /// The navigator rows are the known categories (or `ALL`), so a selection
    /// driven by them cannot name an unknown category; `CategoryFilter::of`
    /// folds `ALL` (any case) and blank names to the unfiltered view.
    fn select_row_name(&mut self, name: &str) {
        self.categories.select_by_name(name);
    }

    /// Apply the category at the current cursor row to the result set.
    ///
    /// Selecting a category re-derives the visible results from [`Self::all`] in
    /// memory and resets the result cursor; it issues no network request and
    /// enables, connects, or installs nothing.
    fn apply_category_cursor(&mut self) {
        let row = self.category_cursor_row();
        self.category_cursor = row;
        let label = self.category_row_label(row).to_string();
        self.select_row_name(&label);
        self.apply_filter();
    }

    /// Adjust [`Self::category_scroll`] so the category cursor stays inside a
    /// navigator viewport of `viewport` rows.
    pub fn ensure_category_visible(&mut self, viewport: usize) {
        let rows = self.category_row_count();
        if viewport == 0 || rows == 0 {
            self.category_scroll = 0;
            return;
        }
        let cursor = self.category_cursor.min(rows - 1);
        if cursor < self.category_scroll {
            self.category_scroll = cursor;
        } else if cursor >= self.category_scroll + viewport {
            self.category_scroll = cursor + 1 - viewport;
        }
        self.category_scroll = self.category_scroll.min(rows.saturating_sub(viewport));
    }

    /// The navigator row carrying `label`: row `0` for the `ALL` sentinel, else
    /// the sorted position plus one; an unknown label reads as `ALL`.
    fn category_row_of(&self, label: &str) -> usize {
        if label.eq_ignore_ascii_case(ALL_CATEGORY) {
            return 0;
        }
        self.categories
            .categories()
            .iter()
            .position(|category| category.eq_ignore_ascii_case(label))
            .map_or(0, |index| index + 1)
    }

    /// Record a failed fetch for inline rendering.
    pub fn set_failed(&mut self, detail: String) {
        self.status = CatalogueBrowseStatus::Failed(detail);
    }

    /// Replace the derived installed-connector-id set.
    pub fn set_installed(&mut self, installed: BTreeSet<String>) {
        self.installed = installed;
    }

    /// Whether `id` is present in the store scan.
    ///
    /// The renderer uses this to choose the installed colour and the
    /// `[installed]` marker for a row; `ENTER` uses it to refuse a re-install.
    #[must_use]
    pub fn is_installed(&self, id: &str) -> bool {
        self.installed.contains(id)
    }

    /// The active category label, for a report or panel title.
    ///
    /// Delegates to [`CategoryFilterState`] so the browser, the panel title, and
    /// the `list`/`claude` report headers all spell the category the same way.
    #[must_use]
    pub fn selected_category_label(&self) -> &str {
        self.categories.selected.label()
    }

    /// The category filter and its derived category set, for a caller that owns
    /// the panel's filter interaction.
    ///
    /// The TUI's `c` key (`/connectors claude`) enumerates [`Self::categories`]
    /// to cycle the filter, so the cycle is driven by the same category set the
    /// browser and the reports use and cannot offer a category the catalogue does
    /// not declare (FR-039).
    #[must_use]
    pub const fn categories(&self) -> &CategoryFilterState {
        &self.categories
    }

    /// The category filter and its category set, mutably.
    ///
    /// A caller that changes the selection must follow it with
    /// [`Self::reapply_filter`] so [`Self::filtered`] and the cursor follow the
    /// new filter.
    pub const fn categories_mut(&mut self) -> &mut CategoryFilterState {
        &mut self.categories
    }

    /// Re-derive [`Self::filtered`] from the current query and category filter
    /// after the filter was changed through [`Self::categories_mut`].
    pub fn reapply_filter(&mut self) {
        self.apply_filter();
    }

    /// Select a category by name, refusing an unknown one.
    ///
    /// The one place an unknown category is refused (via
    /// [`CategoryFilterState::select_when_known`]), so the browser and the
    /// `list`/`claude` reports cannot disagree about which categories exist.
    pub fn select_category(&mut self, name: &str) -> bool {
        let applied = self.categories.select_when_known(name);
        if applied {
            self.apply_filter();
        }
        applied
    }

    /// Record a category filter requested at launch, before the catalogue (and
    /// therefore the real category set) has been fetched (FR-041).
    ///
    /// The filter is *held*, not applied: applying an unvalidated name now would
    /// hide every entry until the fetch lands, and would not distinguish an
    /// unknown category from an empty one. [`CategoryFilter::All`] clears
    /// nothing and drops any filter already held.
    pub fn set_pending_category(&mut self, category: CategoryFilter) {
        self.pending_category = match category {
            CategoryFilter::All => None,
            named => Some(named),
        };
    }

    /// Apply a category filter recorded by [`Self::set_pending_category`] against
    /// the categories now known, refusing an unknown one.
    ///
    /// Called once the fetched entries have landed ([`Self::set_entries`]), so the
    /// name is validated against the categories the catalogue actually declares -
    /// exactly like an in-panel [`Self::select_category`] - and an unknown name is
    /// reported in the footer instead of rendering an indistinguishable empty
    /// list. A no-op when no launch filter is held.
    pub fn apply_pending_category(&mut self) {
        let Some(category) = self.pending_category.take() else {
            return;
        };
        if !self.select_category(category.label()) {
            self.last_install = Some(format!("unknown category `{}`", category.label()));
        }
    }

    /// Apply one enrichment update ([`crate::catalogue_enrich::ManifestUpdate`]) to
    /// the entry it names, then re-apply the filter so a newly filled description
    /// becomes searchable (FR-044).
    ///
    /// Fills only a description the catalogue left blank, so an entry that already
    /// declared one keeps it. A no-op when no entry carries `id`.
    ///
    /// Unlike a user edit, a background enrichment must not disturb the browsing
    /// position: matching uses entry id/name/description/category/tags, and an
    /// enrichment can only *add* text, so the result set is unchanged; the cursor
    /// and scroll are preserved across the re-filter. Without this, the per-entry
    /// enrichment stream would bounce the cursor back to row 0 on every resolved
    /// manifest, making scrolling impossible while the enrichment runs.
    pub fn apply_manifest_update(&mut self, update: crate::catalogue_enrich::ManifestUpdate) {
        let Some(position) = self
            .all
            .iter()
            .position(|entry| entry.id.as_str() == update.id)
        else {
            return;
        };
        let entry = &mut self.all[position];
        let mut changed = false;
        if entry.description.trim().is_empty() {
            if let Some(description) = update.meta.description {
                entry.description = description;
                changed = true;
            }
        }
        if changed {
            self.reapply_filter_preserving_cursor();
        }
    }

    /// Mark the off-loop manifest enrichment finished (FR-044).
    ///
    /// Clears the title's `loading details...` hint; the accumulated updates have
    /// already been applied through [`Self::apply_manifest_update`].
    pub fn finish_enrichment(&mut self) {
        self.enriching = false;
    }

    /// Re-derive [`Self::filtered`] like [`Self::apply_filter`], but keep the
    /// user's browsing position: the cursor is placed on the same underlying entry
    /// as before the re-filter (when that entry still matches).
    ///
    /// Used by background enrichment, which may refine a row's description but
    /// never removes an entry from the result set. If the previously selected
    /// entry is somehow gone, the cursor stays at its filtered-index position
    /// clamped to the new set, so the panel never leaps to the top mid-scroll.
    fn reapply_filter_preserving_cursor(&mut self) {
        let selected_all_index = self.filtered.get(self.cursor).copied();
        self.apply_filter();
        self.cursor = selected_all_index
            .and_then(|index| self.filtered.iter().position(|&i| i == index))
            .unwrap_or(0);
    }

    /// Re-derive [`Self::filtered`] from [`Self::all`], the query, and the
    /// active category, and reset the cursor to the first survivor.
    ///
    /// The text match is a case-insensitive substring over the entry id, name,
    /// description, category, and tags; an empty query matches every entry.
    fn apply_filter(&mut self) {
        let needle = self.query.to_lowercase();
        self.filtered = self
            .all
            .iter()
            .enumerate()
            .filter(|(_, entry)| entry_matches(entry, &needle))
            .filter(|(_, entry)| self.categories.matches(entry))
            .map(|(index, _)| index)
            .collect();
        self.cursor = 0;
    }
}

/// The shared case-insensitive substring match over a descriptor's searchable
/// text. An empty needle matches every entry. `include_description` extends the
/// match to the `description` field (the browse panel matches it; the text
/// search report does not).
#[must_use]
pub(crate) fn descriptor_matches(
    entry: &ConnectorDescriptor,
    needle_lower: &str,
    include_description: bool,
) -> bool {
    if needle_lower.is_empty() {
        return true;
    }
    entry.id.as_str().to_lowercase().contains(needle_lower)
        || entry.name.to_lowercase().contains(needle_lower)
        || (include_description && entry.description.to_lowercase().contains(needle_lower))
        || entry.category.to_lowercase().contains(needle_lower)
        || entry
            .tags
            .iter()
            .any(|tag| tag.to_lowercase().contains(needle_lower))
}

/// Whether `entry` matches a lower-cased query needle over its id, name,
/// description, category, and tags. An empty needle matches every entry.
#[must_use]
pub fn entry_matches(entry: &ConnectorDescriptor, needle_lower: &str) -> bool {
    descriptor_matches(entry, needle_lower, true)
}

/// The `[ok]` report for one browse-panel install.
///
/// `id` is the catalogued connector id the row carried and `outcome` the
/// committed install. The caller only installs an entry whose id is not already
/// in the store scan, so the report states the enabled posture and points at
/// `/connectors enable`, matching the `/connectors add` wording without the
/// add-attribution header.
#[must_use]
pub fn browse_install_report(id: &str, outcome: &crate::manifest::StagedConnector) -> String {
    format!(
        "[ok] Installed connector `{id}` ({}, category: {}).\n\
         Installed to `{}`.\n\
         The connector is recorded **enabled**; run `/connectors enable {id}` \
         to connect its servers now in a running session.",
        outcome.descriptor.name,
        if outcome.descriptor.category.is_empty() {
            "(none)"
        } else {
            outcome.descriptor.category.as_str()
        },
        outcome.installed_dir.display(),
    )
}
