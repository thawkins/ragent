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
    // New defaults: a smaller summary budget and no model override.
    assert_eq!(config.summary_output_tokens(), 1_500);
    assert_eq!(config.tool_output_max_chars(), 2_000);
    assert!(config.model.is_none());
    assert_eq!(config.summary_tokens, None);
    assert_eq!(config.tool_output_max_chars, None);
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

#[test]
fn test_compaction_config_model_override_deserialize() {
    let json = r#"{"model": {"provider_id": "ollama", "model_id": "qwen2.5:1.5b"}}"#;
    let config: CompactionConfig = serde_json::from_str(json).unwrap();
    let m = config
        .model
        .as_ref()
        .expect("model override must be present");
    assert_eq!(m.provider_id, "ollama");
    assert_eq!(m.model_id, "qwen2.5:1.5b");
}

#[test]
fn test_compaction_config_summary_tokens_override() {
    let json = r#"{"summary_tokens": 800}"#;
    let config: CompactionConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.summary_output_tokens(), 800);
}

#[test]
fn test_compaction_config_tool_output_max_chars_override() {
    let json = r#"{"tool_output_max_chars": 500}"#;
    let config: CompactionConfig = serde_json::from_str(json).unwrap();
    assert_eq!(config.tool_output_max_chars(), 500);
}
