//! Tests for the busy-state fallback for graph tools (spec graphCI, T-023, FR-017).
//!
//! FR-017: While a background reindex is in progress and the SQLite store lock
//! is held, the new graph tools (`codeindex_explain`, `codeindex_path`,
//! `codeindex_communities`, `codeindex_godnodes`) shall return a non-blocking
//! `codeindex_busy` response consistent with the existing `codeindex_*` tools'
//! busy behaviour.
//!
//! This test verifies that:
//! 1. The `codeindex_utils` module provides `busy_output` and `with_retry`.
//! 2. All four graph tools use `with_retry` and `busy_output`.
//! 3. The `busy_output` function produces the correct metadata.
//! 4. All four graph tools return `codeindex_busy` when the store lock is held.

use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::{
    CodeIndexConfig, Confidence, EdgeKind, FileEntry, GraphEdge, Symbol, SymbolKind, Visibility,
};
use ragent_tools_extended::Tool;
use ragent_tools_extended::codeindex_communities::CodeIndexCommunitiesTool;
use ragent_tools_extended::codeindex_explain::CodeIndexExplainTool;
use ragent_tools_extended::codeindex_godnodes::CodeIndexGodnodesTool;
use ragent_tools_extended::codeindex_path::CodeIndexPathTool;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".."))
}

fn read_workspace_file(rel: &str) -> String {
    let path = workspace_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {rel}: {e}"))
}

// ── Helper: build a graph index ─────────────────────────────────────────

fn build_graph_index() -> CodeIndex {
    let config = CodeIndexConfig::default();
    let idx = CodeIndex::open_in_memory(&config).unwrap();

    {
        let store = idx.try_lock_store_for_test().expect("store lock");

        let file_entry = FileEntry {
            path: "a.rs".to_string(),
            content_hash: "h1".to_string(),
            byte_size: 100,
            language: Some("rust".to_string()),
            last_indexed: chrono::Utc::now(),
            mtime_ns: 1_000_000_000,
            line_count: 20,
        };
        let file_id = store.upsert_file(&file_entry).unwrap();

        let symbols = vec![
            make_symbol("hub", file_id, 1, 5),
            make_symbol("sat1", file_id, 6, 10),
        ];
        store.upsert_symbols(file_id, &symbols).unwrap();

        let stored = store.get_file_symbols(file_id).unwrap();
        let hub = stored.iter().find(|s| s.name == "hub").unwrap().id;
        let sat1 = stored.iter().find(|s| s.name == "sat1").unwrap().id;

        store
            .upsert_edge_typed(&GraphEdge {
                source_sym: hub,
                target_sym: sat1,
                kind: EdgeKind::Calls,
                confidence: Confidence::Extracted,
                source_file: Some(file_id),
                line: Some(2),
            })
            .unwrap();
    }

    idx
}

fn make_symbol(name: &str, file_id: i64, start: u32, end: u32) -> Symbol {
    Symbol {
        id: 0,
        file_id,
        name: name.to_string(),
        qualified_name: Some(name.to_string()),
        kind: SymbolKind::Function,
        visibility: Visibility::Public,
        start_line: start,
        end_line: end,
        start_col: 0,
        end_col: 0,
        parent_id: None,
        signature: Some(format!("fn {name}()")),
        doc_comment: None,
        body_hash: Some("h".to_string()),
    }
}

fn make_ctx(idx: Arc<CodeIndex>) -> ragent_tools_extended::ToolContext {
    use ragent_tools_extended::ToolContext;
    ToolContext {
        session_id: "test".to_string(),
        working_dir: PathBuf::from("."),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: Some(idx),
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

// ── All four graph tools use with_retry and busy_output ────────────────

#[test]
fn test_all_graph_tools_use_with_retry() {
    let tools = [
        "crates/ragent-tools-extended/src/codeindex_explain.rs",
        "crates/ragent-tools-extended/src/codeindex_path.rs",
        "crates/ragent-tools-extended/src/codeindex_communities.rs",
        "crates/ragent-tools-extended/src/codeindex_godnodes.rs",
    ];

    for path in &tools {
        let source = read_workspace_file(path);
        assert!(
            source.contains("with_retry"),
            "{path} must use with_retry for busy-state fallback (FR-017)"
        );
    }
}

#[test]
fn test_all_graph_tools_use_busy_output() {
    let tools = [
        "crates/ragent-tools-extended/src/codeindex_explain.rs",
        "crates/ragent-tools-extended/src/codeindex_path.rs",
        "crates/ragent-tools-extended/src/codeindex_communities.rs",
        "crates/ragent-tools-extended/src/codeindex_godnodes.rs",
    ];

    for path in &tools {
        let source = read_workspace_file(path);
        assert!(
            source.contains("busy_output"),
            "{path} must use busy_output for busy-state response (FR-017)"
        );
    }
}

// ── All four graph tools return codeindex_busy when index is locked ─────

#[tokio::test]
async fn test_explain_returns_busy_when_locked() {
    let idx = Arc::new(build_graph_index());
    let _guard = idx.try_lock_store_for_test().expect("store lock");
    let idx_for_ctx = Arc::clone(&idx);

    let ctx = make_ctx(idx_for_ctx);
    let tool = CodeIndexExplainTool;

    let output = tool.execute(json!({"symbol": "hub"}), &ctx).await.unwrap();

    assert!(
        output.content.contains("busy"),
        "explain should return busy message"
    );
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["error"], json!("codeindex_busy"));
    assert_eq!(metadata["busy"], json!(true));
}

#[tokio::test]
async fn test_path_returns_busy_when_locked() {
    let idx = Arc::new(build_graph_index());
    let _guard = idx.try_lock_store_for_test().expect("store lock");
    let idx_for_ctx = Arc::clone(&idx);

    let ctx = make_ctx(idx_for_ctx);
    let tool = CodeIndexPathTool;

    let output = tool
        .execute(json!({"from": "hub", "to": "sat1"}), &ctx)
        .await
        .unwrap();

    assert!(
        output.content.contains("busy"),
        "path should return busy message"
    );
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["error"], json!("codeindex_busy"));
}

#[tokio::test]
async fn test_communities_returns_busy_when_locked() {
    let idx = Arc::new(build_graph_index());
    let _guard = idx.try_lock_store_for_test().expect("store lock");
    let idx_for_ctx = Arc::clone(&idx);

    let ctx = make_ctx(idx_for_ctx);
    let tool = CodeIndexCommunitiesTool;

    let output = tool.execute(json!({}), &ctx).await.unwrap();

    assert!(
        output.content.contains("busy"),
        "communities should return busy message"
    );
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["error"], json!("codeindex_busy"));
}

#[tokio::test]
async fn test_godnodes_returns_busy_when_locked() {
    let idx = Arc::new(build_graph_index());
    let _guard = idx.try_lock_store_for_test().expect("store lock");
    let idx_for_ctx = Arc::clone(&idx);

    let ctx = make_ctx(idx_for_ctx);
    let tool = CodeIndexGodnodesTool;

    let output = tool.execute(json!({"n": 5}), &ctx).await.unwrap();

    assert!(
        output.content.contains("busy"),
        "godnodes should return busy message"
    );
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["error"], json!("codeindex_busy"));
}

// ── busy_output metadata includes fallback_tools ────────────────────────

#[test]
fn test_busy_output_includes_fallback_tools() {
    let source = read_workspace_file("crates/ragent-tools-extended/src/codeindex_utils.rs");

    assert!(
        source.contains("fallback_tools"),
        "busy_output must include fallback_tools in metadata"
    );
    assert!(
        source.contains("codeindex_busy"),
        "busy_output must set error to codeindex_busy"
    );
    assert!(
        source.contains("\"busy\": true"),
        "busy_output must set busy: true in metadata"
    );
}

// ── with_retry is non-blocking ──────────────────────────────────────────

#[test]
fn test_with_retry_uses_timeout() {
    let source = read_workspace_file("crates/ragent-tools-extended/src/codeindex_utils.rs");

    assert!(
        source.contains("with_retry_for"),
        "with_retry must delegate to with_retry_for with a timeout"
    );
    assert!(
        source.contains("Duration::from_secs"),
        "with_retry_for must use a Duration timeout"
    );
}

// ── All graph tools have try_* methods on CodeIndex ─────────────────────

#[test]
fn test_codeindex_has_try_methods_for_all_graph_tools() {
    let source = read_workspace_file("crates/ragent-codeindex/src/lib.rs");

    for method in &["try_explain", "try_path", "try_communities", "try_godnodes"] {
        assert!(
            source.contains(&format!("pub fn {method}")),
            "CodeIndex must expose {method}() for non-blocking graph queries (FR-017)"
        );
    }
}
