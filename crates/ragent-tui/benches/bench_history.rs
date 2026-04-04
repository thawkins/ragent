//! Criterion benchmarks for history save/load cycle.
//!
//! Covers COMPLIANCE.md Section 5.A:
//! - save_history with varying history sizes
//! - load_history reading back saved histories

use std::sync::Arc;

use criterion::{BenchmarkId, Criterion, criterion_group, criterion_main};
use ragent_core::{
    agent,
    event::EventBus,
    permission::PermissionChecker,
    provider,
    session::{SessionManager, processor::SessionProcessor},
    storage::Storage,
    tool,
};
use ragent_tui::App;

fn make_app() -> App {
    let event_bus = Arc::new(EventBus::default());
    let storage = Arc::new(Storage::open_in_memory().expect("in-memory storage"));
    let provider_registry = Arc::new(provider::create_default_registry());
    let tool_registry = Arc::new(tool::create_default_registry());
    let permission_checker = Arc::new(tokio::sync::RwLock::new(PermissionChecker::new(vec![])));
    let session_manager = Arc::new(SessionManager::new(storage.clone(), event_bus.clone()));
    let session_processor = Arc::new(SessionProcessor {
        session_manager,
        provider_registry: provider_registry.clone(),
        tool_registry,
        permission_checker,
        event_bus: event_bus.clone(),
        task_manager: std::sync::OnceLock::new(),
        lsp_manager: std::sync::OnceLock::new(),
        team_manager: std::sync::OnceLock::new(),
    });
    let agent_info =
        agent::resolve_agent("general", &Default::default()).expect("resolve general agent");
    App::new(
        event_bus,
        storage,
        provider_registry,
        session_processor,
        agent_info,
        false,
    )
}

fn bench_save_history(c: &mut Criterion) {
    let mut group = c.benchmark_group("save_history");

    for &count in &[100, 500, 2_000] {
        group.bench_with_input(BenchmarkId::new("entries", count), &count, |b, &n| {
            let dir = tempfile::tempdir().expect("tmpdir");
            let hist_path = dir.path().join("bench_history.txt");
            let mut app = make_app();
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
            let mut app = make_app();
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
