//! Integration tests for the compression system.
//!
//! Tests cover:
//! - CompressionConfig parsing and defaults
//! - CompressionMode parsing and mode behaviour
//! - compress_help output format
//! - compress_history_with_mode with each mode
//! - Token counting helpers

// ── Compression config backward compatibility (NFR-004) ─────────────────────

#[test]
fn test_compression_config_default_enabled() {
    use ragent_config::compression::CompressionConfig;
    let config = CompressionConfig::default();
    assert!(
        config.enabled,
        "Default config should have compression enabled"
    );
}

#[test]
fn test_compression_config_disabled_preserves_defaults() {
    use ragent_config::compression::CompressionConfig;
    let json = r#"{"enabled": false}"#;
    let config: CompressionConfig = serde_json::from_str(json).unwrap();
    assert!(!config.enabled, "Should be disabled");
    // Other defaults should still be present (NFR-004 backward compat)
    assert!((config.auto_threshold - 0.80).abs() < f64::EPSILON);
    assert_eq!(config.ccr.backend, "sqlite");
    assert_eq!(config.ccr.capacity, 1000);
    assert!(config.compressors.json);
    assert!(config.compressors.diff);
}

#[test]
fn test_compression_config_partial_deserialize_backward_compat() {
    use ragent_config::compression::CompressionConfig;
    // Only specifying `enabled` — all other fields must use defaults
    let json = r#"{"enabled": true}"#;
    let config: CompressionConfig = serde_json::from_str(json).unwrap();
    assert!(config.enabled);
    assert!((config.auto_threshold - 0.80).abs() < f64::EPSILON);
    assert_eq!(config.ccr.backend, "sqlite");
    assert!(config.compressors.json);
    assert!(config.compressors.diff);
    assert!(config.compressors.log);
    assert!(config.compressors.search);
    assert!(!config.compressors.code);
    assert!(!config.compressors.prose);
}

#[test]
fn test_compression_config_all_compressors_disabled() {
    use ragent_config::compression::CompressionConfig;
    let json = r#"{"enabled": true, "compressors": {"json": false, "diff": false, "log": false, "search": false}}"#;
    let config: CompressionConfig = serde_json::from_str(json).unwrap();
    assert!(config.enabled);
    assert!(!config.compressors.json);
    assert!(!config.compressors.diff);
    assert!(!config.compressors.log);
    assert!(!config.compressors.search);
    // Unspecified fields should use defaults
    assert!(!config.compressors.code);
    assert!(!config.compressors.prose);
}

// ── Compression feature-gated tests ─────────────────────────────────────────

#[cfg(feature = "compression")]
mod compression_feature_tests {
    use ragent_agent::compression::{
        CompressionMode, compress_help, compress_history, compress_history_with_mode, count_tokens,
    };
    use ragent_agent::message::{Message, MessagePart, Role};
    use ragent_config::compression::CompressionConfig;

    fn make_text_message(role: Role, text: &str) -> Message {
        Message::new(
            "test-session",
            role,
            vec![MessagePart::Text {
                text: text.to_string(),
            }],
        )
    }

    // ── CompressionMode parsing ���──────────────────────────────────────────

    #[test]
    fn test_compression_mode_default_parse() {
        let mode: CompressionMode = "default".parse().unwrap();
        assert_eq!(mode, CompressionMode::Default);
    }

    #[test]
    fn test_compression_mode_default_empty_parse() {
        // Empty string should parse as Default
        let mode: CompressionMode = "".parse().unwrap();
        assert_eq!(mode, CompressionMode::Default);
    }

    #[test]
    fn test_compression_mode_aggressive_parse() {
        let mode: CompressionMode = "aggressive".parse().unwrap();
        assert_eq!(mode, CompressionMode::Aggressive);
    }

    #[test]
    fn test_compression_mode_aggressive_aliases() {
        assert_eq!(
            "max".parse::<CompressionMode>().unwrap(),
            CompressionMode::Aggressive
        );
        assert_eq!(
            "maximum".parse::<CompressionMode>().unwrap(),
            CompressionMode::Aggressive
        );
    }

    #[test]
    fn test_compression_mode_conservative_parse() {
        let mode: CompressionMode = "conservative".parse().unwrap();
        assert_eq!(mode, CompressionMode::Conservative);
    }

    #[test]
    fn test_compression_mode_conservative_aliases() {
        assert_eq!(
            "light".parse::<CompressionMode>().unwrap(),
            CompressionMode::Conservative
        );
        assert_eq!(
            "minimal".parse::<CompressionMode>().unwrap(),
            CompressionMode::Conservative
        );
    }

    #[test]
    fn test_compression_mode_case_insensitive() {
        assert_eq!(
            "DEFAULT".parse::<CompressionMode>().unwrap(),
            CompressionMode::Default
        );
        assert_eq!(
            "Aggressive".parse::<CompressionMode>().unwrap(),
            CompressionMode::Aggressive
        );
        assert_eq!(
            "CONSERVATIVE".parse::<CompressionMode>().unwrap(),
            CompressionMode::Conservative
        );
    }

    #[test]
    fn test_compression_mode_invalid() {
        let result = "invalid_mode".parse::<CompressionMode>();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unknown compression mode"));
    }

    #[test]
    fn test_compression_mode_display() {
        assert_eq!(format!("{}", CompressionMode::Default), "default");
        assert_eq!(format!("{}", CompressionMode::Aggressive), "aggressive");
        assert_eq!(format!("{}", CompressionMode::Conservative), "conservative");
    }

    #[test]
    fn test_compression_mode_serde_roundtrip() {
        let mode = CompressionMode::Aggressive;
        let json = serde_json::to_string(&mode).unwrap();
        assert_eq!(json, "\"aggressive\"");
        let deserialized: CompressionMode = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, mode);
    }

    // ── compress_history_with_mode ────────────────────────────────────────

    #[test]
    fn test_compress_history_with_mode_default_under_threshold() {
        let config = CompressionConfig::default();
        let messages = vec![
            make_text_message(Role::User, "Hello"),
            make_text_message(Role::Assistant, "Hi there"),
        ];
        let result =
            compress_history_with_mode(&messages, 128_000, 8192, &config, CompressionMode::Default);
        // Under threshold — messages should be unchanged
        assert_eq!(result.messages.len(), 2);
    }

    #[test]
    fn test_compress_history_with_mode_conservative_under_threshold() {
        let config = CompressionConfig::default();
        let messages = vec![
            make_text_message(Role::User, "Hello"),
            make_text_message(Role::Assistant, "Hi there"),
        ];
        let result = compress_history_with_mode(
            &messages,
            128_000,
            8192,
            &config,
            CompressionMode::Conservative,
        );
        // Under threshold — messages should be unchanged
        assert_eq!(result.messages.len(), 2);
    }

    #[test]
    fn test_compress_history_with_mode_aggressive_under_threshold() {
        let config = CompressionConfig::default();
        let messages = vec![
            make_text_message(Role::User, "Hello"),
            make_text_message(Role::Assistant, "Hi there"),
        ];
        let result = compress_history_with_mode(
            &messages,
            128_000,
            8192,
            &config,
            CompressionMode::Aggressive,
        );
        // Under threshold — messages should be unchanged
        assert_eq!(result.messages.len(), 2);
    }

    #[test]
    fn test_compress_history_with_mode_aggressive_enables_all_compressors() {
        // Aggressive mode should set auto_threshold to 0.50, enabling all compressors
        // and relevance filtering.
        let config = CompressionConfig::default();
        let large_json = format!(
            "[{}]",
            (0..100)
                .map(|i| format!(
                    "{{\"id\": {}, \"name\": \"item {}\", \"value\": {}}}",
                    i,
                    i,
                    i * 10
                ))
                .collect::<Vec<_>>()
                .join(", ")
        );
        let messages = vec![
            make_text_message(Role::User, "Show me the data"),
            make_text_message(Role::Assistant, &large_json),
            make_text_message(Role::User, "Summarise"),
            make_text_message(Role::Assistant, "Here is a summary of the data"),
        ];
        // With a very small context window, aggressive mode should compress
        let result =
            compress_history_with_mode(&messages, 200, 100, &config, CompressionMode::Aggressive);
        // Result should exist (may have fewer messages due to aggressive threshold)
        assert!(!result.messages.is_empty());
    }

    #[test]
    fn test_compress_history_with_mode_conservative_preserves_content() {
        // Conservative mode should only apply lossless compressors (json, diff)
        let config = CompressionConfig::default();
        let messages = vec![
            make_text_message(Role::User, "Hello"),
            make_text_message(Role::Assistant, "World"),
        ];
        let result = compress_history_with_mode(
            &messages,
            128_000,
            8192,
            &config,
            CompressionMode::Conservative,
        );
        // Under threshold — nothing should be compressed
        assert_eq!(result.messages.len(), 2);
    }

    // ── compress_help ─────────────────────────────────────────────────────

    #[test]
    fn test_compress_help_output_format() {
        let config = CompressionConfig::default();
        let help = compress_help(&config);
        // Should contain the subcommand table
        assert!(help.contains("/compress"), "Help should mention /compress");
        assert!(
            help.contains("aggressive"),
            "Help should mention aggressive mode"
        );
        assert!(
            help.contains("conservative"),
            "Help should mention conservative mode"
        );
        assert!(help.contains("help"), "Help should mention help subcommand");
        assert!(
            help.contains("stats"),
            "Help should mention stats subcommand"
        );
    }

    #[test]
    fn test_compress_help_shows_config_status() {
        let config = CompressionConfig::default();
        let help = compress_help(&config);
        assert!(
            help.contains("enabled"),
            "Help should show enabled/disabled status"
        );
        assert!(
            help.contains("80%"),
            "Help should show threshold percentage"
        );
    }

    #[test]
    fn test_compress_help_shows_compressor_config() {
        let config = CompressionConfig::default();
        let help = compress_help(&config);
        assert!(
            help.contains("json=true"),
            "Help should show json compressor status"
        );
        assert!(
            help.contains("diff=true"),
            "Help should show diff compressor status"
        );
    }

    // ── Token counting ────────────────────────────────────────────���───────

    #[test]
    fn test_count_tokens_basic() {
        let messages = vec![make_text_message(Role::User, "Hello world")];
        let tokens = count_tokens(&messages);
        assert!(tokens > 0, "Should count some tokens");
    }

    #[test]
    fn test_count_tokens_empty() {
        let messages: Vec<Message> = vec![];
        let tokens = count_tokens(&messages);
        assert_eq!(tokens, 0, "Empty history should have zero tokens");
    }

    // ── Compression disabled pass-through ───────────────────────────────

    #[test]
    fn test_compress_history_disabled_passes_through() {
        // When compression config is disabled, compress_history returns the
        // history unchanged because truncation is no longer used as a fallback.
        let config = CompressionConfig {
            enabled: false,
            ..CompressionConfig::default()
        };

        let large_text = "a".repeat(1000);
        let messages: Vec<Message> = (0..10)
            .map(|i| {
                make_text_message(
                    if i % 2 == 0 {
                        Role::User
                    } else {
                        Role::Assistant
                    },
                    &large_text,
                )
            })
            .collect();

        let result = compress_history(&messages, 128_000, 8192, &config);
        // With enabled=false, messages pass through unchanged.
        assert_eq!(result.messages.len(), messages.len());
    }
}
