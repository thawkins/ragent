//! Integration tests for stream timeout defaults in the shared config crate.

use ragent_config::{Config, config::StreamConfig};

#[test]
fn test_stream_config_defaults_match_documented_timeout() {
    let config = Config::default();

    assert_eq!(config.stream.initial_response_timeout_secs, 300);
    assert_eq!(config.stream.timeout_secs, 120);
    assert_eq!(config.stream.max_retries, 4);
    assert_eq!(config.stream.retry_backoff_secs, 2);
}

#[test]
fn test_stream_config_deserializes_partial_override() {
    let config: Config = serde_json::from_str(
        r#"{
            "stream": {
                "max_retries": 2
            }
        }"#,
    )
    .expect("stream config should deserialize");

    // Overridden field picks up the new value.
    assert_eq!(config.stream.max_retries, 2);
    // All other fields fall back to their documented defaults.
    assert_eq!(config.stream.initial_response_timeout_secs, 300);
    assert_eq!(config.stream.timeout_secs, 120);
    assert_eq!(config.stream.retry_backoff_secs, 2);
}

#[test]
fn test_stream_config_deserializes_initial_response_override() {
    let config: Config = serde_json::from_str(
        r#"{
            "stream": {
                "initial_response_timeout_secs": 600,
                "timeout_secs": 30
            }
        }"#,
    )
    .expect("stream config should deserialize with initial_response override");

    assert_eq!(config.stream.initial_response_timeout_secs, 600);
    assert_eq!(config.stream.timeout_secs, 30);
}

#[test]
fn test_stream_config_default_constructor_uses_shorter_stall_timeout() {
    let config = StreamConfig::default();

    assert_eq!(config.initial_response_timeout_secs, 300);
    assert_eq!(config.timeout_secs, 120);
    assert_eq!(config.max_retries, 4);
    assert_eq!(config.retry_backoff_secs, 2);
}

#[test]
fn test_stream_config_validate_accepts_defaults() {
    let config = StreamConfig::default();
    assert!(config.validate().is_empty(), "defaults should validate");
}

#[test]
fn test_stream_config_validate_rejects_initial_response_below_stall() {
    let config = StreamConfig {
        initial_response_timeout_secs: 10,
        timeout_secs: 120,
        ..StreamConfig::default()
    };
    let problems = config.validate();
    assert!(
        problems
            .iter()
            .any(|p| p.contains("initial_response_timeout_secs")),
        "expected validation problem mentioning initial_response_timeout_secs, got {problems:?}"
    );
}

#[test]
fn test_stream_config_validate_rejects_too_small_values() {
    let config = StreamConfig {
        initial_response_timeout_secs: 1,
        timeout_secs: 1,
        ..StreamConfig::default()
    };
    let problems = config.validate();
    assert_eq!(problems.len(), 2, "got {problems:?}");
}
