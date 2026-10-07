//! Tests for default-endpoint resolution wired into the catalogue fetch and the
//! `/connectors stores` handler (spec `connectors` T-017; FR-035, FR-037,
//! NFR-002).
//!
//! The T-016 resolver is already covered by `test_connector_store_index.rs`;
//! these tests pin the **wiring**: the fetch resolves the effective endpoint and
//! guards it before any request (FR-037), an absent/blank override keeps the
//! compiled default active (FR-035), and the store-handler rows read the same
//! endpoint and provenance the fetch contacts (FR-036).

use std::collections::BTreeMap;
use std::time::Duration;

use ragent_config::{ConnectorStoreEndpoint, ConnectorStoresConfig, ConnectorsConfig};
use ragent_connectors::{
    CatalogueEndpoint, CatalogueEndpointError, CatalogueKind, CatalogueLimits, ConnectorError,
    DEFAULT_CLAUDE_CATALOGUE_URL, EndpointSource, FixtureCatalogueFetcher, effective_endpoint,
    fetch_effective_catalogue, resolve_catalog, stores_report,
};

/// Build a connector configuration carrying exactly one named catalogue URL.
fn config_with_url(name: &str, url: Option<&str>) -> ConnectorsConfig {
    let mut catalogues = BTreeMap::new();
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

/// A one-entry catalogue document with an expressible remote server.
const ONE_ENTRY_CATALOGUE: &str = r#"{
  "servers": [
    {
      "slug": "echo",
      "name": "Echo",
      "one_liner": "Echoes your input.",
      "categories": ["developer"],
      "remote": {
        "url": "https://mcp.example.test/v1/mcp",
        "transport": "streamable-http",
        "is_authless": true
      }
    }
  ]
}"#;

/// No-cache limits so a fetch always invokes the fetcher.
fn no_cache_limits() -> CatalogueLimits {
    CatalogueLimits::new(Duration::from_secs(5), 1_048_576, Duration::ZERO)
}

// -- Resolution guards the fetch (FR-035, FR-037) ----------------------------

#[test]
fn effective_endpoint_uses_compiled_default_with_no_connectors_block() {
    let connectors = ConnectorsConfig::default();
    let endpoint = effective_endpoint(CatalogueKind::Claude, &connectors)
        .expect("the compiled default resolves without configuration");
    assert_eq!(endpoint.as_str(), DEFAULT_CLAUDE_CATALOGUE_URL);
}

#[test]
fn effective_endpoint_guards_a_refused_override_before_any_fetch() {
    // A non-https override must be a contained refusal, never a fallback.
    let connectors = config_with_url("claude", Some("http://example.org/index.json"));
    let err = effective_endpoint(CatalogueKind::Claude, &connectors)
        .expect_err("a non-https override is refused");
    assert!(
        matches!(
            err,
            ConnectorError::CatalogueEndpoint(CatalogueEndpointError::NotHttps { ref scheme })
                if scheme == "http"
        ),
        "the refused scheme is named, got {err:?}"
    );
}

#[test]
fn resolved_endpoint_parses_through_the_https_guard() {
    // Whatever configuration is supplied, the resolved value is an absolute
    // https URL carrying a host - the property the fetch relies on (FR-037).
    for connectors in [
        ConnectorsConfig::default(),
        config_with_url("claude", None),
        config_with_url("claude", Some("   ")),
        config_with_url("claude", Some("https://catalogue.example.org/index.json")),
    ] {
        let endpoint = effective_endpoint(CatalogueKind::Claude, &connectors)
            .expect("a resolvable endpoint is always a validated https endpoint");
        assert_eq!(endpoint.as_url().scheme(), "https");
        assert!(endpoint.as_url().host_str().is_some());
    }
}

// -- The fetch contacts the resolved effective endpoint ----------------------

#[test]
fn fetch_effective_catalogue_contacts_the_compiled_default_endpoint() {
    // A fresh install with no `connectors.stores` block: the fetch must contact
    // the compiled default endpoint (FR-035). The fixture fetcher is registered
    // only for that default, so a fetch that resolved anything else would fail.
    let connectors = ConnectorsConfig::default();
    let fetcher = FixtureCatalogueFetcher::new()
        .with_index(DEFAULT_CLAUDE_CATALOGUE_URL, ONE_ENTRY_CATALOGUE);

    let fetched = fetch_effective_catalogue(
        &fetcher,
        CatalogueKind::Claude,
        &connectors,
        &no_cache_limits(),
        None,
        1_000,
    )
    .expect("the compiled default endpoint is fetched");

    assert!(!fetched.from_cache);
    assert_eq!(fetched.catalogue.endpoint, DEFAULT_CLAUDE_CATALOGUE_URL);
    assert_eq!(fetched.catalogue.connectors.len(), 1);
    assert_eq!(fetched.catalogue.connectors[0].id.as_str(), "echo");
}

#[test]
fn fetch_effective_catalogue_contacts_a_configured_override() {
    let override_url = "https://catalogue.example.org/index.json";
    let connectors = config_with_url("claude", Some(override_url));
    let fetcher = FixtureCatalogueFetcher::new().with_index(override_url, ONE_ENTRY_CATALOGUE);

    let fetched = fetch_effective_catalogue(
        &fetcher,
        CatalogueKind::Claude,
        &connectors,
        &no_cache_limits(),
        None,
        1_000,
    )
    .expect("a valid https override is fetched");

    assert_eq!(fetched.catalogue.endpoint, override_url);
}

#[test]
fn blank_override_restores_the_default_endpoint_for_the_fetch() {
    // Clearing a previously configured override (empty string / absent / null)
    // restores the compiled default on the next resolution - no rebuild
    // (FR-035, NFR-002). The fixture is registered only for the default, so a
    // fetch still resolving the blank override would fail.
    for connectors in [
        config_with_url("claude", Some("")),
        config_with_url("claude", Some("   ")),
        config_with_url("claude", None),
    ] {
        let fetcher = FixtureCatalogueFetcher::new()
            .with_index(DEFAULT_CLAUDE_CATALOGUE_URL, ONE_ENTRY_CATALOGUE);
        let fetched = fetch_effective_catalogue(
            &fetcher,
            CatalogueKind::Claude,
            &connectors,
            &no_cache_limits(),
            None,
            1_000,
        )
        .expect("a blank override falls back to the compiled default with no error");
        assert_eq!(fetched.catalogue.endpoint, DEFAULT_CLAUDE_CATALOGUE_URL);
    }
}

#[test]
fn fetch_refuses_a_refused_endpoint_without_contacting_any_endpoint() {
    // FR-037: a refused endpoint aborts before the fetcher is invoked. The
    // fetcher is empty, so any attempt to fetch would surface the fetcher's
    // "no fixture" error instead of the endpoint refusal.
    let connectors = config_with_url("claude", Some("http://example.org/index.json"));
    let fetcher = FixtureCatalogueFetcher::new();

    let err = fetch_effective_catalogue(
        &fetcher,
        CatalogueKind::Claude,
        &connectors,
        &no_cache_limits(),
        None,
        1_000,
    )
    .expect_err("a refused endpoint aborts the fetch");

    assert!(
        matches!(err, ConnectorError::CatalogueEndpoint(_)),
        "the refusal is the endpoint guard, not a fetch failure, got {err:?}"
    );
    assert_eq!(
        fetcher.endpoints(),
        Vec::<&str>::new(),
        "no endpoint bytes were requested"
    );
}

// -- The store handler rows read the same value as the fetch (FR-036) --------

#[test]
fn stores_report_tags_the_compiled_default() {
    let connectors = ConnectorsConfig::default();
    let rows = stores_report(&connectors);

    assert_eq!(rows.len(), 1, "one row per catalogue");
    let row = &rows[0];
    assert_eq!(row.kind, CatalogueKind::Claude);
    assert!(!row.is_refused());
    assert_eq!(
        row.endpoint().map(CatalogueEndpoint::as_str),
        Some(DEFAULT_CLAUDE_CATALOGUE_URL)
    );
    assert_eq!(row.source(), Some(EndpointSource::Default));
    assert_eq!(row.source().map(EndpointSource::tag), Some("default"));
}

#[test]
fn stores_report_tags_a_configured_override_as_config() {
    let override_url = "https://catalogue.example.org/index.json";
    let connectors = config_with_url("claude", Some(override_url));
    let row = &stores_report(&connectors)[0];

    assert_eq!(
        row.endpoint().map(CatalogueEndpoint::as_str),
        Some(override_url)
    );
    assert_eq!(row.source(), Some(EndpointSource::Config));
    assert_eq!(row.source().map(EndpointSource::tag), Some("config"));
}

#[test]
fn stores_report_row_matches_the_endpoint_the_fetch_uses() {
    // The row's endpoint and the fetch's endpoint come from the same resolver, so
    // they can never disagree (FR-035, FR-036).
    let override_url = "https://catalogue.example.org/index.json";
    let connectors = config_with_url("claude", Some(override_url));

    let reported = stores_report(&connectors)[0]
        .endpoint()
        .expect("row resolves")
        .clone();
    let effective = effective_endpoint(CatalogueKind::Claude, &connectors).expect("resolves");
    assert_eq!(reported, effective);
}

#[test]
fn stores_report_carries_a_refusal_for_a_non_https_override() {
    let connectors = config_with_url("claude", Some("http://example.org/index.json"));
    let row = &stores_report(&connectors)[0];

    assert!(row.is_refused());
    assert!(
        row.endpoint().is_none(),
        "a refused row carries no endpoint"
    );
    assert!(
        row.source().is_none(),
        "a refused row carries no provenance"
    );
    assert!(matches!(
        row.resolved,
        Err(CatalogueEndpointError::NotHttps { .. })
    ));
}

// -- The launch registry records the resolved endpoints (NFR-002) ------------

#[test]
fn resolve_catalog_records_effective_endpoints_and_skips_refusals() {
    let connectors = config_with_url("claude", Some("https://catalogue.example.org/index.json"));
    let catalog = resolve_catalog(&connectors);
    assert_eq!(
        catalog
            .endpoint(CatalogueKind::Claude)
            .map(CatalogueEndpoint::as_str),
        Some("https://catalogue.example.org/index.json")
    );

    // A refused endpoint is not recorded with an unvalidated value (FR-037).
    let refused = resolve_catalog(&config_with_url(
        "claude",
        Some("http://example.org/index.json"),
    ));
    assert!(refused.endpoint(CatalogueKind::Claude).is_none());
    assert!(refused.is_empty());
}

#[test]
fn resolve_catalog_defaults_to_the_compiled_endpoint() {
    let catalog = resolve_catalog(&ConnectorsConfig::default());
    assert_eq!(
        catalog
            .endpoint(CatalogueKind::Claude)
            .map(CatalogueEndpoint::as_str),
        Some(DEFAULT_CLAUDE_CATALOGUE_URL)
    );
}

// -- Resolution is stable per launch and offline (NFR-002, NFR-003) ----------

#[test]
fn resolution_is_stable_across_repeated_calls_within_a_launch() {
    let connectors = config_with_url("claude", Some("https://catalogue.example.org/index.json"));
    let first = stores_report(&connectors);
    let second = stores_report(&connectors);
    assert_eq!(first, second, "the same config resolves to the same rows");
}

#[test]
fn resolution_performs_no_network_access() {
    // NFR-003: resolving and reporting the endpoint does offline string/config
    // work only. The endpoint values are available immediately with no I/O, and
    // a refusal happens before any attempt to contact the endpoint.
    let connectors = ConnectorsConfig::default();
    let row = &stores_report(&connectors)[0];
    assert_eq!(
        row.endpoint().map(CatalogueEndpoint::as_str),
        Some(DEFAULT_CLAUDE_CATALOGUE_URL)
    );

    let refused = config_with_url("claude", Some("http://127.0.0.1:1/never-fetched.json"));
    assert!(matches!(
        effective_endpoint(CatalogueKind::Claude, &refused),
        Err(ConnectorError::CatalogueEndpoint(_))
    ));
}
