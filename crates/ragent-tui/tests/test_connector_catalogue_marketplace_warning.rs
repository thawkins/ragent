//! Tests that a refused added connector marketplace never blinds the browse
//! panel and is surfaced as a warning in the message window instead (spec
//! `connectors` FR-043).
//!
//! The off-loop fetch delivers a [`ConnectorCatalogueResult`] whose
//! `refused_marketplace` carries the refusal; `poll_connector_catalogue_result`
//! must apply the surviving descriptors AND push a `warn` log line naming the
//! refusal. Driving the poll directly with a hand-built result keeps the test
//! offline (NFR-003).

use ragent_connectors::{
    CatalogueBrowseStatus, CategoryFilter, ConnectorAuthShape, ConnectorDescriptor, ConnectorId,
    ConnectorServer,
};
use ragent_tui::App;
use ragent_tui::app::{ConnectorCatalogueResult, LogLevel};

#[path = "support/mod.rs"]
mod support;

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

fn entry(id: &str) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("id"),
        name: format!("{id} connector"),
        description: String::new(),
        category: "data".to_string(),
        tags: Vec::new(),
        source: String::new(),
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

#[test]
fn a_refused_added_marketplace_warns_and_leaves_the_base_rows_intact() {
    let mut app = open_loading();

    // Deliver a merged result carrying the base catalogue's rows plus the refusal
    // of one added marketplace URL.
    *app.connector_catalogue_result.lock().expect("slot") = Some(ConnectorCatalogueResult {
        outcome: Ok(vec![entry("claude-official"), entry("linear")]),
        skipped: 0,
        refused_marketplace: Some("http://insecure.example.org/servers".to_string()),
    });

    app.poll_connector_catalogue_result();

    // The base rows survive: the refused marketplace never blinds the panel.
    let browser = app.connector_store.as_ref().expect("panel open");
    assert!(
        browser
            .all
            .iter()
            .any(|e| e.id.as_str() == "claude-official"),
        "the base catalogue rows must survive a refused added marketplace"
    );
    assert_eq!(browser.status, CatalogueBrowseStatus::Ready);

    // The refusal is visible as a warning line naming the refused URL, not silent.
    assert!(
        app.log_entries.iter().any(|e| {
            e.level == LogLevel::Warn
                && e.message.contains("marketplace")
                && e.message.contains("http://insecure.example.org/servers")
        }),
        "a warn log line must name the skipped marketplace: {:?}",
        app.log_entries
            .iter()
            .map(|e| (&e.level, &e.message))
            .collect::<Vec<_>>()
    );
}

#[test]
fn a_clean_merge_pushes_no_marketplace_warning() {
    let mut app = open_loading();

    *app.connector_catalogue_result.lock().expect("slot") = Some(ConnectorCatalogueResult {
        outcome: Ok(vec![entry("claude-official")]),
        skipped: 0,
        refused_marketplace: None,
    });

    app.poll_connector_catalogue_result();

    assert!(
        !app.log_entries
            .iter()
            .any(|e| e.message.contains("marketplace was skipped")),
        "a clean merge must push no refused-marketplace warning"
    );
}
