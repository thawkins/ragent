//! Verification test: existing SQLite tables are read-only by the graph layer
//! (spec graphCI, T-030, FR-025).
//!
//! FR-025: The graph layer shall not modify any existing SQLite tables
//! (`symbols`, `indexed_files`, `symbol_refs`, `file_deps`, `schema_version`);
//! all graph writes go to `graph_edges` and `communities` tables only.
//!
//! This test sets up an in-memory store with data in all existing tables,
//! snapshots their contents, runs the full graph layer (edge derivation +
//! community detection + graph queries), then verifies that no existing
//! table was modified.

use chrono::Utc;
use ragent_codeindex::graph::SymbolGraph;
use ragent_codeindex::store::IndexStore;
use ragent_codeindex::types::{
    Confidence, EdgeKind, FileEntry, GraphEdge, ImportEntry, Symbol, SymbolFilter, SymbolKind,
    SymbolRef, Visibility,
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

/// Build a store with:
///  - 1 file in `indexed_files`
///  - 4 symbols in `symbols`
///  - 1 import in `imports`
///  - 1 reference in `symbol_refs`
///  - 1 file dependency in `file_deps`
///  - 3 edges in `graph_edges`
///  - 1 community assignment in `communities`
fn build_store() -> IndexStore {
    let store = IndexStore::open_in_memory().unwrap();

    // ── indexed_files ──
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    // ── symbols ──
    let symbols = vec![
        make_symbol("hub", SymbolKind::Function, file_id, 1, 5),
        make_symbol("sat1", SymbolKind::Function, file_id, 6, 10),
        make_symbol("sat2", SymbolKind::Function, file_id, 11, 15),
        make_symbol("lonely", SymbolKind::Function, file_id, 16, 20),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();
    let hub = stored.iter().find(|s| s.name == "hub").unwrap().id;
    let sat1 = stored.iter().find(|s| s.name == "sat1").unwrap().id;
    let sat2 = stored.iter().find(|s| s.name == "sat2").unwrap().id;

    // ── imports ──
    store
        .upsert_imports(
            file_id,
            &[ImportEntry {
                file_id,
                imported_name: "sat1".to_string(),
                source_module: "std::collections".to_string(),
                alias: None,
                line: 1,
                kind: "use".to_string(),
            }],
        )
        .unwrap();

    // ── symbol_refs ──
    store
        .upsert_refs(
            file_id,
            &[SymbolRef {
                symbol_name: "sat2".to_string(),
                file_id,
                file_path: "a.rs".to_string(),
                line: 3,
                col: 0,
                kind: "call".to_string(),
            }],
        )
        .unwrap();

    // ── file_deps ──
    store
        .set_file_deps(file_id, &[("b.rs".to_string(), "use".to_string())])
        .unwrap();

    // ── graph_edges ──
    for (src, tgt) in [(hub, sat1), (hub, sat2)] {
        store
            .upsert_edge_typed(&GraphEdge {
                source_sym: src,
                target_sym: tgt,
                kind: EdgeKind::Calls,
                confidence: Confidence::Extracted,
                source_file: Some(file_id),
                line: Some(2),
            })
            .unwrap();
    }

    // ── communities ──
    store.upsert_community(hub, 0, Some("core")).unwrap();

    store
}

// ── Helper: snapshot existing-table state ──────────────────────────────

struct TableSnapshot {
    file_count: u64,
    symbol_count: u64,
    symbols: Vec<(i64, String, String)>, // (id, name, kind)
    ref_count: u64,
    file_deps: Vec<(String, String)>, // (target_path, kind)
    schema_version: i64,
    // graph tables — NOT part of the "existing tables" check, but useful
    edge_count: u64,
    community_count: u64,
}

fn snapshot(store: &IndexStore) -> TableSnapshot {
    let symbols = store
        .query_symbols(&SymbolFilter::default())
        .unwrap()
        .into_iter()
        .map(|s| (s.id, s.name.clone(), s.kind.to_string()))
        .collect();

    let file_deps = store
        .get_file_deps(store.get_file_id("a.rs").unwrap().unwrap_or(0))
        .unwrap_or_default();

    TableSnapshot {
        file_count: store.file_count().unwrap(),
        symbol_count: store.symbol_count().unwrap(),
        symbols,
        ref_count: store.reference_count().unwrap(),
        file_deps,
        schema_version: store.schema_version().unwrap(),
        edge_count: store.edge_count().unwrap(),
        community_count: store.community_count().unwrap(),
    }
}

// ── Tests ──────────────────────────────────────────────────────────────

#[test]
fn test_graph_build_does_not_modify_existing_tables() {
    let store = build_store();
    let before = snapshot(&store);

    // Run the full graph build (writes to graph_edges).
    let graph = SymbolGraph::new(&store);
    let _ = graph.build().unwrap();

    let after = snapshot(&store);

    // Existing tables must be unchanged.
    assert_eq!(
        after.file_count, before.file_count,
        "indexed_files unchanged"
    );
    assert_eq!(
        after.symbol_count, before.symbol_count,
        "symbols count unchanged"
    );
    assert_eq!(after.symbols, before.symbols, "symbols content unchanged");
    assert_eq!(
        after.ref_count, before.ref_count,
        "symbol_refs count unchanged"
    );
    assert_eq!(
        after.schema_version, before.schema_version,
        "schema_version unchanged"
    );
    assert_eq!(after.file_deps, before.file_deps, "file_deps unchanged");

    // graph_edges MAY change (that's the whole point of graph build).
    // communities MAY change (community detection clears + rewrites).
}

#[test]
fn test_community_detection_does_not_modify_existing_tables() {
    let store = build_store();
    let before = snapshot(&store);

    // Run community detection (writes to communities).
    let graph = SymbolGraph::new(&store);
    let _ = graph.communities().unwrap();

    let after = snapshot(&store);

    assert_eq!(
        after.file_count, before.file_count,
        "indexed_files unchanged"
    );
    assert_eq!(
        after.symbol_count, before.symbol_count,
        "symbols count unchanged"
    );
    assert_eq!(after.symbols, before.symbols, "symbols content unchanged");
    assert_eq!(
        after.ref_count, before.ref_count,
        "symbol_refs count unchanged"
    );
    assert_eq!(
        after.schema_version, before.schema_version,
        "schema_version unchanged"
    );
    assert_eq!(
        after.edge_count, before.edge_count,
        "graph_edges unchanged by community detection"
    );
    assert_eq!(
        after.file_deps, before.file_deps,
        "file_deps unchanged by community detection"
    );
}

#[test]
fn test_explain_does_not_modify_any_table() {
    let store = build_store();
    let before = snapshot(&store);

    let graph = SymbolGraph::new(&store);
    let _ = graph.explain("hub").unwrap();

    let after = snapshot(&store);

    // Explain is read-only — nothing should change.
    assert_eq!(after.file_count, before.file_count);
    assert_eq!(after.symbol_count, before.symbol_count);
    assert_eq!(after.ref_count, before.ref_count);
    assert_eq!(after.schema_version, before.schema_version);
    assert_eq!(after.edge_count, before.edge_count);
    assert_eq!(after.community_count, before.community_count);
}

#[test]
fn test_shortest_path_does_not_modify_any_table() {
    let store = build_store();
    let before = snapshot(&store);

    let graph = SymbolGraph::new(&store);
    let _ = graph.path("hub", "sat1").unwrap();

    let after = snapshot(&store);

    assert_eq!(after.file_count, before.file_count);
    assert_eq!(after.symbol_count, before.symbol_count);
    assert_eq!(after.ref_count, before.ref_count);
    assert_eq!(after.edge_count, before.edge_count);
    assert_eq!(after.community_count, before.community_count);
}

#[test]
fn test_godnodes_does_not_modify_any_table() {
    let store = build_store();
    let before = snapshot(&store);

    let graph = SymbolGraph::new(&store);
    let _ = graph.godnodes(10).unwrap();

    let after = snapshot(&store);

    assert_eq!(after.file_count, before.file_count);
    assert_eq!(after.symbol_count, before.symbol_count);
    assert_eq!(after.ref_count, before.ref_count);
    assert_eq!(after.edge_count, before.edge_count);
    assert_eq!(after.community_count, before.community_count);
}

#[test]
fn test_graph_layer_only_writes_to_graph_edges_and_communities() {
    // This is the definitive FR-025 test: run ALL graph write operations
    // (build + community detection) and verify that only graph_edges and
    // communities tables changed.
    let store = build_store();
    let before = snapshot(&store);

    let graph = SymbolGraph::new(&store);

    // Run both write operations.
    let _ = graph.build().unwrap();
    let _ = graph.communities().unwrap();

    let after = snapshot(&store);

    // ── Existing tables: MUST be unchanged ──────────────────────────
    assert_eq!(after.file_count, before.file_count, "indexed_files");
    assert_eq!(after.symbol_count, before.symbol_count, "symbols count");
    assert_eq!(after.symbols, before.symbols, "symbols content");
    assert_eq!(after.ref_count, before.ref_count, "symbol_refs");
    assert_eq!(
        after.schema_version, before.schema_version,
        "schema_version"
    );

    // ── Graph tables: MAY change ────────────────────────────────────
    // graph_edges should have edges (build re-derives them).
    assert!(
        after.edge_count > 0,
        "graph_edges should have edges after build"
    );
    // communities should have assignments after detection.
    assert!(
        after.community_count > 0,
        "communities should have assignments after detection"
    );
}

#[test]
fn test_file_deps_unchanged_by_graph_layer() {
    let store = build_store();
    let file_id = store.get_file_id("a.rs").unwrap().unwrap();

    let deps_before = store.get_file_deps(file_id).unwrap();
    assert!(!deps_before.is_empty(), "precondition: file_deps has data");

    let graph = SymbolGraph::new(&store);
    let _ = graph.build().unwrap();
    let _ = graph.communities().unwrap();

    let deps_after = store.get_file_deps(file_id).unwrap();
    assert_eq!(
        deps_before, deps_after,
        "file_deps must not be modified by graph layer"
    );
}
