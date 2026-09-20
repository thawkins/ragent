//! Tests for the `SymbolGraph` struct and public API (spec graphCI, T-003).

use chrono::Utc;
use ragent_codeindex::graph::{BuildResult, SymbolGraph};
use ragent_codeindex::store::IndexStore;
use ragent_codeindex::types::{
    Confidence, EdgeKind, FileEntry, GraphEdge, Symbol, SymbolKind, Visibility,
};

fn make_entry(path: &str, hash: &str) -> FileEntry {
    FileEntry {
        path: path.to_string(),
        content_hash: hash.to_string(),
        byte_size: 100,
        language: Some("rust".to_string()),
        last_indexed: Utc::now(),
        mtime_ns: 1_000_000_000,
        line_count: 10,
    }
}

fn make_symbol(name: &str, kind: SymbolKind, temp_id: i64, file_id: i64) -> Symbol {
    Symbol {
        id: temp_id,
        file_id,
        name: name.to_string(),
        qualified_name: Some(name.to_string()),
        kind,
        visibility: Visibility::Public,
        start_line: 1,
        end_line: 10,
        start_col: 0,
        end_col: 0,
        parent_id: None,
        signature: Some(format!("fn {name}()")),
        doc_comment: None,
        body_hash: Some("hash123".to_string()),
    }
}

#[test]
fn test_symbol_graph_new() {
    let store = IndexStore::open_in_memory().unwrap();
    let _graph = SymbolGraph::new(&store);
}

#[test]
fn test_edge_count_empty() {
    let store = IndexStore::open_in_memory().unwrap();
    let graph = SymbolGraph::new(&store);
    assert_eq!(graph.edge_count().unwrap(), 0);
}

#[test]
fn test_edge_count_after_insert() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("caller", SymbolKind::Function, 0, file_id),
        make_symbol("callee", SymbolKind::Function, 0, file_id),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();
    let caller_id = stored.iter().find(|s| s.name == "caller").unwrap().id;
    let callee_id = stored.iter().find(|s| s.name == "callee").unwrap().id;

    let edge = GraphEdge {
        source_sym: caller_id,
        target_sym: callee_id,
        kind: EdgeKind::Calls,
        confidence: Confidence::Extracted,
        source_file: Some(file_id),
        line: Some(5),
    };
    store.upsert_edge_typed(&edge).unwrap();

    let graph = SymbolGraph::new(&store);
    assert_eq!(graph.edge_count().unwrap(), 1);
}

#[test]
fn test_edge_count_by_confidence() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("a", SymbolKind::Function, 0, file_id),
        make_symbol("b", SymbolKind::Function, 0, file_id),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();
    let a_id = stored[0].id;
    let b_id = stored[1].id;

    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: a_id,
            target_sym: b_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: None,
            line: None,
        })
        .unwrap();

    let graph = SymbolGraph::new(&store);
    assert_eq!(
        graph
            .edge_count_by_confidence(Confidence::Extracted)
            .unwrap(),
        1
    );
    assert_eq!(
        graph
            .edge_count_by_confidence(Confidence::Inferred)
            .unwrap(),
        0
    );
}

#[test]
fn test_all_edges() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("a", SymbolKind::Function, 0, file_id),
        make_symbol("b", SymbolKind::Function, 0, file_id),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();
    let a_id = stored[0].id;
    let b_id = stored[1].id;

    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: a_id,
            target_sym: b_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: None,
            line: None,
        })
        .unwrap();

    let graph = SymbolGraph::new(&store);
    let edges = graph.all_edges().unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].kind, EdgeKind::Calls);
}

#[test]
fn test_godnodes_empty() {
    let store = IndexStore::open_in_memory().unwrap();
    let graph = SymbolGraph::new(&store);
    let nodes = graph.godnodes(10).unwrap();
    assert!(nodes.is_empty());
}

#[test]
fn test_godnodes_with_edges() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("hub", SymbolKind::Function, 0, file_id),
        make_symbol("spoke1", SymbolKind::Function, 0, file_id),
        make_symbol("spoke2", SymbolKind::Function, 0, file_id),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();
    let hub_id = stored.iter().find(|s| s.name == "hub").unwrap().id;
    let spoke1_id = stored.iter().find(|s| s.name == "spoke1").unwrap().id;
    let spoke2_id = stored.iter().find(|s| s.name == "spoke2").unwrap().id;

    // hub calls spoke1 and spoke2 — degree 2
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: hub_id,
            target_sym: spoke1_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: None,
            line: None,
        })
        .unwrap();
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: hub_id,
            target_sym: spoke2_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: None,
            line: None,
        })
        .unwrap();

    let graph = SymbolGraph::new(&store);
    let nodes = graph.godnodes(10).unwrap();
    assert_eq!(nodes.len(), 3);

    // hub should be first (degree 2)
    assert_eq!(nodes[0].name, "hub");
    assert_eq!(nodes[0].degree, 2);

    // spoke1 and spoke2 should have degree 1 each
    assert_eq!(nodes[1].degree, 1);
    assert_eq!(nodes[2].degree, 1);
}

#[test]
fn test_godnodes_limit_n() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("hub", SymbolKind::Function, 0, file_id),
        make_symbol("s1", SymbolKind::Function, 0, file_id),
        make_symbol("s2", SymbolKind::Function, 0, file_id),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();
    let hub_id = stored.iter().find(|s| s.name == "hub").unwrap().id;
    let s1_id = stored.iter().find(|s| s.name == "s1").unwrap().id;
    let s2_id = stored.iter().find(|s| s.name == "s2").unwrap().id;

    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: hub_id,
            target_sym: s1_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: None,
            line: None,
        })
        .unwrap();
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: hub_id,
            target_sym: s2_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: None,
            line: None,
        })
        .unwrap();

    let graph = SymbolGraph::new(&store);
    let nodes = graph.godnodes(1).unwrap();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].name, "hub");
}

#[test]
fn test_build_returns_result() {
    let store = IndexStore::open_in_memory().unwrap();
    let graph = SymbolGraph::new(&store);
    let result = graph.build().unwrap();
    // Stub returns default (all zeros).
    assert_eq!(result.edges_total, 0);
}

#[test]
fn test_build_for_language_returns_result() {
    let store = IndexStore::open_in_memory().unwrap();
    let graph = SymbolGraph::new(&store);
    let result = graph.build_for_language("rust").unwrap();
    assert_eq!(result.edges_total, 0);
}

#[test]
fn test_explain_returns_none_for_unknown() {
    let store = IndexStore::open_in_memory().unwrap();
    let graph = SymbolGraph::new(&store);
    let result = graph.explain("NonExistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_path_returns_none_for_unknown() {
    let store = IndexStore::open_in_memory().unwrap();
    let graph = SymbolGraph::new(&store);
    let result = graph.path("A", "B").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_communities_returns_empty() {
    let store = IndexStore::open_in_memory().unwrap();
    let graph = SymbolGraph::new(&store);
    let result = graph.communities().unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_export_json() {
    let store = IndexStore::open_in_memory().unwrap();
    let graph = SymbolGraph::new(&store);
    let json = graph.export_json().unwrap();
    assert!(json.contains("nodes"));
    assert!(json.contains("edges"));
}

#[test]
fn test_export_report() {
    let store = IndexStore::open_in_memory().unwrap();
    let graph = SymbolGraph::new(&store);
    let report = graph.export_report().unwrap();
    assert!(report.contains("Graph Report"));
}

#[test]
fn test_build_result_display() {
    let result = BuildResult {
        edges_total: 100,
        edges_extracted: 60,
        edges_inferred: 40,
        elapsed_ms: 500,
    };
    let s = format!("{result}");
    assert!(s.contains("100 edges"));
    assert!(s.contains("60 EXTRACTED"));
    assert!(s.contains("40 INFERRED"));
}
