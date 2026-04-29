//! Integration tests for stream timeout defaults in the shared config crate.

use ragent_config::{Config, config::StreamConfig};

#[test]
fn test_stream_config_defaults_match_documented_timeout() {
    let config = Config::default();

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

    assert_eq!(config.stream.timeout_secs, 120);
    assert_eq!(config.stream.max_retries, 2);
    assert_eq!(config.stream.retry_backoff_secs, 2);
}

#[test]
fn test_stream_config_default_constructor_uses_shorter_stall_timeout() {
    let config = StreamConfig::default();

    assert_eq!(config.timeout_secs, 120);
    assert_eq!(config.max_retries, 4);
    assert_eq!(config.retry_backoff_secs, 2);
}
