//! Tests for the `codeindex_path` tool (spec graphCI, T-013).
//!
//! Covers FR-012 (shortest path between two symbols) and FR-017 (non-blocking
//! busy response when the index is locked).

use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::{
    CodeIndexConfig, Confidence, EdgeKind, FileEntry, GraphEdge, Symbol, SymbolKind, Visibility,
};
use ragent_tools_extended::Tool;
use ragent_tools_extended::codeindex_path::CodeIndexPathTool;
use serde_json::json;

// ── Tool metadata ─────��─────────────────────────────────────────────────

#[test]
fn test_tool_name() {
    let tool = CodeIndexPathTool;
    assert_eq!(tool.name(), "codeindex_path");
}

#[test]
fn test_tool_permission_category_is_codeindex_read() {
    let tool = CodeIndexPathTool;
    assert_eq!(tool.permission_category(), "codeindex:read");
}

#[test]
fn test_tool_parameters_schema_has_required_from_and_to() {
    let tool = CodeIndexPathTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["from"].is_object());
    assert!(schema["properties"]["to"].is_object());
    let required = schema["required"].as_array().expect("required is an array");
    assert!(required.contains(&json!("from")));
    assert!(required.contains(&json!("to")));
}

// ── Registry registration ────────────────────────────────────────────────

#[test]
fn test_path_tool_registered_in_extended_registry() {
    let registry = ragent_tools_extended::create_extended_registry();
    assert!(
        registry.contains("codeindex_path"),
        "codeindex_path should be registered in the extended registry",
    );
}

#[test]
fn test_path_tool_registered_with_correct_permission_category() {
    let registry = ragent_tools_extended::create_extended_registry();
    let tool = registry
        .get("codeindex_path")
        .expect("codeindex_path should be registered");
    assert_eq!(tool.permission_category(), "codeindex:read");
}

// ── CodeIndex::path / try_path ────────────────────────────────────────────

/// Build an in-memory CodeIndex with a small graph:
///   hub --calls--> mid --calls--> leaf
/// This gives a 2-hop path from hub to leaf.
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
            line_count: 30,
        };
        let file_id = store.upsert_file(&file_entry).unwrap();

        let symbols = vec![
            make_symbol("hub", file_id, 1, 5),
            make_symbol("mid", file_id, 6, 10),
            make_symbol("leaf", file_id, 11, 15),
            make_symbol("isolated", file_id, 16, 20),
        ];
        store.upsert_symbols(file_id, &symbols).unwrap();

        let stored = store.get_file_symbols(file_id).unwrap();
        let hub = stored.iter().find(|s| s.name == "hub").unwrap().id;
        let mid = stored.iter().find(|s| s.name == "mid").unwrap().id;
        let leaf = stored.iter().find(|s| s.name == "leaf").unwrap().id;

        store
            .upsert_edge_typed(&GraphEdge {
                source_sym: hub,
                target_sym: mid,
                kind: EdgeKind::Calls,
                confidence: Confidence::Extracted,
                source_file: Some(file_id),
                line: Some(2),
            })
            .unwrap();
        store
            .upsert_edge_typed(&GraphEdge {
                source_sym: mid,
                target_sym: leaf,
                kind: EdgeKind::Calls,
                confidence: Confidence::Extracted,
                source_file: Some(file_id),
                line: Some(7),
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

#[test]
fn test_codeindex_path_finds_two_hop_path() {
    let idx = build_graph_index();
    let result = idx.path("hub", "leaf").unwrap();
    assert!(result.is_some(), "path from hub to leaf should exist");
    let result = result.unwrap();
    assert_eq!(result.hops, 2);
    // First step is the source (hub), with no edge kind.
    assert_eq!(result.steps[0].0, "hub");
    assert!(result.steps[0].1.is_none());
    // Subsequent steps have edge kinds.
    assert_eq!(result.steps[1].0, "mid");
    assert_eq!(result.steps[1].1.as_deref(), Some("calls"));
    assert_eq!(result.steps[2].0, "leaf");
    assert_eq!(result.steps[2].1.as_deref(), Some("calls"));
}

#[test]
fn test_codeindex_path_direct_edge_is_one_hop() {
    let idx = build_graph_index();
    let result = idx.path("hub", "mid").unwrap();
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.hops, 1);
    assert_eq!(result.steps[0].0, "hub");
    assert_eq!(result.steps[1].0, "mid");
}

#[test]
fn test_codeindex_path_no_path_returns_none() {
    let idx = build_graph_index();
    // "isolated" has no edges — no path to/from it.
    let result = idx.path("isolated", "hub").unwrap();
    assert!(result.is_none(), "no path from isolated to hub");
}

#[test]
fn test_codeindex_path_nonexistent_symbol_returns_none() {
    let idx = build_graph_index();
    let result = idx.path("hub", "nonexistent").unwrap();
    assert!(result.is_none(), "no path to nonexistent symbol");
}

#[test]
fn test_codeindex_path_same_symbol_is_zero_hops() {
    let idx = build_graph_index();
    let result = idx.path("hub", "hub").unwrap();
    assert!(result.is_some());
    let result = result.unwrap();
    assert_eq!(result.hops, 0);
    assert_eq!(result.steps.len(), 1);
    assert_eq!(result.steps[0].0, "hub");
}

#[test]
fn test_codeindex_path_empty_graph_returns_none() {
    let config = CodeIndexConfig::default();
    let idx = CodeIndex::open_in_memory(&config).unwrap();
    let result = idx.path("a", "b").unwrap();
    assert!(result.is_none(), "no path in empty graph");
}

#[test]
fn test_codeindex_try_path_succeeds_when_unlocked() {
    let idx = build_graph_index();
    let result = idx.try_path("hub", "leaf").unwrap();
    assert!(result.is_some(), "try_path should acquire lock and run");
    let inner = result.unwrap();
    assert!(inner.is_some(), "path should exist");
    let inner = inner.unwrap();
    assert_eq!(inner.hops, 2);
}

#[test]
fn test_codeindex_try_path_returns_none_when_locked() {
    let idx = build_graph_index();
    let _guard = idx.try_lock_store_for_test().expect("store lock");
    let result = idx.try_path("hub", "leaf").unwrap();
    assert!(
        result.is_none(),
        "try_path should return None when the store is locked"
    );
}

// ── Tool execute ─────────────────────────────────────────────────────────

fn make_ctx(idx: Option<CodeIndex>) -> ragent_tools_extended::ToolContext {
    use ragent_tools_extended::ToolContext;
    use std::path::PathBuf;
    use std::sync::Arc;

    ToolContext {
        session_id: "test".to_string(),
        working_dir: PathBuf::from("."),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: idx.map(std::sync::Arc::new),
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

#[tokio::test]
async fn test_path_tool_returns_path_result() {
    let idx = build_graph_index();
    let ctx = make_ctx(Some(idx));
    let tool = CodeIndexPathTool;

    let output = tool
        .execute(json!({"from": "hub", "to": "leaf"}), &ctx)
        .await
        .unwrap();

    assert!(output.content.contains("2 hops"));
    assert!(output.content.contains("hub"));
    assert!(output.content.contains("leaf"));
    assert!(output.content.contains("mid"));

    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["hops"], json!(2));
    assert_eq!(metadata["from"], json!("hub"));
    assert_eq!(metadata["to"], json!("leaf"));
}

#[tokio::test]
async fn test_path_tool_returns_no_path_message() {
    let idx = build_graph_index();
    let ctx = make_ctx(Some(idx));
    let tool = CodeIndexPathTool;

    let output = tool
        .execute(json!({"from": "isolated", "to": "hub"}), &ctx)
        .await
        .unwrap();

    assert!(output.content.contains("No path found"));
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["error"], json!("codeindex_no_path"));
}

#[tokio::test]
async fn test_path_tool_returns_disabled_when_no_code_index() {
    let ctx = make_ctx(None);
    let tool = CodeIndexPathTool;

    let output = tool
        .execute(json!({"from": "a", "to": "b"}), &ctx)
        .await
        .unwrap();

    assert!(output.content.contains("not available"));
    assert_eq!(
        output.metadata.unwrap()["error"],
        json!("codeindex_disabled")
    );
}

#[tokio::test]
async fn test_path_tool_missing_from_parameter() {
    let idx = build_graph_index();
    let ctx = make_ctx(Some(idx));
    let tool = CodeIndexPathTool;

    let output = tool.execute(json!({"to": "leaf"}), &ctx).await.unwrap();

    assert!(output.content.contains("`from`"));
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["error"], json!("codeindex_missing_parameter"));
    assert_eq!(metadata["parameter"], json!("from"));
}

#[tokio::test]
async fn test_path_tool_missing_to_parameter() {
    let idx = build_graph_index();
    let ctx = make_ctx(Some(idx));
    let tool = CodeIndexPathTool;

    let output = tool.execute(json!({"from": "hub"}), &ctx).await.unwrap();

    assert!(output.content.contains("`to`"));
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["error"], json!("codeindex_missing_parameter"));
    assert_eq!(metadata["parameter"], json!("to"));
}

#[tokio::test]
async fn test_path_tool_empty_from_parameter() {
    let idx = build_graph_index();
    let ctx = make_ctx(Some(idx));
    let tool = CodeIndexPathTool;

    let output = tool
        .execute(json!({"from": "", "to": "leaf"}), &ctx)
        .await
        .unwrap();

    assert!(output.content.contains("`from`"));
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["error"], json!("codeindex_missing_parameter"));
}

#[tokio::test]
async fn test_path_tool_empty_to_parameter() {
    let idx = build_graph_index();
    let ctx = make_ctx(Some(idx));
    let tool = CodeIndexPathTool;

    let output = tool
        .execute(json!({"from": "hub", "to": ""}), &ctx)
        .await
        .unwrap();

    assert!(output.content.contains("`to`"));
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["error"], json!("codeindex_missing_parameter"));
}
