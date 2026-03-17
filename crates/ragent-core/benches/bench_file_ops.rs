use criterion::{criterion_group, criterion_main, Criterion};
use tempfile::tempdir;
use std::path::PathBuf;

fn prepare_files(count: usize) -> (tempfile::TempDir, Vec<PathBuf>) {
    let dir = tempdir().expect("tempdir");
    let mut paths = Vec::with_capacity(count);
    for i in 0..count {
        let p = dir.path().join(format!("file_{:03}.txt", i));
        std::fs::write(&p, format!("original {}", i)).expect("write file");
        paths.push(p);
    }
    (dir, paths)
}

fn bench_commit(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_ops_commit");
    let counts = [50usize, 200usize];

    for &count in &counts {
        group.bench_function(format!("commit_concurrency_1_{}", count), |b| {
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async move {
                let (_dir, paths) = prepare_files(count);
                let mut staging = ragent_core::file_ops::EditStaging::new(false);
                for p in &paths {
                    let new = format!("modified {}", p.file_name().unwrap().to_string_lossy());
                    staging.stage_edit(p, new).await.expect("stage");
                }
                let _ = staging.commit_all(1).await.expect("commit");
            })
        });

        group.bench_function(format!("commit_concurrency_par_{}", count), |b| {
            b.to_async(tokio::runtime::Runtime::new().unwrap()).iter(|| async move {
                let (_dir, paths) = prepare_files(count);
                let mut staging = ragent_core::file_ops::EditStaging::new(false);
                for p in &paths {
                    let new = format!("modified {}", p.file_name().unwrap().to_string_lossy());
                    staging.stage_edit(p, new).await.expect("stage");
                }
                let _ = staging.commit_all(num_cpus::get()).await.expect("commit");
            })
        });
    }

    group.finish();
}

criterion_group!(benches, bench_commit);
criterion_main!(benches);
