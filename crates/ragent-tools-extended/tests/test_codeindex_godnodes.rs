//! Tests for the `codeindex_godnodes` tool (spec graphCI, T-015).
//!
//! Covers FR-014 (top-N most-connected symbols) and FR-017 (non-blocking
//! busy response when the index is locked).

use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::{CodeIndexConfig, Confidence, EdgeKind, GraphEdge};
use ragent_tools_extended::Tool;
use ragent_tools_extended::codeindex_godnodes::CodeIndexGodnodesTool;
use serde_json::json;

// ── Tool metadata ───────────────────────────────────────────────────────

#[test]
fn test_tool_name() {
    let tool = CodeIndexGodnodesTool;
    assert_eq!(tool.name(), "codeindex_godnodes");
}

#[test]
fn test_tool_permission_category_is_codeindex_read() {
    let tool = CodeIndexGodnodesTool;
    assert_eq!(tool.permission_category(), "codeindex:read");
}

#[test]
fn test_tool_parameters_schema_has_optional_n() {
    let tool = CodeIndexGodnodesTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"]["n"].is_object());
    // n is not required
    let required = schema["required"].as_array();
    assert!(required.is_none() || required.unwrap().is_empty());
}

// ── Registry registration ───────────────────────────────────────────────

#[test]
fn test_godnodes_tool_registered_in_extended_registry() {
    let registry = ragent_tools_extended::create_extended_registry();
    assert!(
        registry.contains("codeindex_godnodes"),
        "codeindex_godnodes should be registered in the extended registry",
    );
}

#[test]
fn test_godnodes_tool_registered_with_correct_permission_category() {
    let registry = ragent_tools_extended::create_extended_registry();
    let tool = registry
        .get("codeindex_godnodes")
        .expect("codeindex_godnodes should be registered");
    assert_eq!(tool.permission_category(), "codeindex:read");
}

// ── CodeIndex::godnodes / try_godnodes ───────────────────────────────────

/// Build an in-memory CodeIndex with a small graph:
///   hub → sat1, hub → sat2, hub → sat3  (hub degree = 3, sats degree = 1)
fn build_graph_index() -> CodeIndex {
    let config = CodeIndexConfig::default();
    let idx = CodeIndex::open_in_memory(&config).unwrap();

    // We need direct store access to insert symbols and edges.
    // Lock the store, add a file with symbols, then add edges.
    {
        let store = idx.try_lock_store_for_test().expect("store lock");

        // Add a file.
        let file_entry = ragent_codeindex::types::FileEntry {
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
            make_symbol("sat3", file_id, 16, 20),
        ];
        store.upsert_symbols(file_id, &symbols).unwrap();

        let stored = store.get_file_symbols(file_id).unwrap();
        let hub = stored.iter().find(|s| s.name == "hub").unwrap().id;
        let sat1 = stored.iter().find(|s| s.name == "sat1").unwrap().id;
        let sat2 = stored.iter().find(|s| s.name == "sat2").unwrap().id;
        let sat3 = stored.iter().find(|s| s.name == "sat3").unwrap().id;

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
        store
            .upsert_edge_typed(&GraphEdge {
                source_sym: hub,
                target_sym: sat3,
                kind: EdgeKind::Calls,
                confidence: Confidence::Extracted,
                source_file: Some(file_id),
                line: Some(4),
            })
            .unwrap();
    }

    idx
}

fn make_symbol(name: &str, file_id: i64, start: u32, end: u32) -> ragent_codeindex::types::Symbol {
    use ragent_codeindex::types::{SymbolKind, Visibility};
    ragent_codeindex::types::Symbol {
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
fn test_codeindex_godnodes_returns_sorted_by_degree() {
    let idx = build_graph_index();
    let nodes = idx.godnodes(10).unwrap();
    assert!(!nodes.is_empty());
    // Hub has degree 3 (outgoing) + 0 (incoming) = 3 — but actually degree
    // counts all incident edges. hub has 3 outgoing, sats have 1 incoming each.
    // So hub degree = 3, sats degree = 1.
    assert_eq!(nodes[0].name, "hub");
    assert_eq!(nodes[0].degree, 3);
}

#[test]
fn test_codeindex_godnodes_respects_limit() {
    let idx = build_graph_index();
    let nodes = idx.godnodes(1).unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].name, "hub");
}

#[test]
fn test_codeindex_godnodes_empty_graph() {
    let config = CodeIndexConfig::default();
    let idx = CodeIndex::open_in_memory(&config).unwrap();
    let nodes = idx.godnodes(10).unwrap();
    assert!(nodes.is_empty());
}

#[test]
fn test_codeindex_try_godnodes_succeeds_when_unlocked() {
    let idx = build_graph_index();
    let nodes = idx.try_godnodes(10).unwrap();
    assert!(nodes.is_some());
    let nodes = nodes.unwrap();
    assert!(!nodes.is_empty());
    assert_eq!(nodes[0].name, "hub");
}

#[test]
fn test_codeindex_try_godnodes_returns_none_when_locked() {
    let idx = build_graph_index();
    // Hold the store lock.
    let _guard = idx.try_lock_store_for_test().expect("store lock");

    // try_godnodes should return None because the lock is held.
    let result = idx.try_godnodes(10).unwrap();
    assert!(
        result.is_none(),
        "try_godnodes should return None when the store is locked"
    );
}

// ── Tool execute with no code index ──────────────────────────────────────

#[tokio::test]
async fn test_godnodes_tool_returns_disabled_when_no_code_index() {
    use ragent_tools_extended::ToolContext;
    use std::path::PathBuf;
    use std::sync::Arc;

    let tool = CodeIndexGodnodesTool;
    let ctx = ToolContext {
        session_id: "test".to_string(),
        working_dir: PathBuf::from("."),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    };

    let output = tool.execute(json!({"n": 5}), &ctx).await.unwrap();
    assert!(output.content.contains("not available"));
    assert_eq!(
        output.metadata.unwrap()["error"],
        json!("codeindex_disabled")
    );
}
