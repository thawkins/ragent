//! Criterion benchmarks for the M3 async-runtime hot paths (PERF-079).
//!
//! Backs the M3 acceptance tests: `EventBus::publish` moves the event instead
//! of cloning it (PERF-054), and `redact_secrets_cow` borrows a non-secret
//! payload instead of allocating (PERF-055). Run with:
//!
//! ```text
//! cargo bench -p ragent-agent --bench m3_hot_paths
//! ```

#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use ragent_agent::event::{Event, EventBus};
use ragent_agent::sanitize::redact_secrets_cow;

/// Build a representative `TextDelta` (the per-token publish payload).
fn text_delta(i: usize) -> Event {
    Event::TextDelta {
        session_id: "bench-session".into(),
        text: format!("token-{i} ").repeat(8),
    }
}

/// PERF-054: every streamed token publishes an event. Moving the event into
/// the broadcast channel must not deep-clone the payload.
fn bench_event_publish(c: &mut Criterion) {
    // A subscriber must exist so `publish` takes the send path rather than the
    // no-subscriber early return.
    let bus = EventBus::new(64);
    let mut rx = bus.subscribe();

    c.bench_function("event_bus/publish_text_delta", |b| {
        let mut i = 0usize;
        b.iter(|| {
            bus.publish(text_delta(i));
            // Drain so the broadcast channel never fills.
            while rx.try_recv().is_ok() {}
            i = i.wrapping_add(1);
        });
    });
}

/// PERF-055: SSE serialisation runs the redactor over every event; a payload
/// with no secret must borrow, not allocate.
fn bench_redact_secrets(c: &mut Criterion) {
    let clean = "fn main() { println!(\"hello world\"); } // no secrets here";
    let with_bearer = "Authorization: Bearer abcdefghijklmnopqrstuvwxyz0123456789";

    c.bench_function("sanitize/redact_secrets_clean", |b| {
        b.iter(|| {
            let out = redact_secrets_cow(std::hint::black_box(clean));
            std::hint::black_box(out);
        });
    });
    c.bench_function("sanitize/redact_secrets_bearer", |b| {
        b.iter(|| {
            let out = redact_secrets_cow(std::hint::black_box(with_bearer));
            std::hint::black_box(out);
        });
    });
}

criterion_group!(benches, bench_event_publish, bench_redact_secrets);
criterion_main!(benches);
