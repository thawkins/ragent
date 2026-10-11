//! Tests for the connector-catalogue manifest-description enrichment wiring: the
//! off-loop worker that fills each entry's blank description from its own
//! connector manifest, and the poll that applies the streamed updates to the open
//! panel (spec `connectors` FR-044).
//!
//! The manifest source is replaced with an offline implementation, so no live
//! catalogue or manifest is contacted (NFR-003). The enrichment is driven through
//! the real wiring: a catalogue is delivered into the fetch slot and drained by
//! `poll_connector_catalogue_result`, which starts the enrichment of the loaded
//! entries.

use std::sync::Arc;

use ragent_connectors::catalogue_enrich::{ManifestMeta, ManifestSource};
use ragent_connectors::{
    CategoryFilter, ConnectorAuthShape, ConnectorDescriptor, ConnectorId, ConnectorServer,
};
use ragent_tui::App;
use ragent_tui::app::ConnectorCatalogueResult;

#[path = "support/mod.rs"]
mod support;

/// An offline manifest source answering from a fixed table keyed by entry id.
struct FakeSource {
    meta_by_id: Vec<(String, ManifestMeta)>,
}

impl ManifestSource for FakeSource {
    fn manifest_meta(&self, entry: &ConnectorDescriptor) -> Option<ManifestMeta> {
        self.meta_by_id
            .iter()
            .find(|(id, _)| id == entry.id.as_str())
            .map(|(_, meta)| meta.clone())
    }
}

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

/// A catalogue descriptor with a github git install source and a blank
/// description.
fn entry(id: &str) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("valid id"),
        name: id.to_string(),
        description: String::new(),
        category: "developer".to_string(),
        tags: Vec::new(),
        source: format!("git+https://github.com/acme/connectors#v1:packages/{id}"),
        marketplace: String::new(),
        provenance: Default::default(),
        auth: ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers: vec![server()],
        unsupported: Vec::new(),
    }
}

/// An app with the connector panel open and no fetch started.
fn open_loading() -> App {
    let mut app = support::make_app();
    app.open_connector_catalogue("", CategoryFilter::all(), false);
    app
}

/// Deliver a catalogue into the fetch slot, as the off-loop fetch would.
fn deliver(app: &App, entries: Vec<ConnectorDescriptor>) {
    let mut guard = app.connector_catalogue_result.lock().expect("slot");
    *guard = Some(ConnectorCatalogueResult {
        outcome: Ok(entries),
        skipped: 0,
        refused_marketplace: None,
    });
}

/// Drain streamed enrichment updates until the terminal `Finished` clears the
/// loading hint.
async fn drain(app: &mut App) {
    for _ in 0..600 {
        app.poll_connector_enrichment();
        if app
            .connector_store
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
async fn enrichment_fills_the_description_the_catalogue_omitted() {
    let mut app = open_loading();
    app.set_catalogue_manifest_source(Arc::new(FakeSource {
        meta_by_id: vec![(
            "linear".to_string(),
            ManifestMeta {
                description: Some("Manage Linear issues".to_string()),
                ..Default::default()
            },
        )],
    }));
    deliver(&app, vec![entry("linear")]);

    app.poll_connector_catalogue_result();
    drain(&mut app).await;

    let browser = app.connector_store.as_ref().expect("panel open");
    let linear = browser
        .all
        .iter()
        .find(|e| e.id.as_str() == "linear")
        .expect("linear");
    assert_eq!(
        linear.description, "Manage Linear issues",
        "the blank description is filled from the manifest"
    );
    assert!(!browser.enriching, "the loading hint clears on finish");
}

#[tokio::test]
async fn enrichment_keeps_a_description_the_catalogue_already_declared() {
    let mut app = open_loading();
    app.set_catalogue_manifest_source(Arc::new(FakeSource {
        meta_by_id: vec![(
            "linear".to_string(),
            ManifestMeta {
                description: Some("From the manifest".to_string()),
                ..Default::default()
            },
        )],
    }));
    let mut with_description = entry("linear");
    with_description.description = "Declared by the catalogue".to_string();
    deliver(&app, vec![with_description]);

    app.poll_connector_catalogue_result();
    drain(&mut app).await;

    let browser = app.connector_store.as_ref().expect("panel open");
    let linear = browser
        .all
        .iter()
        .find(|e| e.id.as_str() == "linear")
        .expect("linear");
    assert_eq!(
        linear.description, "Declared by the catalogue",
        "the catalogue's declared description is not overwritten"
    );
}

#[tokio::test]
async fn an_update_from_a_stale_generation_is_dropped() {
    // A second enrichment (a re-fetch) bumps the generation, so an update tagged
    // with the earlier generation must not be applied to the current entries.
    let mut app = open_loading();
    app.set_catalogue_manifest_source(Arc::new(FakeSource {
        meta_by_id: vec![(
            "linear".to_string(),
            ManifestMeta {
                description: Some("Manage Linear issues".to_string()),
                ..Default::default()
            },
        )],
    }));
    deliver(&app, vec![entry("linear")]);

    app.poll_connector_catalogue_result();
    // Bump past the started generation before draining, simulating a re-fetch.
    app.connector_enrich_generation
        .fetch_add(1, std::sync::atomic::Ordering::SeqCst);

    for _ in 0..200 {
        app.poll_connector_enrichment();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }

    let browser = app.connector_store.as_ref().expect("panel open");
    let linear = browser
        .all
        .iter()
        .find(|e| e.id.as_str() == "linear")
        .expect("linear");
    assert_eq!(
        linear.description, "",
        "a stale-generation update is not applied to the current entries"
    );
}
