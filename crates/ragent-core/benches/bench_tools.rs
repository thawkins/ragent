#![allow(missing_docs)]

use criterion::{Criterion, criterion_group, criterion_main};
use ragent_core::event::EventBus;
use ragent_core::tool::{Tool, ToolContext, glob::GlobTool, read::ReadTool};
use serde_json::json;
use std::sync::Arc;
use tempfile::tempdir;
use tokio::runtime::Runtime;

fn make_runtime() -> Runtime {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
}

fn make_ctx(dir: &std::path::Path) -> ToolContext {
    ToolContext {
        working_dir: dir.to_path_buf(),
        session_id: "bench-session".to_string(),
        event_bus: Arc::new(EventBus::new(64)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    }
}

fn bench_glob_tool(c: &mut Criterion) {
    let dir = tempdir().expect("tempdir");
    // Create 50 Rust files across 5 subdirectories
    for sub in 0..5_u32 {
        let subdir = dir.path().join(format!("sub{sub}"));
        std::fs::create_dir_all(&subdir).expect("mkdir");
        for f in 0..10_u32 {
            std::fs::write(subdir.join(format!("file{f}.rs")), b"fn main() {}").expect("write");
        }
    }

    let rt = make_runtime();
    let tool = GlobTool;

    c.bench_function("glob_tool_50_rs_files", |b| {
        let ctx = make_ctx(dir.path());
        b.iter(|| {
            let result = rt.block_on(tool.execute(json!({"pattern": "**/*.rs"}), &ctx));
            std::hint::black_box(result.expect("glob ok"));
        });
    });
}

fn bench_read_tool_cached(c: &mut Criterion) {
    let dir = tempdir().expect("tempdir");
    let file = dir.path().join("sample.txt");
    std::fs::write(&file, "line content\n".repeat(200)).expect("write");

    let rt = make_runtime();
    let tool = ReadTool;

    c.bench_function("read_tool_200_lines_cached", |b| {
        let ctx = make_ctx(dir.path());
        b.iter(|| {
            let result = rt.block_on(tool.execute(json!({"path": "sample.txt"}), &ctx));
            std::hint::black_box(result.expect("read ok"));
        });
    });
}

criterion_group!(benches, bench_glob_tool, bench_read_tool_cached);
criterion_main!(benches);
