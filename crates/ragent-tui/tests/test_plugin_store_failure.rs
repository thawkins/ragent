//! Failure-reporting tests for the plugin-store browser: malformed store data
//! and fetch failures are formatted as panel/message reports and never panic
//! (spec `pluginstores` T-010; FR-013, FR-025).
//!
//! FR-013 requires a failed fetch to render an inline error row naming the cause
//! with the panel left open and dismissible. FR-025 forbids any panic or process
//! termination from a store-index fetch, a malformed index entry, an oversized
//! index, or a failed install, and requires every such failure to be contained
//! and reported in the panel or the message window. A malformed entry that the
//! parser skipped is contained data too, so it is reported as a partial result
//! (a `skipped` count in the panel title and empty line) rather than being
//! silently dropped.
//!
//! These tests drive the real renderer through a `TestBackend` and the real poll
//! path through the shared delivery slot, so nothing is contacted over the
//! network and no plugin is installed.

use ratatui::Terminal;
use ratatui::backend::TestBackend;

use ragent_plugins::{StoreEntry, StoreError, StoreIndex, StoreKind};
use ragent_tui::App;
use ragent_tui::app::{PluginStoreBrowser, PluginStoreFetchResult, PluginStoreStatus};
use ragent_tui::layout;

#[path = "support/mod.rs"]
mod support;

fn entry(id: &str, description: &str) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: "1.0.0".to_string(),
        source: format!("https://example.org/{id}.zip"),
        description: description.to_string(),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
    }
}

/// A browser with the given accepted entries and skipped-malformed count loaded.
fn loaded(entries: Vec<StoreEntry>, skipped: usize) -> PluginStoreBrowser {
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    browser.set_index(StoreIndex {
        store: Some("codex".to_string()),
        entries,
        skipped,
    });
    browser
}

/// An app with the Codex panel open and no fetch started.
fn open_loading() -> App {
    let mut app = support::make_app();
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Codex, "", false));
    app
}

/// Deposit one fetch outcome into the app's delivery slot.
fn deliver(app: &App, kind: StoreKind, outcome: Result<StoreIndex, StoreError>) {
    let mut guard = app.plugin_store_result.lock().expect("slot");
    *guard = Some(PluginStoreFetchResult { kind, outcome });
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

/// Every painted row of the panel's inner area (borders stripped).
fn panel_rows(terminal: &Terminal<TestBackend>, app: &App) -> Vec<String> {
    let area = app.plugin_store_area;
    let x0 = area.x + 1;
    let x1 = area.x + area.width.saturating_sub(1);
    (area.y + 1..area.y + area.height.saturating_sub(1))
        .map(|y| row_text(terminal, y, x0, x1))
        .collect()
}

/// The whole painted panel row `y`, borders included.
fn full_row(terminal: &Terminal<TestBackend>, y: u16) -> String {
    let width = terminal.backend().buffer().area.width;
    row_text(terminal, y, 0, width)
}

// ── Malformed entries are carried and counted (FR-025) ──────────────────────

#[test]
fn set_index_carries_the_skipped_malformed_count() {
    let browser = loaded(vec![entry("codex-weather", "Weather")], 3);
    assert_eq!(browser.skipped, 3);
    assert_eq!(browser.all.len(), 1);
}

#[test]
fn set_entries_clears_the_skipped_count() {
    let mut browser = loaded(vec![entry("codex-weather", "Weather")], 4);
    browser.set_entries(vec![entry("codex-time", "Time")]);
    assert_eq!(browser.skipped, 0, "a bare entry list has no skipped count");
}

#[test]
fn a_delivered_index_carries_the_skipped_count_into_the_browser() {
    let mut app = open_loading();
    // One valid entry and two the parser rejected as malformed.
    deliver(
        &app,
        StoreKind::Codex,
        Ok(StoreIndex {
            store: Some("codex".to_string()),
            entries: vec![entry("codex-weather", "Weather")],
            skipped: 2,
        }),
    );

    app.poll_plugin_store_result();

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.status, PluginStoreStatus::Ready);
    assert_eq!(browser.skipped, 2, "the skipped count survives the poll");
}

// ── The skipped count is reported in the panel (FR-025) ─────────────────────

#[test]
fn a_partial_index_reports_the_skipped_count_in_the_title() {
    let mut app = support::make_app();
    app.plugin_store = Some(loaded(
        vec![
            entry("codex-weather", "Weather"),
            entry("codex-time", "Time"),
        ],
        3,
    ));

    let terminal = render(&mut app, 100, 30);
    let title = full_row(&terminal, app.plugin_store_area.y);
    assert!(title.contains("2 of 2"), "title: {title:?}");
    assert!(
        title.contains("3 skipped"),
        "the title names the malformed-entry count (FR-025): {title:?}"
    );
}

#[test]
fn a_clean_index_omits_the_skipped_count_from_the_title() {
    let mut app = support::make_app();
    app.plugin_store = Some(loaded(vec![entry("codex-weather", "Weather")], 0));

    let terminal = render(&mut app, 100, 30);
    let title = full_row(&terminal, app.plugin_store_area.y);
    assert!(
        !title.contains("skipped"),
        "a clean index advertises nothing: {title:?}"
    );
}

#[test]
fn an_all_malformed_index_reports_the_skipped_count_in_the_empty_line() {
    let mut app = support::make_app();
    // Every entry was malformed, so the accepted list is empty but three were
    // seen and rejected: the empty line must say so (FR-017, FR-025).
    app.plugin_store = Some(loaded(Vec::new(), 3));

    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app);
    assert!(
        rows.iter()
            .any(|r| r.contains("no plugins") && r.contains("3 malformed")),
        "the empty line names the malformed count (FR-025): {rows:?}"
    );
}

#[test]
fn a_no_match_query_reports_the_skipped_count() {
    let mut app = support::make_app();
    let mut browser = loaded(vec![entry("codex-weather", "Weather")], 2);
    browser.set_query("zzzzzz".to_string());
    app.plugin_store = Some(browser);

    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app);
    assert!(
        rows.iter()
            .any(|r| r.contains("no matching") && r.contains("2 malformed")),
        "the no-match line names the malformed count (FR-025): {rows:?}"
    );
}

// ── A failed fetch is formatted inline and stays dismissible (FR-013) ────────

#[test]
fn the_error_body_names_the_malformed_json_cause() {
    let mut app = support::make_app();
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Codex, "", false));
    app.plugin_store.as_mut().expect("panel open").set_failed(
        StoreError::MalformedJson {
            detail: "expected value at line 3 column 7".to_string(),
        }
        .to_string(),
    );

    let terminal = render(&mut app, 100, 30);
    let rows = panel_rows(&terminal, &app);
    assert!(
        rows.iter()
            .any(|r| r.contains("failed") && r.contains("line 3 column 7")),
        "the inline error names the parse position: {rows:?}"
    );
    assert!(app.plugin_store.is_some(), "the panel stays open (FR-013)");
}

#[test]
fn a_malformed_index_error_polled_from_the_slot_is_contained() {
    let mut app = open_loading();
    deliver(
        &app,
        StoreKind::Codex,
        Err(StoreError::MalformedJson {
            detail: "expected value at line 1 column 1".to_string(),
        }),
    );

    app.poll_plugin_store_result();

    match &app.plugin_store.as_ref().expect("panel open").status {
        PluginStoreStatus::Failed(detail) => {
            assert!(detail.contains("line 1 column 1"), "detail: {detail}")
        }
        other => panic!("expected a contained inline failure, got {other:?}"),
    }
    assert!(app.plugin_store.is_some(), "no panic closed the panel");
}

// ── No render path panics on hostile/edge store data (FR-025) ───────────────

#[test]
fn an_out_of_range_cursor_with_no_results_does_not_panic_on_render() {
    let mut app = support::make_app();
    let mut browser = PluginStoreBrowser::new(StoreKind::Codex, "", false);
    // Empty filtered set but a non-zero cursor: the renderer must clamp, not panic.
    browser.cursor = 99;
    browser.set_index(StoreIndex {
        store: None,
        entries: Vec::new(),
        skipped: 1,
    });
    browser.cursor = 99;
    app.plugin_store = Some(browser);

    let _ = render(&mut app, 80, 24);
    assert!(app.plugin_store.is_some());
}

#[test]
fn a_failed_state_renders_on_a_small_terminal_without_panicking() {
    let mut app = support::make_app();
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Codex, "", false));
    app.plugin_store
        .as_mut()
        .expect("panel open")
        .set_failed("network unreachable".to_string());

    // A small-but-usable panel must still paint the failure line (truncated to
    // the panel width) and keep the panel open, with no panic (FR-013, FR-018,
    // FR-025).
    let terminal = render(&mut app, 40, 14);
    let rows = panel_rows(&terminal, &app);
    assert!(
        rows.iter().any(|r| r.contains("failed: network")),
        "the small panel still names the cause: {rows:?}"
    );
    assert!(app.plugin_store.is_some());
}

#[test]
fn the_partial_state_panel_body_is_ascii_only() {
    let mut app = support::make_app();
    app.plugin_store = Some(loaded(Vec::new(), 2));
    let terminal = render(&mut app, 100, 30);
    for row in panel_rows(&terminal, &app) {
        assert!(
            row.is_ascii(),
            "the panel body uses only ASCII glyphs (acceptance 10): {row:?}"
        );
    }
}
