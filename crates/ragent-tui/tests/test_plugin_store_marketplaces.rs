//! Tests that a store with additional marketplaces fetches and merges every
//! marketplace into the one browse panel (spec `pluginstores` FR-048).
//!
//! Each test replaces the app's production HTTPS fetcher with an offline
//! [`FixtureStoreFetcher`] seeded for the compiled default endpoint and for an
//! added marketplace URL, and drives the off-loop fetch exactly as the event loop
//! does. No live store is contacted.

use std::time::Duration;

use serde_json::json;

use ragent_config::{MarketplaceEntry, PluginStoresConfig};
use ragent_plugins::{FixtureStoreFetcher, StoreKind};
use ragent_tui::App;
use ragent_tui::app::{PluginStoreBrowser, PluginStoreStatus};

#[path = "support/mod.rs"]
mod support;

/// The community marketplace URL the resolver produces for
/// `anthropics/claude-plugins-community`.
const COMMUNITY_URL: &str = "https://raw.githubusercontent.com/anthropics/claude-plugins-community/main/.claude-plugin/marketplace.json";

/// A store-index document with `entries`.
fn index_bytes(entries: Vec<serde_json::Value>) -> Vec<u8> {
    serde_json::to_vec(&json!({ "plugins": entries })).expect("index json")
}

/// An entry with `id` and an archive source.
fn entry(id: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": id,
        "version": "1.0.0",
        "source": format!("https://example.org/{id}.zip"),
    })
}

/// A `plugins.stores` block adding `COMMUNITY_URL` to the Claude store.
fn stores_with_community() -> PluginStoresConfig {
    PluginStoresConfig {
        marketplaces: vec![MarketplaceEntry {
            store: "claude".to_string(),
            name: "claude-plugins-community".to_string(),
            url: Some(COMMUNITY_URL.to_string()),
            source: "anthropics/claude-plugins-community".to_string(),
        }],
        ..PluginStoresConfig::default()
    }
}

/// An app whose fetch seam serves the compiled Claude default plus the community
/// marketplace, with the Claude panel open and no fetch started.
fn app_with_community() -> App {
    let default_url = StoreKind::Claude.default_url();
    let fetcher = FixtureStoreFetcher::new()
        .with_index(
            default_url,
            index_bytes(vec![entry("claude-official"), entry("shared-plugin")]),
        )
        .with_index(
            COMMUNITY_URL,
            index_bytes(vec![entry("community-one"), entry("shared-plugin")]),
        );
    let mut app = support::make_app();
    app.set_plugin_store_fetcher(std::sync::Arc::new(fetcher));
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Claude, "", false));
    app
}

/// Drive the off-loop fetch for the Claude store and apply the delivered result.
async fn drive_fetch(app: &mut App) {
    app.spawn_plugin_store_fetch_with_stores(StoreKind::Claude, &stores_with_community());

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
    assert!(delivered, "the off-loop fetch delivered a result");

    app.poll_plugin_store_result();
}

#[tokio::test]
async fn every_configured_marketplace_merges_into_the_one_panel() {
    let mut app = app_with_community();

    drive_fetch(&mut app).await;

    let browser = app.plugin_store.as_ref().expect("panel open");
    assert_eq!(browser.status, PluginStoreStatus::Ready);
    // Entries from the compiled default and from the added community marketplace
    // are both present in the single panel.
    for id in ["claude-official", "community-one"] {
        assert!(
            browser.all.iter().any(|e| e.id == id),
            "the merged set carries `{id}` from every marketplace: {:?}",
            browser.all.iter().map(|e| &e.id).collect::<Vec<_>>()
        );
    }
}

#[tokio::test]
async fn each_row_is_attributed_to_the_marketplace_it_came_from() {
    // Every row is tagged with the display name of the marketplace it was fetched
    // from: an entry from the store's base endpoint carries the store label, and an
    // entry from an added marketplace carries the name it was added under (FR-048).
    let mut app = app_with_community();

    drive_fetch(&mut app).await;

    let browser = app.plugin_store.as_ref().expect("panel open");
    let marketplace_of = |id: &str| {
        browser
            .all
            .iter()
            .find(|e| e.id == id)
            .map(|e| e.marketplace.clone())
            .unwrap_or_default()
    };
    assert_eq!(
        marketplace_of("claude-official"),
        "Claude",
        "the base endpoint attributes to the store label"
    );
    assert_eq!(
        marketplace_of("community-one"),
        "claude-plugins-community",
        "an added marketplace attributes to its configured name"
    );
}

#[tokio::test]
async fn an_id_listed_by_two_marketplaces_appears_once() {
    let mut app = app_with_community();

    drive_fetch(&mut app).await;

    let browser = app.plugin_store.as_ref().expect("panel open");
    let shared = browser
        .all
        .iter()
        .filter(|e| e.id == "shared-plugin")
        .count();
    assert_eq!(shared, 1, "a plugin listed twice is de-duplicated (FR-048)");
}

#[tokio::test]
async fn a_refused_base_endpoint_still_reports_the_panel_error() {
    // A non-https base override with an added marketplace present: the base is
    // refused and, because it is the store's primary endpoint, the panel reports
    // the refusal rather than silently serving only the added marketplace.
    let stores = PluginStoresConfig {
        claude: Some(ragent_config::PluginStoreEndpoint {
            url: Some("http://insecure.example/claude.json".to_string()),
        }),
        marketplaces: stores_with_community().marketplaces,
        ..PluginStoresConfig::default()
    };
    let mut app = support::make_app();
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Claude, "", false));
    app.spawn_plugin_store_fetch_with_stores(StoreKind::Claude, &stores);

    let browser = app.plugin_store.as_ref().expect("panel open");
    match &browser.status {
        PluginStoreStatus::Failed(detail) => {
            assert!(
                detail.contains("https"),
                "refusal names the scheme: {detail}"
            )
        }
        other => panic!("expected a refused base endpoint to be reported, got {other:?}"),
    }
}
