#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use ragent_core::snapshot::{incremental_save, restore_snapshot, take_snapshot};
use std::path::PathBuf;
use tempfile::tempdir;

fn bench_take_snapshot(c: &mut Criterion) {
    let dir = tempdir().expect("tempdir");
    let files: Vec<PathBuf> = (0..10)
        .map(|i| {
            let p = dir.path().join(format!("file{i}.txt"));
            std::fs::write(&p, format!("Hello, world! line {i}\n").repeat(100)).expect("write");
            p
        })
        .collect();

    c.bench_function("take_snapshot_10_files", |b| {
        b.iter(|| {
            let snap = take_snapshot("session-bench", "msg-1", &files).expect("snapshot");
            std::hint::black_box(snap);
        });
    });
}

fn bench_restore_snapshot(c: &mut Criterion) {
    let dir = tempdir().expect("tempdir");
    let files: Vec<PathBuf> = (0..10)
        .map(|i| {
            let p = dir.path().join(format!("file{i}.txt"));
            std::fs::write(&p, "initial content\n".repeat(100)).expect("write");
            p
        })
        .collect();
    let snap = take_snapshot("session-bench", "msg-1", &files).expect("snapshot");

    c.bench_function("restore_snapshot_10_files", |b| {
        b.iter(|| {
            restore_snapshot(&snap).expect("restore");
        });
    });
}

fn bench_incremental_save(c: &mut Criterion) {
    let dir = tempdir().expect("tempdir");
    let files: Vec<PathBuf> = (0..10)
        .map(|i| {
            let p = dir.path().join(format!("file{i}.txt"));
            std::fs::write(&p, format!("Hello, world! line {i}\n").repeat(100)).expect("write");
            p
        })
        .collect();
    let base = take_snapshot("session-bench", "msg-1", &files).expect("snapshot");

    // Mutate half the files so there are real diffs to compute
    for (i, p) in files.iter().enumerate().take(5) {
        std::fs::write(p, format!("Changed content {i}\n").repeat(100)).expect("write");
    }

    c.bench_function("incremental_save_10_files_5_changed", |b| {
        b.iter(|| {
            let delta = incremental_save(&base, "msg-2", &files).expect("incremental");
            std::hint::black_box(delta);
        });
    });
}

criterion_group!(
    benches,
    bench_take_snapshot,
    bench_restore_snapshot,
    bench_incremental_save
);
criterion_main!(benches);
