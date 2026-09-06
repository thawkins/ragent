//! Tests for the non-blocking `codeindex_status` tool behaviour (FR-017).
//!
//! The status tool must respond immediately when the store/FTS lock is held by
//! a background reindex or graph build: a single `try_status()` probe, and on
//! `None` an immediate busy report built from the lock-free progress atomics —
//! never a retry-and-wait loop.

// The tests intentionally hold the store MutexGuard across await points: that
// is the busy condition under test (mirrors test_codeindex_busy_state.rs).
#![allow(clippy::await_holding_lock)]

use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::CodeIndexConfig;
use ragent_tools_extended::Tool;
use ragent_tools_extended::codeindex_status::CodeIndexStatusTool;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

fn make_ctx(idx: Arc<CodeIndex>) -> ragent_tools_extended::ToolContext {
    ragent_tools_extended::ToolContext {
        session_id: "test".to_string(),
        working_dir: PathBuf::from("."),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: Some(idx),
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

fn open_index() -> CodeIndex {
    let dir = tempfile::TempDir::new().unwrap();
    let config = CodeIndexConfig {
        enabled: true,
        project_root: dir.path().to_path_buf(),
        index_dir: dir.path().join(".ragent/codeindex"),
        scan_config: ragent_codeindex::types::ScanConfig::default(),
    };
    CodeIndex::open(&config).unwrap()
}

#[tokio::test]
async fn test_status_tool_reports_busy_immediately_when_store_locked() {
    let idx = Arc::new(open_index());
    let _guard = idx.try_lock_store_for_test().expect("store lock");

    let ctx = make_ctx(Arc::clone(&idx));
    let tool = CodeIndexStatusTool;

    let start = Instant::now();
    let output = tool.execute(json!({}), &ctx).await.unwrap();
    let elapsed = start.elapsed();

    // Immediate: far below the old 5s with_retry window.
    assert!(
        elapsed < std::time::Duration::from_millis(500),
        "status must respond immediately, took {elapsed:?}"
    );
    assert!(
        output.content.contains("busy"),
        "status should report the busy state: {}",
        output.content
    );
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["busy"], json!(true));
    assert_eq!(metadata["error"], json!("codeindex_busy"));
}

#[tokio::test]
async fn test_status_tool_busy_output_reports_graph_build_progress() {
    let idx = Arc::new(open_index());
    // Simulate a graph build in progress via the lock-free atomics.
    idx.set_graph_busy_for_test(true);
    idx.set_graph_progress_for_test(3, 10);
    let _guard = idx.try_lock_store_for_test().expect("store lock");

    let ctx = make_ctx(Arc::clone(&idx));
    let output = CodeIndexStatusTool.execute(json!({}), &ctx).await.unwrap();

    assert!(output.content.contains("busy"));
    assert!(
        output.content.contains("3/10"),
        "should show graph progress"
    );
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["graph_building"], json!(true));
    assert_eq!(metadata["graph_done"], json!(3));
    assert_eq!(metadata["graph_total"], json!(10));
}

#[tokio::test]
async fn test_status_tool_reports_normal_stats_when_lock_free() {
    let idx = Arc::new(open_index());
    let ctx = make_ctx(Arc::clone(&idx));

    let output = CodeIndexStatusTool.execute(json!({}), &ctx).await.unwrap();

    assert!(
        !output.content.contains("busy"),
        "idle index must not report busy: {}",
        output.content
    );
    assert!(output.content.contains("Files indexed"));
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["busy"], serde_json::Value::Null);
}

#[test]
fn test_status_tool_uses_single_try_probe() {
    // Source-level guard: the status tool must not use with_retry (the
    // blocking retry loop) — it must answer immediately.
    let source = include_str!("../src/codeindex_status.rs");
    assert!(
        !source.contains("with_retry"),
        "codeindex_status must not retry-wait; it must respond immediately"
    );
    assert!(
        source.contains("status_busy_output"),
        "busy path must report atomic progress instead of stalling"
    );
}
