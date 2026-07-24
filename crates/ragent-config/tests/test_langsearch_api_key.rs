//! Tests for loading, merging, and serialising the `langsearch_api_key`
//! configuration field (T-010, FR-002, FR-006).

use ragent_config::Config;
use std::collections::HashMap;

// ---------------------------------------------------------------------------
// Merge precedence
// ---------------------------------------------------------------------------

#[test]
fn test_merge_overlay_sets_langsearch_api_key() {
    let base = Config::default();
    let mut overlay = Config::default();
    overlay.langsearch_api_key = Some("ls-overlay".to_string());

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.langsearch_api_key.as_deref(), Some("ls-overlay"));
}

#[test]
fn test_merge_overlay_none_preserves_base() {
    let mut base = Config::default();
    base.langsearch_api_key = Some("ls-base".to_string());
    let overlay = Config::default();

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.langsearch_api_key.as_deref(), Some("ls-base"));
}

#[test]
fn test_merge_overlay_overrides_base() {
    let mut base = Config::default();
    base.langsearch_api_key = Some("ls-base".to_string());
    let mut overlay = Config::default();
    overlay.langsearch_api_key = Some("ls-overlay".to_string());

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.langsearch_api_key.as_deref(), Some("ls-overlay"));
}

// ---------------------------------------------------------------------------
// JSON loading
// ---------------------------------------------------------------------------

#[test]
fn test_load_file_parses_langsearch_api_key() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ragent.json");
    std::fs::write(
        &path,
        serde_json::json!({ "langsearch_api_key": "ls-project" }).to_string(),
    )
    .expect("write config");

    let content = std::fs::read_to_string(&path).expect("read config");
    let config: Config = serde_json::from_str(&content).expect("parse config");
    assert_eq!(config.langsearch_api_key.as_deref(), Some("ls-project"));
}

#[test]
fn test_load_file_omitted_langsearch_api_key_defaults_to_none() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ragent.json");
    std::fs::write(
        &path,
        serde_json::json!({ "default_agent": "coder" }).to_string(),
    )
    .expect("write config");

    let content = std::fs::read_to_string(&path).expect("read config");
    let config: Config = serde_json::from_str(&content).expect("parse config");
    assert!(config.langsearch_api_key.is_none());
}

// ---------------------------------------------------------------------------
// Serialisation
// ---------------------------------------------------------------------------

#[test]
fn test_serialised_default_omits_langsearch_api_key() {
    let config = Config::default();
    let json = serde_json::to_value(&config).expect("serialise config");
    assert!(
        json.get("langsearch_api_key").is_none(),
        "default config should not contain langsearch_api_key"
    );
}

#[test]
fn test_serialised_explicit_key_includes_langsearch_api_key() {
    let mut config = Config::default();
    config.langsearch_api_key = Some("ls-12345".to_string());
    let json = serde_json::to_value(&config).expect("serialise config");
    assert_eq!(
        json["langsearch_api_key"].as_str(),
        Some("ls-12345"),
        "explicit key must be serialised"
    );
}

#[test]
fn test_merge_preserves_other_top_level_fields() {
    let mut base = Config::default();
    base.default_agent = "base-agent".to_string();
    let mut overlay = Config::default();
    overlay.langsearch_api_key = Some("ls-overlay".to_string());
    overlay.default_agent = "overlay-agent".to_string();

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.langsearch_api_key.as_deref(), Some("ls-overlay"));
    assert_eq!(merged.default_agent, "overlay-agent");
}

#[test]
fn test_config_roundtrip_preserves_langsearch_api_key() {
    let mut config = Config::default();
    config.langsearch_api_key = Some("ls-roundtrip".to_string());
    config.config_paths = Vec::new();

    let json = serde_json::to_string(&config).expect("serialise config");
    let decoded: Config = serde_json::from_str(&json).expect("deserialise config");

    assert_eq!(decoded.langsearch_api_key.as_deref(), Some("ls-roundtrip"));
}

#[test]
fn test_merge_preserves_provider_configs_when_merging_key() {
    let mut base = Config::default();
    base.provider.insert(
        "anthropic".to_string(),
        ragent_config::ProviderConfig::default(),
    );
    let mut overlay = Config::default();
    overlay.langsearch_api_key = Some("ls-only".to_string());

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.langsearch_api_key.as_deref(), Some("ls-only"));
    assert!(merged.provider.contains_key("anthropic"));
}

#[test]
fn test_merge_with_hashmap_base_none_overlay_some() {
    // Verify that merging does not require other optional fields to be set.
    let mut base = Config::default();
    base.provider = HashMap::new();
    let mut overlay = Config::default();
    overlay.provider = HashMap::new();
    overlay.langsearch_api_key = Some("ls-minimal".to_string());

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.langsearch_api_key.as_deref(), Some("ls-minimal"));
}
