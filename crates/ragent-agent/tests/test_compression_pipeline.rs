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

    // ── Per-iteration threshold with LLM-reported count ─────────────────

    #[test]
    fn test_should_compress_with_reported_uses_reported_count_when_available() {
        // When the LLM has reported an input token count from a previous
        // call, the threshold check must use that count (it matches the
        // provider's tokenizer exactly) instead of the local Headroom
        // estimate. This prevents the estimate from triggering compression
        // earlier than the configured `auto_threshold`.
        use ragent_agent::llm::{ChatContent, ChatMessage as LlmChatMessage};
        use ragent_agent::session::processor::should_compress_with_reported;

        let chat_messages = vec![LlmChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Hello".to_string()),
        }];
        let context_window = 128_000_usize;
        let auto_threshold = 0.80_f64;

        // LLM reported 70% of context — below 80% threshold, no compression.
        assert!(
            !should_compress_with_reported(
                &chat_messages,
                context_window,
                auto_threshold,
                (context_window as f64 * 0.70) as u64,
            ),
            "70% reported count should not trigger compression at 80% threshold"
        );

        // LLM reported 80% of context — at the threshold, should compress.
        assert!(
            should_compress_with_reported(
                &chat_messages,
                context_window,
                auto_threshold,
                (context_window as f64 * 0.80) as u64,
            ),
            "80% reported count should trigger compression at 80% threshold"
        );

        // LLM reported 95% of context — well above, should compress.
        assert!(
            should_compress_with_reported(
                &chat_messages,
                context_window,
                auto_threshold,
                (context_window as f64 * 0.95) as u64,
            ),
            "95% reported count should trigger compression at 80% threshold"
        );
    }

    #[test]
    fn test_should_compress_with_reported_falls_back_to_estimate_when_zero() {
        // When `last_reported_input_tokens` is 0 (first call in a turn, or
        // provider omits usage), the helper must fall back to the local
        // Headroom estimate so the threshold check still works.
        use ragent_agent::llm::{ChatContent, ChatMessage as LlmChatMessage};
        use ragent_agent::session::processor::should_compress_with_reported;

        let context_window = 128_000_usize;
        let auto_threshold = 0.80_f64;

        // Small payload — estimate should be well under threshold.
        let small_payload = vec![LlmChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Hello, world!".to_string()),
        }];
        assert!(
            !should_compress_with_reported(&small_payload, context_window, auto_threshold, 0,),
            "Small payload with zero reported count should fall back to estimate (under threshold)"
        );

        // Large payload — estimate should be over threshold.
        let large_text = "x ".repeat(50_000);
        let large_payload = vec![LlmChatMessage {
            role: "assistant".to_string(),
            content: ChatContent::Text(large_text),
        }];
        // Use a small context window so the estimate exceeds the threshold.
        assert!(
            should_compress_with_reported(&large_payload, 1_000, auto_threshold, 0,),
            "Large payload with zero reported count should fall back to estimate (over threshold)"
        );
    }

    #[test]
    fn test_should_compress_with_reported_reported_takes_precedence_over_estimate() {
        // Critical regression test: if the LLM reports 70% but the local
        // estimate would say 95%, the helper must trust the LLM's count
        // (more accurate) and NOT trigger compression. This is the core
        // fix for the "triggering before 80%" bug.
        use ragent_agent::llm::{ChatContent, ChatMessage as LlmChatMessage};
        use ragent_agent::session::processor::should_compress_with_reported;

        // A large payload that, by the local estimate, would exceed 80%
        // of a small context window — but the LLM only saw 70%.
        let large_text = "x ".repeat(20_000);
        let chat_messages = vec![LlmChatMessage {
            role: "assistant".to_string(),
            content: ChatContent::Text(large_text),
        }];
        let context_window = 1_000_usize; // Tiny window so estimate is huge
        let auto_threshold = 0.80_f64;
        // LLM reported 700 tokens = 70% of 1000 — should NOT compress
        // even though the local estimate would say we're way over.
        assert!(
            !should_compress_with_reported(&chat_messages, context_window, auto_threshold, 700,),
            "LLM-reported 70% must override an estimate that would say >80%"
        );
    }

    /// Regression test for the "compression not firing at 80%" bug
    /// (reported against Ollama Cloud / Kimi K2 with a custom tokenizer).
    ///
    /// Scenario: the LLM reports we're at 99% of the context window
    /// (260_000 input tokens out of 262_144), but the LOCAL `EstimatingCounter`
    /// returns a much smaller number (e.g. 50_000) because the local
    /// tokenizer diverges from the provider's tokenizer.
    ///
    /// Before the fix, the per-iteration gate would fire (because the
    /// reported count exceeds threshold) but `compress_history` would
    /// bail out internally (because the local estimate is under
    /// threshold), so the payload was never actually reduced and the
    /// context window kept overflowing.
    ///
    /// The fix removes the local-estimate threshold check from
    /// `compress_history`. The callers already enforce the gate via
    /// `should_compress_with_reported` (per-iteration) and
    /// `should_compress` (initial-history), and the compressor logic
    /// keeps the original content if no benefit is found — so removing
    /// the threshold check can never increase output size.
    #[test]
    fn test_per_iteration_compression_actually_reduces_payload_when_reported_is_high() {
        use ragent_agent::compression::pipeline::compress_chat_messages;
        use ragent_agent::llm::{ChatContent, ChatMessage as LlmChatMessage};
        use ragent_agent::session::processor::should_compress_with_reported;

        // Build a chat-history payload with JSON, diff and log content as
        // separate parts so each compressor routes to the right one.
        let big_json = r#"{"users":[
            {"id":1,"name":"Alice","email":"alice@example.com","role":"admin","created_at":"2025-01-01T00:00:00Z"},
            {"id":2,"name":"Bob","email":"bob@example.com","role":"user","created_at":"2025-01-02T00:00:00Z"},
            {"id":3,"name":"Charlie","email":"charlie@example.com","role":"user","created_at":"2025-01-03T00:00:00Z"}
        ]}"#.repeat(50);
        let big_diff = "diff --git a/foo b/foo\n--- a/foo\n+++ b/foo\n@@ -1,5 +1,5 @@\n-old line\n+new line\n context line\n more context\n".repeat(40);
        let big_log = "[INFO] 2025-01-01 request received\n[WARN] 2025-01-01 slow response\n[ERROR] 2025-01-02 timeout\n[INFO] 2025-01-02 retry succeeded\n".repeat(40);

        let chat_messages = vec![
            LlmChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("Read these files please".to_string()),
            },
            LlmChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Text(big_json),
            },
            LlmChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Text(big_diff),
            },
            LlmChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Text(big_log),
            },
        ];
        let config = CompressionConfig::default(); // auto_threshold = 0.80
        let context_window: usize = 128_000;
        // LLM reports 99% of the context window — gate must fire.
        let last_reported_input_tokens: u64 = 260_000;

        // Pre-condition: the gate fires (reported count exceeds threshold).
        assert!(
            should_compress_with_reported(
                &chat_messages,
                context_window,
                0.80,
                last_reported_input_tokens,
            ),
            "Gate should fire when LLM reports 260K tokens in a 128K window"
        );

        // Run the actual compression pipeline.
        let result = compress_chat_messages(&chat_messages, context_window, 8192, &config);

        // Post-condition: compression actually reduces the payload.
        assert!(
            result.stats.compressed_tokens < result.stats.original_tokens,
            "compress_chat_messages should reduce the payload when called \
             after the gate fires. Original: {}, Compressed: {}",
            result.stats.original_tokens,
            result.stats.compressed_tokens,
        );
    }
}
