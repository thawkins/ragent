//! Tests for loading, merging, and serialising the `serper_api_key`
//! configuration field.

use ragent_config::Config;

// ---------------------------------------------------------------------------
// Merge precedence
// ---------------------------------------------------------------------------

#[test]
fn test_merge_overlay_sets_serper_api_key() {
    let base = Config::default();
    let mut overlay = Config::default();
    overlay.serper_api_key = Some("serp-overlay".to_string());

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.serper_api_key.as_deref(), Some("serp-overlay"));
}

#[test]
fn test_merge_overlay_none_preserves_base() {
    let mut base = Config::default();
    base.serper_api_key = Some("serp-base".to_string());
    let overlay = Config::default();

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.serper_api_key.as_deref(), Some("serp-base"));
}

#[test]
fn test_merge_overlay_overrides_base() {
    let mut base = Config::default();
    base.serper_api_key = Some("serp-base".to_string());
    let mut overlay = Config::default();
    overlay.serper_api_key = Some("serp-overlay".to_string());

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.serper_api_key.as_deref(), Some("serp-overlay"));
}

// ---------------------------------------------------------------------------
// JSON loading
// ---------------------------------------------------------------------------

#[test]
fn test_load_file_parses_serper_api_key() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ragent.json");
    std::fs::write(
        &path,
        serde_json::json!({ "serper_api_key": "serp-project" }).to_string(),
    )
    .expect("write config");

    let content = std::fs::read_to_string(&path).expect("read config");
    let config: Config = serde_json::from_str(&content).expect("parse config");
    assert_eq!(config.serper_api_key.as_deref(), Some("serp-project"));
}

#[test]
fn test_load_file_omitted_serper_api_key_defaults_to_none() {
    let dir = tempfile::tempdir().expect("temp dir");
    let path = dir.path().join("ragent.json");
    std::fs::write(
        &path,
        serde_json::json!({ "default_agent": "coder" }).to_string(),
    )
    .expect("write config");

    let content = std::fs::read_to_string(&path).expect("read config");
    let config: Config = serde_json::from_str(&content).expect("parse config");
    assert!(config.serper_api_key.is_none());
}

// ---------------------------------------------------------------------------
// Serialisation
// ---------------------------------------------------------------------------

#[test]
fn test_serialised_default_omits_serper_api_key() {
    let config = Config::default();
    let json = serde_json::to_value(&config).expect("serialise config");
    assert!(
        json.get("serper_api_key").is_none(),
        "default config should not contain serper_api_key"
    );
}

#[test]
fn test_serialised_explicit_key_includes_serper_api_key() {
    let mut config = Config::default();
    config.serper_api_key = Some("serp-12345".to_string());
    let json = serde_json::to_value(&config).expect("serialise config");
    assert_eq!(
        json["serper_api_key"].as_str(),
        Some("serp-12345"),
        "explicit key must be serialised"
    );
}
