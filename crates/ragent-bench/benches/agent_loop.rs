//! PERFPLAN Milestone F-2: Criterion benchmarks for the agent action loop
//! hot path.
//!
//! These benches exercise the per-turn and per-step operations that
//! Milestones A–E optimised, so regressions are caught before they ship.
//! They run hermetically (no network, no real LLM) using synthetic fixtures
//! and the [`ragent_bench::MockLlmClient`] for the streaming path.
//!
//! Run with: `cargo bench -p ragent-bench --bench agent_loop`
//!
//! Covered scenarios:
//! - `history_to_chat_messages` — per-turn history→ChatMessage conversion
//!   (P-22-adjacent; the function awaits image reads so it stays async).
//! - `tool_result_content_for_llm` — per-tool-call result truncation (P-16).
//! - `estimate_request_bytes` — per-step request-size estimate (P-7).
//! - `estimate_tool_definition_bytes` — one-time tool-definition byte sum.
//! - `interim_save_hash` — per-step change-detection hash (P-12).
//! - `mock_llm_chat_stream` — MockLlmClient stream throughput (F-1).

use std::sync::Arc;
use std::time::Instant;

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};
use futures::StreamExt;
use ragent_agent::session::processor::{
    estimate_request_bytes, estimate_tool_definition_bytes, history_to_chat_messages,
    tool_result_content_for_llm,
};
use ragent_agent::message::{Message, MessagePart, Role};
use ragent_agent::llm::{ChatContent, ChatMessage, ChatRequest, ContentPart, LlmClient, ToolDefinition};
use ragent_bench::{MockLlmClient, MockLlmScript};
use ragent_types::StreamEvent;
use serde_json::json;

/// Build a synthetic history of `n` user/assistant turns with tool calls.
fn synthetic_history(n: usize) -> Vec<Message> {
    let mut messages = Vec::with_capacity(n * 2);
    for i in 0..n {
        let user = Message::user_text("session-bench", &format!("turn {i}: do work"));
        messages.push(user);
        let assistant = Message::new(
            "session-bench",
            Role::Assistant,
            vec![
                MessagePart::Text { text: format!("response {i}") },
                MessagePart::ToolCall {
                    tool: "bash".to_string(),
                    call_id: format!("call-{i}"),
                    state: ragent_agent::message::ToolCallState {
                        status: ragent_agent::message::ToolCallStatus::Completed,
                        input: json!({"command": "echo hi"}),
                        output: Some(json!({"content": "hi\n"})),
                        error: None,
                        duration_ms: Some(5),
                    },
                },
            ],
        );
        messages.push(assistant);
    }
    messages
}

/// Build a synthetic `ChatRequest` with `n_messages` messages and a
/// realistic-sized tool list.
fn synthetic_request(n_messages: usize) -> ChatRequest {
    let messages: Arc<Vec<ChatMessage>> = Arc::new(
        (0..n_messages)
            .map(|i| ChatMessage {
                role: if i % 2 == 0 { "user" } else { "assistant" }.to_string(),
                content: ChatContent::Text(format!("message {i} with some content")),
            })
            .collect(),
    );
    ChatRequest {
        model: "bench-model".to_string(),
        messages,
        tools: Arc::new(Vec::new()),
        temperature: None,
        top_p: None,
        max_tokens: None,
        system: Some(Arc::from("bench system prompt")),
        options: Default::default(),
        session_id: Some("session-bench".to_string()),
        request_id: Some("req-bench".to_string()),
        stream_timeout_secs: None,
        thinking: None,
    }
}

/// Build `n` synthetic tool definitions with non-trivial parameter schemas.
fn synthetic_tool_definitions(n: usize) -> Vec<ToolDefinition> {
    (0..n)
        .map(|i| ToolDefinition {
            name: format!("tool_{i}"),
            description: format!("Tool number {i} — does something useful for the benchmark."),
            parameters: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string", "description": "a file path"},
                    "mode": {"type": "string", "enum": ["read", "write"]},
                    "timeout": {"type": "integer", "minimum": 0}
                },
                "required": ["path"]
            }),
        })
        .collect()
}

/// Per-step change-detection hash mirroring the P-12 interim-save hash.
fn interim_save_hash(parts: &[MessagePart]) -> u64 {
    use rustc_hash::FxHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = FxHasher::default();
    for part in parts {
        std::mem::discriminant(part).hash(&mut hasher);
        match part {
            MessagePart::Text { text } => text.hash(&mut hasher),
            MessagePart::ToolCall { tool, call_id, state } => {
                tool.hash(&mut hasher);
                call_id.hash(&mut hasher);
                std::mem::discriminant(&state.status).hash(&mut hasher);
                if let Ok(bytes) = serde_json::to_vec(&state.input) {
                    bytes.hash(&mut hasher);
                }
                if let Some(out) = &state.output {
                    if let Ok(bytes) = serde_json::to_vec(out) {
                        bytes.hash(&mut hasher);
                    }
                }
                if let Some(err) = &state.error {
                    err.hash(&mut hasher);
                }
                if let Some(dur) = &state.duration_ms {
                    dur.hash(&mut hasher);
                }
            }
            MessagePart::Reasoning { text } => text.hash(&mut hasher),
            MessagePart::Image(img) => {
                img.mime_type.hash(&mut hasher);
                img.path.hash(&mut hasher);
            }
        }
    }
    hasher.finish()
}

fn bench_history_to_chat_messages(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("history_to_chat_messages");
    for n in [10usize, 50, 200, 800] {
        let history = synthetic_history(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &history, |b, history| {
            b.iter(|| {
                let h = history.clone();
                rt.block_on(async move {
                    black_box(history_to_chat_messages(&h).await)
                })
            });
        });
    }
    group.finish();
}

fn bench_tool_result_content_for_llm(c: &mut Criterion) {
    let mut group = c.benchmark_group("tool_result_content_for_llm");
    // Short result — fast path (no truncation).
    let short = "hello world\n".to_string();
    group.bench_function("short", |b| {
        b.iter(|| black_box(tool_result_content_for_llm("bash", &short, None)));
    });
    // Long result — truncation path.
    let long: String = "x".repeat(50_000);
    group.bench_function("long", |b| {
        b.iter(|| black_box(tool_result_content_for_llm("bash", &long, None)));
    });
    group.finish();
}

fn bench_estimate_request_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("estimate_request_bytes");
    for n in [10usize, 100, 500] {
        let request = synthetic_request(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &request, |b, request| {
            b.iter(|| black_box(estimate_request_bytes(request)));
        });
    }
    group.finish();
}

fn bench_estimate_tool_definition_bytes(c: &mut Criterion) {
    let mut group = c.benchmark_group("estimate_tool_definition_bytes");
    for n in [10usize, 50, 111] {
        let defs = synthetic_tool_definitions(n);
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &defs, |b, defs| {
            b.iter(|| black_box(estimate_tool_definition_bytes(defs)));
        });
    }
    group.finish();
}

fn bench_interim_save_hash(c: &mut Criterion) {
    let mut group = c.benchmark_group("interim_save_hash");
    for n in [1usize, 5, 20] {
        let parts: Vec<MessagePart> = (0..n)
            .map(|i| MessagePart::ToolCall {
                tool: "bash".to_string(),
                call_id: format!("call-{i}"),
                state: ragent_agent::message::ToolCallState {
                    status: ragent_agent::message::ToolCallStatus::Completed,
                    input: json!({"command": "ls", "args": ["-la"]}),
                    output: Some(json!({"content": "file1\nfile2\n"})),
                    error: None,
                    duration_ms: Some(5),
                },
            })
            .collect();
        group.throughput(Throughput::Elements(n as u64));
        group.bench_with_input(BenchmarkId::from_parameter(n), &parts, |b, parts| {
            b.iter(|| black_box(interim_save_hash(parts)));
        });
    }
    group.finish();
}

fn bench_mock_llm_chat_stream(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let mut group = c.benchmark_group("mock_llm_chat_stream");
    // Text-only script — measures TTFT + stream drain.
    let text_script = MockLlmScript::text_only("hello from the mock");
    let text_client = MockLlmClient::new(text_script);
    group.bench_function("text_only", |b| {
        b.iter(|| {
            let client = text_client.clone();
            rt.block_on(async move {
                let req = synthetic_request(4);
                let mut stream = client.chat(req).await.unwrap();
                let mut count = 0u64;
                let _start = Instant::now();
                while let Some(ev) = stream.next().await {
                    if matches!(ev, StreamEvent::TextDelta { .. }) {
                        count += 1;
                    }
                }
                black_box(count);
            });
        });
    });
    // Tool-call script — measures tool-call assembly throughput.
    let tool_script =
        MockLlmScript::single_tool_call("bash", r#"{"command":"echo hi"}"#);
    let tool_client = MockLlmClient::new(tool_script);
    group.bench_function("single_tool_call", |b| {
        b.iter(|| {
            let client = tool_client.clone();
            rt.block_on(async move {
                let req = synthetic_request(4);
                let mut stream = client.chat(req).await.unwrap();
                let mut tool_events = 0u64;
                while let Some(ev) = stream.next().await {
                    if matches!(
                        ev,
                        StreamEvent::ToolCallStart { .. }
                            | StreamEvent::ToolCallDelta { .. }
                            | StreamEvent::ToolCallEnd { .. }
                    ) {
                        tool_events += 1;
                    }
                }
                black_box(tool_events);
            });
        });
    });
    group.finish();
}

criterion_group!(
    benches,
    bench_history_to_chat_messages,
    bench_tool_result_content_for_llm,
    bench_estimate_request_bytes,
    bench_estimate_tool_definition_bytes,
    bench_interim_save_hash,
    bench_mock_llm_chat_stream,
);
criterion_main!(benches);