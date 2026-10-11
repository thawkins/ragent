//! Tests for the `/connectors marketplace show` and
//! `/connectors marketplace claude list` reports (spec `connectors` FR-042,
//! FR-043): one table over the Claude catalogue with one row per configured
//! marketplace endpoint (name, url, description), each tagged with its
//! provenance.
//!
//! Like the `/connectors stores` report these are pure config reads: the tests
//! are offline, exercise only the renderers and the shared dispatch ladder, and
//! install nothing.

use std::collections::BTreeMap;

use ragent_config::{
    ConnectorMarketplaceEntry, ConnectorStoreEndpoint, ConnectorStoresConfig, ConnectorsConfig,
};
use ragent_connectors::{
    DEFAULT_CLAUDE_CATALOGUE_URL, parse_marketplace_command, render_marketplace_list_report,
    render_marketplace_show_report,
};

/// A config carrying a `claude` catalogue override plus `marketplaces`.
fn config_with(
    url: Option<&str>,
    marketplaces: Vec<ConnectorMarketplaceEntry>,
) -> ConnectorsConfig {
    let mut catalogues = BTreeMap::new();
    catalogues.insert(
        "claude".to_string(),
        ConnectorStoreEndpoint {
            url: url.map(str::to_string),
        },
    );
    ConnectorsConfig {
        stores: Some(ConnectorStoresConfig {
            catalogues,
            marketplaces,
            ..ConnectorStoresConfig::default()
        }),
        ..ConnectorsConfig::default()
    }
}

/// One added marketplace entry for tests.
fn added(name: &str, url: Option<&str>) -> ConnectorMarketplaceEntry {
    ConnectorMarketplaceEntry {
        store: "claude".to_string(),
        name: name.to_string(),
        url: url.map(str::to_string),
        source: format!("acme/{name}"),
    }
}

#[test]
fn show_report_attributes_to_the_marketplace_show_subcommand() {
    let report = render_marketplace_show_report(&ConnectorsConfig::default());
    assert!(
        report.starts_with("From: /connectors marketplace show\n"),
        "report must carry the attribution header: {report}"
    );
}

#[test]
fn show_report_lists_the_compiled_default_when_nothing_is_configured() {
    let report = render_marketplace_show_report(&ConnectorsConfig::default());
    assert!(
        report.contains("## Claude connector catalogue marketplace"),
        "{report}"
    );
    assert!(report.contains("| Name | Url | Description |"), "{report}");
    assert!(report.contains(DEFAULT_CLAUDE_CATALOGUE_URL), "{report}");
    assert!(report.contains("(default)"), "{report}");
}

#[test]
fn show_report_tags_a_configured_base_endpoint_config() {
    let config = config_with(Some("https://example.org/claude.json"), Vec::new());
    let report = render_marketplace_show_report(&config);
    assert!(
        report.contains("https://example.org/claude.json"),
        "{report}"
    );
    assert!(report.contains("(config)"), "{report}");
    assert!(
        !report.contains(DEFAULT_CLAUDE_CATALOGUE_URL),
        "the override wins wholesale: {report}"
    );
}

#[test]
fn show_report_lists_each_added_marketplace_tagged_added() {
    let config = config_with(
        None,
        vec![
            added("acme", Some("https://example.org/acme/marketplace.json")),
            added("beta", Some("https://example.org/beta/marketplace.json")),
        ],
    );
    let report = render_marketplace_show_report(&config);
    let base = report.find("(default)").expect("base row");
    let acme = report
        .find("| acme | https://example.org/acme/marketplace.json |")
        .expect("acme row");
    let beta = report
        .find("| beta | https://example.org/beta/marketplace.json |")
        .expect("beta row");
    assert!(base < acme, "the base endpoint renders first: {report}");
    assert!(
        acme < beta,
        "added marketplaces render in config order: {report}"
    );
    assert_eq!(report.matches("(added)").count(), 2, "{report}");
}

#[test]
fn show_report_renders_a_refused_added_endpoint_as_an_err_row() {
    let config = config_with(
        None,
        vec![added("broken", Some("http://insecure.example.org/i.json"))],
    );
    let report = render_marketplace_show_report(&config);
    assert!(
        report.contains("[err]") && report.contains("invalid"),
        "the refused endpoint names its refusal: {report}"
    );
    // The accepted base endpoint still renders.
    assert!(report.contains(DEFAULT_CLAUDE_CATALOGUE_URL), "{report}");
}

#[test]
fn list_report_renders_the_same_rows_for_the_one_catalogue() {
    let config = config_with(
        None,
        vec![added(
            "acme",
            Some("https://example.org/acme/marketplace.json"),
        )],
    );
    let report = render_marketplace_list_report(&config);
    assert!(
        report.starts_with("From: /connectors marketplace\n"),
        "{report}"
    );
    assert!(
        report.contains("| acme | https://example.org/acme/marketplace.json |"),
        "{report}"
    );
    assert!(report.contains("(added)"), "{report}");
}

// -- Shared parser ladder (FR-042, FR-043) -------------------------------------

#[test]
fn show_parses_and_a_bare_marketplace_does_not() {
    assert!(parse_marketplace_command("show").is_some());
    assert!(parse_marketplace_command("").is_none());
    assert!(parse_marketplace_command("codex list").is_none());
}

#[test]
fn claude_list_add_remove_parse() {
    use ragent_connectors::MarketplaceCommand;
    assert!(matches!(
        parse_marketplace_command("claude list"),
        Some(Ok(MarketplaceCommand::List))
    ));
    assert!(matches!(
        parse_marketplace_command("claude add owner/repo --force"),
        Some(Ok(MarketplaceCommand::Add { source, force }))
            if source == "owner/repo" && force
    ));
    assert!(matches!(
        parse_marketplace_command("claude remove acme"),
        Some(Ok(MarketplaceCommand::Remove { name })) if name == "acme"
    ));
    // Malformed operations are recognised but carry the usage error.
    assert!(
        parse_marketplace_command("claude add")
            .expect("recognised")
            .is_err()
    );
    assert!(
        parse_marketplace_command("claude remove")
            .expect("recognised")
            .is_err()
    );
}
