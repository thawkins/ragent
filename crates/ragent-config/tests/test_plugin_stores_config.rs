//! Tests for the `plugins.stores` configuration block (spec `pluginstores`
//! T-001; FR-019).

use ragent_config::{Config, PluginStoreEndpoint, PluginStoresConfig, PluginsConfig};

#[test]
fn stores_absent_by_default() {
    let plugins = PluginsConfig::default();
    assert!(plugins.stores.is_none());
    assert!(plugins.stores_or_default().codex.is_none());
    assert!(plugins.stores_or_default().claude.is_none());
}

#[test]
fn stores_defaults_match_spec() {
    let stores = PluginStoresConfig::default();
    assert_eq!(stores.timeout_ms, 10_000);
    assert_eq!(stores.max_index_bytes, 2_097_152);
    assert_eq!(stores.cache_ttl_secs, 3_600);
    assert!(stores.codex.is_none());
    assert!(stores.claude.is_none());
}

#[test]
fn stores_or_default_falls_back_when_block_absent() {
    let plugins = PluginsConfig::default();
    let stores = plugins.stores_or_default();
    assert_eq!(stores.timeout_ms, 10_000);
    assert_eq!(stores.max_index_bytes, 2_097_152);
    assert_eq!(stores.cache_ttl_secs, 3_600);
}

#[test]
fn stores_block_parses_full() {
    let config: Config = serde_json::from_str(
        r#"{
            "plugins": {
                "enabled": true,
                "stores": {
                    "codex":  { "url": "https://example.org/codex/index.json" },
                    "claude": { "url": "https://example.org/claude/index.json" },
                    "timeout_ms": 2500,
                    "max_index_bytes": 1048576,
                    "cache_ttl_secs": 0
                }
            }
        }"#,
    )
    .expect("config should parse");

    let plugins = config.plugins.expect("plugins block should be present");
    let stores = plugins.stores.expect("stores block should be present");
    assert_eq!(
        stores.codex.as_ref().and_then(|e| e.url.as_deref()),
        Some("https://example.org/codex/index.json")
    );
    assert_eq!(
        stores.claude.as_ref().and_then(|e| e.url.as_deref()),
        Some("https://example.org/claude/index.json")
    );
    assert_eq!(stores.timeout_ms, 2_500);
    assert_eq!(stores.max_index_bytes, 1_048_576);
    assert_eq!(stores.cache_ttl_secs, 0);
}

#[test]
fn stores_block_partial_uses_defaults() {
    let config: Config = serde_json::from_str(
        r#"{
            "plugins": {
                "stores": { "codex": { "url": "https://example.org/codex.json" } }
            }
        }"#,
    )
    .expect("config should parse");

    let stores = config
        .plugins
        .expect("plugins block should be present")
        .stores
        .expect("stores block should be present");
    assert_eq!(
        stores.codex.as_ref().and_then(|e| e.url.as_deref()),
        Some("https://example.org/codex.json")
    );
    assert!(stores.claude.is_none());
    assert_eq!(stores.timeout_ms, 10_000);
    assert_eq!(stores.max_index_bytes, 2_097_152);
    assert_eq!(stores.cache_ttl_secs, 3_600);
}

#[test]
fn stores_block_absent_leaves_existing_fields_intact() {
    let config: Config = serde_json::from_str(r#"{ "plugins": { "max_execution_ms": 2500 } }"#)
        .expect("config should parse");

    let plugins = config.plugins.expect("plugins block should be present");
    assert_eq!(plugins.max_execution_ms, 2_500);
    assert!(plugins.stores.is_none());
}

#[test]
fn stores_serialise_roundtrip() {
    let plugins = PluginsConfig {
        stores: Some(PluginStoresConfig {
            codex: Some(PluginStoreEndpoint {
                url: Some("https://example.org/codex.json".to_string()),
            }),
            claude: None,
            timeout_ms: 5_000,
            max_index_bytes: 512_000,
            cache_ttl_secs: 60,
        }),
        ..PluginsConfig::default()
    };
    let mut config = Config::default();
    config.plugins = Some(plugins);

    let json = serde_json::to_string(&config).expect("config should serialise");
    let reparsed: Config = serde_json::from_str(&json).expect("serialised config should parse");
    assert_eq!(reparsed.plugins, config.plugins);

    // `claude: None` is skipped in serialisation but still parses back to None.
    assert!(
        !json.contains("\"claude\""),
        "absent store endpoint should be omitted: {json}"
    );
}

#[test]
fn stores_omitted_entirely_when_section_absent() {
    let json = serde_json::to_string(&Config::default()).expect("config should serialise");
    assert!(
        !json.contains("\"stores\""),
        "absent stores must not serialise: {json}"
    );
}

#[test]
fn stores_overlay_from_project_wins() {
    let mut base = Config::default();
    base.plugins = Some(PluginsConfig::default());

    let mut overlay = Config::default();
    overlay.plugins = Some(PluginsConfig {
        stores: Some(PluginStoresConfig {
            codex: Some(PluginStoreEndpoint {
                url: Some("https://project.example.org/index.json".to_string()),
            }),
            ..PluginStoresConfig::default()
        }),
        ..PluginsConfig::default()
    });

    let merged = Config::merge(base, overlay);
    let stores = merged
        .plugins
        .expect("plugins should survive the merge")
        .stores
        .expect("stores should survive the merge");
    assert_eq!(
        stores.codex.as_ref().and_then(|e| e.url.as_deref()),
        Some("https://project.example.org/index.json")
    );
}
