#![allow(clippy::assert_is_empty)]
//! Tests for the `codeindex_explain` tool (spec graphCI, T-012).
//!
//! Covers FR-011 (explain a symbol's metadata and connections) and FR-017
//! (non-blocking busy response when the index is locked).

use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::{
    CodeIndexConfig, Confidence, EdgeKind, FileEntry, GraphEdge, Symbol, SymbolKind, Visibility,
};
use ragent_tools_extended::Tool;
use ragent_tools_extended::codeindex_explain::CodeIndexExplainTool;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

// ── Tool metadata ───────────────────────────────────────────────────────

#[test]
fn test_tool_name() {
    let tool = CodeIndexExplainTool;
    assert_eq!(tool.name(), "codeindex_explain");
}

#[test]
fn test_tool_permission_category_is_codeindex_read() {
    let tool = CodeIndexExplainTool;
    assert_eq!(tool.permission_category(), "codeindex:read");
}

#[test]
fn test_tool_parameters_schema_has_required_symbol() {
    let tool = CodeIndexExplainTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["symbol"].is_object());
    let required = schema["required"].as_array().expect("required is an array");
    assert!(required.contains(&json!("symbol")));
}

// ── Registry registration ───────────────────────────────────────────────

#[test]
fn test_explain_tool_registered_in_extended_registry() {
    let registry = ragent_tools_extended::create_extended_registry();
    assert!(
        registry.contains("codeindex_explain"),
        "codeindex_explain should be registered in the extended registry",
    );
}

#[test]
fn test_explain_tool_registered_with_correct_permission_category() {
    let registry = ragent_tools_extended::create_extended_registry();
    let tool = registry
        .get("codeindex_explain")
        .expect("codeindex_explain should be registered");
    assert_eq!(tool.permission_category(), "codeindex:read");
}

// ── CodeIndex::explain / try_explain ────────────────────────────────────

/// Build an in-memory CodeIndex with a small graph:
///   hub --calls--> sat1
///   hub --calls--> sat2
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
            make_symbol("sat2", file_id, 11, 15),
        ];
        store.upsert_symbols(file_id, &symbols).unwrap();

        let stored = store.get_file_symbols(file_id).unwrap();
        let hub = stored.iter().find(|s| s.name == "hub").unwrap().id;
        let sat1 = stored.iter().find(|s| s.name == "sat1").unwrap().id;
        let sat2 = stored.iter().find(|s| s.name == "sat2").unwrap().id;

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
        store
            .upsert_edge_typed(&GraphEdge {
                source_sym: hub,
                target_sym: sat2,
                kind: EdgeKind::Calls,
                confidence: Confidence::Extracted,
                source_file: Some(file_id),
                line: Some(3),
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
fn test_codeindex_explain_returns_result_for_known_symbol() {
    let idx = build_graph_index();
    let result = idx.explain("hub").unwrap();
    assert!(result.is_some(), "explain should find hub");
    let result = result.unwrap();
    assert_eq!(result.name, "hub");
    assert_eq!(result.source_file, "a.rs");
    assert_eq!(result.line, 1);
    // hub has 2 outgoing edges, 0 incoming.
    assert_eq!(result.outgoing.len(), 2);
    assert_eq!(result.incoming.len(), 0);
    assert_eq!(result.degree, 2);
}

#[test]
fn test_codeindex_explain_returns_none_for_unknown_symbol() {
    let idx = build_graph_index();
    let result = idx.explain("nonexistent").unwrap();
    assert!(
        result.is_none(),
        "explain should return None for unknown symbol"
    );
}

#[test]
fn test_codeindex_explain_incoming_edges() {
    let idx = build_graph_index();
    let result = idx.explain("sat1").unwrap().unwrap();
    // sat1 has 1 incoming edge from hub, 0 outgoing.
    assert_eq!(result.incoming.len(), 1);
    assert_eq!(result.incoming[0].symbol, "hub");
    assert_eq!(result.incoming[0].kind, "calls");
    assert_eq!(result.outgoing.len(), 0);
}

#[test]
fn test_codeindex_explain_empty_graph_returns_none() {
    let config = CodeIndexConfig::default();
    let idx = CodeIndex::open_in_memory(&config).unwrap();
    let result = idx.explain("anything").unwrap();
    assert!(result.is_none(), "explain returns None when graph is empty");
}

#[test]
fn test_codeindex_try_explain_succeeds_when_unlocked() {
    let idx = build_graph_index();
    let result = idx.try_explain("hub").unwrap();
    assert!(result.is_some(), "try_explain should acquire lock and run");
    let inner = result.unwrap();
    assert!(inner.is_some(), "symbol should be found");
    let inner = inner.unwrap();
    assert_eq!(inner.name, "hub");
}

#[test]
fn test_codeindex_try_explain_returns_none_when_locked() {
    let idx = build_graph_index();
    let _guard = idx.try_lock_store_for_test().expect("store lock");
    let result = idx.try_explain("hub").unwrap();
    assert!(
        result.is_none(),
        "try_explain should return None when the store is locked"
    );
}

// ── Tool execute ───────────────────────────────────────────────────────

fn make_ctx(idx: Option<CodeIndex>) -> ragent_tools_extended::ToolContext {
    use ragent_tools_extended::ToolContext;
    ToolContext {
        session_id: "test".to_string(),
        working_dir: PathBuf::from("."),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: idx.map(Arc::new),
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

#[tokio::test]
async fn test_explain_tool_returns_result() {
    let idx = build_graph_index();
    let ctx = make_ctx(Some(idx));
    let tool = CodeIndexExplainTool;

    let output = tool.execute(json!({"symbol": "hub"}), &ctx).await.unwrap();

    assert!(output.content.contains("hub"));
    assert!(output.content.contains("a.rs"));
    assert!(output.content.contains("Incoming"));
    assert!(output.content.contains("Outgoing"));

    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["name"], json!("hub"));
    assert_eq!(metadata["source_file"], json!("a.rs"));
    assert_eq!(metadata["degree"], json!(2));
}

#[tokio::test]
async fn test_explain_tool_returns_not_found() {
    let idx = build_graph_index();
    let ctx = make_ctx(Some(idx));
    let tool = CodeIndexExplainTool;

    let output = tool
        .execute(json!({"symbol": "nonexistent"}), &ctx)
        .await
        .unwrap();

    assert!(output.content.contains("not found"));
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["error"], json!("codeindex_symbol_not_found"));
}

#[tokio::test]
async fn test_explain_tool_returns_disabled_when_no_code_index() {
    let ctx = make_ctx(None);
    let tool = CodeIndexExplainTool;

    let output = tool.execute(json!({"symbol": "hub"}), &ctx).await.unwrap();

    assert!(output.content.contains("not available"));
    assert_eq!(
        output.metadata.unwrap()["error"],
        json!("codeindex_disabled")
    );
}

#[tokio::test]
async fn test_explain_tool_missing_symbol_parameter() {
    let idx = build_graph_index();
    let ctx = make_ctx(Some(idx));
    let tool = CodeIndexExplainTool;

    let output = tool.execute(json!({}), &ctx).await.unwrap();

    assert!(output.content.contains("`symbol`"));
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["error"], json!("codeindex_missing_parameter"));
    assert_eq!(metadata["parameter"], json!("symbol"));
}

#[tokio::test]
async fn test_explain_tool_empty_symbol_parameter() {
    let idx = build_graph_index();
    let ctx = make_ctx(Some(idx));
    let tool = CodeIndexExplainTool;

    let output = tool.execute(json!({"symbol": ""}), &ctx).await.unwrap();

    assert!(output.content.contains("`symbol`"));
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["error"], json!("codeindex_missing_parameter"));
}
