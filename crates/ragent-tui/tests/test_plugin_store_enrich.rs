//! Tests for the plugin-store manifest-metadata enrichment: the off-loop worker
//! that fills each entry's version/description from the plugin's own manifest, and
//! the poll that applies the streamed updates to the open panel (spec `pluginstores`
//! FR-048).
//!
//! The manifest source is replaced with an offline implementation, so no live
//! store or manifest is contacted (NFR-003). The enrichment is driven through the
//! real wiring: a store index is delivered into the fetch slot and drained by
//! `poll_plugin_store_result`, which starts the enrichment of the loaded entries.

use ragent_plugins::{ManifestMeta, ManifestSource, StoreEntry, StoreIndex, StoreKind};
use ragent_tui::App;
use ragent_tui::app::{PluginStoreBrowser, PluginStoreFetchResult};

#[path = "support/mod.rs"]
mod support;

/// An offline manifest source answering from a fixed table.
struct FakeSource {
    meta_by_id: Vec<(String, ManifestMeta)>,
}

impl ManifestSource for FakeSource {
    fn manifest_meta(&self, entry: &StoreEntry, _kind: StoreKind) -> Option<ManifestMeta> {
        self.meta_by_id
            .iter()
            .find(|(id, _)| id == &entry.id)
            .map(|(_, meta)| meta.clone())
    }
}

fn entry(id: &str, source: &str) -> StoreEntry {
    StoreEntry {
        id: id.to_string(),
        name: id.to_string(),
        version: String::new(),
        source: source.to_string(),
        description: String::new(),
        dialect: None,
        tags: Vec::new(),
        homepage: None,
        category: None,
        marketplace: String::new(),
    }
}

/// An app with the Codex panel open and no fetch started.
fn open_loading() -> App {
    let mut app = support::make_app();
    app.plugin_store = Some(PluginStoreBrowser::new(StoreKind::Codex, "", false));
    app
}

/// Deliver a store index into the fetch slot, as the off-loop fetch would.
fn deliver(app: &App, kind: StoreKind, entries: Vec<StoreEntry>) {
    let mut guard = app.plugin_store_result.lock().expect("slot");
    *guard = Some(PluginStoreFetchResult {
        kind,
        outcome: Ok(StoreIndex {
            store: Some(kind.token().to_string()),
            entries,
            skipped: 0,
        }),
    });
}

/// Drain streamed enrichment updates until the terminal `Finished` clears the
/// loading hint.
async fn drain(app: &mut App) {
    for _ in 0..600 {
        app.poll_store_enrichment();
        if app
            .plugin_store
            .as_ref()
            .map(|b| !b.enriching)
            .unwrap_or(false)
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    panic!("the off-loop enrichment did not finish");
}

#[tokio::test]
async fn enrichment_fills_the_version_and_description_the_index_omitted() {
    let mut app = open_loading();
    app.set_manifest_source(std::sync::Arc::new(FakeSource {
        meta_by_id: vec![
            (
                "linear".to_string(),
                ManifestMeta {
                    version: Some("1.2.3".to_string()),
                    description: Some("Manage Linear issues".to_string()),
                },
            ),
            (
                "slack".to_string(),
                ManifestMeta {
                    version: None,
                    description: Some("Work with Slack".to_string()),
                },
            ),
        ],
    }));
    deliver(
        &app,
        StoreKind::Codex,
        vec![
            entry(
                "linear",
                "git+https://github.com/openai/plugins#main:plugins/linear",
            ),
            entry(
                "slack",
                "git+https://github.com/openai/plugins#main:plugins/slack",
            ),
        ],
    );

    app.poll_plugin_store_result();
    // The panel is now enriching; a title hint would be visible.
    assert!(
        app.plugin_store.as_ref().expect("panel open").enriching,
        "the index landing starts enrichment"
    );
    drain(&mut app).await;

    let browser = app.plugin_store.as_ref().expect("panel open");
    let linear = browser
        .all
        .iter()
        .find(|e| e.id == "linear")
        .expect("linear");
    assert_eq!(
        linear.version, "1.2.3",
        "the manifest version filled the row"
    );
    assert_eq!(linear.description, "Manage Linear issues");
    let slack = browser.all.iter().find(|e| e.id == "slack").expect("slack");
    assert_eq!(slack.version, "", "the manifest omitted a version");
    assert_eq!(slack.description, "Work with Slack");
    assert!(!browser.enriching, "the loading hint clears on finish");
}

#[tokio::test]
async fn enrichment_keeps_a_version_the_index_already_declared() {
    let mut app = open_loading();
    app.set_manifest_source(std::sync::Arc::new(FakeSource {
        meta_by_id: vec![(
            "clangd".to_string(),
            ManifestMeta {
                version: Some("1.0.0".to_string()),
                description: Some("LSP".to_string()),
            },
        )],
    }));
    let mut with_version = entry(
        "clangd",
        "git+https://github.com/anthropics/x#main:plugins/clangd",
    );
    with_version.version = "9.9.9".to_string();
    deliver(&app, StoreKind::Codex, vec![with_version]);

    app.poll_plugin_store_result();
    drain(&mut app).await;

    let browser = app.plugin_store.as_ref().expect("panel open");
    let clangd = browser
        .all
        .iter()
        .find(|e| e.id == "clangd")
        .expect("clangd");
    assert_eq!(
        clangd.version, "9.9.9",
        "the index's declared version is not overwritten by the manifest"
    );
    assert_eq!(
        clangd.description, "LSP",
        "a blank description is still filled"
    );
}

#[tokio::test]
async fn an_update_from_a_stale_generation_is_dropped() {
    // A second enrichment (a re-fetch) bumps the generation, so an update tagged
    // with the earlier generation must not be applied to the current entries.
    let mut app = open_loading();
    app.set_manifest_source(std::sync::Arc::new(FakeSource {
        meta_by_id: vec![(
            "linear".to_string(),
            ManifestMeta {
                version: Some("1.2.3".to_string()),
                description: Some("Manage Linear issues".to_string()),
            },
        )],
    }));
    deliver(
        &app,
        StoreKind::Codex,
        vec![entry(
            "linear",
            "git+https://github.com/openai/plugins#main:plugins/linear",
        )],
    );

    app.poll_plugin_store_result();
    // Bump past the started generation before draining, simulating a re-fetch.
    app.store_enrich_generation
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    for _ in 0..200 {
        app.poll_store_enrichment();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    let browser = app.plugin_store.as_ref().expect("panel open");
    let linear = browser
        .all
        .iter()
        .find(|e| e.id == "linear")
        .expect("linear");
    assert_eq!(
        linear.version, "",
        "a stale-generation update is not applied to the current entries"
    );
}
