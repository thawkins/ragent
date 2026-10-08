//! Fixture-driven acceptance walk for the plugin-store category navigator
//! (spec `catnav` T-009; Acceptance criteria 1-10; TESTPLAN TC-001..TC-013).
//!
//! The repository fixtures under `assets/plugins/fixtures/stores/` are the same
//! artifacts the manual TESTPLAN walk stages, so a regression here means the
//! documented walk would also fail. Each test replaces the app's production HTTPS
//! fetcher with an offline [`FixtureStoreFetcher`] seeded with the fixture index
//! bytes, opens the panel through the real `/plugins` launch path, drains the
//! off-loop fetch exactly as the event loop does, then drives the real keyboard
//! and mouse handlers.
//!
//! Every test is deterministic and offline: no live store is contacted and no
//! plugin is installed. Both the keyboard and the mouse paths are exercised so
//! neither channel is left unverified, and the category-less and malformed
//! fixtures pin the `ALL`-only and inline-error fallbacks.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseButton, MouseEvent, MouseEventKind};
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::style::Color;

use ragent_connectors::CategoryFilter;
use ragent_plugins::{FixtureStoreFetcher, StoreKind};
use ragent_tui::App;
use ragent_tui::app::PluginStoreBrowser;
use ragent_tui::input::handle_key;
use ragent_tui::layout;

#[path = "support/mod.rs"]
mod support;

static TEMP_SEQ: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

/// A unique scratch directory under `target/temp/` (no `/tmp`, per AGENTS.md).
fn temp_dir(name: &str) -> PathBuf {
    let unique = TEMP_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(format!(
        "../../target/temp/plugin-store-category-acceptance/{name}-{}-{unique}",
        std::process::id()
    ));
    std::fs::create_dir_all(&path).expect("temp dir creatable");
    path
}

/// The raw bytes of one store-index fixture under
/// `assets/plugins/fixtures/stores/`.
fn fixture_store_bytes(file: &str) -> Vec<u8> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/plugins/fixtures/stores")
        .join(file);
    std::fs::read(&path).unwrap_or_else(|e| panic!("read fixture {}: {e}", path.display()))
}

/// The Codex fixture index bytes (declares several categories).
fn codex_index() -> Vec<u8> {
    fixture_store_bytes("index-codex.json")
}

/// The Claude fixture index bytes.
fn claude_index() -> Vec<u8> {
    fixture_store_bytes("index-claude.json")
}

/// An app whose project store is an isolated scratch tree.
fn new_app() -> App {
    let mut app = support::make_app();
    app.cwd_path = temp_dir("walk");
    app
}

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

fn mouse(kind: MouseEventKind, col: u16, row: u16) -> MouseEvent {
    MouseEvent {
        kind,
        column: col,
        row,
        modifiers: KeyModifiers::empty(),
    }
}

fn left_click(col: u16, row: u16) -> MouseEvent {
    mouse(MouseEventKind::Down(MouseButton::Left), col, row)
}

fn wheel_down(col: u16, row: u16) -> MouseEvent {
    mouse(MouseEventKind::ScrollDown, col, row)
}

/// Install the fixture seam for `kind` and open the panel through the real
/// launch path, then drain the off-loop fetch.
async fn open_fixture(app: &mut App, kind: StoreKind, bytes: Vec<u8>) {
    app.set_plugin_store_fetcher(Arc::new(
        FixtureStoreFetcher::new().with_index(kind.default_url(), bytes),
    ));
    app.open_plugin_store(kind, "", false);
    drain_fetch(app).await;
}

/// Wait (bounded) for the off-loop fetch to deposit its result, then apply it.
async fn drain_fetch(app: &mut App) {
    let mut delivered = false;
    for _ in 0..300 {
        if app
            .plugin_store_result
            .lock()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
        {
            delivered = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert!(delivered, "the off-loop fixture fetch delivered a result");
    app.poll_plugin_store_result();
}

/// The browser behind the open panel.
fn browser(app: &App) -> &PluginStoreBrowser {
    app.plugin_store.as_ref().expect("panel open")
}

/// The ids behind the current filtered set, in order.
fn filtered_ids(app: &App) -> Vec<String> {
    let browser = browser(app);
    browser
        .filtered
        .iter()
        .map(|index| browser.all[*index].id.clone())
        .collect()
}

/// The derived navigator rows without the implicit `ALL`.
fn category_rows(app: &App) -> Vec<String> {
    browser(app).categories.categories().to_vec()
}

/// The absolute navigator row carrying `label` (row `0` is `ALL`).
fn row_of(app: &App, label: &str) -> usize {
    let browser = browser(app);
    (0..browser.category_row_count())
        .find(|&row| browser.category_row_label(row).eq_ignore_ascii_case(label))
        .unwrap_or_else(|| panic!("no navigator row for `{label}`"))
}

/// Focus the navigator, reset to `ALL`, and move the category cursor onto
/// `label` using the real keyboard path.
async fn select_category(app: &mut App, label: &str) {
    if !app.plugin_store_nav_focused() {
        handle_key(app, key(KeyCode::Tab)).await;
    }
    handle_key(app, key(KeyCode::Char('c'))).await; // reset to ALL
    let target = row_of(app, label);
    for _ in 0..target {
        handle_key(app, key(KeyCode::Down)).await;
    }
    assert!(
        browser(app).active_category().eq_ignore_ascii_case(label),
        "the cursor reached `{label}`"
    );
}

/// Render one frame and return the terminal so the painted buffer can be read.
fn render(app: &mut App, width: u16, height: u16) -> Terminal<TestBackend> {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).expect("test terminal");
    terminal
        .draw(|frame| layout::render(frame, app))
        .expect("draw");
    terminal
}

/// Read one painted row between `x0` (inclusive) and `x1` (exclusive).
fn row_text(terminal: &Terminal<TestBackend>, y: u16, x0: u16, x1: u16) -> String {
    let buffer = terminal.backend().buffer();
    (x0..x1)
        .map(|x| buffer[(x, y)].symbol().to_string())
        .collect()
}

/// Background colour of the first non-space glyph on painted panel row `y`.
fn first_glyph_bg(terminal: &Terminal<TestBackend>, app: &App, y: u16) -> Color {
    let area = app.plugin_store_area;
    let buffer = terminal.backend().buffer();
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (x0..x1)
        .find(|&x| buffer[(x, y)].symbol() != " ")
        .map(|x| buffer[(x, y)].bg)
        .unwrap_or(Color::Reset)
}

// ---------------------------------------------------------------------------
// TC-001 - The navigator lists ALL plus the distinct categories (AC1, FR-001,
// FR-004, FR-006, FR-018)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tc_001_navigator_lists_all_and_the_distinct_fixture_categories_sorted() {
    let mut app = new_app();
    open_fixture(&mut app, StoreKind::Codex, codex_index()).await;

    // The declared categories fold case-insensitively and sort
    // (`Utility`/`utility` collapse into one row). Because `codex-weather` also
    // declares tags and FR-021 is implemented, its distinct tags follow the
    // declared set as extra rows; only an entry that also declares a category
    // contributes tags, so the tag-less `codex-uncat` adds nothing.
    assert_eq!(
        category_rows(&app),
        ["Database", "Language", "Utility", "Web", "weather", "http"],
        "the declared categories fold and sort, then the tag rows follow (FR-021)"
    );
    // The uncategorised `codex-uncat` declares no row (FR-003, FR-027).
    assert!(
        !category_rows(&app)
            .iter()
            .any(|c| c.eq_ignore_ascii_case("codex-uncat")),
        "an absent category produces no row"
    );
    assert_eq!(browser(&app).category_row_label(0), "ALL");
    assert_eq!(
        browser(&app).active_category(),
        "ALL",
        "ALL is selected first"
    );

    // The Claude panel shows its own categories from the same navigator (FR-006).
    let mut claude = new_app();
    open_fixture(&mut claude, StoreKind::Claude, claude_index()).await;
    assert_eq!(category_rows(&claude), ["Database", "Productivity", "todo"]);

    // The panel keeps its title, search field, and ASCII-only body (FR-004).
    let terminal = render(&mut app, 120, 40);
    let area = app.plugin_store_area;
    let buffer = terminal.backend().buffer();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            assert!(
                buffer[(x, y)].symbol().is_ascii(),
                "non-ASCII glyph at ({x}, {y})"
            );
        }
    }
    let title = row_text(&terminal, area.y, area.x, area.x + area.width);
    assert!(title.contains("Codex"), "title names the store: {title:?}");
    assert!(title.contains("search:"), "title carries the search field");
}

// ---------------------------------------------------------------------------
// TC-002 - Down/Up move the category cursor and narrow on the same press (AC2)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tc_002_down_up_move_the_cursor_and_apply_on_the_same_press() {
    let mut app = new_app();
    open_fixture(&mut app, StoreKind::Codex, codex_index()).await;
    let total = browser(&app).all.len();

    handle_key(&mut app, key(KeyCode::Tab)).await; // focus the navigator
    assert!(app.plugin_store_nav_focused());

    // Down lands on `Database` and applies it on the same keypress (A4, FR-007).
    handle_key(&mut app, key(KeyCode::Down)).await;
    assert_eq!(browser(&app).active_category(), "Database");
    assert_eq!(filtered_ids(&app), ["codex-db"]);
    assert_eq!(browser(&app).all.len(), total, "no re-fetch on filter");

    // Down again advances to `Language`.
    handle_key(&mut app, key(KeyCode::Down)).await;
    assert_eq!(browser(&app).active_category(), "Language");
    assert_eq!(filtered_ids(&app), ["codex-lsp"]);

    // Up returns to `Database` and re-applies it (FR-008).
    handle_key(&mut app, key(KeyCode::Up)).await;
    assert_eq!(browser(&app).active_category(), "Database");

    // Up clamps at `ALL` and does not wrap (FR-007, FR-008).
    handle_key(&mut app, key(KeyCode::Up)).await;
    assert_eq!(browser(&app).active_category(), "ALL");
    assert_eq!(filtered_ids(&app).len(), total, "ALL restores the full set");
}

// ---------------------------------------------------------------------------
// TC-003 - Selecting a category narrows; ALL restores (AC6, FR-014, FR-015,
// FR-018)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tc_003_selecting_a_category_narrows_and_all_restores_the_full_set() {
    let mut app = new_app();
    open_fixture(&mut app, StoreKind::Codex, codex_index()).await;

    select_category(&mut app, "Database").await;
    assert_eq!(filtered_ids(&app), ["codex-db"]);

    // `c` clears back to ALL and re-derives the full set, including the
    // uncategorised entry (FR-015, FR-018).
    handle_key(&mut app, key(KeyCode::Char('c'))).await;
    assert_eq!(browser(&app).active_category(), "ALL");
    assert_eq!(browser(&app).category_cursor_row(), 0);
    let ids = filtered_ids(&app);
    assert_eq!(ids.len(), 6, "every entry returns under ALL: {ids:?}");
    assert!(ids.iter().any(|id| id == "codex-uncat"));
}

// ---------------------------------------------------------------------------
// TC-004 - Category and query compose; clearing the query restores (AC5)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tc_004_category_and_query_compose_then_clearing_restores() {
    let mut app = new_app();
    open_fixture(&mut app, StoreKind::Codex, codex_index()).await;

    select_category(&mut app, "Utility").await;
    assert_eq!(filtered_ids(&app), ["codex-time", "codex-shell"]);

    // Typing filters within the category (FR-017 still routes printable keys to
    // the query while the navigator is focused).
    for c in "time".chars() {
        handle_key(&mut app, key(KeyCode::Char(c))).await;
    }
    assert_eq!(filtered_ids(&app), ["codex-time"]);

    // Clearing the query restores every `Utility` entry (FR-014).
    for _ in 0..4 {
        handle_key(&mut app, key(KeyCode::Backspace)).await;
    }
    assert_eq!(filtered_ids(&app), ["codex-time", "codex-shell"]);

    // A combination that matches nothing shows the explicit empty line while the
    // navigator stays visible (FR-019).
    handle_key(&mut app, key(KeyCode::Char('z'))).await;
    handle_key(&mut app, key(KeyCode::Char('z'))).await;
    assert_eq!(filtered_ids(&app), Vec::<String>::new(), "no entry matches");
    let terminal = render(&mut app, 120, 40);
    let area = app.plugin_store_area;
    let body = (area.y + 1..area.y + area.height - 1)
        .map(|y| {
            row_text(
                &terminal,
                y,
                area.x + 1,
                area.x + area.width.saturating_sub(1),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        body.contains("no matching"),
        "the empty-state line is shown: {body:?}"
    );
    assert!(
        app.plugin_store_nav_area.width > 0,
        "the navigator stays visible"
    );
}

// ---------------------------------------------------------------------------
// TC-005 - Mouse: left-click a category row selects it (AC3, FR-002, FR-011)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tc_005_left_click_on_a_category_row_selects_and_colours_it() {
    let mut app = new_app();
    open_fixture(&mut app, StoreKind::Codex, codex_index()).await;
    let _ = render(&mut app, 120, 40);
    let nav = app.plugin_store_nav_area;
    let db_row = row_of(&app, "Database") as u16;

    app.handle_mouse_event(left_click(nav.x + 1, nav.y + db_row));

    assert_eq!(browser(&app).active_category(), "Database");
    assert!(
        app.plugin_store_nav_focused(),
        "the click focuses the navigator"
    );
    assert_eq!(filtered_ids(&app), ["codex-db"]);

    // The clicked row carries the active cursor colour; a different row does not
    // (FR-002).
    let terminal = render(&mut app, 120, 40);
    assert_eq!(
        first_glyph_bg(&terminal, &app, nav.y + db_row),
        Color::Cyan,
        "the selected category row is painted with the active cursor colour"
    );
    assert_ne!(
        first_glyph_bg(&terminal, &app, nav.y),
        Color::Cyan,
        "the previously selected ALL row is no longer highlighted"
    );
}

// ---------------------------------------------------------------------------
// TC-006 - Mouse wheel scrolls the navigator without changing the category
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tc_006_wheel_scrolls_the_navigator_without_changing_the_category() {
    let mut app = new_app();
    open_fixture(&mut app, StoreKind::Codex, codex_index()).await;
    let _ = render(&mut app, 120, 40);
    let nav = app.plugin_store_nav_area;
    assert_eq!(browser(&app).category_scroll, 0);

    app.handle_mouse_event(wheel_down(nav.x + 1, nav.y + 1));
    assert_eq!(browser(&app).category_scroll, 3, "the viewport scrolled");
    assert_eq!(browser(&app).active_category(), "ALL", "category unchanged");
}

// ---------------------------------------------------------------------------
// TC-007 - Focus transfer; ENTER applies without installing (AC9, FR-009,
// FR-010, FR-024)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tc_007_focus_transfer_and_enter_applies_without_installing() {
    let mut app = new_app();
    open_fixture(&mut app, StoreKind::Codex, codex_index()).await;

    assert!(!app.plugin_store_nav_focused(), "result list focused first");
    handle_key(&mut app, key(KeyCode::Tab)).await;
    assert!(app.plugin_store_nav_focused(), "Tab focuses the navigator");

    select_category(&mut app, "Database").await;
    handle_key(&mut app, key(KeyCode::Enter)).await;

    // ENTER over the navigator applies the category and moves focus to the
    // result list, installing nothing (FR-010, FR-024).
    assert!(
        !app.plugin_store_nav_focused(),
        "focus moves to the results"
    );
    assert_eq!(browser(&app).active_category(), "Database");
    assert!(
        app.plugin_store_install_result
            .lock()
            .map(|g| g.is_none())
            .unwrap_or(false),
        "no navigator keypress installs anything"
    );

    // The `--category` launch pre-selects the row through the same validation
    // (FR-022).
    let mut launched = new_app();
    launched.set_plugin_store_fetcher(Arc::new(
        FixtureStoreFetcher::new().with_index(StoreKind::Codex.default_url(), codex_index()),
    ));
    launched.open_plugin_store_category(
        StoreKind::Codex,
        "",
        false,
        CategoryFilter::of("Language"),
    );
    drain_fetch(&mut launched).await;
    assert_eq!(browser(&launched).active_category(), "Language");
    assert_eq!(
        browser(&launched).category_cursor_row(),
        row_of(&launched, "Language")
    );
    assert_eq!(filtered_ids(&launched), ["codex-lsp"]);
}

// ---------------------------------------------------------------------------
// TC-008 - Loading state shows an ALL-only navigator (FR-016)
// ---------------------------------------------------------------------------

#[test]
fn tc_008_loading_state_shows_an_all_only_navigator() {
    // No fetch is started, so the panel stays in its loading state.
    let mut app = new_app();
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Codex, "", false));
    let terminal = render(&mut app, 120, 40);

    assert_eq!(
        browser(&app).category_row_count(),
        1,
        "ALL only while loading"
    );
    assert_eq!(browser(&app).category_row_label(0), "ALL");
    let area = app.plugin_store_area;
    let body = (area.y + 1..area.y + area.height - 1)
        .map(|y| {
            row_text(
                &terminal,
                y,
                area.x + 1,
                area.x + area.width.saturating_sub(1),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        body.contains("loading"),
        "the loading row is shown: {body:?}"
    );
}

// ---------------------------------------------------------------------------
// TC-009 - A store with no categories browses with ALL only (AC7, FR-018)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tc_009_a_category_less_index_browses_normally_with_all_only() {
    let mut app = new_app();
    open_fixture(
        &mut app,
        StoreKind::Codex,
        fixture_store_bytes("index-nocat.json"),
    )
    .await;

    assert_eq!(browser(&app).category_row_count(), 1, "ALL only");
    assert_eq!(category_rows(&app).len(), 0);
    assert!(
        !app.plugin_store
            .as_mut()
            .expect("panel open")
            .category_move_down(),
        "no row below ALL"
    );
    assert_eq!(browser(&app).all.len(), 6, "every entry still browses");

    // The query still filters normally and the navigator stays on ALL.
    for c in "time".chars() {
        handle_key(&mut app, key(KeyCode::Char(c))).await;
    }
    assert_eq!(filtered_ids(&app), ["codex-time"]);
    assert_eq!(browser(&app).active_category(), "ALL");
}

// ---------------------------------------------------------------------------
// TC-010 - A category matches only its own entries, exactly and case-insensitively
// (FR-003, FR-027)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tc_010_a_category_matches_its_entries_case_insensitively() {
    let mut app = new_app();
    open_fixture(&mut app, StoreKind::Codex, codex_index()).await;

    // `codex-time` (`Utility`) and `codex-shell` (`utility`) both fold into the
    // single `Utility` row.
    let utility_rows = category_rows(&app)
        .iter()
        .filter(|c| c.eq_ignore_ascii_case("utility"))
        .count();
    assert_eq!(utility_rows, 1, "one Utility-equivalent row only");

    select_category(&mut app, "Utility").await;
    assert_eq!(filtered_ids(&app), ["codex-time", "codex-shell"]);
    assert!(
        !filtered_ids(&app).iter().any(|id| id == "codex-uncat"),
        "the uncategorised entry is not under a specific category"
    );

    handle_key(&mut app, key(KeyCode::Char('c'))).await;
    assert!(
        filtered_ids(&app).iter().any(|id| id == "codex-uncat"),
        "ALL shows the uncategorised entry again"
    );
}

// ---------------------------------------------------------------------------
// TC-011 - Empty / error states keep the navigator visible (AC8, FR-019, FR-026)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tc_011_error_state_keeps_the_navigator_visible_and_does_not_panic() {
    let mut app = new_app();
    let malformed = br#"{ "store": "codex", "plugins": ["#.to_vec();
    open_fixture(&mut app, StoreKind::Codex, malformed).await;

    // The malformed index renders the inline failure line and an ALL-only
    // navigator; the process neither panics nor exits (FR-026).
    assert_eq!(browser(&app).category_row_count(), 1);
    let terminal = render(&mut app, 120, 40);
    let area = app.plugin_store_area;
    let body = (area.y + 1..area.y + area.height - 1)
        .map(|y| {
            row_text(
                &terminal,
                y,
                area.x + 1,
                area.x + area.width.saturating_sub(1),
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        body.contains("store index failed"),
        "the failure line is shown: {body:?}"
    );
    assert!(
        app.plugin_store_nav_area.width > 0,
        "navigator still visible"
    );
}

// ---------------------------------------------------------------------------
// TC-012 - Resize re-derives the navigator width (FR-004, FR-020)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tc_012_resize_re_derives_the_navigator_width() {
    let mut app = new_app();
    open_fixture(&mut app, StoreKind::Codex, codex_index()).await;

    let wide = render(&mut app, 120, 40);
    let _wide_area = app.plugin_store_area;
    let wide_nav = app.plugin_store_nav_area;
    drop(wide);

    let narrow = render(&mut app, 60, 20);
    let narrow_area = app.plugin_store_area;
    let narrow_nav = app.plugin_store_nav_area;
    drop(narrow);

    assert!(narrow_area.width <= 60 && narrow_area.height <= 20);
    assert!(
        narrow_nav.width > 0,
        "the navigator stays visible when narrow"
    );
    assert!(
        narrow_nav.width <= wide_nav.width,
        "the navigator never grows past the wide layout"
    );
    assert!(
        narrow_nav.x + narrow_nav.width <= narrow_area.x + narrow_area.width,
        "the navigator is never clipped off the panel"
    );
}

// ---------------------------------------------------------------------------
// TC-013 - Keyboard-only path reaches every category (FR-023, FR-024)
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tc_013_every_category_is_reachable_by_keyboard_alone() {
    let mut app = new_app();
    open_fixture(&mut app, StoreKind::Codex, codex_index()).await;
    handle_key(&mut app, key(KeyCode::Tab)).await;

    let rows = browser(&app).category_row_count();
    let mut visited = Vec::new();
    for _ in 0..rows + 2 {
        visited.push(browser(&app).active_category().to_string());
        handle_key(&mut app, key(KeyCode::Down)).await;
    }
    for row in 0..rows {
        let label = browser(&app).category_row_label(row).to_string();
        assert!(
            visited.iter().any(|seen| seen.eq_ignore_ascii_case(&label)),
            "row {row} (`{label}`) was reachable by keyboard: {visited:?}"
        );
    }
    assert!(
        app.plugin_store_install_result
            .lock()
            .map(|g| g.is_none())
            .unwrap_or(false),
        "keyboard navigation installs nothing"
    );
}

// ---------------------------------------------------------------------------
// AC10 - the panel is ASCII-only, including the fixture categories
// ---------------------------------------------------------------------------

#[tokio::test]
async fn acceptance_10_the_panel_is_ascii_only_with_the_fixture_categories() {
    let mut app = new_app();
    open_fixture(&mut app, StoreKind::Codex, codex_index()).await;
    let terminal = render(&mut app, 120, 40);
    let area = app.plugin_store_area;
    let buffer = terminal.backend().buffer();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            assert!(
                buffer[(x, y)].symbol().is_ascii(),
                "non-ASCII glyph {:?} at ({x}, {y})",
                buffer[(x, y)].symbol()
            );
        }
    }
}
