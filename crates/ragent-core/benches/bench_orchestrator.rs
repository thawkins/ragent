#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use ragent_core::orchestrator::{AgentRegistry, Coordinator, JobDescriptor};
use tokio::runtime::Runtime;

fn make_runtime() -> Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

fn bench_coordinator_new(c: &mut Criterion) {
    c.bench_function("coordinator_new", |b| {
        b.iter(|| {
            let registry = AgentRegistry::new();
            let coord = Coordinator::new(registry);
            std::hint::black_box(coord);
        });
    });
}

fn bench_start_job_no_agents(c: &mut Criterion) {
    let rt = make_runtime();
    c.bench_function("start_job_sync_no_agents", |b| {
        b.iter(|| {
            let registry = AgentRegistry::new();
            let coord = Coordinator::new(registry);
            let desc = JobDescriptor {
                id: "bench-job".to_string(),
                payload: "hello".to_string(),
                required_capabilities: vec![],
            };
            // Expected to return an error (no agents), but we measure the overhead
            let _result = rt.block_on(coord.start_job_sync(desc));
        });
    });
}

fn bench_metrics_snapshot(c: &mut Criterion) {
    let registry = AgentRegistry::new();
    let coord = Coordinator::new(registry);
    c.bench_function("coordinator_metrics_snapshot", |b| {
        b.iter(|| {
            let snap = coord.metrics_snapshot();
            std::hint::black_box(snap);
        });
    });
}

criterion_group!(
    benches,
    bench_coordinator_new,
    bench_start_job_no_agents,
    bench_metrics_snapshot
);
criterion_main!(benches);
