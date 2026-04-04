//! Criterion benchmarks for markdown rendering and ASCII table normalization.
//!
//! Covers COMPLIANCE.md Section 5.B and 5.D:
//! - render_markdown_to_ascii with varying input sizes (1 KB, 10 KB, 100 KB)
//! - normalize_ascii_tables with varying table sizes

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
        mcp_client: std::sync::OnceLock::new(),
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

/// Generate a markdown document of approximately `target_bytes` in size.
/// Includes headings, bold, lists, code blocks, and tables.
fn generate_markdown(target_bytes: usize) -> String {
    let prefix = "From: /bench\n\n";
    let block = concat!(
        "## Section Heading\n\n",
        "This is a paragraph with **bold text** and *italic text* that demonstrates ",
        "typical markdown content one might find in a chat response.\n\n",
        "- List item one with some detail\n",
        "- List item two with **emphasis**\n",
        "- List item three\n\n",
        "```rust\nfn example() {\n    println!(\"hello\");\n}\n```\n\n",
        "| Column A | Column B | Column C |\n",
        "|----------|----------|----------|\n",
        "| value 1  | value 2  | value 3  |\n",
        "| alpha    | beta     | gamma    |\n\n",
    );
    let block_len = block.len();
    let repeats = (target_bytes.saturating_sub(prefix.len()) / block_len).max(1);
    let mut doc = String::with_capacity(target_bytes + block_len);
    doc.push_str(prefix);
    for _ in 0..repeats {
        doc.push_str(block);
    }
    doc
}

/// Generate a table string (pipe-separated with │ borders) of `rows` lines.
fn generate_table(rows: usize) -> String {
    let mut buf = String::with_capacity(rows * 60);
    buf.push_str("│ Name         │ Value │ Description        │\n");
    buf.push_str("──────────────────────────────────────────────\n");
    for i in 0..rows {
        buf.push_str(&format!(
            "│ item_{i:04}    │ {val:5} │ row number {i:4}     │\n",
            val = i * 7
        ));
    }
    buf
}

fn bench_render_markdown(c: &mut Criterion) {
    let mut app = make_app();

    let mut group = c.benchmark_group("render_markdown_to_ascii");
    for &size in &[1_000, 10_000, 100_000] {
        let md = generate_markdown(size);
        let label = format!("{:.0}KB", size as f64 / 1000.0);

        // Uncached: clear cache each iteration to measure full pipeline cost.
        group.bench_with_input(BenchmarkId::new("uncached", &label), &md, |b, input| {
            b.iter(|| {
                app.md_render_cache.clear();
                app.render_markdown_to_ascii(input)
            });
        });

        // Cached: pre-populate cache, then measure cache-hit cost.
        app.md_render_cache.clear();
        app.render_markdown_to_ascii(&md);
        group.bench_with_input(BenchmarkId::new("cached", &label), &md, |b, input| {
            b.iter(|| app.render_markdown_to_ascii(input));
        });
    }
    group.finish();
}

fn bench_normalize_ascii_tables(c: &mut Criterion) {
    let mut app = make_app();

    let mut group = c.benchmark_group("normalize_ascii_tables");
    for &rows in &[10, 100, 1_000] {
        let table = generate_table(rows);
        group.bench_with_input(BenchmarkId::new("rows", rows), &table, |b, input| {
            b.iter(|| app.normalize_ascii_tables(input));
        });
    }
    group.finish();
}

criterion_group!(benches, bench_render_markdown, bench_normalize_ascii_tables);
criterion_main!(benches);
