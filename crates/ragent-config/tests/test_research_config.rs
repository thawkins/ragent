//! Tests for the research subsystem configuration (spec `hyperresearch` FR-011, FR-012).

use ragent_config::{Config, ResearchConfig};

#[test]
fn research_config_defaults_to_disabled() {
    let config = Config::default();

    assert!(!config.research.open_access_recovery);
    assert!(config.research.contact_email.is_none());
    assert_eq!(config.research.oa_min_full_text_chars, 1000);
    assert!(config.research.is_empty());
}

#[test]
fn research_config_deserializes_all_fields() {
    let config: Config = serde_json::from_str(
        r#"{
            "research": {
                "open_access_recovery": true,
                "contact_email": "researcher@example.com",
                "oa_min_full_text_chars": 2000
            }
        }"#,
    )
    .expect("research config should deserialize");

    assert!(config.research.open_access_recovery);
    assert_eq!(
        config.research.contact_email.as_deref(),
        Some("researcher@example.com")
    );
    assert_eq!(config.research.oa_min_full_text_chars, 2000);
    assert!(!config.research.is_empty());
}

#[test]
fn research_config_deserializes_partial_override() {
    let config: Config = serde_json::from_str(
        r#"{
            "research": {
                "open_access_recovery": true
            }
        }"#,
    )
    .expect("research config should deserialize partial override");

    assert!(config.research.open_access_recovery);
    assert!(config.research.contact_email.is_none());
    assert_eq!(config.research.oa_min_full_text_chars, 1000);
}

#[test]
fn research_config_merges_overlay_values() {
    let base: Config = serde_json::from_str(
        r#"{
            "research": {
                "open_access_recovery": false,
                "contact_email": "base@example.com",
                "oa_min_full_text_chars": 500
            }
        }"#,
    )
    .expect("base config should deserialize");
    let overlay: Config = serde_json::from_str(
        r#"{
            "research": {
                "open_access_recovery": true,
                "oa_min_full_text_chars": 1500
            }
        }"#,
    )
    .expect("overlay config should deserialize");

    let merged = Config::merge(base, overlay);

    assert!(merged.research.open_access_recovery);
    assert_eq!(
        merged.research.contact_email.as_deref(),
        Some("base@example.com")
    );
    assert_eq!(merged.research.oa_min_full_text_chars, 1500);
}

#[test]
fn research_config_merge_contact_email_override_wins() {
    let base: Config = serde_json::from_str(
        r#"{
            "research": {
                "contact_email": "base@example.com"
            }
        }"#,
    )
    .expect("base config should deserialize");
    let overlay: Config = serde_json::from_str(
        r#"{
            "research": {
                "contact_email": "overlay@example.com"
            }
        }"#,
    )
    .expect("overlay config should deserialize");

    let merged = Config::merge(base, overlay);

    assert_eq!(
        merged.research.contact_email.as_deref(),
        Some("overlay@example.com")
    );
}

#[test]
fn research_config_default_threshold_matches_open_access_default() {
    // The config default must stay in sync with the research crate's default
    // so users who do not override the threshold get consistent behavior.
    // The research crate defines DEFAULT_OA_MIN_FULL_TEXT_CHARS as 1000.
    assert_eq!(ResearchConfig::default().oa_min_full_text_chars, 1000);
}

#[test]
fn research_config_round_trips_through_json() {
    let original = ResearchConfig {
        open_access_recovery: true,
        contact_email: Some("oa@example.com".to_string()),
        oa_min_full_text_chars: 750,
    };

    let json = serde_json::to_string(&original).expect("serialize");
    let restored: ResearchConfig = serde_json::from_str(&json).expect("deserialize");

    assert_eq!(original, restored);
}

#[test]
fn research_config_is_omitted_when_default() {
    let config = Config::default();
    let json = serde_json::to_string(&config).expect("serialize default config");
    let value: serde_json::Value = serde_json::from_str(&json).expect("parse serialized json");

    assert!(
        value.get("research").is_none(),
        "default research config should be omitted from serialized output"
    );
}

#[test]
fn research_config_is_included_when_non_default() {
    let mut config = Config::default();
    config.research.open_access_recovery = true;
    let json = serde_json::to_string(&config).expect("serialize config");
    let value: serde_json::Value = serde_json::from_str(&json).expect("parse serialized json");

    assert_eq!(
        value
            .get("research")
            .and_then(|r| r.get("open_access_recovery"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );
}
