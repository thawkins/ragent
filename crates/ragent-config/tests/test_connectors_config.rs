//! Tests for the `connectors` configuration block (spec `connectors` T-001;
//! FR-007, FR-021, FR-024).

use ragent_config::{
    Config, ConnectorCredentialsConfig, ConnectorStoreEndpoint, ConnectorStoresConfig,
    ConnectorsConfig,
};

#[test]
fn connectors_config_is_none_by_default() {
    let config = Config::default();
    assert!(config.connectors.is_none());
}

#[test]
fn connectors_config_defaults_match_spec() {
    let connectors = ConnectorsConfig::default();
    assert!(connectors.enabled);
    assert!(connectors.is_enabled());
    assert!(connectors.store_dir.is_none());
    assert!(connectors.stores.is_none());
    assert!(connectors.credentials.is_empty());
}

#[test]
fn connectors_config_partial_block_parses_with_defaults() {
    let config: Config = serde_json::from_str(r#"{ "connectors": { "enabled": false } }"#)
        .expect("config should parse");

    let connectors = config
        .connectors
        .expect("connectors block should be present");
    assert!(!connectors.enabled);
    assert!(!connectors.is_enabled());
    assert!(connectors.store_dir.is_none());
    assert!(connectors.stores.is_none());
}

#[test]
fn connectors_config_full_block_parses() {
    let config: Config = serde_json::from_str(
        r#"{
            "connectors": {
                "enabled": true,
                "store_dir": "/tmp/connector-store",
                "stores": {
                    "community": { "url": "https://example.org/connectors/index.json" },
                    "timeout_ms": 2500,
                    "max_index_bytes": 1048576,
                    "cache_ttl_secs": 0
                },
                "credentials": { "google-drive": { "token": "GDRIVE_TOKEN" } }
            }
        }"#,
    )
    .expect("config should parse");

    let connectors = config
        .connectors
        .expect("connectors block should be present");
    assert!(connectors.enabled);
    assert_eq!(
        connectors.store_dir.as_deref(),
        Some(std::path::Path::new("/tmp/connector-store"))
    );
    let stores = connectors.stores.expect("stores block should be present");
    assert_eq!(
        stores
            .catalogues
            .get("community")
            .and_then(|e| e.url.as_deref()),
        Some("https://example.org/connectors/index.json")
    );
    assert_eq!(stores.timeout_ms, 2_500);
    assert_eq!(stores.max_index_bytes, 1_048_576);
    assert_eq!(stores.cache_ttl_secs, 0);
    assert_eq!(
        connectors
            .credentials
            .get("google-drive")
            .and_then(|c| c.token.as_deref()),
        Some("GDRIVE_TOKEN")
    );
}

#[test]
fn connectors_stores_defaults_match_spec() {
    let stores = ConnectorStoresConfig::default();
    assert_eq!(stores.timeout_ms, 10_000);
    assert_eq!(stores.max_index_bytes, 2_097_152);
    assert_eq!(stores.cache_ttl_secs, 3_600);
    assert!(stores.catalogues.is_empty());
}

#[test]
fn connectors_stores_or_default_falls_back_when_block_absent() {
    let stores = ConnectorsConfig::default().stores_or_default();
    assert_eq!(stores.timeout_ms, 10_000);
    assert_eq!(stores.max_index_bytes, 2_097_152);
    assert_eq!(stores.cache_ttl_secs, 3_600);
    assert!(stores.catalogues.is_empty());
}

#[test]
fn connectors_catalogue_url_reports_only_non_empty_overrides() {
    let connectors = ConnectorsConfig {
        stores: Some(ConnectorStoresConfig {
            catalogues: [
                (
                    "claude".to_string(),
                    ConnectorStoreEndpoint {
                        url: Some("https://example.org/claude.json".to_string()),
                    },
                ),
                (
                    "community".to_string(),
                    ConnectorStoreEndpoint {
                        url: Some("   ".to_string()),
                    },
                ),
                ("empty".to_string(), ConnectorStoreEndpoint { url: None }),
            ]
            .into_iter()
            .collect(),
            ..ConnectorStoresConfig::default()
        }),
        ..ConnectorsConfig::default()
    };

    assert_eq!(
        connectors.catalogue_url("claude"),
        Some("https://example.org/claude.json")
    );
    // An empty or whitespace-only override means the compiled default applies.
    assert_eq!(connectors.catalogue_url("community"), None);
    assert_eq!(connectors.catalogue_url("empty"), None);
    assert_eq!(connectors.catalogue_url("missing"), None);
}

#[test]
fn connectors_overlay_from_project_wins() {
    let mut base = Config::default();
    base.connectors = Some(ConnectorsConfig::default());

    let mut overlay = Config::default();
    overlay.connectors = Some(ConnectorsConfig {
        stores: Some(ConnectorStoresConfig {
            catalogues: std::collections::BTreeMap::from([(
                "community".to_string(),
                ConnectorStoreEndpoint {
                    url: Some("https://project.example.org/index.json".to_string()),
                },
            )]),
            ..ConnectorStoresConfig::default()
        }),
        ..ConnectorsConfig::default()
    });

    let merged = Config::merge(base, overlay);
    let connectors = merged
        .connectors
        .expect("connectors should survive the merge");
    assert_eq!(
        connectors.catalogue_url("community"),
        Some("https://project.example.org/index.json")
    );
}

#[test]
fn connectors_absent_overlay_keeps_base_section() {
    let mut base = Config::default();
    base.connectors = Some(ConnectorsConfig {
        enabled: false,
        ..ConnectorsConfig::default()
    });

    let merged = Config::merge(base, Config::default());
    let connectors = merged
        .connectors
        .expect("connectors should survive the merge");
    assert!(!connectors.enabled);
}

#[test]
fn connectors_serialise_roundtrip() {
    let connectors = ConnectorsConfig {
        stores: Some(ConnectorStoresConfig {
            catalogues: std::collections::BTreeMap::from([(
                "claude".to_string(),
                ConnectorStoreEndpoint {
                    url: Some("https://example.org/claude.json".to_string()),
                },
            )]),
            timeout_ms: 5_000,
            max_index_bytes: 512_000,
            cache_ttl_secs: 60,
        }),
        credentials: std::collections::BTreeMap::from([(
            "google-drive".to_string(),
            ConnectorCredentialsConfig {
                token: Some("GDRIVE_TOKEN".to_string()),
            },
        )]),
        ..ConnectorsConfig::default()
    };
    let mut config = Config::default();
    config.connectors = Some(connectors);

    let json = serde_json::to_string(&config).expect("config should serialise");
    let reparsed: Config = serde_json::from_str(&json).expect("serialised config should parse");
    assert_eq!(reparsed.connectors, config.connectors);
}

#[test]
fn connectors_omitted_entirely_when_section_absent() {
    let json = serde_json::to_string(&Config::default()).expect("config should serialise");
    assert!(
        !json.contains("\"connectors\""),
        "absent connectors must not serialise: {json}"
    );
}
