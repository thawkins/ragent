//! Tests for the research subsystem configuration (spec `hyperresearch` FR-011, FR-012).

use ragent_config::{Config, ResearchConfig};

#[test]
fn research_config_defaults_to_disabled() {
    let config = Config::default();

    assert!(!config.research.open_access_recovery);
    assert!(!config.research.exclude_academic_engines);
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
    assert!(
        !config.research.is_empty(),
        "expected non-empty config.research"
    );
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
        exclude_academic_engines: false,
        contact_email: Some("oa@example.com".to_string()),
        oa_min_full_text_chars: 750,
        max_concepts: 8,
        max_findings: 25,
        models: ragent_config::ResearchModelsConfig::default(),
        supervisor: ragent_config::ResearchSupervisorConfig::default(),
        evaluate: ragent_config::ResearchEvaluateConfig::default(),
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

// ── `exclude_academic_engines` (spec `researchnoacc` FR-012) ─────────────────

#[test]
fn research_config_exclude_academic_engines_deserializes() {
    let config: Config = serde_json::from_str(
        r#"{
            "research": {
                "exclude_academic_engines": true
            }
        }"#,
    )
    .expect("research config should deserialize academic-exclusion flag");

    assert!(config.research.exclude_academic_engines);
    assert!(
        !config.research.is_empty(),
        "expected non-empty config.research"
    );
}

#[test]
fn research_config_exclude_academic_engines_defaults_off() {
    let config: Config = serde_json::from_str(r#"{"research": {}}"#)
        .expect("empty research block should deserialize");

    assert!(!config.research.exclude_academic_engines);
}

#[test]
fn research_config_exclude_academic_engines_merges_with_or() {
    let base: Config = serde_json::from_str(
        r#"{
            "research": {
                "exclude_academic_engines": false
            }
        }"#,
    )
    .expect("base config should deserialize");
    let overlay: Config = serde_json::from_str(
        r#"{
            "research": {
                "exclude_academic_engines": true
            }
        }"#,
    )
    .expect("overlay config should deserialize");

    let merged = Config::merge(base, overlay);
    assert!(merged.research.exclude_academic_engines);
}

#[test]
fn research_config_exclude_academic_engines_included_when_non_default() {
    let mut config = Config::default();
    config.research.exclude_academic_engines = true;
    let json = serde_json::to_string(&config).expect("serialize config");
    let value: serde_json::Value = serde_json::from_str(&json).expect("parse serialized json");

    assert_eq!(
        value
            .get("research")
            .and_then(|r| r.get("exclude_academic_engines"))
            .and_then(|v| v.as_bool()),
        Some(true)
    );
}

// ── Concept/finding limits (spec `researchmax` FR-008, FR-017) ──────────────

#[test]
fn research_config_output_limits_default_to_spec_values() {
    let config = ResearchConfig::default();
    assert_eq!(config.max_concepts, 5);
    assert_eq!(config.max_findings, 20);
}

#[test]
fn research_config_output_limits_deserialize() {
    let config: Config = serde_json::from_str(
        r#"{
            "research": {
                "max_concepts": 9,
                "max_findings": 40
            }
        }"#,
    )
    .expect("research config should deserialize the output limits");

    assert_eq!(config.research.max_concepts, 9);
    assert_eq!(config.research.max_findings, 40);
    assert!(
        !config.research.is_empty(),
        "expected non-empty config.research"
    );
}

#[test]
fn research_config_output_limits_default_when_absent() {
    let config: Config = serde_json::from_str(r#"{"research": {}}"#)
        .expect("empty research block should deserialize");

    assert_eq!(config.research.max_concepts, 5);
    assert_eq!(config.research.max_findings, 20);
}

#[test]
fn research_config_output_limits_survive_merge() {
    let base = Config::default();
    let overlay: Config = serde_json::from_str(
        r#"{
            "research": {
                "max_concepts": 11,
                "max_findings": 21
            }
        }"#,
    )
    .expect("overlay config should deserialize");

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.research.max_concepts, 11);
    assert_eq!(merged.research.max_findings, 21);
}

#[test]
fn research_config_output_limits_keep_base_when_overlay_is_default() {
    // An overlay that omits the keys carries the built-in defaults; merging it
    // must not clobber a non-default base value (FR-017 precedence).
    let base: Config = serde_json::from_str(
        r#"{
            "research": {
                "max_concepts": 7,
                "max_findings": 19
            }
        }"#,
    )
    .expect("base config should deserialize");
    let overlay: Config =
        serde_json::from_str(r#"{"research": {}}"#).expect("empty overlay should deserialize");

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.research.max_concepts, 7);
    assert_eq!(merged.research.max_findings, 19);
}

#[test]
fn research_config_output_limits_partial_overlay_keeps_other_base_value() {
    // A partial overlay overriding only `max_concepts` must leave the base
    // `max_findings` untouched (FR-017).
    let base: Config = serde_json::from_str(
        r#"{
            "research": {
                "max_concepts": 7,
                "max_findings": 19
            }
        }"#,
    )
    .expect("base config should deserialize");
    let overlay: Config = serde_json::from_str(
        r#"{
            "research": {
                "max_concepts": 3
            }
        }"#,
    )
    .expect("partial overlay should deserialize");

    let merged = Config::merge(base, overlay);
    assert_eq!(merged.research.max_concepts, 3);
    assert_eq!(merged.research.max_findings, 19);
}

#[test]
fn research_config_output_limits_are_omitted_when_default() {
    let config = Config::default();
    let json = serde_json::to_string(&config).expect("serialize default config");
    let value: serde_json::Value = serde_json::from_str(&json).expect("parse serialized json");

    assert!(
        value.get("research").is_none(),
        "default output limits should not force the research block into the output"
    );
}
