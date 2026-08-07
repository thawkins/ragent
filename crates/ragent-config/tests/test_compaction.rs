//! Integration tests for `ragent-config` compaction configuration.

use ragent_config::CompactionConfig;

#[test]
fn test_compaction_config_default() {
    let config = CompactionConfig::default();
    assert!(config.auto);
    assert_eq!(config.threshold, Some(0.7));
    assert!((config.buffer - 0.10).abs() < f64::EPSILON);
    assert_eq!(config.keep.tokens, Some(0.20));
    assert!((config.keep_fraction() - 0.20).abs() < f64::EPSILON);
    assert_eq!(config.summary_output_tokens(), 4_096);
    assert_eq!(config.tool_output_max_chars(), 2_000);
}

#[test]
fn test_compaction_config_serde_roundtrip() {
    let config = CompactionConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: CompactionConfig = serde_json::from_str(&json).unwrap();
    assert!(deserialized.auto);
    assert_eq!(deserialized.threshold, Some(0.7));
    assert!((deserialized.buffer - 0.10).abs() < f64::EPSILON);
    assert_eq!(deserialized.keep.tokens, Some(0.20));
}

#[test]
fn test_compaction_config_partial_deserialize() {
    let json = r#"{"auto": false, "buffer": 0.15}"#;
    let config: CompactionConfig = serde_json::from_str(json).unwrap();
    assert!(!config.auto);
    assert!((config.buffer - 0.15).abs() < f64::EPSILON);
    assert_eq!(config.keep.tokens, Some(0.20)); // default
}

#[test]
fn test_compaction_config_threshold_deserialize() {
    let json = r#"{"threshold": 0.8}"#;
    let config: CompactionConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.threshold, Some(0.8));
    assert!((config.buffer - 0.10).abs() < f64::EPSILON); // default
}

#[test]
fn test_compaction_config_keep_override() {
    let json = r#"{"keep": {"tokens": 0.1}}"#;
    let config: CompactionConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.keep.tokens, Some(0.10));
    assert!((config.keep_fraction() - 0.10).abs() < f64::EPSILON);
}
