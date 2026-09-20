//! Tests for the `plugins` configuration block (spec `plugins` T-001).

use ragent_config::{Config, PluginsConfig};

#[test]
fn plugins_config_is_none_by_default() {
    let config = Config::default();
    assert!(config.plugins.is_none());
}

#[test]
fn plugins_config_defaults_match_spec() {
    let plugins = PluginsConfig::default();
    assert!(plugins.enabled);
    assert!(plugins.is_enabled());
    assert_eq!(plugins.max_execution_ms, 5_000);
    assert_eq!(plugins.max_entry_ms, 10_000);
    assert_eq!(plugins.max_memory_mb, 64);
    assert!(plugins.store_dir.is_none());
    assert!(plugins.permissions.is_empty());
}

#[test]
fn plugins_config_partial_block_parses_with_defaults() {
    let config: Config = serde_json::from_str(
        r#"{
            "plugins": { "max_execution_ms": 2500 }
        }"#,
    )
    .expect("config should parse");

    let plugins = config.plugins.expect("plugins block should be present");
    assert!(plugins.enabled);
    assert_eq!(plugins.max_execution_ms, 2_500);
    assert_eq!(plugins.max_entry_ms, 10_000);
    assert_eq!(plugins.max_memory_mb, 64);
}

#[test]
fn plugins_config_full_block_parses() {
    let config: Config = serde_json::from_str(
        r#"{
            "plugins": {
                "enabled": false,
                "max_execution_ms": 1000,
                "max_entry_ms": 2000,
                "max_memory_mb": 32,
                "store_dir": "/tmp/plugin-store",
                "permissions": { "codex-weather": ["network.outbound"] }
            }
        }"#,
    )
    .expect("config should parse");

    let plugins = config.plugins.expect("plugins block should be present");
    assert!(!plugins.enabled);
    assert!(!plugins.is_enabled());
    assert_eq!(plugins.max_execution_ms, 1_000);
    assert_eq!(plugins.max_entry_ms, 2_000);
    assert_eq!(plugins.max_memory_mb, 32);
    assert_eq!(
        plugins.store_dir.as_deref(),
        Some(std::path::Path::new("/tmp/plugin-store"))
    );
    assert_eq!(
        plugins.permissions.get("codex-weather").map(Vec::as_slice),
        Some(&["network.outbound".to_string()][..])
    );
}

#[test]
fn plugins_config_overlay_wins_when_present() {
    let mut base = Config::default();
    base.plugins = Some(PluginsConfig {
        max_execution_ms: 1_000,
        ..PluginsConfig::default()
    });

    let mut overlay = Config::default();
    overlay.plugins = Some(PluginsConfig {
        enabled: false,
        max_memory_mb: 128,
        ..PluginsConfig::default()
    });

    let merged = Config::merge(base, overlay);
    let plugins = merged.plugins.expect("plugins should survive the merge");
    // Overlay section wins wholesale when present.
    assert!(!plugins.enabled);
    assert_eq!(plugins.max_memory_mb, 128);
    assert_eq!(plugins.max_execution_ms, 5_000);
}

#[test]
fn plugins_config_base_kept_when_overlay_absent() {
    let mut base = Config::default();
    base.plugins = Some(PluginsConfig {
        max_entry_ms: 7_500,
        ..PluginsConfig::default()
    });

    let overlay = Config::default();
    assert!(overlay.plugins.is_none());

    let merged = Config::merge(base, overlay);
    let plugins = merged.plugins.expect("plugins should survive the merge");
    assert_eq!(plugins.max_entry_ms, 7_500);
}

#[test]
fn plugins_config_roundtrip_omits_none_section() {
    let config = Config::default();
    let json = serde_json::to_string(&config).expect("config should serialise");
    assert!(
        !json.contains("\"plugins\""),
        "absent plugins block must not appear in serialised config: {json}"
    );
}

#[test]
fn plugins_config_roundtrip_serialises_present_section() {
    let mut config = Config::default();
    config.plugins = Some(PluginsConfig::default());

    let json = serde_json::to_string(&config).expect("config should serialise");
    let reparsed: Config = serde_json::from_str(&json).expect("serialised config should parse");
    assert_eq!(reparsed.plugins, config.plugins);

    // Default-only sub-fields with serde(default) still round-trip.
    let plugins = reparsed.plugins.expect("plugins block should be present");
    assert!(plugins.enabled);
    assert_eq!(plugins.max_execution_ms, 5_000);
    assert!(plugins.store_dir.is_none());
    assert!(plugins.permissions.is_empty());
}
