//! Tests for the merged multi-marketplace catalogue fetch (spec `connectors`
//! FR-043): the browse panel fetches the base endpoint and every added
//! marketplace, merges their entries dropping duplicate ids (first occurrence -
//! the base endpoint - wins), attributes each added entry to its marketplace,
//! skips a refused added URL without blinding the catalogue, and fails only
//! when the base endpoint is refused or nothing answered at all.

use std::collections::BTreeMap;

use ragent_config::{
    ConnectorMarketplaceEntry, ConnectorStoreEndpoint, ConnectorStoresConfig, ConnectorsConfig,
};
use ragent_connectors::{
    CatalogueKind, FixtureCatalogueFetcher, fetch_all_catalogues, marketplace_name_for,
};

/// A config with an optional `claude` override plus `marketplaces`.
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

fn added(name: &str, url: Option<&str>) -> ConnectorMarketplaceEntry {
    ConnectorMarketplaceEntry {
        store: "claude".to_string(),
        name: name.to_string(),
        url: url.map(str::to_string),
        source: format!("acme/{name}"),
    }
}

/// Build a two-entry catalogue document whose entry ids are `id_a`/`id_b`, each
/// carrying a readable slug and an expressible remote server.
fn catalogue(id_a: &str, id_b: &str) -> String {
    let entry = |id: &str| {
        format!(
            r#"{{
        "slug": "{id}",
        "name": "{id} connector",
        "one_liner": "The {id} connector.",
        "remote": {{
          "url": "https://mcp.example.test/{id}/mcp",
          "transport": "streamable-http",
          "is_authless": true
        }}
    }}"#
        )
    };
    format!(r#"{{ "servers": [ {}, {} ] }}"#, entry(id_a), entry(id_b))
}

const BASE_URL: &str = "https://example.org/base/servers";
const ACME_URL: &str = "https://example.org/acme/servers";
const BETA_URL: &str = "https://example.org/beta/servers";

fn limits() -> ragent_connectors::CatalogueLimits {
    ragent_connectors::CatalogueLimits::from(&ConnectorsConfig::default().stores_or_default())
}

#[test]
fn added_endpoints_follow_the_base_and_are_guarded() {
    let config = config_with(
        Some(BASE_URL),
        vec![
            added("acme", Some(ACME_URL)),
            added("broken", Some("http://insecure.example.org/servers")),
        ],
    );
    let (endpoints, refused) = CatalogueKind::Claude.effective_endpoints_with_source(&config);
    assert_eq!(
        endpoints.len(),
        2,
        "the refused URL is dropped: {endpoints:?}"
    );
    assert_eq!(endpoints[0].0.as_str(), BASE_URL);
    assert_eq!(endpoints[1].0.as_str(), ACME_URL);
    assert!(refused.is_some(), "the refusal is carried: {refused:?}");
}

#[test]
fn marketplace_name_for_falls_back_to_the_catalogue_label_for_the_base() {
    let config = config_with(Some(BASE_URL), vec![added("acme", Some(ACME_URL))]);
    let (endpoints, _) = CatalogueKind::Claude.effective_endpoints_with_source(&config);
    assert_eq!(
        marketplace_name_for(CatalogueKind::Claude, &config, &endpoints[0].0),
        "Claude"
    );
    assert_eq!(
        marketplace_name_for(CatalogueKind::Claude, &config, &endpoints[1].0),
        "acme"
    );
}

#[test]
fn merge_fetches_every_endpoint_and_drops_duplicate_ids() {
    let fetcher = FixtureCatalogueFetcher::new()
        .with_index(BASE_URL, catalogue("echo", "shared"))
        // `shared` collides with the base entry: the base occurrence wins.
        .with_index(ACME_URL, catalogue("shared", "acme-only"))
        .with_index(BETA_URL, catalogue("beta-only", "beta-two"));
    let config = config_with(
        Some(BASE_URL),
        vec![added("acme", Some(ACME_URL)), added("beta", Some(BETA_URL))],
    );

    let merged = fetch_all_catalogues(&fetcher, CatalogueKind::Claude, &config, &limits(), None, 0)
        .expect("the merge succeeds when at least one endpoint answered");

    let ids: Vec<&str> = merged.connectors.iter().map(|d| d.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["echo", "shared", "acme-only", "beta-only", "beta-two"],
        "base first, then each added marketplace in config order, duplicates dropped: {ids:?}"
    );
    // Base-endpoint rows are attributed to the catalogue label; added rows carry
    // the marketplace they were added under (FR-043). `source` keeps its own
    // meaning (the entry's install origin).
    let acme_only = merged
        .connectors
        .iter()
        .find(|d| d.id.as_str() == "acme-only")
        .expect("added row present");
    assert_eq!(acme_only.marketplace, "acme");
    let base_row = merged
        .connectors
        .iter()
        .find(|d| d.id.as_str() == "echo")
        .expect("base row present");
    assert_eq!(base_row.marketplace, "Claude");
    assert!(merged.refused.is_none());
}

#[test]
fn a_refused_added_url_is_skipped_and_carried() {
    let fetcher = FixtureCatalogueFetcher::new().with_index(BASE_URL, catalogue("echo", "two"));
    let config = config_with(
        Some(BASE_URL),
        vec![added("broken", Some("http://insecure.example.org/servers"))],
    );

    let merged = fetch_all_catalogues(&fetcher, CatalogueKind::Claude, &config, &limits(), None, 0)
        .expect("a refused added marketplace never blinds the catalogue");
    assert_eq!(merged.connectors.len(), 2);
    assert!(
        merged.refused.is_some(),
        "the refusal is surfaced: {:?}",
        merged.refused
    );
}

#[test]
fn a_failing_endpoint_is_skipped_without_hiding_the_rest() {
    let fetcher = FixtureCatalogueFetcher::new().with_index(BASE_URL, catalogue("echo", "two"));
    // ACME is registered in config but has no fixture: the fixture fetcher
    // errors for it.
    let config = config_with(Some(BASE_URL), vec![added("acme", Some(ACME_URL))]);

    let merged = fetch_all_catalogues(&fetcher, CatalogueKind::Claude, &config, &limits(), None, 0)
        .expect("one failing marketplace does not fail the merge");
    assert_eq!(merged.connectors.len(), 2);
}

#[test]
fn the_merge_fails_when_nothing_answered() {
    let fetcher = FixtureCatalogueFetcher::new();
    let config = config_with(Some(BASE_URL), Vec::new());
    let outcome =
        fetch_all_catalogues(&fetcher, CatalogueKind::Claude, &config, &limits(), None, 0);
    assert!(
        outcome.is_err(),
        "no endpoints answered -> the contained failure is returned"
    );
}

#[test]
fn a_refused_base_endpoint_fails_before_any_fetch() {
    let fetcher = FixtureCatalogueFetcher::new().with_index(BASE_URL, catalogue("echo", "two"));
    let config = config_with(
        Some("http://insecure.example.org/base"),
        vec![added("acme", Some(ACME_URL))],
    );
    let outcome =
        fetch_all_catalogues(&fetcher, CatalogueKind::Claude, &config, &limits(), None, 0);
    assert!(
        outcome.is_err(),
        "a refused base endpoint fails the fetch (FR-037)"
    );
}
