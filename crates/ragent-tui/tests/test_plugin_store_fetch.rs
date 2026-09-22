//! Tests for the off-loop plugin-store index fetch: the spawned blocking fetch,
//! its delivery back through the shared result slot, and the UI-thread poll that
//! applies the outcome to the open panel without blocking the event loop (spec
//! `pluginstores` T-008; FR-007, FR-013, FR-016, FR-017, FR-024, FR-026).
//!
//! The delivery path is deterministic and offline: the tests deposit a
//! [`PluginStoreFetchResult`] directly into the slot and drive
//! `poll_plugin_store_result`, so no live store is contacted. The one off-loop
//! spawn test points at an unreachable loopback endpoint, so the contained
//! [`StoreError`] arrives without any external network dependency.

use std::time::Duration;

use ragent_config::{PluginStoreEndpoint, PluginStoresConfig};
use ragent_plugins::{StoreEntry, StoreError, StoreIndex, StoreKind};
use ragent_tui::App;
use ragent_tui::app::{PluginStoreBrowser, PluginStoreFetchResult, PluginStoreStatus};

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

fn index(entries: Vec<StoreEntry>) -> StoreIndex {
    StoreIndex {
        store: Some("test".to_string()),
        entries,
        skipped: 0,
    }
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

/// The status of the open browser.
fn status(app: &App) -> PluginStoreStatus {
    app.plugin_store
        .as_ref()
        .expect("panel open")
        .status
        .clone()
}

// ── Poll with nothing pending (FR-016) ──────────────────────────────────────

#[test]
fn poll_with_no_pending_result_is_a_noop() {
    let mut app = open_loading();
    app.needs_redraw = false;

    app.poll_plugin_store_result();

    // Nothing arrived, so the panel stays loading and no repaint is requested.
    assert_eq!(status(&app), PluginStoreStatus::Loading);
    assert!(!app.needs_redraw);
}

// ── A delivered index fills the open panel (FR-007, FR-016, FR-017) ─────────

#[test]
fn a_delivered_index_fills_the_open_panel() {
    let mut app = open_loading();
    app.needs_redraw = false;
    deliver(
        &app,
        StoreKind::Codex,
        Ok(index(vec![
            entry("codex-weather", "Weather lookups"),
            entry("codex-todo", "Todo list"),
        ])),
    );

    app.poll_plugin_store_result();

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.status, PluginStoreStatus::Ready);
    assert_eq!(browser.all.len(), 2);
    assert_eq!(browser.filtered.len(), 2);
    assert!(app.needs_redraw, "applying the result requests a repaint");
    // The slot is drained so the same result is not applied twice.
    assert!(app.plugin_store_result.lock().expect("slot").is_none());
}

#[test]
fn a_delivered_empty_index_yields_the_empty_status() {
    let mut app = open_loading();
    deliver(&app, StoreKind::Codex, Ok(index(Vec::new())));

    app.poll_plugin_store_result();

    assert_eq!(status(&app), PluginStoreStatus::Empty);
    assert!(!app.plugin_store.as_ref().expect("panel").has_results());
}

#[test]
fn a_prefilled_query_still_filters_the_delivered_entries() {
    let mut app = support::make_app();
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Codex, "weather", false));
    deliver(
        &app,
        StoreKind::Codex,
        Ok(index(vec![
            entry("codex-weather", "Weather lookups"),
            entry("codex-todo", "Todo list"),
        ])),
    );

    app.poll_plugin_store_result();

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.status, PluginStoreStatus::Ready);
    assert_eq!(browser.filtered.len(), 1, "only the weather entry matches");
    assert_eq!(browser.selected().expect("a match").id, "codex-weather");
}

// ── A failed fetch renders inline (FR-013) ──────────────────────────────────

#[test]
fn a_delivered_error_sets_the_inline_failure() {
    let mut app = open_loading();
    deliver(
        &app,
        StoreKind::Codex,
        Err(StoreError::Http { status: 503 }),
    );

    app.poll_plugin_store_result();

    match status(&app) {
        PluginStoreStatus::Failed(detail) => assert!(
            detail.contains("503"),
            "the inline error names the cause: {detail}"
        ),
        other => panic!("expected a Failed status, got {other:?}"),
    }
    assert!(app.needs_redraw);
}

// ── Stale results are discarded (FR-007) ────────────────────────────────────

#[test]
fn a_result_for_the_other_store_is_discarded() {
    let mut app = open_loading();
    deliver(
        &app,
        StoreKind::Claude,
        Ok(index(vec![entry("claude-todo", "Todo")])),
    );

    app.poll_plugin_store_result();

    // The Codex panel is untouched; the foreign result is consumed and dropped.
    assert_eq!(status(&app), PluginStoreStatus::Loading);
    assert!(app.plugin_store_result.lock().expect("slot").is_none());
}

#[test]
fn a_result_after_the_panel_closed_is_discarded() {
    let mut app = open_loading();
    app.close_plugin_store();
    deliver(
        &app,
        StoreKind::Codex,
        Ok(index(vec![entry("codex-weather", "Weather")])),
    );

    app.poll_plugin_store_result();

    assert!(app.plugin_store.is_none(), "the panel stays closed");
    assert!(app.plugin_store_result.lock().expect("slot").is_none());
}

// ── Endpoint refusal short-circuits the spawn (FR-024) ──────────────────────

#[test]
fn a_refused_non_https_endpoint_reports_inline_without_fetching() {
    let mut app = open_loading();
    // A non-https override is refused at resolution, so no spawn happens and the
    // error is rendered as the panel's inline detail (FR-013, FR-024).
    let stores = PluginStoresConfig {
        codex: Some(PluginStoreEndpoint {
            url: Some("http://cfg.example/codex.json".to_string()),
        }),
        ..PluginStoresConfig::default()
    };

    app.spawn_plugin_store_fetch_with_stores(StoreKind::Codex, &stores);

    match status(&app) {
        PluginStoreStatus::Failed(detail) => {
            assert!(
                detail.contains("https"),
                "refusal names the scheme: {detail}"
            )
        }
        other => panic!("expected a Failed status, got {other:?}"),
    }
    assert!(app.plugin_store_result.lock().expect("slot").is_none());
}

// ── No reactor: the fetch is skipped, not run inline (FR-016, FR-026) ───────

#[test]
fn spawning_without_a_runtime_skips_the_fetch_and_keeps_loading() {
    let mut app = open_loading();

    // This plain #[test] has no tokio reactor, so the off-loop fetch cannot be
    // started; the panel must stay in its loading state rather than blocking the
    // caller with an inline fetch.
    app.spawn_plugin_store_fetch_with_stores(StoreKind::Codex, &PluginStoresConfig::default());

    assert_eq!(status(&app), PluginStoreStatus::Loading);
    assert!(app.plugin_store_result.lock().expect("slot").is_none());
}

// ── A poisoned slot never panics the poll (FR-025) ──────────────────────────

#[test]
fn poll_recovers_a_poisoned_slot() {
    let mut app = open_loading();
    // Poison the slot by panicking while its lock is held.
    let slot = std::sync::Arc::clone(&app.plugin_store_result);
    let handle = std::thread::spawn(move || {
        let _guard = slot.lock().expect("slot");
        panic!("poison the slot");
    });
    assert!(handle.join().is_err(), "the thread panicked as intended");

    // The poll must recover (recover_poisoned) and return without panicking.
    app.poll_plugin_store_result();

    assert_eq!(status(&app), PluginStoreStatus::Loading);
}

// ── The spawned off-loop path delivers a contained error (FR-016, FR-026) ───

#[tokio::test]
async fn an_off_loop_fetch_delivers_a_contained_error_for_an_unreachable_endpoint() {
    let mut app = open_loading();
    // Loopback port 9 refuses immediately, so the blocking fetch returns a
    // contained error without any external network dependency.
    let stores = PluginStoresConfig {
        codex: Some(PluginStoreEndpoint {
            url: Some("https://127.0.0.1:9/index.json".to_string()),
        }),
        timeout_ms: 2000,
        ..PluginStoresConfig::default()
    };

    app.spawn_plugin_store_fetch_with_stores(StoreKind::Codex, &stores);

    // The spawned blocking task deposits its outcome; wait (bounded) for it.
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
    assert!(delivered, "the off-loop fetch delivered its result");

    app.poll_plugin_store_result();

    match status(&app) {
        PluginStoreStatus::Failed(_) => {}
        other => panic!("expected a contained inline failure, got {other:?}"),
    }
}
