//! Tests for the connector-catalogue manifest-description enrichment (spec
//! `connectors` FR-044): deriving the raw manifest URLs from a git install
//! source, parsing a manifest's description, and driving the off-loop
//! enrichment through an injectable, offline manifest source (NFR-003).

use ragent_connectors::catalogue_enrich::{
    ManifestMeta, ManifestSource, NetworkManifestSource, catalogue_manifest_urls,
    enrich_catalogue_into, parse_manifest_meta,
};
use ragent_connectors::{ConnectorAuthShape, ConnectorDescriptor, ConnectorId, ConnectorServer};

/// A minimal expressible stdio server.
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

/// A catalogue descriptor with `id` and the given install `source`.
fn descriptor(id: &str, source: &str) -> ConnectorDescriptor {
    ConnectorDescriptor {
        id: ConnectorId::new(id).expect("valid id"),
        name: id.to_string(),
        description: String::new(),
        category: "developer".to_string(),
        tags: Vec::new(),
        source: source.to_string(),
        marketplace: String::new(),
        provenance: Default::default(),
        auth: ConnectorAuthShape::None,
        auth_scope: Vec::new(),
        credential: None,
        servers: vec![server()],
        unsupported: Vec::new(),
    }
}

#[test]
fn a_github_git_source_derives_the_raw_manifest_urls() {
    let urls =
        catalogue_manifest_urls("git+https://github.com/acme/connectors#v1.2.0:packages/echo");
    assert_eq!(
        urls,
        vec![
            "https://raw.githubusercontent.com/acme/connectors/v1.2.0/packages/echo/connector.json",
            "https://raw.githubusercontent.com/acme/connectors/v1.2.0/packages/echo/.claude-plugin/connector.json",
        ]
    );
}

#[test]
fn a_non_github_source_derives_no_manifest_url() {
    assert_eq!(
        catalogue_manifest_urls("https://example.org/echo.zip"),
        [] as [std::string::String; 0]
    );
    assert_eq!(
        catalogue_manifest_urls("not-a-source"),
        [] as [std::string::String; 0]
    );
}

#[test]
fn a_non_https_git_scheme_derives_no_manifest_url() {
    // SEC-ragent-plugins-001: only `https://` and `file://` remotes are accepted,
    // so an ssh or plain-http remote derives nothing.
    assert_eq!(
        catalogue_manifest_urls("git+ssh://git@github.com/acme/connectors#v1"),
        [] as [std::string::String; 0]
    );
    assert_eq!(
        catalogue_manifest_urls("git+http://github.com/acme/connectors#v1"),
        [] as [std::string::String; 0]
    );
    assert_eq!(
        catalogue_manifest_urls("git+git://github.com/acme/connectors#v1"),
        [] as [std::string::String; 0]
    );
}

#[test]
fn a_dangerous_git_ref_or_subpath_is_dropped_not_emitted() {
    // SEC-ragent-plugins-001: a `git+` source is untrusted, so a ref git would
    // parse as an option (`--upload-pack=...`) is refused and the URL falls back
    // to the default branch; the raw ref never reaches the derived URL.
    let urls = catalogue_manifest_urls(
        "git+https://github.com/acme/connectors#--upload-pack=/bin/sh:packages/echo",
    );
    assert_ne!(urls, [] as [std::string::String; 0]);
    for url in &urls {
        assert!(!url.contains("--upload-pack"), "{url}");
        assert!(
            url.contains("/HEAD/"),
            "the unsafe ref must fall back to HEAD: {url}"
        );
    }
    // A traversal subpath is dropped entirely, leaving a subpath-less URL.
    for url in catalogue_manifest_urls("git+https://github.com/acme/connectors#v1:../etc") {
        assert!(!url.contains(".."), "{url}");
        assert!(url.contains("/v1/"), "the safe ref is kept: {url}");
        assert!(
            !url.contains("/etc/"),
            "the unsafe subpath is dropped: {url}"
        );
    }
    // A safe ref still derives the URLs (no subpath).
    let urls = catalogue_manifest_urls("git+https://github.com/acme/connectors#v1.2.0");
    assert_eq!(
        urls,
        vec![
            "https://raw.githubusercontent.com/acme/connectors/v1.2.0/connector.json",
            "https://raw.githubusercontent.com/acme/connectors/v1.2.0/.claude-plugin/connector.json",
        ]
    );
}

#[test]
fn parse_manifest_reads_the_top_level_description() {
    let meta = parse_manifest_meta(br#"{"description": "Manage Linear issues"}"#);
    assert_eq!(meta.description.as_deref(), Some("Manage Linear issues"));
}

#[test]
fn parse_manifest_falls_back_to_the_interface_block() {
    let meta = parse_manifest_meta(br#"{"interface": {"shortDescription": "Short"}}"#);
    assert_eq!(meta.description.as_deref(), Some("Short"));
}

#[test]
fn parse_manifest_ignores_a_blank_description() {
    let meta = parse_manifest_meta(br#"{"description": "  "}"#);
    assert!(meta.description.is_none());
}

#[test]
fn parse_manifest_treats_malformed_json_as_empty() {
    let meta = parse_manifest_meta(b"not json");
    assert!(meta.description.is_none());
}

/// An offline manifest source answering from a fixed table.
struct FakeSource {
    by_url: Vec<(String, ManifestMeta)>,
}

impl ManifestSource for FakeSource {
    fn manifest_meta(&self, entry: &ConnectorDescriptor) -> Option<ManifestMeta> {
        for url in catalogue_manifest_urls(&entry.source) {
            if let Some((_, meta)) = self.by_url.iter().find(|(u, _)| u == &url) {
                return Some(meta.clone());
            }
        }
        None
    }
}

#[test]
fn enrichment_fills_only_a_blank_description_and_reports_each_update() {
    let entries = vec![
        descriptor(
            "echo",
            "git+https://github.com/acme/connectors#v1:packages/echo",
        ),
        descriptor("other", "https://example.org/other.zip"),
    ];
    let source = FakeSource {
        by_url: vec![(
            "https://raw.githubusercontent.com/acme/connectors/v1/packages/echo/connector.json"
                .to_string(),
            // The shared metadata record carries an optional `version` the
            // connector catalogue never consumes.
            ManifestMeta {
                description: Some("The echo connector.".to_string()),
                ..Default::default()
            },
        )],
    };
    let updates = std::sync::Mutex::new(Vec::new());
    enrich_catalogue_into(&entries, &source, &|update| {
        updates.lock().expect("updates lock").push(update);
    });
    let updates = updates.into_inner().expect("updates lock");

    assert_eq!(updates.len(), 1, "only the git-sourced entry resolved");
    assert_eq!(updates[0].id, "echo");
    assert_eq!(
        updates[0].meta.description.as_deref(),
        Some("The echo connector.")
    );
}

#[test]
fn the_network_source_is_constructible_without_a_lookup() {
    // Constructing the production source must not build a blocking client (which
    // would spawn a runtime); a lookup is only ever made off-loop.
    let _source = NetworkManifestSource::new();
}
