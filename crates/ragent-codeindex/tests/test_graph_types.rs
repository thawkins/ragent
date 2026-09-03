#![allow(clippy::assert_is_empty)]
//! Tests for `GraphEdge`, `EdgeKind`, and `Confidence` types (spec graphCI, T-002).

use ragent_codeindex::store::IndexStore;
use ragent_codeindex::types::{Confidence, EdgeKind, GraphEdge};
use std::str::FromStr;

// ── EdgeKind Display / FromStr round-trip ───────────────────────────────────

#[test]
fn test_edge_kind_display() {
    assert_eq!(EdgeKind::Calls.to_string(), "calls");
    assert_eq!(EdgeKind::Imports.to_string(), "imports");
    assert_eq!(EdgeKind::Inherits.to_string(), "inherits");
    assert_eq!(EdgeKind::References.to_string(), "references");
    assert_eq!(EdgeKind::MixesIn.to_string(), "mixes_in");
    assert_eq!(EdgeKind::Implements.to_string(), "implements");
}

#[test]
fn test_edge_kind_from_str() {
    assert_eq!(EdgeKind::from_str("calls").unwrap(), EdgeKind::Calls);
    assert_eq!(EdgeKind::from_str("imports").unwrap(), EdgeKind::Imports);
    assert_eq!(EdgeKind::from_str("inherits").unwrap(), EdgeKind::Inherits);
    assert_eq!(
        EdgeKind::from_str("references").unwrap(),
        EdgeKind::References
    );
    assert_eq!(EdgeKind::from_str("mixes_in").unwrap(), EdgeKind::MixesIn);
    assert_eq!(
        EdgeKind::from_str("implements").unwrap(),
        EdgeKind::Implements
    );
}

#[test]
fn test_edge_kind_from_str_invalid() {
    assert!(EdgeKind::from_str("unknown").is_err());
    assert!(EdgeKind::from_str("").is_err());
    assert!(EdgeKind::from_str("CALLS").is_err()); // case-sensitive
}

#[test]
fn test_edge_kind_round_trip() {
    for kind in [
        EdgeKind::Calls,
        EdgeKind::Imports,
        EdgeKind::Inherits,
        EdgeKind::References,
        EdgeKind::MixesIn,
        EdgeKind::Implements,
    ] {
        let s = kind.to_string();
        assert_eq!(EdgeKind::from_str(&s).unwrap(), kind);
    }
}

// ── Confidence Display / FromStr round-trip ─────────────────────────────────

#[test]
fn test_confidence_display() {
    assert_eq!(Confidence::Extracted.to_string(), "EXTRACTED");
    assert_eq!(Confidence::Inferred.to_string(), "INFERRED");
}

#[test]
fn test_confidence_from_str() {
    assert_eq!(
        Confidence::from_str("EXTRACTED").unwrap(),
        Confidence::Extracted
    );
    assert_eq!(
        Confidence::from_str("INFERRED").unwrap(),
        Confidence::Inferred
    );
}

#[test]
fn test_confidence_from_str_invalid() {
    assert!(Confidence::from_str("extracted").is_err()); // case-sensitive
    assert!(Confidence::from_str("inferred").is_err());
    assert!(Confidence::from_str("").is_err());
}

#[test]
fn test_confidence_round_trip() {
    for conf in [Confidence::Extracted, Confidence::Inferred] {
        let s = conf.to_string();
        assert_eq!(Confidence::from_str(&s).unwrap(), conf);
    }
}

// ── GraphEdge struct ────────────────────────────────────────────────────────

#[test]
fn test_graph_edge_construction() {
    let edge = GraphEdge {
        source_sym: 1,
        target_sym: 2,
        kind: EdgeKind::Calls,
        confidence: Confidence::Extracted,
        source_file: Some(10),
        line: Some(42),
    };
    assert_eq!(edge.source_sym, 1);
    assert_eq!(edge.target_sym, 2);
    assert_eq!(edge.kind, EdgeKind::Calls);
    assert_eq!(edge.confidence, Confidence::Extracted);
    assert_eq!(edge.source_file, Some(10));
    assert_eq!(edge.line, Some(42));
}

#[test]
fn test_graph_edge_equality() {
    let e1 = GraphEdge {
        source_sym: 1,
        target_sym: 2,
        kind: EdgeKind::Imports,
        confidence: Confidence::Inferred,
        source_file: None,
        line: None,
    };
    let e2 = GraphEdge {
        source_sym: 1,
        target_sym: 2,
        kind: EdgeKind::Imports,
        confidence: Confidence::Inferred,
        source_file: None,
        line: None,
    };
    assert_eq!(e1, e2);
}

#[test]
fn test_graph_edge_inequality() {
    let e1 = GraphEdge {
        source_sym: 1,
        target_sym: 2,
        kind: EdgeKind::Calls,
        confidence: Confidence::Extracted,
        source_file: None,
        line: None,
    };
    let e2 = GraphEdge {
        source_sym: 1,
        target_sym: 3,
        kind: EdgeKind::Calls,
        confidence: Confidence::Extracted,
        source_file: None,
        line: None,
    };
    assert_ne!(e1, e2);
}

#[test]
fn test_graph_edge_serde() {
    let edge = GraphEdge {
        source_sym: 1,
        target_sym: 2,
        kind: EdgeKind::Calls,
        confidence: Confidence::Extracted,
        source_file: Some(10),
        line: Some(42),
    };
    let json = serde_json::to_string(&edge).unwrap();
    let deserialized: GraphEdge = serde_json::from_str(&json).unwrap();
    assert_eq!(edge, deserialized);
}

// ── Store typed methods ───────────────────────────────────��─────────────────

use chrono::Utc;
use ragent_codeindex::types::{FileEntry, Symbol, SymbolKind, Visibility};

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
fn test_upsert_edge_typed() {
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

    assert_eq!(store.edge_count().unwrap(), 1);
    assert_eq!(
        store
            .edge_count_by_confidence_typed(Confidence::Extracted)
            .unwrap(),
        1
    );
}

#[test]
fn test_query_all_edges_typed() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let symbols = vec![
        make_symbol("a", SymbolKind::Function, 0, file_id),
        make_symbol("b", SymbolKind::Function, 0, file_id),
        make_symbol("c", SymbolKind::Struct, 0, file_id),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let a_id = stored.iter().find(|s| s.name == "a").unwrap().id;
    let b_id = stored.iter().find(|s| s.name == "b").unwrap().id;
    let c_id = stored.iter().find(|s| s.name == "c").unwrap().id;

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
    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: b_id,
            target_sym: c_id,
            kind: EdgeKind::References,
            confidence: Confidence::Inferred,
            source_file: None,
            line: None,
        })
        .unwrap();

    let edges = store.query_all_edges_typed().unwrap();
    assert_eq!(edges.len(), 2);
    assert!(edges.iter().any(|e| e.kind == EdgeKind::Calls));
    assert!(edges.iter().any(|e| e.kind == EdgeKind::References));
}

#[test]
fn test_query_edges_for_symbol_typed() {
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

    store
        .upsert_edge_typed(&GraphEdge {
            source_sym: caller_id,
            target_sym: callee_id,
            kind: EdgeKind::Calls,
            confidence: Confidence::Inferred,
            source_file: Some(file_id),
            line: Some(7),
        })
        .unwrap();

    let edges = store.query_edges_for_symbol_typed(caller_id).unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].source_sym, caller_id);
    assert_eq!(edges[0].target_sym, callee_id);
    assert_eq!(edges[0].kind, EdgeKind::Calls);
    assert_eq!(edges[0].confidence, Confidence::Inferred);
    assert_eq!(edges[0].source_file, Some(file_id));
    assert_eq!(edges[0].line, Some(7));
}
