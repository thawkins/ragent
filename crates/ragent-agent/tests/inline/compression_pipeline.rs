//! Compression pipeline tests (M8/T8.2).
//! Compiled as a submodule of compression::pipeline via #[path].

    use super::*;

    #[test]
    fn test_compression_stats_from_tokens() {
        let stats = CompressionStats::from_tokens(10000, 3000);
        assert_eq!(stats.original_tokens, 10000);
        assert_eq!(stats.compressed_tokens, 3000);
        assert!((stats.compression_ratio - 3.333).abs() < 0.01);
        assert_eq!(stats.ccr_entries_stashed, 0);
        assert_eq!(stats.messages_compressed, 0);
    }

    #[test]
    fn test_compression_stats_from_tokens_no_compression() {
        let stats = CompressionStats::from_tokens(5000, 5000);
        assert!((stats.compression_ratio - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_compression_stats_from_tokens_zero_compressed() {
        let stats = CompressionStats::from_tokens(5000, 0);
        assert!((stats.compression_ratio - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_content_type_display() {
        assert_eq!(ContentType::Json.to_string(), "JSON");
        assert_eq!(ContentType::Diff.to_string(), "Diff");
        assert_eq!(ContentType::Log.to_string(), "Log");
        assert_eq!(ContentType::Search.to_string(), "Search");
        assert_eq!(ContentType::Code.to_string(), "Code");
        assert_eq!(ContentType::Prose.to_string(), "Prose");
    }

    #[test]
    fn test_detect_content_type_heuristic_json() {
        assert_eq!(
            detect_content_type_heuristic("{\"key\": \"value\"}"),
            ContentType::Json
        );
        assert_eq!(
            detect_content_type_heuristic("[1, 2, 3]"),
            ContentType::Json
        );
    }

    #[test]
    fn test_detect_content_type_heuristic_diff() {
        assert_eq!(
            detect_content_type_heuristic("diff --git a/file.rs b/file.rs"),
            ContentType::Diff
        );
        assert_eq!(
            detect_content_type_heuristic("--- a/file.rs\n+++ b/file.rs"),
            ContentType::Diff
        );
    }

    #[test]
    fn test_detect_content_type_heuristic_log() {
        assert_eq!(
            detect_content_type_heuristic("ERROR: something failed"),
            ContentType::Log
        );
        assert_eq!(
            detect_content_type_heuristic("WARN: deprecated"),
            ContentType::Log
        );
        assert_eq!(
            detect_content_type_heuristic("INFO: started"),
            ContentType::Log
        );
    }

    #[test]
    fn test_detect_content_type_heuristic_prose() {
        assert_eq!(
            detect_content_type_heuristic("Hello, world!"),
            ContentType::Prose
        );
    }

    #[test]
    fn test_detect_content_type_heuristic_search() {
        let search_output =
            "src/main.rs:42:fn main() {\nsrc/main.rs:43:    println!(\"hello\");\nsrc/main.rs:44:}";
        assert_eq!(
            detect_content_type_heuristic(search_output),
            ContentType::Search
        );
    }

    #[test]
    fn test_count_tokens_text() {
        let count = count_tokens_text("Hello, world!");
        assert!(count > 0, "Token count should be positive");
        assert!(
            count < 100,
            "Short text should not have excessive token count"
        );
    }

    #[test]
    fn test_count_tokens_text_empty() {
        let count = count_tokens_text("");
        assert_eq!(count, 0, "Empty text should have zero tokens");
    }

    #[test]
    fn test_should_compress_under_threshold() {
        let messages = vec![Message::user_text("test-session", "Hello, world!")];
        assert!(!should_compress(&messages, 128_000, 0.80));
    }

    #[test]
    fn test_should_compress_over_threshold() {
        let long_text = "x ".repeat(500_000);
        let messages = vec![Message::user_text("test-session", long_text)];
        assert!(should_compress(&messages, 10_000, 0.80));
    }

    #[test]
    fn test_headroom_tokenizer_available() {
        let estimator = headroom_core::tokenizer::EstimatingCounter::default();
        let count = estimator.count_text("Hello, world!");
        assert!(count > 0, "EstimatingCounter should count at least 1 token");
    }

    #[test]
    fn test_headroom_hello() {
        assert_eq!(headroom_core::hello(), "headroom-core");
    }

    #[test]
    fn test_headroom_diff_compressor_available() {
        let compressor = DiffCompressor::default();
        let result = compressor.compress("diff --git a/test.rs b/test.rs\n--- a/test.rs\n+++ b/test.rs\n@@ -1,3 +1,3 @@\n-old line\n+new line\n context\n", "");
        // DiffCompressor should produce output (possibly unchanged for short diffs).
        assert!(!result.compressed.is_empty() || result.original_line_count == 0);
    }

    #[test]
    fn test_find_protected_messages_protects_first_and_last_user() {
        let messages = vec![
            Message::user_text("s1", "First message"), // idx 0: protected (first)
            Message::new("s1", Role::Assistant, vec![]), // idx 1: not protected
            Message::user_text("s1", "Middle message"), // idx 2: not protected
            Message::new("s1", Role::Assistant, vec![]), // idx 3: not protected
            Message::user_text("s1", "Latest message"), // idx 4: protected (last user)
        ];
        let protected = find_protected_messages(&messages);
        assert!(protected.contains(&0), "First message should be protected");
        assert!(
            protected.contains(&4),
            "Last user message should be protected"
        );
        assert!(
            !protected.contains(&1),
            "Assistant message should not be protected"
        );
        assert!(
            !protected.contains(&2),
            "Middle user message should not be protected"
        );
        assert!(
            !protected.contains(&3),
            "Assistant message should not be protected"
        );
    }

    #[test]
    fn test_find_protected_messages_single_message() {
        let messages = vec![Message::user_text("s1", "Only message")];
        let protected = find_protected_messages(&messages);
        assert!(protected.contains(&0), "Single message should be protected");
    }

    #[test]
    fn test_find_char_boundary_ascii() {
        let s = "Hello, world!";
        let boundary = find_char_boundary(s, 5);
        assert_eq!(&s[..boundary], "Hello");
    }

    #[test]
    fn test_find_char_boundary_empty() {
        let s = "";
        let boundary = find_char_boundary(s, 10);
        assert_eq!(boundary, 0);
    }

    #[test]
    fn test_find_char_boundary_short() {
        let s = "Hi";
        let boundary = find_char_boundary(s, 100);
        assert_eq!(boundary, s.len());
    }

    #[test]
    fn test_find_char_boundary_unicode() {
        // "café" has a multi-byte 'é' at the end.
        let s = "café";
        // The byte length of "caf" is 3; "é" starts at byte 3.
        let boundary = find_char_boundary(s, 3);
        assert_eq!(boundary, 3);
        assert!(s.is_char_boundary(boundary));
    }

    #[test]
    fn test_compress_json_minification() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        let json = r#"{"key1": "value1", "key2": "value2", "key3": "value3"}"#;
        let compressed = compress_json(json, &mut ccr, &config);
        // Minified JSON should be shorter or same length.
        assert!(
            compressed.len() <= json.len(),
            "Minified JSON should not be longer"
        );
    }

    #[test]
    fn test_compress_json_array_sampling() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        // Create a large JSON array.
        let items: Vec<String> = (0..100).map(|i| format!(r#"{{"id": {i}}}"#)).collect();
        let json = format!("[{}]", items.join(", "));
        let compressed = compress_json(&json, &mut ccr, &config);
        // For arrays > 50 items, compression should reduce size.
        assert!(
            compressed.len() < json.len(),
            "Large JSON array should be compressed"
        );
    }

    #[test]
    fn test_compress_log_priority_filtering() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        let log = "ERROR: critical failure\nINFO: starting\nINFO: running\nINFO: still running\nWARN: deprecated\nINFO: done\n";
        let compressed = compress_log(log, &mut ccr, &config);
        // ERROR and WARN should be preserved.
        assert!(compressed.contains("ERROR: critical failure"));
        assert!(compressed.contains("WARN: deprecated"));
    }

    #[test]
    fn test_compress_search_dedup() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        // Create search results with > 5 results per file.
        let mut search = String::new();
        for i in 0..20 {
            search.push_str(&format!("src/main.rs:{i}:some code line {i}\n"));
        }
        let compressed = compress_search(&search, &mut ccr, &config);
        // Should keep at most 5 results per file.
        assert!(
            compressed.contains("more results"),
            "Should indicate omitted results"
        );
    }

    #[test]
    fn test_chat_messages_round_trip_preserves_text_and_tool_pair() {
        use crate::llm::{ChatContent, ChatMessage as LlmChatMessage, ContentPart};
        use serde_json::json;

        let chat_messages = vec![
            LlmChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("What is 2+2?".to_string()),
            },
            LlmChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Parts(vec![
                    ContentPart::Text {
                        text: "I will calculate that.".to_string(),
                    },
                    ContentPart::ToolUse {
                        id: "call_1".to_string(),
                        name: "calculator".to_string(),
                        input: json!({"expression": "2+2"}),
                    },
                ]),
            },
            LlmChatMessage {
                role: "user".to_string(),
                content: ChatContent::Parts(vec![ContentPart::ToolResult {
                    tool_use_id: "call_1".to_string(),
                    content: "4".to_string().into(),
                }]),
            },
        ];

        let messages = chat_messages_to_messages(&chat_messages);
        assert_eq!(messages.len(), 3);
        // ToolUse/ToolResult should be paired into a single assistant ToolCall.
        assert_eq!(messages[1].parts.len(), 2);
        assert!(
            matches!(&messages[1].parts[1], MessagePart::ToolCall { state, .. }
            if state.output.as_ref().and_then(|v| v.as_str()) == Some("4"))
        );

        let round_tripped = messages_to_chat_messages(&messages);
        assert_eq!(round_tripped.len(), 4);
        assert_eq!(round_tripped[0].role, "user");
        assert_eq!(round_tripped[1].role, "assistant");
        assert_eq!(round_tripped[2].role, "user");
        assert_eq!(round_tripped[3].role, "user");
    }

    #[test]
    fn test_compress_chat_messages_under_threshold_passes_through() {
        use crate::llm::{ChatContent, ChatMessage as LlmChatMessage};

        let chat_messages = vec![LlmChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Short question".to_string()),
        }];
        let config = CompressionConfig::default();
        let result = compress_chat_messages(&chat_messages, 128_000, 8192, &config);
        assert_eq!(result.chat_messages.len(), chat_messages.len());
        assert_eq!(result.stats.original_tokens, result.stats.compressed_tokens);
    }

    #[test]
    fn test_compress_chat_messages_over_threshold_triggers_compression() {
        use crate::llm::{ChatContent, ChatMessage as LlmChatMessage};

        let long_text = "x ".repeat(50_000);
        let chat_messages = vec![
            LlmChatMessage {
                role: "user".to_string(),
                content: ChatContent::Text("Start".to_string()),
            },
            LlmChatMessage {
                role: "assistant".to_string(),
                content: ChatContent::Text(long_text.clone()),
            },
        ];
        let config = CompressionConfig::default();
        let result = compress_chat_messages(&chat_messages, 1_000, 512, &config);
        assert!(
            result.stats.compressed_tokens < result.stats.original_tokens,
            "Long assistant content should be compressed"
        );
        assert!(!result.chat_messages.is_empty());
    }

    #[test]
    fn test_stray_tool_result_kept_as_text() {
        use crate::llm::{ChatContent, ChatMessage as LlmChatMessage, ContentPart};

        let chat_messages = vec![LlmChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ToolResult {
                tool_use_id: "orphan".to_string(),
                content: "lost result".to_string().into(),
            }]),
        }];
        let messages = chat_messages_to_messages(&chat_messages);
        assert_eq!(messages.len(), 1);
        assert!(matches!(&messages[0].parts[0], MessagePart::Text { text }
            if text.contains("lost result")));
    }

    #[test]
    fn test_image_url_round_trip() {
        use crate::llm::{ChatContent, ChatMessage as LlmChatMessage, ContentPart};

        let chat_messages = vec![LlmChatMessage {
            role: "user".to_string(),
            content: ChatContent::Parts(vec![ContentPart::ImageUrl {
                url: "data:image/png;base64,ABC".to_string(),
            }]),
        }];
        let messages = chat_messages_to_messages(&chat_messages);
        assert!(matches!(&messages[0].parts[0], MessagePart::Image(img)
            if img.path.to_string_lossy() == "data:image/png;base64,ABC"));
        let round_tripped = messages_to_chat_messages(&messages);
        assert!(matches!(
            &round_tripped[0].content,
            ChatContent::Parts(parts)
            if matches!(parts.first(), Some(ContentPart::ImageUrl { url }) if url == "data:image/png;base64,ABC")
        ));
    }

    #[test]
    fn test_compress_history_under_threshold() {
        let messages = vec![
            Message::user_text("s1", "Hello"),
            Message::new(
                "s1",
                Role::Assistant,
                vec![MessagePart::Text {
                    text: "Hi there!".into(),
                }],
            ),
        ];
        let config = CompressionConfig::default();
        let result = compress_history(&messages, 128_000, 8192, &config);
        // Under threshold — should return unchanged.
        assert_eq!(result.messages.len(), messages.len());
        assert_eq!(result.stats.original_tokens, result.stats.compressed_tokens);
    }

    #[test]
    fn test_compress_history_preserves_protected_messages() {
        // Create messages that exceed the threshold.
        let long_text = "x ".repeat(50_000);
        let messages = vec![Message::user_text("s1", &long_text)];
        let config = CompressionConfig::default();
        // Use a very small context window to force compression.
        let result = compress_history(&messages, 1000, 512, &config);
        // The first (and only) message is both first AND last user message,
        // so it should be protected and passed through.
        assert!(!result.messages.is_empty());
    }

    #[test]
    fn test_compress_text_short_content() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        let compressed = compress_text("Hello", ContentType::Prose, &mut ccr, &config);
        assert_eq!(
            compressed, "Hello",
            "Short content should pass through unchanged"
        );
    }

    #[test]
    fn test_compress_text_empty() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        let compressed = compress_text("", ContentType::Prose, &mut ccr, &config);
        assert!(compressed.is_empty());
    }

    #[test]
    fn test_compress_diff_with_real_diff() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        let diff = "diff --git a/test.rs b/test.rs\n--- a/test.rs\n+++ b/test.rs\n@@ -1,5 +1,5 @@\n-old line 1\n-old line 2\n context\n+new line 1\n+new line 2\n context\n";
        let compressed = compress_diff(diff, &mut ccr, &config);
        // DiffCompressor should produce output.
        assert!(!compressed.is_empty());
    }

    #[test]
    fn test_compress_code_truncation_with_ccr() {
        let mut ccr = CcrStoreHandle::in_memory();
        let config = CompressionConfig::default();
        // Create very long code content.
        let long_code = format!("fn main() {{\n{}\n}}", "// comment\n".repeat(50000));
        assert!(long_code.len() > 50_000, "Should be over 50k chars");
        let compressed = compress_text(&long_code, ContentType::Code, &mut ccr, &config);
        assert!(
            compressed.len() < long_code.len(),
            "Long code should be truncated"
        );
        // Should contain a CCR marker.
        assert!(
            compressed.contains("<<ccr:"),
            "Truncated content should contain CCR marker"
        );
        // Should be able to retrieve the original.
        let key = crate::compression::ccr_store::parse_ccr_marker(&compressed).unwrap();
        let original = ccr.retrieve(&key);
        assert_eq!(original.as_deref(), Some(long_code.as_str()));
    }

#[test]
fn test_should_compress_chat_messages_under_threshold() {
    use crate::llm::{ChatContent, ChatMessage as LlmChatMessage};

    let chat_messages = vec![LlmChatMessage {
        role: "user".to_string(),
        content: ChatContent::Text("Short question".to_string()),
    }];
    assert!(!should_compress_chat_messages(
        &chat_messages,
        128_000,
        0.80
    ));
}

#[test]
fn test_should_compress_chat_messages_over_threshold() {
    use crate::llm::{ChatContent, ChatMessage as LlmChatMessage};

    let long_text = "x ".repeat(50_000);
    let chat_messages = vec![
        LlmChatMessage {
            role: "user".to_string(),
            content: ChatContent::Text("Start".to_string()),
        },
        LlmChatMessage {
            role: "assistant".to_string(),
            content: ChatContent::Text(long_text.clone()),
        },
    ];
    assert!(should_compress_chat_messages(&chat_messages, 1_000, 0.80));
}

#[test]
fn test_count_tokens_includes_tool_call_input() {
    use crate::message::{MessagePart, Role, ToolCallState, ToolCallStatus};
    use serde_json::json;

    let large_input = "x ".repeat(50_000);
    let state = ToolCallState {
        status: ToolCallStatus::Completed,
        input: json!({"content": large_input}),
        output: Some(json!({"ok": true})),
        error: None,
        duration_ms: Some(1),
    };
    let msg = Message::new(
        "test-session",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "write_file".to_string(),
            call_id: "call-1".to_string(),
            state,
        }],
    );

    let count = count_tokens(&[msg]);
    assert!(
        count > 12_000,
        "count_tokens should include large tool-call input arguments; got {count}"
    );
}

#[test]
fn test_count_tokens_tool_call_input_changes_threshold() {
    use crate::message::{MessagePart, Role, ToolCallState, ToolCallStatus};
    use serde_json::json;

    let large_input = "x ".repeat(40_000);
    let state = ToolCallState {
        status: ToolCallStatus::Completed,
        input: json!({"content": large_input}),
        output: Some(json!({"ok": true})),
        error: None,
        duration_ms: Some(1),
    };
    let msg = Message::new(
        "test-session",
        Role::Assistant,
        vec![MessagePart::ToolCall {
            tool: "write_file".to_string(),
            call_id: "call-2".to_string(),
            state,
        }],
    );

    // 80% of a 10_000-token window is 8_000. The large input alone is ~40k
    // chars / 4 + overhead, so it should cross the threshold.
    assert!(
        should_compress(&[msg], 10_000, 0.80),
        "should_compress must fire when tool-call input dominates the window"
    );

}
