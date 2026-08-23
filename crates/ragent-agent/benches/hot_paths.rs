//! Criterion benchmarks for hot-path functions optimised in the agentopt
//! spec (FR-002 — regression/performance benchmarks).
//!
//! Run with:
//! ```text
//! cargo bench -p ragent-agent
//! ```

use std::path::Path;

use criterion::{BenchmarkId, Criterion, black_box, criterion_group, criterion_main};

use ragent_agent::goal::build_evaluation_context;
use ragent_agent::skill::args::substitute_args;
use ragent_agent::task::TaskEntry;
use ragent_types::message::{Message, MessagePart, Role};

// ── TaskEntry clone (T-012, FR-006/FR-016) ────────────────────────────────

fn bench_task_entry_clone(c: &mut Criterion) {
    let mut group = c.benchmark_group("task_entry_clone");

    for size in [100, 1_000, 10_000, 100_000] {
        let result = "x".repeat(size);
        let entry = TaskEntry {
            id: "explore-abc123".to_string(),
            parent_session_id: "parent-sess".to_string(),
            child_session_id: "child-sess".to_string(),
            agent_name: "explore".to_string(),
            task_prompt: "Summarise the architecture".to_string(),
            background: true,
            status: ragent_agent::task::TaskStatus::Completed,
            result: Some(std::sync::Arc::from(result.as_str())),
            error: None,
            created_at: chrono::Utc::now(),
            completed_at: Some(chrono::Utc::now()),
            reported: false,
            waiter_count: 0,
            output_file: None,
            report_status: ragent_agent::task::ReportStatus::default(),
        };

        group.bench_with_input(BenchmarkId::from_parameter(size), &entry, |b, entry| {
            b.iter(|| {
                let cloned = black_box(entry).clone();
                black_box(cloned);
            });
        });
    }

    group.finish();
}

// ── Skill argument substitution (T-017, FR-011) ──────────────────────────

fn bench_substitute_args(c: &mut Criterion) {
    let body = "Deploy $ARGUMENTS to $0 environment. Session: ${RAGENT_SESSION_ID}. \
                Skill dir: ${RAGENT_SKILL_DIR}. Args: $1 $2 $3 $4 $5."
        .repeat(20);

    c.bench_function("substitute_args", |b| {
        b.iter(|| {
            let result = substitute_args(
                black_box(&body),
                "staging prod west region us-east-1",
                "sess-bench-123",
                Path::new("/skills/deploy"),
            );
            black_box(result);
        });
    });
}

// ── Goal evaluation context builder (T-018, FR-012) ──────────────────────

fn bench_build_evaluation_context(c: &mut Criterion) {
    let mut messages = Vec::new();
    for i in 0..200 {
        let role = if i % 2 == 0 {
            Role::User
        } else {
            Role::Assistant
        };
        messages.push(Message::new(
            "sess-bench",
            role,
            vec![MessagePart::Text {
                text: format!("Message {i}: doing some work on the codebase."),
            }],
        ));
    }

    c.bench_function("build_evaluation_context", |b| {
        b.iter(|| {
            let ctx = build_evaluation_context(black_box(&messages), 10_000);
            black_box(ctx);
        });
    });
}

// ── CanonicalPathCache (T-014, FR-017) ───────────────────────────────────

fn bench_canonical_path_cache(c: &mut Criterion) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().to_path_buf();

    // Create a few files to canonicalize.
    for name in &["a.rs", "b.rs", "c.rs", "src/mod.rs"] {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(&path, "// content").unwrap();
    }

    let file_path = root.join("src/mod.rs");

    // Benchmark uncached (fresh cache each iteration).
    c.bench_function("check_path_within_root_uncached", |b| {
        b.iter(|| {
            let cache = ragent_tools_core::CanonicalPathCache::new();
            ragent_tools_core::check_path_within_root_cached(
                black_box(&file_path),
                black_box(&root),
                black_box(&cache),
            )
            .unwrap();
        });
    });

    // Benchmark cached (pre-populated cache — simulates second call in same step).
    let warm_cache = ragent_tools_core::CanonicalPathCache::new();
    ragent_tools_core::check_path_within_root_cached(&file_path, &root, &warm_cache).unwrap();

    c.bench_function("check_path_within_root_cached", |b| {
        b.iter(|| {
            ragent_tools_core::check_path_within_root_cached(
                black_box(&file_path),
                black_box(&root),
                black_box(&warm_cache),
            )
            .unwrap();
        });
    });
}

criterion_group!(
    benches,
    bench_task_entry_clone,
    bench_substitute_args,
    bench_build_evaluation_context,
    bench_canonical_path_cache,
);
criterion_main!(benches);
