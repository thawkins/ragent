//! End-to-end tests that the injected store-fetch seam drives the browser's
//! off-loop fetch against each compiled default endpoint with no network (spec
//! `pluginstores` T-019; FR-032, FR-033, FR-037, NFR-003, NFR-004).
//!
//! Each test replaces the app's production HTTPS fetcher with an offline
//! [`FixtureStoreFetcher`] seeded for both compiled defaults, launches the fetch
//! with no `plugins.stores` override, and drains the delivery slot exactly as the
//! event loop does. No live store is contacted and no plugin is installed.

use std::time::Duration;

use ragent_config::PluginStoresConfig;
use ragent_plugins::{FixtureStoreFetcher, StoreKind};
use ragent_tui::App;
use ragent_tui::app::{PluginStoreBrowser, PluginStoreStatus};

#[path = "support/mod.rs"]
mod support;

/// An app whose fetch seam is the offline fixture fetcher for both compiled
/// defaults, with the browse panel for `kind` open and no fetch started.
fn app_with_fixture_seam(kind: StoreKind) -> App {
    let mut app = support::make_app();
    app.set_plugin_store_fetcher(std::sync::Arc::new(
        FixtureStoreFetcher::new().with_default_endpoints(),
    ));
    app.plugin_store = Some(PluginStoreBrowser::new(kind, "", false));
    app
}

/// Wait (bounded) for the off-loop fetch to deposit its result, then apply it.
async fn drive_fetch(app: &mut App, kind: StoreKind) {
    // No `plugins.stores` block: the effective endpoint is the compiled default
    // and the fixture fetcher serves it offline (FR-033, FR-037).
    app.spawn_plugin_store_fetch_with_stores(kind, &PluginStoresConfig::default());

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

#[tokio::test]
async fn the_fixture_seam_populates_the_codex_panel_offline() {
    let mut app = app_with_fixture_seam(StoreKind::Codex);

    drive_fetch(&mut app, StoreKind::Codex).await;

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.status, PluginStoreStatus::Ready);
    assert!(
        browser.all.iter().any(|e| e.id == "codex-weather"),
        "the compiled Codex default fixture populated the result set (FR-032)"
    );
    assert!(
        browser.all.iter().any(|e| e.id == "codex-time"),
        "the tolerant entry without optional fields also loaded"
    );
}

#[tokio::test]
async fn the_fixture_seam_populates_the_claude_panel_offline() {
    let mut app = app_with_fixture_seam(StoreKind::Claude);

    drive_fetch(&mut app, StoreKind::Claude).await;

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.status, PluginStoreStatus::Ready);
    assert!(
        browser.all.iter().any(|e| e.id == "claude-todo"),
        "the compiled Claude default fixture populated the result set (FR-032, NFR-004)"
    );
}

#[tokio::test]
async fn the_fixture_seam_serves_both_defaults_without_a_stores_block() {
    // No `plugins.stores` block is present for either store, so both resolve to
    // their compiled default and both fetch succeeds offline (FR-033, NFR-004).
    for kind in StoreKind::ALL {
        let mut app = app_with_fixture_seam(kind);
        drive_fetch(&mut app, kind).await;
        let status = app
            .plugin_store
            .as_ref()
            .expect("panel open")
            .status
            .clone();
        assert_eq!(
            status,
            PluginStoreStatus::Ready,
            "{} default fixture populated the panel",
            kind.token()
        );
    }
}
