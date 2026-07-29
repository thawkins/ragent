//! Tests for the `agentgrep` structure-aware search tool.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use ragent_tools_extended::agentgrep::AgentGrepTool;
use ragent_tools_extended::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

fn test_ctx(
    working_dir: PathBuf,
    code_index: Option<Arc<ragent_codeindex::CodeIndex>>,
) -> ToolContext {
    ToolContext {
        storage: None,
        event_bus: Arc::new(EventBus::new(128)),
        session_id: "test-session".to_string(),
        working_dir,
        code_index,
        config: None,
        read_timestamps: Arc::new(RwLock::new(HashMap::new())),
    }
}

/// Create a tiny in-memory code index over a temporary directory with one Rust file.
fn make_index(root: &std::path::Path) -> anyhow::Result<Arc<ragent_codeindex::CodeIndex>> {
    let config = ragent_codeindex::types::CodeIndexConfig {
        enabled: true,
        project_root: root.to_path_buf(),
        index_dir: root.join(".ragent").join("codeindex"),
        scan_config: ragent_codeindex::types::ScanConfig::default(),
    };
    let idx = ragent_codeindex::CodeIndex::open(&config)?;
    let result = idx.full_reindex().unwrap();
    assert_eq!(result.files_added, 1);
    Ok(Arc::new(idx))
}

#[tokio::test]
async fn test_agentgrep_grep_returns_regions() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let src = root.join("lib.rs");
    std::fs::write(
        &src,
        "pub fn alpha() -> i32 { 1 }\npub fn beta() -> i32 { 2 }\n",
    )
    .unwrap();

    let idx = make_index(root).unwrap();
    let ctx = test_ctx(root.to_path_buf(), Some(idx));
    let tool = AgentGrepTool;

    let out = tool
        .execute(
            json!({"mode": "grep", "query": "fn alpha", "path": root.to_str()}),
            &ctx,
        )
        .await
        .expect("execute");

    assert!(out.content.contains("alpha"), "content: {}", out.content);
    assert!(
        out.content.contains("function") || out.content.contains("[fn]"),
        "kind marker missing: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata");
    assert!(meta["total_results"].as_u64().unwrap_or(0) >= 1);
}

#[tokio::test]
async fn test_agentgrep_outline_returns_symbols() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let src = root.join("lib.rs");
    std::fs::write(&src, "pub fn one() {}\npub fn two() {}\nstruct S;\n").unwrap();

    let idx = make_index(root).unwrap();
    let ctx = test_ctx(root.to_path_buf(), Some(idx));
    let tool = AgentGrepTool;

    let out = tool
        .execute(
            json!({"mode": "outline", "query": "", "path": root.to_str()}),
            &ctx,
        )
        .await
        .expect("execute");

    assert!(out.content.contains("one"), "content: {}", out.content);
    assert!(out.content.contains("two"), "content: {}", out.content);
    assert!(out.content.contains("S"), "content: {}", out.content);
}

#[tokio::test]
async fn test_agentgrep_find_returns_files() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let src = root.join("widget.rs");
    std::fs::write(&src, "pub struct Widget;\n").unwrap();

    let idx = make_index(root).unwrap();
    let ctx = test_ctx(root.to_path_buf(), Some(idx));
    let tool = AgentGrepTool;

    let out = tool
        .execute(
            json!({"mode": "find", "query": "widget.rs", "path": root.to_str()}),
            &ctx,
        )
        .await
        .expect("execute");

    assert!(
        out.content.contains("widget.rs"),
        "content: {}",
        out.content
    );
}

#[tokio::test]
async fn test_agentgrep_smart_falls_back_to_grep() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let src = root.join("lib.rs");
    std::fs::write(&src, "// custom helper\npub fn custom_helper() {}\n").unwrap();

    let idx = make_index(root).unwrap();
    let ctx = test_ctx(root.to_path_buf(), Some(idx));
    let tool = AgentGrepTool;

    let out = tool
        .execute(
            json!({"mode": "smart", "query": "custom helper", "path": root.to_str()}),
            &ctx,
        )
        .await
        .expect("execute");

    assert!(
        out.content.contains("custom_helper") || out.content.contains("custom helper"),
        "content: {}",
        out.content
    );
}

#[tokio::test]
async fn test_agentgrep_skips_already_read_files() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let src = root.join("lib.rs");
    std::fs::write(&src, "pub fn already_read() {}\n").unwrap();

    let idx = make_index(root).unwrap();
    let ctx = test_ctx(root.to_path_buf(), Some(idx));
    ctx.read_timestamps.write().unwrap().insert(src.clone(), 0);

    let tool = AgentGrepTool;
    let out = tool
        .execute(
            json!({"mode": "grep", "query": "already_read", "path": root.to_str()}),
            &ctx,
        )
        .await
        .expect("execute");

    assert!(
        !out.content.contains("already_read"),
        "already-read region should be omitted: {}",
        out.content
    );
}
