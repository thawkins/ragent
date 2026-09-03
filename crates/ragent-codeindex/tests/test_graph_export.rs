#![allow(clippy::assert_is_empty)]
//! Tests for graph export: `graph.json` and `GRAPH_REPORT.md` (spec graphCI, T-011).
//!
//! Covers FR-010 (export to graph.json + GRAPH_REPORT.md) and FR-020
//! (node/edge attributes in a visualisation-compatible format).

use chrono::Utc;
use ragent_codeindex::graph::export;
use ragent_codeindex::store::IndexStore;
use ragent_codeindex::types::{
    Confidence, EdgeKind, FileEntry, GraphEdge, Symbol, SymbolKind, Visibility,
};
use serde_json::Value;

// ── Helpers ──────────────────────────────────────────────────────────────

fn make_entry(path: &str, hash: &str) -> FileEntry {
    FileEntry {
        path: path.to_string(),
        content_hash: hash.to_string(),
        byte_size: 100,
        language: Some("rust".to_string()),
        last_indexed: Utc::now(),
        mtime_ns: 1_000_000_000,
        line_count: 30,
    }
}

fn make_symbol(name: &str, kind: SymbolKind, file_id: i64, start: u32, end: u32) -> Symbol {
    Symbol {
        id: 0,
        file_id,
        name: name.to_string(),
        qualified_name: Some(name.to_string()),
        kind,
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

/// Build a store with:
///   hub --calls--> sat1 (EXTRACTED)
///   hub --calls--> sat2 (INFERRED)
///   hub --implements--> trait_a (EXTRACTED)
fn build_store() -> IndexStore {
    let store = IndexStore::open_in_memory().unwrap();

    let file_id = store.upsert_file(&make_entry("src/hub.rs", "h1")).unwrap();

    let symbols = vec![
        make_symbol("hub", SymbolKind::Function, file_id, 1, 5),
        make_symbol("sat1", SymbolKind::Function, file_id, 6, 10),
        make_symbol("sat2", SymbolKind::Function, file_id, 11, 15),
        make_symbol("trait_a", SymbolKind::Trait, file_id, 16, 20),
        make_symbol("isolated", SymbolKind::Function, file_id, 21, 25),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let hub = stored.iter().find(|s| s.name == "hub").unwrap().id;
    let sat1 = stored.iter().find(|s| s.name == "sat1").unwrap().id;
    let sat2 = stored.iter().find(|s| s.name == "sat2").unwrap().id;
    let trait_a = stored.iter().find(|s| s.name == "trait_a").unwrap().id;

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
            confidence: Confidence::Inferred,
            source_file: Some(file_id),
            line: Some(3),
        })
        .unwrap();
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: hub,
            target_sym: trait_a,
            kind: EdgeKind::Implements,
            confidence: Confidence::Extracted,
            source_file: Some(file_id),
            line: Some(4),
        })
        .unwrap();

    // Community assignment.
    store.upsert_community(hub, 0, Some("core")).unwrap();
    store.upsert_community(sat1, 0, Some("core")).unwrap();
    store.upsert_community(sat2, 1, Some("satellite")).unwrap();

    store
}

// ── to_json ─────────────────────────────────────────────────────────────

#[test]
fn test_to_json_empty_graph() {
    let store = IndexStore::open_in_memory().unwrap();
    let json = export::to_json(&store).unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["nodes"].is_array());
    assert!(parsed["edges"].is_array());
    assert_eq!(parsed["nodes"].as_array().unwrap().len(), 0);
    assert_eq!(parsed["edges"].as_array().unwrap().len(), 0);
}

#[test]
fn test_to_json_has_nodes_and_edges() {
    let store = build_store();
    let json = export::to_json(&store).unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();

    let nodes = parsed["nodes"].as_array().unwrap();
    let edges = parsed["edges"].as_array().unwrap();
    assert!(nodes.len() >= 5, "should have all symbols as nodes");
    assert_eq!(edges.len(), 3, "should have 3 edges");
}

#[test]
fn test_to_json_node_attributes() {
    let store = build_store();
    let json = export::to_json(&store).unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();

    let nodes = parsed["nodes"].as_array().unwrap();
    let hub_node = nodes
        .iter()
        .find(|n| n["name"] == "hub")
        .expect("hub node should exist");

    // FR-020: node attributes include community, degree, kind, source_file, line.
    assert!(hub_node["id"].is_i64(), "node has id");
    assert_eq!(hub_node["name"], "hub");
    assert!(hub_node["kind"].is_string(), "node has kind");
    assert_eq!(hub_node["source_file"], "src/hub.rs");
    assert!(hub_node["line"].is_u64(), "node has line");
    assert!(hub_node["community"].is_i64(), "node has community");
    assert!(hub_node["degree"].is_u64(), "node has degree");
    // hub has 3 outgoing edges, so degree = 3.
    assert_eq!(hub_node["degree"], 3);
}

#[test]
fn test_to_json_edge_attributes() {
    let store = build_store();
    let json = export::to_json(&store).unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();

    let edges = parsed["edges"].as_array().unwrap();
    let call_edge = edges
        .iter()
        .find(|e| e["kind"] == "calls" && e["confidence"] == "EXTRACTED")
        .expect("should have an EXTRACTED calls edge");

    // FR-020: edge attributes include kind and confidence.
    assert!(call_edge["source"].is_i64(), "edge has source");
    assert!(call_edge["target"].is_i64(), "edge has target");
    assert_eq!(call_edge["kind"], "calls");
    assert_eq!(call_edge["confidence"], "EXTRACTED");
    assert!(call_edge["line"].is_u64(), "edge has line");
}

#[test]
fn test_to_json_is_valid_json() {
    let store = build_store();
    let json = export::to_json(&store).unwrap();
    // Should parse without error.
    let _: Value = serde_json::from_str(&json).expect("output must be valid JSON");
}

#[test]
fn test_to_json_node_without_edges_has_zero_degree() {
    let store = build_store();
    let json = export::to_json(&store).unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();

    let nodes = parsed["nodes"].as_array().unwrap();
    let isolated = nodes
        .iter()
        .find(|n| n["name"] == "isolated")
        .expect("isolated node should exist");
    assert_eq!(isolated["degree"], 0, "isolated node has degree 0");
    assert!(
        isolated["community"].is_null(),
        "isolated has no community assignment"
    );
}

#[test]
fn test_to_json_contains_extracted_and_inferred_edges() {
    let store = build_store();
    let json = export::to_json(&store).unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();

    let edges = parsed["edges"].as_array().unwrap();
    let has_extracted = edges.iter().any(|e| e["confidence"] == "EXTRACTED");
    let has_inferred = edges.iter().any(|e| e["confidence"] == "INFERRED");
    assert!(has_extracted, "should contain EXTRACTED edges");
    assert!(has_inferred, "should contain INFERRED edges");
}

// ── to_report ───────────────────────────────────────────────────────────

#[test]
fn test_to_report_empty_graph() {
    let store = IndexStore::open_in_memory().unwrap();
    let report = export::to_report(&store).unwrap();
    assert!(
        report.contains("No graph data available"),
        "empty graph message"
    );
    assert!(
        report.contains("/codeindex graph build"),
        "should mention /codeindex graph build"
    );
}

#[test]
fn test_to_report_has_title() {
    let store = build_store();
    let report = export::to_report(&store).unwrap();
    assert!(report.starts_with("# Graph Report"), "report has title");
}

#[test]
fn test_to_report_has_statistics() {
    let store = build_store();
    let report = export::to_report(&store).unwrap();
    assert!(report.contains("## Statistics"), "has statistics section");
    assert!(report.contains("**Nodes:**"), "has node count");
    assert!(report.contains("**Edges:**"), "has edge count");
    assert!(report.contains("**Extracted:**"), "has extracted count");
    assert!(report.contains("**Inferred:**"), "has inferred count");
}

#[test]
fn test_to_report_has_god_nodes() {
    let store = build_store();
    let report = export::to_report(&store).unwrap();
    assert!(report.contains("## Top God-Nodes"), "has god-nodes section");
    assert!(report.contains("hub"), "mentions hub (highest degree)");
    assert!(report.contains("Degree"), "has degree column");
}

#[test]
fn test_to_report_has_communities() {
    let store = build_store();
    let report = export::to_report(&store).unwrap();
    assert!(report.contains("## Communities"), "has communities section");
    assert!(report.contains("core"), "mentions community label 'core'");
}

#[test]
fn test_to_report_has_edge_kind_distribution() {
    let store = build_store();
    let report = export::to_report(&store).unwrap();
    assert!(
        report.contains("## Edge Kind Distribution"),
        "has edge kind distribution section"
    );
    assert!(report.contains("calls"), "mentions 'calls' kind");
    assert!(report.contains("implements"), "mentions 'implements' kind");
}

#[test]
fn test_to_report_statistics_counts_are_correct() {
    let store = build_store();
    let report = export::to_report(&store).unwrap();
    // 5 symbols, 3 edges, 2 extracted, 1 inferred.
    assert!(report.contains("**Nodes:** 5"), "correct node count");
    assert!(report.contains("**Edges:** 3"), "correct edge count");
    assert!(
        report.contains("**Extracted:** 2"),
        "correct extracted count"
    );
    assert!(report.contains("**Inferred:** 1"), "correct inferred count");
}

// ── SymbolGraph::export_json / export_report ────────────────────────────

#[test]
fn test_symbol_graph_export_json() {
    let store = build_store();
    let graph = ragent_codeindex::graph::SymbolGraph::new(&store);
    let json = graph.export_json().unwrap();
    let parsed: Value = serde_json::from_str(&json).unwrap();
    assert!(parsed["nodes"].is_array());
    assert!(parsed["edges"].is_array());
    assert_eq!(parsed["edges"].as_array().unwrap().len(), 3);
}

#[test]
fn test_symbol_graph_export_report() {
    let store = build_store();
    let graph = ragent_codeindex::graph::SymbolGraph::new(&store);
    let report = graph.export_report().unwrap();
    assert!(report.starts_with("# Graph Report"));
    assert!(report.contains("## Statistics"));
}
