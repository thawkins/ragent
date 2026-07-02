#![allow(missing_docs)]
//! Benchmarks for the agent loop performance optimizations.
//!
//! Measures the per-step overhead of tool-definition caching, history
//! compression, and request byte estimation.
//!
//! To run:
//!     cargo bench -p ragent-agent --bench agent_loop

use std::sync::Arc;

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use ragent_agent::llm::{ChatContent, ChatMessage, ChatRequest, ToolDefinition};
use serde_json::json;

/// Build a [`ChatRequest`] with `n` messages and `t` tools.
fn build_request(msg_count: usize, tool_count: usize) -> ChatRequest {
    let messages: Vec<ChatMessage> = (0..msg_count)
        .map(|i| ChatMessage {
            role: if i % 2 == 0 { "user" } else { "assistant" }.to_string(),
            content: ChatContent::Text(format!("Message number {i} with some content.")),
        })
        .collect();

    let tools: Vec<ToolDefinition> = (0..tool_count)
        .map(|i| ToolDefinition {
            name: format!("tool_{i}"),
            description: format!("Description for tool {i}"),
            parameters: json!({"type":"object","properties":{}}),
        })
        .collect();

    ChatRequest {
        model: "claude-test".to_string(),
        messages: Arc::new(messages),
        tools: Arc::new(tools),
        temperature: Some(0.2),
        top_p: None,
        max_tokens: Some(1024),
        system: Some(std::sync::Arc::from("You are a helpful assistant.")),
        options: Default::default(),
        session_id: Some("bench-session".to_string()),
        request_id: Some("bench-req".to_string()),
        stream_timeout_secs: None,
        thinking: None,
    }
}

fn bench_estimate_request_bytes(c: &mut Criterion) {
    let small = build_request(2, 0);
    let medium = build_request(20, 50);
    let large = build_request(100, 111);

    c.bench_function("estimate_bytes_small", |b| {
        b.iter(|| ragent_agent::session::processor::estimate_request_bytes(black_box(&small)))
    });
    c.bench_function("estimate_bytes_medium", |b| {
        b.iter(|| ragent_agent::session::processor::estimate_request_bytes(black_box(&medium)))
    });
    c.bench_function("estimate_bytes_large", |b| {
        b.iter(|| ragent_agent::session::processor::estimate_request_bytes(black_box(&large)))
    });
}

fn bench_chat_request_payload_bytes(c: &mut Criterion) {
    let small = build_request(2, 0);
    let medium = build_request(20, 50);
    let large = build_request(100, 111);

    c.bench_function("serde_bytes_small", |b| {
        b.iter(|| {
            let bytes = serde_json::to_vec(black_box(&small))
                .map(|v| v.len() as u64)
                .unwrap_or(0);
            black_box(bytes);
        })
    });
    c.bench_function("serde_bytes_medium", |b| {
        b.iter(|| {
            let bytes = serde_json::to_vec(black_box(&medium))
                .map(|v| v.len() as u64)
                .unwrap_or(0);
            black_box(bytes);
        })
    });
    c.bench_function("serde_bytes_large", |b| {
        b.iter(|| {
            let bytes = serde_json::to_vec(black_box(&large))
                .map(|v| v.len() as u64)
                .unwrap_or(0);
            black_box(bytes);
        })
    });
}

#[cfg(feature = "compression")]
fn bench_compress_history(c: &mut Criterion) {
    use ragent_agent::compression::compress_history;
    use ragent_agent::message::{Message, MessagePart, Role};
    use ragent_config::compression::CompressionConfig;

    let config = CompressionConfig::default();

    // Small history that fits in context window
    let small: Vec<Message> = (0..10)
        .map(|i| {
            let text = if i % 2 == 0 {
                format!("User message {i}")
            } else {
                format!("Assistant response {i}")
            };
            Message::new(
                "bench",
                if i % 2 == 0 {
                    Role::User
                } else {
                    Role::Assistant
                },
                vec![MessagePart::Text { text }],
            )
        })
        .collect();

    // Large history that exceeds context window
    let large: Vec<Message> = (0..100)
        .map(|i| {
            let text = format!(
                "Message content that is reasonably long to simulate real chat history. Index: {}",
                i
            );
            Message::new(
                "bench",
                if i % 2 == 0 {
                    Role::User
                } else {
                    Role::Assistant
                },
                vec![MessagePart::Text { text }],
            )
        })
        .collect();

    c.bench_function("compress_small_fits", |b| {
        b.iter(|| {
            let result = compress_history(black_box(&small), 128_000, 8192, &config);
            black_box(result);
        })
    });

    c.bench_function("compress_large_exceeds", |b| {
        b.iter(|| {
            let result = compress_history(black_box(&large), 128_000, 8192, &config);
            black_box(result);
        })
    });
}
#[cfg(not(feature = "compression"))]
fn bench_compress_history(_c: &mut Criterion) {}

fn bench_compiled_dir_lists(c: &mut Criterion) {
    use globset::GlobSet;
    use ragent_config::dir_lists::{get_compiled_allowlist, get_compiled_denylist};

    // Ensure lists are loaded
    ragent_config::dir_lists::load_from_config();

    c.bench_function("compiled_allowlist", |b| {
        b.iter(|| {
            let g: Arc<GlobSet> = get_compiled_allowlist();
            black_box(g.is_match("src/main.rs"));
        })
    });
    c.bench_function("compiled_denylist", |b| {
        b.iter(|| {
            let g: Arc<GlobSet> = get_compiled_denylist();
            black_box(g.is_match("/etc/passwd"));
        })
    });
}

fn bench_tool_result_truncation(c: &mut Criterion) {
    use ragent_agent::session::processor::tool_result_content_for_llm;

    // Small payload (no truncation)
    let small = "a".repeat(100);
    // Large payload (triggers truncation)
    let large = "a".repeat(18_000);

    c.bench_function("tool_result_small", |b| {
        b.iter(|| {
            let r = tool_result_content_for_llm("read", black_box(&small), None);
            black_box(r);
        })
    });
    c.bench_function("tool_result_large", |b| {
        b.iter(|| {
            let r = tool_result_content_for_llm("read", black_box(&large), None);
            black_box(r);
        })
    });
}

criterion_group!(
    benches,
    bench_estimate_request_bytes,
    bench_chat_request_payload_bytes,
    bench_compress_history,
    bench_compiled_dir_lists,
    bench_tool_result_truncation
);
criterion_main!(benches);
