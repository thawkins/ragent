//! Tests for BFS shortest-path traversal (spec graphCI, T-008).

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

fn build_test_graph() -> IndexStore {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    // A → B → C → D  (chain)
    // A → C (shortcut, making A→C 1 hop instead of 2)
    let symbols = vec![
        make_symbol("A", SymbolKind::Function, file_id, 1, 5),
        make_symbol("B", SymbolKind::Function, file_id, 7, 10),
        make_symbol("C", SymbolKind::Function, file_id, 12, 15),
        make_symbol("D", SymbolKind::Function, file_id, 17, 20),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let a_id = stored.iter().find(|s| s.name == "A").unwrap().id;
    let b_id = stored.iter().find(|s| s.name == "B").unwrap().id;
    let c_id = stored.iter().find(|s| s.name == "C").unwrap().id;
    let d_id = stored.iter().find(|s| s.name == "D").unwrap().id;

    // A calls B
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: a_id,
            target_sym: b_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: Some(file_id),
            line: Some(2),
        })
        .unwrap();
    // B calls C
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: b_id,
            target_sym: c_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: Some(file_id),
            line: Some(8),
        })
        .unwrap();
    // C calls D
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: c_id,
            target_sym: d_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: Some(file_id),
            line: Some(13),
        })
        .unwrap();
    // A calls C (shortcut)
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: a_id,
            target_sym: c_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: Some(file_id),
            line: Some(3),
        })
        .unwrap();

    store
}

// ── Basic path tests ────────────────────────────────────────────────────

#[test]
fn test_path_direct_edge() {
    let store = build_test_graph();
    let graph = SymbolGraph::new(&store);
    let result = graph.path("A", "B").unwrap();
    assert!(result.is_some(), "path A→B should exist");
    let path = result.unwrap();
    assert_eq!(path.hops, 1);
    assert_eq!(path.steps.len(), 2);
    assert_eq!(path.steps[0].0, "A");
    assert_eq!(path.steps[0].1, None);
    assert_eq!(path.steps[1].0, "B");
    assert_eq!(path.steps[1].1.as_deref(), Some("calls"));
}

#[test]
fn test_path_shortest_via_shortcut() {
    let store = build_test_graph();
    let graph = SymbolGraph::new(&store);
    // A→C should be 1 hop (direct shortcut), not 2 (A→B→C)
    let result = graph.path("A", "C").unwrap();
    assert!(result.is_some());
    let path = result.unwrap();
    assert_eq!(path.hops, 1, "A→C should be 1 hop via shortcut");
    assert_eq!(path.steps.len(), 2);
    assert_eq!(path.steps[0].0, "A");
    assert_eq!(path.steps[1].0, "C");
}

#[test]
fn test_path_multi_hop() {
    let store = build_test_graph();
    let graph = SymbolGraph::new(&store);
    // A→D should be 2 hops (A→C→D)
    let result = graph.path("A", "D").unwrap();
    assert!(result.is_some());
    let path = result.unwrap();
    assert_eq!(path.hops, 2, "A→D should be 2 hops");
    assert_eq!(path.steps.len(), 3);
    assert_eq!(path.steps[0].0, "A");
    assert_eq!(path.steps[1].0, "C");
    assert_eq!(path.steps[2].0, "D");
}

#[test]
fn test_path_same_symbol() {
    let store = build_test_graph();
    let graph = SymbolGraph::new(&store);
    let result = graph.path("A", "A").unwrap();
    assert!(result.is_some());
    let path = result.unwrap();
    assert_eq!(path.hops, 0);
    assert_eq!(path.steps.len(), 1);
    assert_eq!(path.steps[0].0, "A");
}

#[test]
fn test_path_no_path_exists() {
    let store = build_test_graph();
    let graph = SymbolGraph::new(&store);
    // D has no outgoing edges, so D→A has no path.
    let result = graph.path("D", "A").unwrap();
    assert!(result.is_none(), "D→A should have no path");
}

#[test]
fn test_path_source_not_found() {
    let store = build_test_graph();
    let graph = SymbolGraph::new(&store);
    let result = graph.path("NonExistent", "A").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_path_target_not_found() {
    let store = build_test_graph();
    let graph = SymbolGraph::new(&store);
    let result = graph.path("A", "NonExistent").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_path_empty_graph() {
    let store = IndexStore::open_in_memory().unwrap();
    let graph = SymbolGraph::new(&store);
    let result = graph.path("A", "B").unwrap();
    assert!(result.is_none());
}

#[test]
fn test_path_no_edges() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("A", SymbolKind::Function, file_id, 1, 5),
        make_symbol("B", SymbolKind::Function, file_id, 7, 10),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let graph = SymbolGraph::new(&store);
    let result = graph.path("A", "B").unwrap();
    assert!(result.is_none(), "no path without edges");
}

// ── Edge kind in path ──────────────────────────────────────────────────

#[test]
fn test_path_records_edge_kind() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("caller", SymbolKind::Function, file_id, 1, 5),
        make_symbol("callee", SymbolKind::Function, file_id, 7, 10),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();
    let caller_id = stored[0].id;
    let callee_id = stored[1].id;

    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: caller_id,
            target_sym: callee_id,
            kind: EdgeKind::Imports,
            confidence: Confidence::Extracted,
            source_file: Some(file_id),
            line: Some(2),
        })
        .unwrap();

    let graph = SymbolGraph::new(&store);
    let result = graph.path("caller", "callee").unwrap();
    assert!(result.is_some());
    let path = result.unwrap();
    assert_eq!(path.hops, 1);
    assert_eq!(path.steps[1].1.as_deref(), Some("imports"));
}

// ── Longer path preference ─────────────────────────────────────────────

#[test]
fn test_path_prefers_shorter_over_longer() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("A", SymbolKind::Function, file_id, 1, 5),
        make_symbol("B", SymbolKind::Function, file_id, 7, 10),
        make_symbol("C", SymbolKind::Function, file_id, 12, 15),
        make_symbol("D", SymbolKind::Function, file_id, 17, 20),
        make_symbol("E", SymbolKind::Function, file_id, 22, 25),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();
    let ids: Vec<i64> = stored.iter().map(|s| s.id).collect();

    // Long path: A→B→C→D→E (4 hops)
    // Short path: A→D→E (2 hops)
    for (src, tgt) in [(0, 1), (1, 2), (2, 3), (3, 4), (0, 3), (3, 4)] {
        store
            .upsert_edge_typed(&GraphEdge {
                source_sym: ids[src],
                target_sym: ids[tgt],
                kind: EdgeKind::Calls,
                confidence: Confidence::Extracted,
                source_file: Some(file_id),
                line: None,
            })
            .unwrap();
    }

    let graph = SymbolGraph::new(&store);
    let result = graph.path("A", "E").unwrap();
    assert!(result.is_some());
    let path = result.unwrap();
    assert_eq!(path.hops, 2, "should prefer 2-hop path over 4-hop");
    assert_eq!(path.steps[0].0, "A");
    assert_eq!(path.steps[1].0, "D");
    assert_eq!(path.steps[2].0, "E");
}

// ── Disconnected components ────────────────────────────────────────────

#[test]
fn test_path_disconnected_components() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("X", SymbolKind::Function, file_id, 1, 5),
        make_symbol("Y", SymbolKind::Function, file_id, 7, 10),
        make_symbol("Z", SymbolKind::Function, file_id, 12, 15),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();
    let x_id = stored[0].id;
    let y_id = stored[1].id;

    // Only X→Y edge; Z is disconnected.
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: x_id,
            target_sym: y_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: Some(file_id),
            line: None,
        })
        .unwrap();

    let graph = SymbolGraph::new(&store);
    // X→Z should have no path.
    assert!(graph.path("X", "Z").unwrap().is_none());
    // Z→X should have no path.
    assert!(graph.path("Z", "X").unwrap().is_none());
    // X→Y should have a path.
    assert!(graph.path("X", "Y").unwrap().is_some());
}
