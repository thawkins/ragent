//! Criterion benchmarks for history save/load cycle.
//!
//! Covers COMPLIANCE.md Section 5.A:
//! - save_history with varying history sizes
//! - load_history reading back saved histories

#![allow(missing_docs)]

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};

#[path = "../tests/support/mod.rs"]
mod support;

fn bench_save_history(c: &mut Criterion) {
    let mut group = c.benchmark_group("save_history");

    for &count in &[100, 500, 2_000] {
        group.bench_with_input(BenchmarkId::new("entries", count), &count, |b, &n| {
            let dir = tempfile::tempdir().expect("tmpdir");
            let hist_path = dir.path().join("bench_history.txt");
            let mut app = support::make_app();
            app.set_history_file(hist_path.clone());
            // Populate history
            for i in 0..n {
                app.input_history.push(format!(
                    "benchmark entry {i} with some typical length content 🦀"
                ));
            }
            b.iter(|| {
                let _ = app.save_history();
            });
        });
    }
    group.finish();
}

fn bench_load_history(c: &mut Criterion) {
    let mut group = c.benchmark_group("load_history");

    for &count in &[100, 500, 2_000] {
        group.bench_with_input(BenchmarkId::new("entries", count), &count, |b, &n| {
            let dir = tempfile::tempdir().expect("tmpdir");
            let hist_path = dir.path().join("bench_history.txt");
            // Pre-populate and save
            let mut app = support::make_app();
            app.set_history_file(hist_path.clone());
            for i in 0..n {
                app.input_history.push(format!(
                    "benchmark entry {i} with some typical length content 🦀"
                ));
            }
            let _ = app.save_history();

            b.iter(|| {
                app.input_history.clear();
                let _ = app.load_history();
            });
        });
    }
    group.finish();
}

criterion_group!(benches, bench_save_history, bench_load_history);
criterion_main!(benches);
