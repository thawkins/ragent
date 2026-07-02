//! Integration tests for `ragent-config` compression configuration.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/compression.rs`
//! (T-005 of the testconsolidate spec).

use ragent_config::{CcrConfig, CompressionConfig, TokenizerConfig};

#[test]
fn test_compression_config_default() {
    let config = CompressionConfig::default();
    assert!(config.enabled);
    assert!((config.auto_threshold - 0.80).abs() < f64::EPSILON);
    assert_eq!(config.ccr.backend, "sqlite");
    assert_eq!(config.ccr.capacity, 1000);
    assert_eq!(config.ccr.ttl_secs, 300);
    assert!(config.compressors.json);
    assert!(config.compressors.diff);
    assert!(config.compressors.log);
    assert!(config.compressors.search);
    assert!(!config.compressors.code);
    assert!(!config.compressors.prose);
    assert!(!config.relevance.enabled);
    assert_eq!(config.relevance.scorer, "bm25");
    assert_eq!(config.relevance.keep_top_k, 20);
    assert_eq!(config.tokenizer.backend, "auto");
}

#[test]
fn test_compression_config_serde_roundtrip() {
    let config = CompressionConfig::default();
    let json = serde_json::to_string(&config).unwrap();
    let deserialized: CompressionConfig = serde_json::from_str(&json).unwrap();
    assert!(deserialized.enabled);
    assert!((deserialized.auto_threshold - 0.80).abs() < f64::EPSILON);
}

#[test]
fn test_compression_config_partial_deserialize() {
    // Missing keys should use defaults (backward compatibility)
    let json = r#"{"enabled": false}"#;
    let config: CompressionConfig = serde_json::from_str(json).unwrap();
    assert!(!config.enabled);
    // Defaults for missing fields
    assert!((config.auto_threshold - 0.80).abs() < f64::EPSILON);
    assert_eq!(config.ccr.backend, "sqlite");
}

#[test]
fn test_compression_config_disabled_all_compressors() {
    let json = r#"{"enabled": true, "compressors": {"json": false, "diff": false, "log": false, "search": false}}"#;
    let config: CompressionConfig = serde_json::from_str(json).unwrap();
    assert!(config.enabled);
    assert!(!config.compressors.json);
    assert!(!config.compressors.diff);
    assert!(!config.compressors.log);
    assert!(!config.compressors.search);
    // Unspecified fields use defaults
    assert!(!config.compressors.code);
    assert!(!config.compressors.prose);
}

#[test]
fn test_ccr_config_custom() {
    let json = r#"{"backend": "memory", "capacity": 500, "ttl_secs": 600}"#;
    let ccr: CcrConfig = serde_json::from_str(json).unwrap();
    assert_eq!(ccr.backend, "memory");
    assert_eq!(ccr.capacity, 500);
    assert_eq!(ccr.ttl_secs, 600);
}

#[test]
fn test_tokenizer_config_tiktoken() {
    let json = r#"{"backend": "tiktoken"}"#;
    let tc: TokenizerConfig = serde_json::from_str(json).unwrap();
    assert_eq!(tc.backend, "tiktoken");
}