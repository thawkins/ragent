//! Tests for the `/connectors stores` report and its provenance tagging (spec
//! `connectors` T-018; FR-036).
//!
//! The T-016 resolver and the T-017 wiring are covered by
//! `test_connector_store_index.rs` and `test_connector_store_ops.rs`; these tests
//! pin the **report wording**: each catalogue row prints its effective endpoint
//! with the provenance tag `default`/`config` read from the same resolution the
//! fetch uses (FR-036), a refused endpoint is reported with its reason rather
//! than substituted (FR-037), rendering is ASCII and prefixed `From: /connectors
//! stores` (FR-006), and the opt-in `--check` probe contacts the endpoint the row
//! reports (NFR-003).

use std::collections::BTreeMap;

use ragent_config::{ConnectorStoreEndpoint, ConnectorStoresConfig, ConnectorsConfig};
use ragent_connectors::{
    CatalogueKind, DEFAULT_CLAUDE_CATALOGUE_URL, FixtureCatalogueFetcher, StoreProbe, probe_stores,
    render_stores_report, render_stores_report_with_probes, stores_check_requested, stores_report,
    stores_report_with_check,
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

/// A one-connector catalogue document with an expressible remote server.
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

// -- Provenance tags (FR-036) ------------------------------------------------

#[test]
fn fresh_install_tags_the_claude_endpoint_default() {
    // No `connectors` block: the compiled default is in effect (FR-034).
    let report = render_stores_report(&ConnectorsConfig::default());
    assert!(
        report.contains(&format!(
            "- claude: [default] {DEFAULT_CLAUDE_CATALOGUE_URL}"
        )),
        "a fresh install tags the compiled default `default`, got:\n{report}"
    );
    assert!(
        !report.contains("[config]"),
        "no override means no `config` tag, got:\n{report}"
    );
}

#[test]
fn a_configured_override_is_tagged_config_and_shows_the_override() {
    let connectors = config_with_url("claude", Some("https://catalogue.example.org/index.json"));
    let report = render_stores_report(&connectors);
    assert!(
        report.contains("- claude: [config] https://catalogue.example.org/index.json"),
        "an override is tagged `config` and prints the override, got:\n{report}"
    );
    assert!(
        !report.contains(DEFAULT_CLAUDE_CATALOGUE_URL),
        "the compiled default must not appear while an override is active, got:\n{report}"
    );
}

#[test]
fn blank_and_absent_overrides_fall_back_to_the_default_tag() {
    for connectors in [
        config_with_url("claude", Some("   ")),
        config_with_url("claude", Some("")),
        config_with_url("claude", None),
    ] {
        let report = render_stores_report(&connectors);
        assert!(
            report.contains(&format!(
                "- claude: [default] {DEFAULT_CLAUDE_CATALOGUE_URL}"
            )),
            "a blank/absent override restores the default tag without error, got:\n{report}"
        );
    }
}

#[test]
fn the_tag_comes_from_the_same_resolution_the_fetch_uses() {
    // The report reads `StoreEndpointRow`, which resolves through the fetch's
    // resolver, so the endpoint the row prints is the endpoint the fetch contacts.
    for (connectors, want_tag) in [
        (ConnectorsConfig::default(), "default"),
        (
            config_with_url("claude", Some("https://catalogue.example.org/index.json")),
            "config",
        ),
    ] {
        let row = &stores_report(&connectors)[0];
        assert_eq!(row.kind, CatalogueKind::Claude);
        let endpoint = row.endpoint().expect("resolved");
        let report = render_stores_report(&connectors);
        assert!(
            report.contains(&format!("- claude: [{want_tag}] {}", endpoint.as_str())),
            "the row's endpoint and tag appear verbatim, got:\n{report}"
        );
    }
}

// -- Refusals, not substitution (FR-037) -------------------------------------

#[test]
fn a_non_https_override_is_reported_not_substituted() {
    let connectors = config_with_url("claude", Some("http://example.org/index.json"));
    let report = render_stores_report(&connectors);
    assert!(
        report.contains("- claude: [err]") && report.contains("https") && report.contains("http"),
        "a non-https endpoint is refused naming the scheme, got:\n{report}"
    );
    assert!(
        !report.contains(DEFAULT_CLAUDE_CATALOGUE_URL),
        "a refused endpoint must not silently fall back to the default, got:\n{report}"
    );
}

#[test]
fn a_malformed_override_is_reported_as_malformed() {
    let connectors = config_with_url("claude", Some("not-a-url"));
    let report = render_stores_report(&connectors);
    assert!(
        report.contains("- claude: [err]") && report.to_lowercase().contains("url"),
        "a malformed endpoint is refused with its reason, got:\n{report}"
    );
    assert!(
        !report.contains(DEFAULT_CLAUDE_CATALOGUE_URL),
        "a malformed endpoint must not fall back to the default, got:\n{report}"
    );
}

// -- Wording (FR-006) --------------------------------------------------------

#[test]
fn the_report_is_prefixed_and_ascii_only() {
    let report = render_stores_report(&ConnectorsConfig::default());
    assert!(
        report.starts_with("From: /connectors stores"),
        "the report carries the attribution prefix, got:\n{report}"
    );
    assert!(
        report.is_ascii(),
        "the report contains no non-ASCII characters, got:\n{report}"
    );
}

// -- The `--check` probe (FR-036, NFR-003) -----------------------------------

#[test]
fn check_requested_matches_the_flag_anywhere() {
    assert!(stores_check_requested("--check"));
    assert!(stores_check_requested("  --check  "));
    assert!(stores_check_requested("claude --check"));
    assert!(!stores_check_requested(""));
    assert!(!stores_check_requested("--verbose"));
}

#[test]
fn without_check_no_probe_is_appended_and_no_fetch_happens() {
    // An empty fixture fetcher would error if consulted; because `--check` is
    // absent it is never called, so the report is the plain config report.
    let connectors = ConnectorsConfig::default();
    let fetcher = FixtureCatalogueFetcher::new();
    let report = stores_report_with_check(&connectors, &fetcher, "");
    assert_eq!(report, render_stores_report(&connectors));
    assert!(!report.contains("[ok]"));
}

#[test]
fn check_appends_an_available_probe_with_the_connector_count() {
    let connectors = ConnectorsConfig::default();
    let fetcher = FixtureCatalogueFetcher::new()
        .with_index(DEFAULT_CLAUDE_CATALOGUE_URL, ONE_ENTRY_CATALOGUE);
    let report = stores_report_with_check(&connectors, &fetcher, "--check");
    assert!(
        report.contains(&format!(
            "- claude: [default] {DEFAULT_CLAUDE_CATALOGUE_URL} [ok] available, 1 connectors"
        )),
        "`--check` probes the endpoint the row reports, got:\n{report}"
    );
}

#[test]
fn check_reports_an_unreachable_endpoint_as_unavailable() {
    let connectors = ConnectorsConfig::default();
    let fetcher = FixtureCatalogueFetcher::new();
    let report = stores_report_with_check(&connectors, &fetcher, "--check");
    assert!(
        report.contains("- claude: [default]") && report.contains("[err] unavailable:"),
        "an unreachable endpoint yields a contained unavailable probe, got:\n{report}"
    );
}

#[test]
fn check_a_refused_endpoint_probe_carries_the_refusal_reason() {
    let connectors = config_with_url("claude", Some("http://example.org/index.json"));
    let fetcher = FixtureCatalogueFetcher::new();
    let probes = probe_stores(&connectors, &fetcher);
    assert_eq!(probes.len(), CatalogueKind::ALL.len());
    assert!(
        matches!(&probes[0], StoreProbe::Unavailable { detail } if detail.contains("https")),
        "the probe names the refused scheme, got {:?}",
        probes[0]
    );
}

#[test]
fn probe_stores_is_aligned_with_every_catalogue() {
    let connectors = ConnectorsConfig::default();
    let fetcher = FixtureCatalogueFetcher::new()
        .with_index(DEFAULT_CLAUDE_CATALOGUE_URL, ONE_ENTRY_CATALOGUE);
    let probes = probe_stores(&connectors, &fetcher);
    assert_eq!(probes.len(), CatalogueKind::ALL.len());
    assert!(matches!(probes[0], StoreProbe::Available { count: 1 }));
}

#[test]
fn rendering_with_probes_never_fetches_by_itself() {
    // `render_stores_report_with_probes` takes probes by value, so a caller that
    // renders without probing performs no network access (NFR-003).
    let connectors = ConnectorsConfig::default();
    let report = render_stores_report_with_probes(&connectors, None);
    assert_eq!(report, render_stores_report(&connectors));
}
