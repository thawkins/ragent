//! Criterion benchmarks for the M1 agent turn-loop hot path (PERF-079).
//!
//! These benches sit behind the M1 acceptance tests: the per-turn history
//! conversion (PERF-032/033), the request-token estimator (PERF-036), and the
//! compaction `select` split (PERF-036/038). Run with:
//!
//! ```text
//! cargo bench -p ragent-agent --bench turn_loop
//! ```

#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ragent_agent::compaction::{estimate_request_tokens, select, serialize_message};
use ragent_agent::llm::{ChatContent, ChatMessage, ToolDefinition};
use ragent_config::compaction::CompactionConfig;
use ragent_types::message::{Message, MessagePart, Role};

/// Build `n` alternating user/assistant text messages.
fn build_history(n: usize) -> Vec<Message> {
    (0..n)
        .map(|i| {
            let role = if i % 2 == 0 {
                Role::User
            } else {
                Role::Assistant
            };
            Message::new(
                "bench-session",
                role,
                vec![MessagePart::Text {
                    text: format!("Turn {i}: working through the codebase checklist."),
                }],
            )
        })
        .collect()
}

/// PERF-032/033: converting persisted history into provider-facing
/// `ChatMessage`s runs on every turn; a full-transcript deep clone shows up
/// here as a superlinear cost as the session grows.
fn bench_history_conversion(c: &mut Criterion) {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime");

    let mut group = c.benchmark_group("history_to_chat_messages");
    for n in [10usize, 100, 500] {
        let history = build_history(n);
        group.bench_with_input(BenchmarkId::from_parameter(n), &history, |b, history| {
            b.to_async(&rt).iter(|| async {
                let msgs = ragent_agent::session::history::history_to_chat_messages(history).await;
                std::hint::black_box(msgs);
            });
        });
    }
    group.finish();
}

/// PERF-036: the request-token estimator is consulted by the pre-send
/// compaction check on every agent-loop step.
fn bench_estimate_request_tokens(c: &mut Criterion) {
    let tools: Vec<ToolDefinition> = (0..111)
        .map(|i| ToolDefinition {
            name: format!("tool_{i}"),
            description: format!("Description for tool {i}"),
            parameters: serde_json::json!({"type": "object", "properties": {}}),
        })
        .collect();
    let system = "You are a helpful coding agent.";

    let mut group = c.benchmark_group("estimate_request_tokens");
    for n in [10usize, 100, 500] {
        let messages: Vec<ChatMessage> = (0..n)
            .map(|i| ChatMessage {
                role: if i % 2 == 0 { "user" } else { "assistant" }.to_string(),
                content: ChatContent::Text(format!("Message {i} with content.")),
            })
            .collect();
        group.bench_with_input(BenchmarkId::from_parameter(n), &messages, |b, messages| {
            b.iter(|| {
                let total = estimate_request_tokens(Some(system), messages, &tools);
                std::hint::black_box(total);
            });
        });
    }
    group.finish();
}

/// PERF-036/038: the compaction `select` split serialises every message each
/// call; the acceptance criterion is that per-step work is not O(history^2).
fn bench_compaction_select(c: &mut Criterion) {
    let config = CompactionConfig::default();

    let mut group = c.benchmark_group("compaction_select");
    for n in [50usize, 200, 800] {
        group.bench_with_input(BenchmarkId::from_parameter(n), &n, |b, &n| {
            b.iter_batched(
                || build_history(n),
                |history| {
                    let split = select(history, &config, 128_000);
                    std::hint::black_box(split);
                },
                criterion::BatchSize::SmallInput,
            );
        });
    }
    group.finish();
}

/// PERF-038: serialising one message is the inner loop of `select`.
fn bench_serialize_message(c: &mut Criterion) {
    let msg = Message::new(
        "bench-session",
        Role::Assistant,
        vec![
            MessagePart::Text {
                text: "Here is the plan.".to_string(),
            },
            MessagePart::Reasoning {
                text: "Let me consider the tradeoffs.".to_string(),
            },
        ],
    );
    c.bench_function("serialize_message", |b| {
        b.iter(|| {
            let s = serialize_message(std::hint::black_box(&msg), 2000);
            std::hint::black_box(s);
        });
    });
}

criterion_group!(
    benches,
    bench_history_conversion,
    bench_estimate_request_tokens,
    bench_compaction_select,
    bench_serialize_message
);
criterion_main!(benches);
