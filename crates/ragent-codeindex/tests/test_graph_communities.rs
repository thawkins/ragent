#![allow(clippy::assert_is_empty)]
//! Tests for community detection (spec graphCI, T-010).
//!
//! Covers FR-013 (community detection via label propagation) and FR-019
//! (auto-labelling from the highest-degree node's symbol name).

use chrono::Utc;
use ragent_codeindex::graph::SymbolGraph;
use ragent_codeindex::graph::communities::{detect_communities, list_communities};
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

fn add_edge(store: &IndexStore, src: i64, tgt: i64, kind: EdgeKind) {
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: src,
            target_sym: tgt,
            kind,
            confidence: Confidence::Extracted,
            source_file: None,
            line: None,
        })
        .unwrap();
}

/// Build a store with two disconnected cliques:
///   clique A: a1 — a2 — a3
///   clique B: b1 — b2 — b3
fn build_two_cliques() -> (IndexStore, i64, i64) {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let symbols = vec![
        make_symbol("a1", SymbolKind::Function, file_id, 1, 2),
        make_symbol("a2", SymbolKind::Function, file_id, 3, 4),
        make_symbol("a3", SymbolKind::Function, file_id, 5, 6),
        make_symbol("b1", SymbolKind::Function, file_id, 7, 8),
        make_symbol("b2", SymbolKind::Function, file_id, 9, 10),
        make_symbol("b3", SymbolKind::Function, file_id, 11, 12),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let get = |n: &str| stored.iter().find(|s| s.name == n).unwrap().id;
    let a1 = get("a1");
    let a2 = get("a2");
    let a3 = get("a3");
    let b1 = get("b1");
    let b2 = get("b2");
    let b3 = get("b3");

    // Clique A: a1-a2, a2-a3, a1-a3
    add_edge(&store, a1, a2, EdgeKind::Calls);
    add_edge(&store, a2, a3, EdgeKind::Calls);
    add_edge(&store, a1, a3, EdgeKind::Calls);

    // Clique B: b1-b2, b2-b3, b1-b3
    add_edge(&store, b1, b2, EdgeKind::Calls);
    add_edge(&store, b2, b3, EdgeKind::Calls);
    add_edge(&store, b1, b3, EdgeKind::Calls);

    (store, a1, b1)
}

// ── detect_communities ──────────────────────────────────────────────────

#[test]
fn test_detect_empty_graph_returns_empty() {
    let store = IndexStore::open_in_memory().unwrap();
    let communities = detect_communities(&store).unwrap();
    assert!(communities.is_empty());
}

#[test]
fn test_detect_single_node_no_edges_returns_empty() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![make_symbol("lonely", SymbolKind::Function, file_id, 1, 2)];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let communities = detect_communities(&store).unwrap();
    assert!(communities.is_empty(), "no edges → no communities");
}

#[test]
fn test_detect_two_cliques_produces_two_communities() {
    let (store, _a1, _b1) = build_two_cliques();
    let communities = detect_communities(&store).unwrap();
    assert_eq!(communities.len(), 2, "should detect 2 communities");
}

#[test]
fn test_detect_all_members_in_some_community() {
    let (store, _a1, _b1) = build_two_cliques();
    let communities = detect_communities(&store).unwrap();
    let total: usize = communities.iter().map(|c| c.member_count).sum();
    assert_eq!(total, 6, "all 6 symbols should be assigned");
}

#[test]
fn test_detect_communities_have_distinct_ids() {
    let (store, _a1, _b1) = build_two_cliques();
    let communities = detect_communities(&store).unwrap();
    let ids: Vec<i64> = communities.iter().map(|c| c.id).collect();
    assert_eq!(ids.len(), 2);
    assert_ne!(ids[0], ids[1], "community IDs must be distinct");
}

#[test]
fn test_detect_community_member_count_matches() {
    let (store, _a1, _b1) = build_two_cliques();
    let communities = detect_communities(&store).unwrap();
    // Both cliques have 3 members.
    for c in &communities {
        assert_eq!(c.member_count, 3, "each clique has 3 members");
    }
}

#[test]
fn test_detect_persists_to_communities_table() {
    let (store, _a1, _b1) = build_two_cliques();
    detect_communities(&store).unwrap();
    assert_eq!(store.community_count().unwrap(), 2);
    assert_eq!(store.query_all_communities().unwrap().len(), 6);
}

#[test]
fn test_detect_clears_previous_assignments() {
    let (store, _a1, _b1) = build_two_cliques();
    detect_communities(&store).unwrap();
    assert_eq!(store.community_count().unwrap(), 2);

    // Run again — should not double up.
    detect_communities(&store).unwrap();
    assert_eq!(store.community_count().unwrap(), 2);
    assert_eq!(store.query_all_communities().unwrap().len(), 6);
}

#[test]
fn test_detect_auto_label_from_highest_degree_node() {
    // Build a star: hub connected to 3 satellites.
    // hub has degree 3 (highest), so the community label should contain "hub".
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("hub", SymbolKind::Function, file_id, 1, 2),
        make_symbol("sat1", SymbolKind::Function, file_id, 3, 4),
        make_symbol("sat2", SymbolKind::Function, file_id, 5, 6),
        make_symbol("sat3", SymbolKind::Function, file_id, 7, 8),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let hub = stored.iter().find(|s| s.name == "hub").unwrap().id;
    let sat1 = stored.iter().find(|s| s.name == "sat1").unwrap().id;
    let sat2 = stored.iter().find(|s| s.name == "sat2").unwrap().id;
    let sat3 = stored.iter().find(|s| s.name == "sat3").unwrap().id;

    add_edge(&store, hub, sat1, EdgeKind::Calls);
    add_edge(&store, hub, sat2, EdgeKind::Calls);
    add_edge(&store, hub, sat3, EdgeKind::Calls);

    let communities = detect_communities(&store).unwrap();
    assert_eq!(communities.len(), 1);
    let label = communities[0].label.as_ref().expect("label should be set");
    assert!(
        label.contains("hub"),
        "label should contain 'hub', got: {label}"
    );
}

#[test]
fn test_detect_communities_sorted_by_size_descending() {
    // Build a large community (5 nodes) and a small one (2 nodes).
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let mut symbols = vec![];
    for i in 1..=5 {
        symbols.push(make_symbol(
            &format!("big{i}"),
            SymbolKind::Function,
            file_id,
            i * 2,
            i * 2 + 1,
        ));
    }
    for i in 1..=2 {
        symbols.push(make_symbol(
            &format!("small{i}"),
            SymbolKind::Function,
            file_id,
            20 + i * 2,
            20 + i * 2 + 1,
        ));
    }
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();
    let get = |n: &str| stored.iter().find(|s| s.name == n).unwrap().id;

    // Fully connect the "big" group (clique of 5).
    for i in 1..=5 {
        for j in (i + 1)..=5 {
            add_edge(
                &store,
                get(&format!("big{i}")),
                get(&format!("big{j}")),
                EdgeKind::Calls,
            );
        }
    }
    // Connect the "small" group (clique of 2).
    add_edge(&store, get("small1"), get("small2"), EdgeKind::Calls);

    let communities = detect_communities(&store).unwrap();
    assert_eq!(communities.len(), 2);
    assert!(
        communities[0].member_count >= communities[1].member_count,
        "communities should be sorted by size descending"
    );
}

// ── list_communities ────────────────────────────────────────────────────

#[test]
fn test_list_empty_when_no_detection() {
    let store = IndexStore::open_in_memory().unwrap();
    let communities = list_communities(&store).unwrap();
    assert!(communities.is_empty());
}

#[test]
fn test_list_returns_persisted_communities() {
    let (store, _a1, _b1) = build_two_cliques();
    detect_communities(&store).unwrap();

    let communities = list_communities(&store).unwrap();
    assert_eq!(communities.len(), 2);
    let total: usize = communities.iter().map(|c| c.member_count).sum();
    assert_eq!(total, 6);
}

#[test]
fn test_list_returns_labels() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("hub", SymbolKind::Function, file_id, 1, 2),
        make_symbol("sat", SymbolKind::Function, file_id, 3, 4),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();
    let hub = stored.iter().find(|s| s.name == "hub").unwrap().id;
    let sat = stored.iter().find(|s| s.name == "sat").unwrap().id;
    add_edge(&store, hub, sat, EdgeKind::Calls);

    detect_communities(&store).unwrap();
    let communities = list_communities(&store).unwrap();
    assert_eq!(communities.len(), 1);
    assert!(communities[0].label.is_some());
}

// ── SymbolGraph::communities ─────────────────────────────────────────────

#[test]
fn test_symbol_graph_communities_runs_detection() {
    let (store, _a1, _b1) = build_two_cliques();
    let graph = SymbolGraph::new(&store);
    let communities = graph.communities().unwrap();
    assert_eq!(communities.len(), 2);
}

#[test]
fn test_symbol_graph_communities_empty_on_empty_graph() {
    let store = IndexStore::open_in_memory().unwrap();
    let graph = SymbolGraph::new(&store);
    let communities = graph.communities().unwrap();
    assert!(communities.is_empty());
}

// ── Convergence and determinism ──────────────────────────────────────────

#[test]
fn test_detect_is_deterministic() {
    let (store, _, _) = build_two_cliques();
    let first = detect_communities(&store).unwrap();
    // Run again (clears + re-detects).
    let second = detect_communities(&store).unwrap();
    assert_eq!(first.len(), second.len());
    for (a, b) in first.iter().zip(second.iter()) {
        assert_eq!(a.member_count, b.member_count);
    }
}

#[test]
fn test_connected_graph_one_community() {
    // A chain: a -> b -> c -> d  (all connected via calls).
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("a", SymbolKind::Function, file_id, 1, 2),
        make_symbol("b", SymbolKind::Function, file_id, 3, 4),
        make_symbol("c", SymbolKind::Function, file_id, 5, 6),
        make_symbol("d", SymbolKind::Function, file_id, 7, 8),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();
    let get = |n: &str| stored.iter().find(|s| s.name == n).unwrap().id;

    add_edge(&store, get("a"), get("b"), EdgeKind::Calls);
    add_edge(&store, get("b"), get("c"), EdgeKind::Calls);
    add_edge(&store, get("c"), get("d"), EdgeKind::Calls);

    let communities = detect_communities(&store).unwrap();
    assert_eq!(communities.len(), 1, "connected graph → 1 community");
    assert_eq!(communities[0].member_count, 4);
}
