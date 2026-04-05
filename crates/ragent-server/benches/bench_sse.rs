//! Criterion benchmark for SSE event serialization.
//!
//! Measures the cost of converting [`Event`] variants into Axum SSE events
//! via [`event_to_sse`]. The typed-struct approach should avoid intermediate
//! `serde_json::Value` allocations compared to the previous `json!` macro
//! implementation.

#![allow(missing_docs)]

use criterion::{Criterion, black_box, criterion_group, criterion_main};
use ragent_core::event::{Event, FinishReason};
use ragent_server::sse::{event_to_parts, event_to_sse};

fn make_events() -> Vec<(&'static str, Event)> {
    vec![
        (
            "SessionCreated",
            Event::SessionCreated {
                session_id: "sess-001".into(),
            },
        ),
        (
            "TextDelta",
            Event::TextDelta {
                session_id: "sess-001".into(),
                text: "Hello, world! This is a moderate-length text delta chunk.".into(),
            },
        ),
        (
            "ToolCallEnd",
            Event::ToolCallEnd {
                session_id: "sess-001".into(),
                call_id: "call-42".into(),
                tool: "read_file".into(),
                error: None,
                duration_ms: 123,
            },
        ),
        (
            "MessageEnd",
            Event::MessageEnd {
                session_id: "sess-001".into(),
                message_id: "msg-99".into(),
                reason: FinishReason::Stop,
            },
        ),
        (
            "ToolResult",
            Event::ToolResult {
                session_id: "sess-001".into(),
                call_id: "call-42".into(),
                tool: "read_file".into(),
                content: "fn main() {\n    println!(\"Hello\");\n}\n".into(),
                content_line_count: 3,
                metadata: Some(serde_json::json!({"lines_read": 3})),
                success: true,
            },
        ),
        (
            "SubagentStart",
            Event::SubagentStart {
                session_id: "sess-001".into(),
                task_id: "task-7".into(),
                child_session_id: "child-sess-002".into(),
                agent: "explore".into(),
                task: "Find all usages of the Config struct".into(),
                background: true,
            },
        ),
        (
            "TeammateMessage",
            Event::TeammateMessage {
                session_id: "sess-001".into(),
                team_name: "code-review".into(),
                from: "tm-001".into(),
                to: "lead".into(),
                preview: "I've finished reviewing the auth module.".into(),
            },
        ),
    ]
}

fn bench_event_to_sse(c: &mut Criterion) {
    let events = make_events();
    let mut group = c.benchmark_group("event_to_sse");

    for (name, event) in &events {
        group.bench_function(*name, |b| {
            b.iter(|| {
                let _ = black_box(event_to_sse(black_box(event)));
            });
        });
    }

    group.finish();
}

fn bench_event_to_sse_mixed(c: &mut Criterion) {
    let events = make_events();
    c.bench_function("event_to_sse_mixed_batch", |b| {
        b.iter(|| {
            for (_, event) in &events {
                let _ = black_box(event_to_sse(black_box(event)));
            }
        });
    });
}

fn bench_event_to_parts(c: &mut Criterion) {
    let events = make_events();
    let mut group = c.benchmark_group("event_to_parts");

    for (name, event) in &events {
        group.bench_function(*name, |b| {
            b.iter(|| {
                let _ = black_box(event_to_parts(black_box(event)));
            });
        });
    }

    group.finish();
}

fn bench_rate_limiter(c: &mut Criterion) {
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::time::Instant;

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let mut group = c.benchmark_group("rate_limiter");

    // Benchmark lock + insert for a single session.
    group.bench_function("single_session_insert", |b| {
        let limiter = Arc::new(tokio::sync::Mutex::new(
            HashMap::<String, (u32, Instant)>::new(),
        ));
        b.iter(|| {
            rt.block_on(async {
                let mut lim = limiter.lock().await;
                let now = Instant::now();
                let entry = lim.entry("session-1".to_string()).or_insert((0, now));
                if now.duration_since(entry.1).as_secs() >= 60 {
                    *entry = (1, now);
                } else {
                    entry.0 += 1;
                }
                black_box(entry.0);
            });
        });
    });

    // Benchmark with many sessions to measure HashMap overhead.
    group.bench_function("multi_session_lookup", |b| {
        let limiter = Arc::new(tokio::sync::Mutex::new(
            HashMap::<String, (u32, Instant)>::new(),
        ));
        // Pre-populate 1000 sessions.
        rt.block_on(async {
            let mut lim = limiter.lock().await;
            let now = Instant::now();
            for i in 0..1000 {
                lim.insert(format!("session-{i}"), (1, now));
            }
        });
        let mut idx = 0u32;
        b.iter(|| {
            let key = format!("session-{}", idx % 1000);
            idx = idx.wrapping_add(1);
            rt.block_on(async {
                let mut lim = limiter.lock().await;
                let now = Instant::now();
                let entry = lim.entry(key).or_insert((0, now));
                entry.0 += 1;
                black_box(entry.0);
            });
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    bench_event_to_sse,
    bench_event_to_sse_mixed,
    bench_event_to_parts,
    bench_rate_limiter
);
criterion_main!(benches);
