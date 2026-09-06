#![allow(clippy::unwrap_used)]
//! Regression tests for graph name resolution (codeindex_path / explain).
//!
//! `query_symbols` performs a case-insensitive substring match, so a name like
//! `SessionProcessor` also matches `CachedSessionProcessor`. The resolver must
//! prefer exact matches and definition kinds over impl/trait container nodes,
//! otherwise the graph traversal starts from a near-zero-edge node and reports
//! "No path found" for well-connected symbols.

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
        signature: None,
        doc_comment: None,
        body_hash: None,
    }
}

/// Chain: Start -> Mid -> End, plus a substring-shadowing trait
/// `CachedStart` that must NOT be chosen for `Start`.
#[test]
fn test_path_exact_definition_beats_substring_trait() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let symbols = vec![
        make_symbol("CachedStart", SymbolKind::Trait, file_id, 40, 45),
        make_symbol("Start", SymbolKind::Struct, file_id, 1, 5),
        make_symbol("Mid", SymbolKind::Function, file_id, 7, 10),
        make_symbol("End", SymbolKind::Struct, file_id, 12, 15),
        make_symbol("Default for End", SymbolKind::Impl, file_id, 17, 20),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let by_name = |n: &str| stored.iter().find(|s| s.name == n).unwrap().id;

    for (src, tgt) in [("Start", "Mid"), ("Mid", "End")] {
        store
            .upsert_edge_typed(&GraphEdge {
                source_sym: by_name(src),
                target_sym: by_name(tgt),
                kind: EdgeKind::Calls,
                confidence: Confidence::Extracted,
                source_file: Some(file_id),
                line: Some(2),
            })
            .unwrap();
    }

    let graph = SymbolGraph::new(&store);
    let result = graph.path("Start", "End").unwrap().unwrap();

    assert_eq!(result.hops, 2, "expected Start -> Mid -> End");
    assert_eq!(result.steps[0].0, "Start");
    assert_eq!(result.steps[2].0, "End");
}

/// Impl nodes named `Default for X` must never shadow the definition `X`.
#[test]
fn test_path_impl_name_does_not_shadow_definition() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h2")).unwrap();

    let symbols = vec![
        make_symbol("Hub", SymbolKind::Struct, file_id, 1, 5),
        make_symbol("Default for Hub", SymbolKind::Impl, file_id, 7, 10),
        make_symbol("Leaf", SymbolKind::Function, file_id, 12, 15),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let by_name = |n: &str| stored.iter().find(|s| s.name == n).unwrap().id;

    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: by_name("Hub"),
            target_sym: by_name("Leaf"),
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: Some(file_id),
            line: Some(2),
        })
        .unwrap();

    let graph = SymbolGraph::new(&store);
    // Without definition-first ranking this resolves to the impl node, which
    // has no outgoing edges -> "No path found".
    let result = graph.path("Hub", "Leaf").unwrap().unwrap();
    assert_eq!(result.hops, 1);
    assert_eq!(result.steps[0].0, "Hub");
}

/// Exact matches are preferred over substring matches even when the substring
/// match has a "better" kind rank.
#[test]
fn test_path_exact_name_wins_over_substring_struct() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h3")).unwrap();

    let symbols = vec![
        make_symbol("Config", SymbolKind::Struct, file_id, 12, 15),
        make_symbol("AppConfig", SymbolKind::Struct, file_id, 1, 5),
        make_symbol("Sink", SymbolKind::Function, file_id, 17, 20),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let by_name = |n: &str| stored.iter().find(|s| s.name == n).unwrap().id;

    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: by_name("Config"),
            target_sym: by_name("Sink"),
            kind: EdgeKind::Calls,
            confidence: Confidence::Extracted,
            source_file: Some(file_id),
            line: Some(13),
        })
        .unwrap();

    let graph = SymbolGraph::new(&store);
    let result = graph.path("Config", "Sink").unwrap().unwrap();
    assert_eq!(result.hops, 1);
    assert_eq!(result.steps[0].0, "Config", "exact match must win");
}
