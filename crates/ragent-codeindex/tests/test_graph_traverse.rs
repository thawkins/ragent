//! Tests for the explain query (spec graphCI, T-009).

use chrono::Utc;
use ragent_codeindex::graph::SymbolGraph;
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
        line_count: 20,
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

/// Build a store with a hub symbol that has incoming and outgoing edges.
fn build_explain_store() -> IndexStore {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let symbols = vec![
        make_symbol("hub", SymbolKind::Function, file_id, 5, 20),
        make_symbol("caller1", SymbolKind::Function, file_id, 1, 4),
        make_symbol("caller2", SymbolKind::Function, file_id, 22, 25),
        make_symbol("callee1", SymbolKind::Function, file_id, 27, 30),
        make_symbol("callee2", SymbolKind::Function, file_id, 32, 35),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let hub_id = stored.iter().find(|s| s.name == "hub").unwrap().id;
    let caller1_id = stored.iter().find(|s| s.name == "caller1").unwrap().id;
    let caller2_id = stored.iter().find(|s| s.name == "caller2").unwrap().id;
    let callee1_id = stored.iter().find(|s| s.name == "callee1").unwrap().id;
    let callee2_id = stored.iter().find(|s| s.name == "callee2").unwrap().id;

    // Incoming: caller1 → hub (calls)
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: caller1_id,
            target_sym: hub_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: Some(file_id),
            line: Some(2),
        })
        .unwrap();

    // Incoming: caller2 → hub (imports)
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: caller2_id,
            target_sym: hub_id,
            kind: EdgeKind::Imports,
            confidence: Confidence::Inferred,
            source_file: Some(file_id),
            line: Some(23),
        })
        .unwrap();

    // Outgoing: hub → callee1 (calls)
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: hub_id,
            target_sym: callee1_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: Some(file_id),
            line: Some(10),
        })
        .unwrap();

    // Outgoing: hub → callee2 (references)
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: hub_id,
            target_sym: callee2_id,
            kind: EdgeKind::References,
            confidence: Confidence::Extracted,
            source_file: Some(file_id),
            line: Some(15),
        })
        .unwrap();

    store
}

// ── Basic explain tests ────────────────────────────────────────────────

#[test]
fn test_explain_found() {
    let store = build_explain_store();
    let graph = SymbolGraph::new(&store);
    let result = graph.explain("hub").unwrap();
    assert!(result.is_some(), "hub should be found");
    let explain = result.unwrap();
    assert_eq!(explain.name, "hub");
}

#[test]
fn test_explain_not_found() {
    let store = build_explain_store();
    let graph = SymbolGraph::new(&store);
    let result = graph.explain("NonExistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_explain_node_metadata() {
    let store = build_explain_store();
    let graph = SymbolGraph::new(&store);
    let explain = graph.explain("hub").unwrap().unwrap();
    assert_eq!(explain.name, "hub");
    assert_eq!(explain.source_file, "a.rs");
    assert_eq!(explain.line, 5); // start_line
}

#[test]
fn test_explain_degree() {
    let store = build_explain_store();
    let graph = SymbolGraph::new(&store);
    let explain = graph.explain("hub").unwrap().unwrap();
    // hub has 2 incoming + 2 outgoing = degree 4
    assert_eq!(explain.degree, 4);
}

#[test]
fn test_explain_incoming_edges() {
    let store = build_explain_store();
    let graph = SymbolGraph::new(&store);
    let explain = graph.explain("hub").unwrap().unwrap();
    assert_eq!(explain.incoming.len(), 2, "should have 2 incoming edges");

    // Check that incoming edges reference the callers
    let incoming_names: Vec<&str> = explain.incoming.iter().map(|c| c.symbol.as_str()).collect();
    assert!(incoming_names.contains(&"caller1"));
    assert!(incoming_names.contains(&"caller2"));
}

#[test]
fn test_explain_outgoing_edges() {
    let store = build_explain_store();
    let graph = SymbolGraph::new(&store);
    let explain = graph.explain("hub").unwrap().unwrap();
    assert_eq!(explain.outgoing.len(), 2, "should have 2 outgoing edges");

    // Check that outgoing edges reference the callees
    let outgoing_names: Vec<&str> = explain.outgoing.iter().map(|c| c.symbol.as_str()).collect();
    assert!(outgoing_names.contains(&"callee1"));
    assert!(outgoing_names.contains(&"callee2"));
}

#[test]
fn test_explain_edge_kind_in_connection() {
    let store = build_explain_store();
    let graph = SymbolGraph::new(&store);
    let explain = graph.explain("hub").unwrap().unwrap();

    // Find the incoming "calls" edge from caller1
    let call_conn = explain
        .incoming
        .iter()
        .find(|c| c.symbol == "caller1")
        .expect("should find caller1 in incoming");
    assert_eq!(call_conn.kind, "calls");
    assert_eq!(call_confidence(&call_conn.confidence), "EXTRACTED");
}

#[test]
fn test_explain_edge_confidence_in_connection() {
    let store = build_explain_store();
    let graph = SymbolGraph::new(&store);
    let explain = graph.explain("hub").unwrap().unwrap();

    // Find the incoming "imports" edge from caller2 (INFERRED)
    let import_conn = explain
        .incoming
        .iter()
        .find(|c| c.symbol == "caller2")
        .expect("should find caller2 in incoming");
    assert_eq!(import_conn.kind, "imports");
    assert_eq!(call_confidence(&import_conn.confidence), "INFERRED");
}

#[test]
fn test_explain_line_number_in_connection() {
    let store = build_explain_store();
    let graph = SymbolGraph::new(&store);
    let explain = graph.explain("hub").unwrap().unwrap();

    // The incoming "calls" edge from caller1 is at line 2
    let call_conn = explain
        .incoming
        .iter()
        .find(|c| c.symbol == "caller1")
        .expect("should find caller1");
    assert_eq!(call_conn.line, Some(2));
}

#[test]
fn test_explain_community_none_without_detection() {
    // Without running community detection, community should be None.
    let store = build_explain_store();
    let graph = SymbolGraph::new(&store);
    let explain = graph.explain("hub").unwrap().unwrap();
    assert_eq!(explain.community, None);
}

#[test]
fn test_explain_community_after_assignment() {
    let store = build_explain_store();
    let stored = store
        .get_file_symbols(store.get_file_id("a.rs").unwrap().unwrap())
        .unwrap();
    let hub_id = stored.iter().find(|s| s.name == "hub").unwrap().id;

    // Manually assign a community.
    store.upsert_community(hub_id, 3, Some("core")).unwrap();

    let graph = SymbolGraph::new(&store);
    let explain = graph.explain("hub").unwrap().unwrap();
    assert_eq!(explain.community, Some(3));
}

#[test]
fn test_explain_source_file_in_connection() {
    let store = build_explain_store();
    let graph = SymbolGraph::new(&store);
    let explain = graph.explain("hub").unwrap().unwrap();

    // All connections should have the source file "a.rs"
    for conn in explain.incoming.iter().chain(explain.outgoing.iter()) {
        assert_eq!(conn.source_file, "a.rs");
    }
}

#[test]
fn test_explain_empty_graph() {
    let store = IndexStore::open_in_memory().unwrap();
    let graph = SymbolGraph::new(&store);
    let result = graph.explain("anything").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_explain_symbol_with_no_edges() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![make_symbol("lonely", SymbolKind::Function, file_id, 1, 5)];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let graph = SymbolGraph::new(&store);
    let explain = graph.explain("lonely").unwrap().unwrap();
    assert_eq!(explain.name, "lonely");
    assert_eq!(explain.degree, 0);
    assert!(explain.incoming.is_empty());
    assert!(explain.outgoing.is_empty());
}

// ── Connection limit (FR-011: top 50) ──────────────────────────────────

#[test]
fn test_explain_limits_to_50_connections() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    // Create 1 hub + 60 callers (all calling hub)
    let mut symbols = vec![make_symbol("hub", SymbolKind::Function, file_id, 1, 5)];
    for i in 0..60 {
        symbols.push(make_symbol(
            &format!("caller{i}"),
            SymbolKind::Function,
            file_id,
            10 + i as u32 * 3,
            12 + i as u32 * 3,
        ));
    }
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let hub_id = stored.iter().find(|s| s.name == "hub").unwrap().id;
    for sym in &stored {
        if sym.name == "hub" {
            continue;
        }
        store
            .upsert_edge_typed(&GraphEdge {
                source_sym: sym.id,
                target_sym: hub_id,
                kind: EdgeKind::Calls,
                confidence: Confidence::Extracted,
                source_file: Some(file_id),
                line: None,
            })
            .unwrap();
    }

    let graph = SymbolGraph::new(&store);
    let explain = graph.explain("hub").unwrap().unwrap();
    // Total connections should be limited to 50
    let total = explain.incoming.len() + explain.outgoing.len();
    assert!(total <= 50, "total connections ({total}) should be <= 50");
    // Degree should still report the real count (60)
    assert_eq!(explain.degree, 60);
}

/// Helper: convert Confidence to a comparable string.
fn call_confidence(c: &Confidence) -> &'static str {
    match c {
        Confidence::Extracted => "EXTRACTED",
        Confidence::Inferred => "INFERRED",
    }
}
