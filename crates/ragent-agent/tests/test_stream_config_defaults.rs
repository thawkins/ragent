//! Integration tests for stream timeout defaults in the agent config mirror.

use ragent_agent::config::StreamConfig;

#[test]
fn test_stream_config_defaults_match_documented_timeout() {
    let config = StreamConfig::default();

    assert_eq!(config.timeout_secs, 120);
    assert_eq!(config.max_retries, 4);
    assert_eq!(config.retry_backoff_secs, 2);
}

#[test]
fn test_stream_config_deserializes_partial_override() {
    let config: StreamConfig = serde_json::from_str(
        r#"{
            "max_retries": 1
        }"#,
    )
    .expect("stream config should deserialize");

    assert_eq!(config.timeout_secs, 120);
    assert_eq!(config.max_retries, 1);
    assert_eq!(config.retry_backoff_secs, 2);
}
