#![allow(clippy::assert_is_empty)]
//! Tests for the `codeindex_communities` tool (spec graphCI, T-014).
//!
//! Covers FR-013 (community detection) and FR-017 (non-blocking busy
//! response when the index is locked).

use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::{
    CodeIndexConfig, Confidence, EdgeKind, FileEntry, GraphEdge, Symbol, SymbolKind, Visibility,
};
use ragent_tools_extended::Tool;
use ragent_tools_extended::codeindex_communities::CodeIndexCommunitiesTool;
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

// ── Tool metadata ───────────────────────────────────────────────────────

#[test]
fn test_tool_name() {
    let tool = CodeIndexCommunitiesTool;
    assert_eq!(tool.name(), "codeindex_communities");
}

#[test]
fn test_tool_permission_category_is_codeindex_read() {
    let tool = CodeIndexCommunitiesTool;
    assert_eq!(tool.permission_category(), "codeindex:read");
}

#[test]
fn test_tool_parameters_schema_is_empty_object() {
    let tool = CodeIndexCommunitiesTool;
    let schema = tool.parameters_schema();
    assert_eq!(schema["type"], "object");
    // No required parameters.
    let required = schema["required"].as_array();
    assert!(required.is_none() || required.unwrap().is_empty());
}

// ── Registry registration ───────────────────────────────────────────────

#[test]
fn test_communities_tool_registered_in_extended_registry() {
    let registry = ragent_tools_extended::create_extended_registry();
    assert!(
        registry.contains("codeindex_communities"),
        "codeindex_communities should be registered in the extended registry",
    );
}

#[test]
fn test_communities_tool_registered_with_correct_permission_category() {
    let registry = ragent_tools_extended::create_extended_registry();
    let tool = registry
        .get("codeindex_communities")
        .expect("codeindex_communities should be registered");
    assert_eq!(tool.permission_category(), "codeindex:read");
}

// ── CodeIndex::communities / try_communities ────────────────────────────

/// Build an in-memory CodeIndex with a small graph:
///   hub --calls--> sat1
///   hub --calls--> sat2
///   hub --calls--> sat3
/// This creates a single community (all connected).
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
fn test_codeindex_communities_returns_detected_communities() {
    let idx = build_graph_index();
    let communities = idx.communities().unwrap();
    assert!(
        !communities.is_empty(),
        "should detect at least one community"
    );
    // All 4 symbols are connected, so they form a single community.
    let total_members: usize = communities.iter().map(|c| c.member_count).sum();
    assert_eq!(total_members, 4, "all 4 symbols should be in communities");
}

#[test]
fn test_codeindex_communities_empty_graph() {
    let config = CodeIndexConfig::default();
    let idx = CodeIndex::open_in_memory(&config).unwrap();
    let communities = idx.communities().unwrap();
    assert!(communities.is_empty(), "no communities when graph is empty");
}

#[test]
fn test_codeindex_try_communities_succeeds_when_unlocked() {
    let idx = build_graph_index();
    let result = idx.try_communities().unwrap();
    assert!(
        result.is_some(),
        "try_communities should acquire lock and run"
    );
    let communities = result.unwrap();
    assert!(!communities.is_empty());
}

#[test]
fn test_codeindex_try_communities_returns_none_when_locked() {
    let idx = build_graph_index();
    let _guard = idx.try_lock_store_for_test().expect("store lock");
    let result = idx.try_communities().unwrap();
    assert!(
        result.is_none(),
        "try_communities should return None when the store is locked"
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
async fn test_communities_tool_returns_communities() {
    let idx = build_graph_index();
    let ctx = make_ctx(Some(idx));
    let tool = CodeIndexCommunitiesTool;

    let output = tool.execute(json!({}), &ctx).await.unwrap();

    assert!(output.content.contains("## Communities"));
    assert!(output.content.contains("Community"));
    assert!(output.content.contains("Label"));
    assert!(output.content.contains("Members"));

    let metadata = output.metadata.unwrap();
    assert!(metadata["total_communities"].as_u64().unwrap() > 0);
}

#[tokio::test]
async fn test_communities_tool_returns_empty_graph_message() {
    let config = CodeIndexConfig::default();
    let idx = CodeIndex::open_in_memory(&config).unwrap();
    let ctx = make_ctx(Some(idx));
    let tool = CodeIndexCommunitiesTool;

    let output = tool.execute(json!({}), &ctx).await.unwrap();

    assert!(output.content.contains("No graph data available"));
    let metadata = output.metadata.unwrap();
    assert_eq!(metadata["error"], json!("codeindex_empty_graph"));
}

#[tokio::test]
async fn test_communities_tool_returns_disabled_when_no_code_index() {
    let ctx = make_ctx(None);
    let tool = CodeIndexCommunitiesTool;

    let output = tool.execute(json!({}), &ctx).await.unwrap();

    assert!(output.content.contains("not available"));
    assert_eq!(
        output.metadata.unwrap()["error"],
        json!("codeindex_disabled")
    );
}
