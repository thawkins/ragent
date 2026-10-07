//! Table-driven tests for the search-provider API-key configuration fields
//! (`langsearch_api_key`, `perplexity_api_key`, `serper_api_key`).
//!
//! These three fields previously had a near-identical copy-pasted test file
//! each (T-605). They now share one parameterised test over a field-accessor
//! table, so a behaviour added for one key is exercised for all three.

use ragent_config::Config;

/// Read accessor: returns the field's value as an `Option<&str>`.
type Get = for<'a> fn(&'a Config) -> Option<&'a str>;
/// Write accessor: sets the field.
type Set = fn(&mut Config, Option<String>);

/// One API-key config field and its accessor pair.
struct KeyField {
    /// JSON/serialised field name.
    name: &'static str,
    /// Value used when the field must be present.
    value: &'static str,
    get: Get,
    set: Set,
}

/// The three search-provider API-key fields under test.
const FIELDS: &[KeyField] = &[
    KeyField {
        name: "langsearch_api_key",
        value: "ls-value",
        get: |c| c.langsearch_api_key.as_deref(),
        set: |c, v| c.langsearch_api_key = v,
    },
    KeyField {
        name: "perplexity_api_key",
        value: "pplx-value",
        get: |c| c.perplexity_api_key.as_deref(),
        set: |c, v| c.perplexity_api_key = v,
    },
    KeyField {
        name: "serper_api_key",
        value: "serp-value",
        get: |c| c.serper_api_key.as_deref(),
        set: |c, v| c.serper_api_key = v,
    },
];

#[test]
fn test_api_key_defaults_to_none() {
    for field in FIELDS {
        let config = Config::default();
        assert!(
            (field.get)(&config).is_none(),
            "{} should default to None",
            field.name
        );
    }
}

#[test]
fn test_api_key_merge_overlay_sets_value() {
    for field in FIELDS {
        let base = Config::default();
        let mut overlay = Config::default();
        (field.set)(&mut overlay, Some(field.value.to_string()));

        let merged = Config::merge(base, overlay);
        assert_eq!(
            (field.get)(&merged),
            Some(field.value),
            "{} should be set by a non-empty overlay",
            field.name
        );
    }
}

#[test]
fn test_api_key_merge_overlay_none_preserves_base() {
    for field in FIELDS {
        let mut base = Config::default();
        (field.set)(&mut base, Some("base-value".to_string()));
        let overlay = Config::default();

        let merged = Config::merge(base, overlay);
        assert_eq!(
            (field.get)(&merged),
            Some("base-value"),
            "{} base value should survive a None overlay",
            field.name
        );
    }
}

#[test]
fn test_api_key_merge_overlay_overrides_base() {
    for field in FIELDS {
        let mut base = Config::default();
        (field.set)(&mut base, Some("base-value".to_string()));
        let mut overlay = Config::default();
        (field.set)(&mut overlay, Some("overlay-value".to_string()));

        let merged = Config::merge(base, overlay);
        assert_eq!(
            (field.get)(&merged),
            Some("overlay-value"),
            "{} overlay should override base",
            field.name
        );
    }
}

#[test]
fn test_api_key_load_file_parses_value() {
    for field in FIELDS {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("ragent.json");
        let mut object = serde_json::Map::new();
        object.insert(
            field.name.to_string(),
            serde_json::Value::String(field.value.to_string()),
        );
        std::fs::write(&path, serde_json::Value::Object(object).to_string()).expect("write config");

        let content = std::fs::read_to_string(&path).expect("read config");
        let config: Config = serde_json::from_str(&content).expect("parse config");
        assert_eq!(
            (field.get)(&config),
            Some(field.value),
            "{} should parse from JSON",
            field.name
        );
    }
}

#[test]
fn test_api_key_load_file_omitted_defaults_to_none() {
    for field in FIELDS {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("ragent.json");
        std::fs::write(
            &path,
            serde_json::json!({ "default_agent": "coder" }).to_string(),
        )
        .expect("write config");

        let content = std::fs::read_to_string(&path).expect("read config");
        let config: Config = serde_json::from_str(&content).expect("parse config");
        assert!(
            (field.get)(&config).is_none(),
            "{} omitted from JSON should default to None",
            field.name
        );
    }
}

#[test]
fn test_api_key_serialised_default_omits_field() {
    for field in FIELDS {
        let config = Config::default();
        let json = serde_json::to_value(&config).expect("serialise config");
        assert!(
            json.get(field.name).is_none(),
            "default config should not contain {}",
            field.name
        );
    }
}

#[test]
fn test_api_key_serialised_explicit_includes_field() {
    for field in FIELDS {
        let mut config = Config::default();
        (field.set)(&mut config, Some(field.value.to_string()));
        let json = serde_json::to_value(&config).expect("serialise config");
        assert_eq!(
            json[field.name].as_str(),
            Some(field.value),
            "explicit {} must be serialised",
            field.name
        );
    }
}

#[test]
fn test_api_key_roundtrip_preserves_value() {
    for field in FIELDS {
        let mut config = Config::default();
        (field.set)(&mut config, Some(field.value.to_string()));
        config.config_paths = Vec::new();

        let json = serde_json::to_string(&config).expect("serialise config");
        let decoded: Config = serde_json::from_str(&json).expect("deserialise config");
        assert_eq!(
            (field.get)(&decoded),
            Some(field.value),
            "{} should survive a serialise/deserialise round-trip",
            field.name
        );
    }
}

#[test]
fn test_api_key_merge_preserves_unrelated_fields() {
    // Field-specific extra: merging an API key must not drop unrelated fields
    // (provider map, top-level scalars).
    for field in FIELDS {
        let mut base = Config::default();
        base.default_agent = "base-agent".to_string();
        base.provider.insert(
            "anthropic".to_string(),
            ragent_config::ProviderConfig::default(),
        );
        let mut overlay = Config::default();
        (field.set)(&mut overlay, Some("only-value".to_string()));

        let merged = Config::merge(base, overlay);
        assert_eq!((field.get)(&merged), Some("only-value"), "{}", field.name);
        assert!(
            merged.provider.contains_key("anthropic"),
            "merging {} must preserve unrelated provider configs",
            field.name
        );
        assert_eq!(
            merged.default_agent, "base-agent",
            "merging {} must preserve unrelated top-level fields",
            field.name
        );
    }
}
