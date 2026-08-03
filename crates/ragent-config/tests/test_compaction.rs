//! Integration tests for `ragent-config` compaction configuration.
//!
//! Covers the new OpenCode-derived `compaction` configuration section and the
//! legacy `compression` alias migration.

use ragent_config::{CompactionConfig, KeepConfig};

#[test]
fn test_compaction_config_default() {
    let config = CompactionConfig::default();
    assert!(config.auto);
    assert_eq!(config.threshold, None);
    assert_eq!(config.buffer, 20_000);
    assert_eq!(config.keep.tokens, Some(8_000));
    assert_eq!(config.keep_tokens(), 8_000);
    assert_eq!(config.summary_output_tokens(), 4_096);
    assert_eq!(config.tool_output_max_chars(), 2_000);
}

#[test]
fn test_compaction_config_serde_roundtrip() {
    let config = CompactionConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: CompactionConfig = serde_json::from_str(&json).unwrap();
    assert!(deserialized.auto);
    assert_eq!(deserialized.threshold, None);
    assert_eq!(deserialized.buffer, 20_000);
    assert_eq!(deserialized.keep.tokens, Some(8_000));
}

#[test]
fn test_compaction_config_partial_deserialize() {
    let json = r#"{"auto": false, "buffer": 10000}"#;
    let config: CompactionConfig = serde_json::from_str(json).unwrap();
    assert!(!config.auto);
    assert_eq!(config.buffer, 10_000);
    assert_eq!(config.keep.tokens, Some(8_000)); // default
}

#[test]
fn test_compaction_config_threshold_deserialize() {
    let json = r#"{"threshold": 0.8}"#;
    let config: CompactionConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.threshold, Some(0.8));
    assert_eq!(config.buffer, 20_000); // default
}

#[test]
fn test_compaction_config_keep_override() {
    let json = r#"{"keep": {"tokens": 4000}}"#;
    let config: CompactionConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.keep.tokens, Some(4_000));
    assert_eq!(config.keep_tokens(), 4_000);
}

#[test]
fn test_legacy_compression_alias_maps_to_auto() {
    let config = CompactionConfig::default();
    let legacy = ragent_config::LegacyCompressionConfig {
        enabled: Some(false),
        auto_threshold: Some(0.8),
    };
    let merged = ragent_config::apply_legacy_compression_alias(config, &legacy);
    assert!(!merged.auto);
    assert_eq!(merged.threshold, Some(0.8));
}

#[test]
fn test_legacy_compression_alias_carries_auto_threshold() {
    let config = CompactionConfig::default();
    let legacy = ragent_config::LegacyCompressionConfig {
        enabled: None,
        auto_threshold: Some(0.8),
    };
    let merged = ragent_config::apply_legacy_compression_alias(config, &legacy);
    // `enabled` left untouched (default true) but threshold carried across.
    assert!(merged.auto);
    assert_eq!(merged.threshold, Some(0.8));
}

#[test]
fn test_legacy_compression_alias_does_not_force_true() {
    let config = CompactionConfig {
        auto: true,
        threshold: None,
        buffer: 20_000,
        keep: KeepConfig::default(),
    };
    let legacy = ragent_config::LegacyCompressionConfig {
        enabled: Some(false),
        auto_threshold: None,
    };
    let merged = ragent_config::apply_legacy_compression_alias(config, &legacy);
    assert!(!merged.auto);
    assert_eq!(merged.buffer, 20_000);
}

#[test]
fn test_legacy_auto_threshold_does_not_override_new_threshold() {
    // Explicit new-section `threshold` wins over the legacy `auto_threshold`.
    let config = CompactionConfig {
        auto: true,
        threshold: Some(0.9),
        buffer: 20_000,
        keep: KeepConfig::default(),
    };
    let legacy = ragent_config::LegacyCompressionConfig {
        enabled: None,
        auto_threshold: Some(0.8),
    };
    let merged = ragent_config::apply_legacy_compression_alias(config, &legacy);
    assert_eq!(merged.threshold, Some(0.9));
}
