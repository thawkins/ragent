//! Tests for loading, merging, and serialising the `perplexity_api_key`
//! configuration field.

use ragent_config::Config;

// ---------------------------------------------------------------------------
// Merge precedence
// ---------------------------------------------------------------------------

#[test]
fn test_merge_overlay_sets_perplexity_api_key() {
    let base = Config::default();
    let mut overlay = Config::default();
    overlay.perplexity_api_key = Some("pplx-overlay".to_string());

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.perplexity_api_key.as_deref(), Some("pplx-overlay"));
}

#[test]
fn test_merge_overlay_none_preserves_base() {
    let mut base = Config::default();
    base.perplexity_api_key = Some("pplx-base".to_string());
    let overlay = Config::default();

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.perplexity_api_key.as_deref(), Some("pplx-base"));
}

#[test]
fn test_merge_overlay_overrides_base() {
    let mut base = Config::default();
    base.perplexity_api_key = Some("pplx-base".to_string());
    let mut overlay = Config::default();
    overlay.perplexity_api_key = Some("pplx-overlay".to_string());

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.perplexity_api_key.as_deref(), Some("pplx-overlay"));
}

// ---------------------------------------------------------------------------
// JSON loading
// ---------------------------------------------------------------------------

#[test]
fn test_load_file_parses_perplexity_api_key() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ragent.json");
    std::fs::write(
        &path,
        serde_json::json!({ "perplexity_api_key": "pplx-project" }).to_string(),
    )
    .expect("write config");

    let content = std::fs::read_to_string(&path).expect("read config");
    let config: Config = serde_json::from_str(&content).expect("parse config");
    assert_eq!(config.perplexity_api_key.as_deref(), Some("pplx-project"));
}

#[test]
fn test_load_file_omitted_perplexity_api_key_defaults_to_none() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ragent.json");
    std::fs::write(
        &path,
        serde_json::json!({ "default_agent": "coder" }).to_string(),
    )
    .expect("write config");

    let content = std::fs::read_to_string(&path).expect("read config");
    let config: Config = serde_json::from_str(&content).expect("parse config");
    assert!(config.perplexity_api_key.is_none());
}

// ---------------------------------------------------------------------------
// Serialisation
// ---------------------------------------------------------------------------

#[test]
fn test_serialised_default_omits_perplexity_api_key() {
    let config = Config::default();
    let json = serde_json::to_value(&config).expect("serialise config");
    assert!(
        json.get("perplexity_api_key").is_none(),
        "default config should not contain perplexity_api_key"
    );
}

#[test]
fn test_serialised_explicit_key_includes_perplexity_api_key() {
    let mut config = Config::default();
    config.perplexity_api_key = Some("pplx-12345".to_string());
    let json = serde_json::to_value(&config).expect("serialise config");
    assert_eq!(
        json["perplexity_api_key"].as_str(),
        Some("pplx-12345"),
        "explicit key must be serialised"
    );
}

// ---------------------------------------------------------------------------
// Round-trip and interaction with other fields
// ---------------------------------------------------------------------------

#[test]
fn test_config_roundtrip_preserves_perplexity_api_key() {
    let mut config = Config::default();
    config.perplexity_api_key = Some("pplx-roundtrip".to_string());
    let json = serde_json::to_string(&config).expect("serialise config");
    let decoded: Config = serde_json::from_str(&json).expect("deserialise config");
    assert_eq!(
        decoded.perplexity_api_key.as_deref(),
        Some("pplx-roundtrip")
    );
}

#[test]
fn test_merge_preserves_provider_configs_when_merging_key() {
    let mut base = Config::default();
    base.perplexity_api_key = Some("pplx-base".to_string());
    let mut overlay = Config::default();
    overlay.perplexity_api_key = Some("pplx-only".to_string());

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.perplexity_api_key.as_deref(), Some("pplx-only"));
}

#[test]
fn test_merge_with_hashmap_base_none_overlay_some() {
    let mut overlay = Config::default();
    overlay.perplexity_api_key = Some("pplx-minimal".to_string());
    let base = Config::default();
    let merged = Config::merge(base, overlay);
    assert_eq!(merged.perplexity_api_key.as_deref(), Some("pplx-minimal"));
}
