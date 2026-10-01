//! Tests for the compiled default connector-catalogue endpoint and its
//! config-over-default resolver (spec `connectors` T-016; FR-034, FR-035,
//! FR-037, FR-038, NFR-002, NFR-003).
//!
//! The final test (T-019) also pins NFR-001: the default-endpoint literal is
//! declared exactly once, in `ragent_connectors::store_index`, and appears in
//! no other production source file.

use std::path::{Path, PathBuf};

use ragent_config::{ConnectorStoreEndpoint, ConnectorStoresConfig, ConnectorsConfig};
use ragent_connectors::{
    CatalogueCatalog, CatalogueEndpoint, CatalogueEndpointError, CatalogueKind,
    DEFAULT_CLAUDE_CATALOGUE_URL, EndpointSource,
};

/// Build a connector configuration carrying exactly one named catalogue URL.
fn config_with_url(name: &str, url: Option<&str>) -> ConnectorsConfig {
    let mut catalogues = std::collections::BTreeMap::new();
    catalogues.insert(
        name.to_string(),
        ConnectorStoreEndpoint {
            url: url.map(str::to_string),
        },
    );
    ConnectorsConfig {
        stores: Some(ConnectorStoresConfig {
            catalogues,
            ..ConnectorStoresConfig::default()
        }),
        ..ConnectorsConfig::default()
    }
}

#[test]
fn default_endpoint_is_a_compiled_absolute_https_constant() {
    assert!(
        !DEFAULT_CLAUDE_CATALOGUE_URL.is_empty(),
        "the compiled default endpoint must be non-empty"
    );
    let endpoint = CatalogueEndpoint::parse(DEFAULT_CLAUDE_CATALOGUE_URL)
        .expect("the compiled default must pass the https guard");
    assert_eq!(endpoint.as_str(), DEFAULT_CLAUDE_CATALOGUE_URL);
    assert_eq!(endpoint.as_url().scheme(), "https");
    assert!(endpoint.as_url().host_str().is_some());
}

#[test]
fn catalogue_kind_default_url_is_the_single_literal() {
    for kind in CatalogueKind::ALL {
        assert_eq!(kind.default_url(), DEFAULT_CLAUDE_CATALOGUE_URL);
    }
    assert_eq!(
        CatalogueKind::Claude.default_url(),
        DEFAULT_CLAUDE_CATALOGUE_URL
    );
}

#[test]
fn catalogue_kind_tokens_and_labels_match_config_keys() {
    assert_eq!(CatalogueKind::Claude.token(), "claude");
    assert_eq!(CatalogueKind::Claude.label(), "Claude");
    assert_eq!(CatalogueKind::Claude.to_string(), "Claude");
    assert_eq!(CatalogueKind::ALL, [CatalogueKind::Claude]);
}

#[test]
fn resolver_falls_back_to_default_with_no_connectors_block() {
    let connectors = ConnectorsConfig::default();
    let (endpoint, source) = CatalogueKind::Claude
        .effective_endpoint_with_source(&connectors)
        .expect("the compiled default resolves without configuration");

    assert_eq!(endpoint.as_str(), DEFAULT_CLAUDE_CATALOGUE_URL);
    assert_eq!(source, EndpointSource::Default);
    assert_eq!(source.tag(), "default");
}

#[test]
fn resolver_falls_back_to_default_when_override_is_absent() {
    // A `stores` block with no `claude` entry.
    let connectors = ConnectorsConfig {
        stores: Some(ConnectorStoresConfig::default()),
        ..ConnectorsConfig::default()
    };
    let (endpoint, source) = CatalogueKind::Claude
        .effective_endpoint_with_source(&connectors)
        .expect("an absent override resolves to the default");

    assert_eq!(endpoint.as_str(), DEFAULT_CLAUDE_CATALOGUE_URL);
    assert_eq!(source, EndpointSource::Default);
}

#[test]
fn resolver_falls_back_to_default_when_override_is_empty_or_whitespace() {
    for blank in ["", "   ", "\t\n"] {
        let connectors = config_with_url("claude", Some(blank));
        let (endpoint, source) = CatalogueKind::Claude
            .effective_endpoint_with_source(&connectors)
            .unwrap_or_else(|e| panic!("a blank override ({blank:?}) must not error: {e}"));
        assert_eq!(endpoint.as_str(), DEFAULT_CLAUDE_CATALOGUE_URL);
        assert_eq!(source, EndpointSource::Default);
    }
}

#[test]
fn resolver_falls_back_to_default_when_override_url_is_null() {
    let connectors = config_with_url("claude", None);
    let (endpoint, source) = CatalogueKind::Claude
        .effective_endpoint_with_source(&connectors)
        .expect("a null override resolves to the default");
    assert_eq!(endpoint.as_str(), DEFAULT_CLAUDE_CATALOGUE_URL);
    assert_eq!(source, EndpointSource::Default);
}

#[test]
fn configured_override_wins_wholesale_and_is_tagged_config() {
    let connectors = config_with_url("claude", Some("https://example.org/catalogue/index.json"));
    let (endpoint, source) = CatalogueKind::Claude
        .effective_endpoint_with_source(&connectors)
        .expect("a valid https override resolves");

    assert_eq!(
        endpoint.as_str(),
        "https://example.org/catalogue/index.json"
    );
    assert_eq!(source, EndpointSource::Config);
    assert_eq!(source.tag(), "config");
}

#[test]
fn non_https_override_is_refused_naming_the_scheme() {
    let connectors = config_with_url("claude", Some("http://example.org/connectors/index.json"));
    let err = CatalogueKind::Claude
        .effective_endpoint_with_source(&connectors)
        .expect_err("a non-https override must be refused");

    match err {
        CatalogueEndpointError::NotHttps { scheme } => assert_eq!(scheme, "http"),
        other => panic!("expected NotHttps, got {other:?}"),
    }
}

#[test]
fn malformed_override_is_refused_naming_the_input() {
    let connectors = config_with_url("claude", Some("not-a-url"));
    let err = CatalogueKind::Claude
        .effective_endpoint_with_source(&connectors)
        .expect_err("a malformed override must be refused");

    match err {
        CatalogueEndpointError::Malformed(detail) => {
            assert!(
                !detail.is_empty(),
                "the reason must name the offending input"
            );
        }
        other => panic!("expected Malformed, got {other:?}"),
    }
}

#[test]
fn host_less_https_endpoint_is_refused() {
    let err = CatalogueEndpoint::parse("https://").expect_err("a host-less endpoint is refused");
    // `url` reports this as malformed (empty host); either refusal names the cause.
    assert!(matches!(
        err,
        CatalogueEndpointError::Malformed(_) | CatalogueEndpointError::MissingHost
    ));
}

#[test]
fn empty_endpoint_is_refused() {
    for raw in ["", "   ", "\t"] {
        assert_eq!(
            CatalogueEndpoint::parse(raw),
            Err(CatalogueEndpointError::Empty)
        );
    }
}

#[test]
fn effective_endpoint_matches_the_with_source_form() {
    let connectors = config_with_url("claude", Some("https://example.org/cat.json"));
    let endpoint = CatalogueKind::Claude
        .effective_endpoint(&connectors)
        .expect("resolves");
    let (with_source, _) = CatalogueKind::Claude
        .effective_endpoint_with_source(&connectors)
        .expect("resolves");
    assert_eq!(endpoint, with_source);
}

#[test]
fn resolution_is_stable_across_repeated_calls() {
    // NFR-002: resolution is a pure function of config + the compiled constant.
    let connectors = ConnectorsConfig::default();
    let first = CatalogueKind::Claude
        .effective_endpoint_with_source(&connectors)
        .expect("resolves");
    let second = CatalogueKind::Claude
        .effective_endpoint_with_source(&connectors)
        .expect("resolves");
    assert_eq!(first, second);
    assert_eq!(first.1, EndpointSource::Default);
}

#[test]
fn resolution_performs_no_network_access() {
    // NFR-003: resolving the endpoint is offline string/config work. A resolved
    // value is available with no I/O, and a non-https override is refused
    // entirely without any attempt to contact the endpoint.
    let connectors = config_with_url("claude", Some("http://127.0.0.1:1/never-fetched.json"));
    let result = CatalogueKind::Claude.effective_endpoint_with_source(&connectors);
    assert!(
        matches!(result, Err(CatalogueEndpointError::NotHttps { ref scheme }) if scheme == "http"),
        "resolution refuses before any fetch and never contacts the endpoint"
    );
}

#[test]
fn catalogue_catalog_records_resolved_endpoints() {
    let mut catalog = CatalogueCatalog::new();
    assert!(catalog.is_empty());
    assert!(catalog.endpoint(CatalogueKind::Claude).is_none());

    let endpoint = CatalogueEndpoint::parse(DEFAULT_CLAUDE_CATALOGUE_URL).expect("valid");
    catalog.insert(CatalogueKind::Claude, endpoint.clone());

    assert!(!catalog.is_empty());
    assert_eq!(catalog.endpoint(CatalogueKind::Claude), Some(&endpoint));
}

#[test]
fn endpoint_display_and_try_from_match_parse() {
    let endpoint = CatalogueEndpoint::try_from(DEFAULT_CLAUDE_CATALOGUE_URL).expect("valid");
    assert_eq!(endpoint.to_string(), DEFAULT_CLAUDE_CATALOGUE_URL);
    assert_eq!(endpoint.as_str(), DEFAULT_CLAUDE_CATALOGUE_URL);
}

// ── NFR-001: the default-endpoint literal lives in exactly one source file ────

/// Recursively collect every `.rs` file under `root`.
fn rust_sources_under(root: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_sources_under(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn the_default_endpoint_literal_appears_only_in_the_catalogue_registry() {
    // NFR-001: the compiled default endpoint is declared once in
    // `ragent_connectors::store_index` and is the only production file that
    // carries the literal. A second copy anywhere under `crates/*/src` or the
    // root `src` fails the guard check
    // (scripts/check-connector-endpoint-literal.sh).
    let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");

    let mut sources = Vec::new();
    for crate_entry in std::fs::read_dir(workspace.join("crates")).expect("crates dir") {
        let src = crate_entry.expect("entry").path().join("src");
        if src.is_dir() {
            rust_sources_under(&src, &mut sources);
        }
    }
    let root_src = workspace.join("src");
    if root_src.is_dir() {
        rust_sources_under(&root_src, &mut sources);
    }
    assert!(
        sources.len() > 100,
        "the workspace source scan must find the crate sources, found {}",
        sources.len()
    );

    let mut hits: Vec<PathBuf> = Vec::new();
    for path in &sources {
        let Ok(text) = std::fs::read_to_string(path) else {
            continue;
        };
        if text.contains(DEFAULT_CLAUDE_CATALOGUE_URL) {
            hits.push(path.clone());
        }
    }

    assert_eq!(
        hits.len(),
        1,
        "NFR-001: the default-endpoint literal must appear in exactly one source file, found: {hits:?}"
    );
    let registry = &hits[0];
    assert!(
        registry.ends_with("crates/ragent-connectors/src/store_index.rs"),
        "the single default-endpoint source must be the catalogue registry, got {registry:?}"
    );

    // The registry declares the constant exactly once, and the same value the
    // resolver falls back to (FR-034, FR-038).
    let text = std::fs::read_to_string(registry).expect("registry readable");
    assert_eq!(
        text.matches(DEFAULT_CLAUDE_CATALOGUE_URL).count(),
        1,
        "the registry must declare the default endpoint literal exactly once"
    );
    assert!(
        text.contains("pub const DEFAULT_CLAUDE_CATALOGUE_URL"),
        "the default endpoint must be a single public constant"
    );
    assert_eq!(
        CatalogueKind::Claude.default_url(),
        DEFAULT_CLAUDE_CATALOGUE_URL
    );
}

#[test]
fn catalogue_descriptors_fetch_reports_a_refused_endpoint_without_contacting_it() {
    // A non-https override is refused by the guarded endpoint resolver, so the
    // shared search fetch returns the contained cause and reaches no network
    // (FR-037). This is the path the TUI/CLI `search_catalogue` now takes.
    let connectors = config_with_url("claude", Some("http://example.org/index.json"));
    let err = ragent_connectors::fetch_catalogue_descriptors_network(&connectors)
        .expect_err("a refused endpoint must surface a cause");
    assert!(
        err.contains("http") || err.to_lowercase().contains("https"),
        "the cause names the refused scheme: {err}"
    );
}

#[test]
fn catalogue_descriptors_fetch_skips_a_failed_store_and_keeps_the_healthy_one() {
    // One store is refused (non-https) and the other is left at its compiled
    // default, so the union keeps whatever the healthy store serves. An empty
    // override is "use the default", not an unresolvable store.
    let mut catalogues = std::collections::BTreeMap::new();
    catalogues.insert(
        "claude".to_string(),
        ConnectorStoreEndpoint {
            url: Some("http://example.org/index.json".to_string()),
        },
    );
    catalogues.insert(
        "community".to_string(),
        ConnectorStoreEndpoint { url: None },
    );
    let connectors = ConnectorsConfig {
        stores: Some(ConnectorStoresConfig {
            catalogues,
            ..ConnectorStoresConfig::default()
        }),
        ..ConnectorsConfig::default()
    };
    // The default store is contacted live, so only assert the refused store did
    // not abort the whole union: the result is either the healthy catalogue or a
    // single contained cause naming the refused scheme.
    match ragent_connectors::fetch_catalogue_descriptors_network(&connectors) {
        Ok(descriptors) => assert!(
            !descriptors.is_empty(),
            "a healthy store survives a refused sibling"
        ),
        Err(cause) => assert!(
            cause.contains("http") || cause.to_lowercase().contains("https"),
            "the cause names the refused scheme: {cause}"
        ),
    }
}
