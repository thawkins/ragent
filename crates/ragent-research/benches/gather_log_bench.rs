//! Criterion benchmark for the gather-log JSONL writer (PERF-049 / PERF-050).
//!
//! Measures append throughput for the per-URL record path. Before PERF-049 each
//! record performed `open` + two `write_all` + `flush`; the writer is now opened
//! once behind a 64 KiB `BufWriter`, so a record is an in-memory write. PERF-050
//! additionally removed the per-record `serde_json::Value` tree and the detail
//! key/value clones.

#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use ragent_research::gather_log::GatherLog;
use std::hint::black_box;
use tempfile::TempDir;

fn bench_append_records(c: &mut Criterion) {
    let dir = TempDir::new().expect("tempdir");
    let log = GatherLog::new(dir.path(), "bench").expect("gather log");
    let detail = serde_json::json!({"relevance": "High", "content_chars": 4096});

    c.bench_function("gather_log/append_url_record", |b| {
        b.iter(|| {
            log.log_url(
                black_box("https://example.com/a-page"),
                black_box("rust async runtime hygiene"),
                black_box("considered"),
                black_box("A Page Title"),
                black_box("mf_search"),
                black_box("openalex"),
                None,
                Some(black_box(&detail)),
            )
            .expect("append");
        });
    });

    c.bench_function("gather_log/append_event_marker", |b| {
        b.iter(|| {
            log.log_event(black_box(&serde_json::json!({
                "event": "queries_decomposed",
                "queries": ["a", "b", "c"],
            })))
            .expect("append");
        });
    });

    log.flush().expect("flush");
}

criterion_group!(benches, bench_append_records);
criterion_main!(benches);
