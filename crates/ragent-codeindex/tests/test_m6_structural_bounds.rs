//! ANTIPAT M6.4 structural/complexity-debt regression tests.
//!
//! Pins the observable contracts of the M6.4 changes:
//!
//! - 4.1 - the graph-derivation full-table loaders return every row of a
//!   normal-sized index (the `MAX_GRAPH_LOAD_ROWS` cap never truncates real
//!   output) and the named cap is exposed.
//! - 4.2 - `derive_impl_edges` keeps every candidate below
//!   `MAX_IMPL_CANDIDATES` and bounds the fan-out above it.
//! - 4.4 - a pathologically deep parse tree still parses without exhausting
//!   the stack, and the `MAX_TREE_DEPTH` guard is exposed.
//! - 4.6 - `IndexStore::rollback_transaction` is a proper transactional API:
//!   it discards an open transaction and tolerates being called with none
//!   active (no panic), so the raw `conn` escape hatch could be made private.

use chrono::Utc;
use ragent_codeindex::graph::edges::{derive_edges_from_inputs, load_graph_inputs};
use ragent_codeindex::parser::LanguageParser;
use ragent_codeindex::parser::rust::RustParser;
use ragent_codeindex::parser::util::MAX_TREE_DEPTH;
use ragent_codeindex::store::{IndexStore, MAX_GRAPH_LOAD_ROWS};
use ragent_codeindex::types::{
    EdgeKind, FileEntry, ImportEntry, Symbol, SymbolFilter, SymbolKind, SymbolRef, Visibility,
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

fn make_symbol(name: &str, kind: SymbolKind, file_id: i64, temp_id: i64) -> Symbol {
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
        signature: None,
        doc_comment: None,
        body_hash: Some("hash".to_string()),
    }
}

/// 4.1: the named cap exists and is a sane, large bound.
#[test]
fn test_max_graph_load_rows_is_exposed_and_large() {
    assert!(
        std::hint::black_box(MAX_GRAPH_LOAD_ROWS) >= 100_000,
        "cap must be large enough not to truncate a realistic index"
    );
}

/// 4.1: a full-table load returns every row of a normal-sized index - the cap
/// only truncates a pathologically large index.
#[test]
fn test_graph_full_table_loads_return_all_normal_rows() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let symbols: Vec<Symbol> = (0..50)
        .map(|i| make_symbol(&format!("fn_{i}"), SymbolKind::Function, file_id, i))
        .collect();
    store.upsert_symbols(file_id, &symbols).unwrap();

    let imports: Vec<ImportEntry> = (0..40)
        .map(|i| ImportEntry {
            file_id: 0,
            imported_name: format!("mod_{i}"),
            source_module: "crate".to_string(),
            alias: None,
            line: i as u32 + 1,
            kind: "use".to_string(),
        })
        .collect();
    store.upsert_imports(file_id, &imports).unwrap();

    let refs: Vec<SymbolRef> = (0..30)
        .map(|i| SymbolRef {
            symbol_name: format!("ref_{i}"),
            file_id: 0,
            file_path: String::new(),
            line: i as u32 + 1,
            col: 0,
            kind: "call".to_string(),
        })
        .collect();
    store.upsert_refs(file_id, &refs).unwrap();

    assert_eq!(
        store.query_symbols(&SymbolFilter::default()).unwrap().len(),
        50
    );
    assert_eq!(store.list_all_imports().unwrap().len(), 40);
    assert_eq!(store.query_all_refs().unwrap().len(), 30);
}

/// 4.1: the graph edge scan returns every persisted edge for a normal index.
#[test]
fn test_query_all_edges_returns_all_normal_rows() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("a", SymbolKind::Function, file_id, 0),
        make_symbol("b", SymbolKind::Function, file_id, 0),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();
    let a_id = stored.iter().find(|s| s.name == "a").unwrap().id;
    let b_id = stored.iter().find(|s| s.name == "b").unwrap().id;

    store
        .upsert_edge_typed(&ragent_codeindex::types::GraphEdge {
            source_sym: a_id,
            target_sym: b_id,
            kind: EdgeKind::Calls,
            confidence: ragent_codeindex::types::Confidence::Extracted,
            source_file: Some(file_id),
            line: Some(2),
        })
        .unwrap();

    assert_eq!(store.query_all_edges().unwrap().len(), 1);
    assert_eq!(store.query_all_edges_typed().unwrap().len(), 1);
}

/// 4.2: with a candidate count below the cap, every `Inherits` edge is still
/// produced (below-cap behaviour is unchanged).
#[test]
fn test_derive_impl_edges_keeps_all_candidates_below_cap() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    // One impl named `Foo` plus 10 other symbols named `Foo` (well below the
    // 32-candidate cap).
    let mut symbols = vec![make_symbol("Foo", SymbolKind::Impl, file_id, 0)];
    for i in 0..10 {
        symbols.push(make_symbol("Foo", SymbolKind::Function, file_id, i + 1));
    }
    store.upsert_symbols(file_id, &symbols).unwrap();

    let inputs = load_graph_inputs(&store).unwrap();
    let edges = derive_edges_from_inputs(&inputs, None);
    let inherits = edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Inherits)
        .count();
    assert_eq!(
        inherits, 10,
        "every non-self candidate must yield an Inherits edge below the cap"
    );
}

/// 4.2: a candidate fan-out above the cap is bounded to at most
/// `MAX_IMPL_CANDIDATES` (`MAX_CANDIDATES_PER_REF`) edges.
#[test]
fn test_derive_impl_edges_bounds_candidates_above_cap() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    // One impl named `Bar` plus 200 other symbols named `Bar` - a common-name
    // fan-out that previously produced an unbounded pair set.
    let mut symbols = vec![make_symbol("Bar", SymbolKind::Impl, file_id, 0)];
    for i in 0..200 {
        symbols.push(make_symbol("Bar", SymbolKind::Function, file_id, i + 1));
    }
    store.upsert_symbols(file_id, &symbols).unwrap();

    let inputs = load_graph_inputs(&store).unwrap();
    let edges = derive_edges_from_inputs(&inputs, None);
    let inherits = edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Inherits)
        .count();
    // Bounded by the shared per-reference candidate cap (32), not the raw 200.
    assert!(
        inherits <= 32,
        "impl fan-out must be capped at MAX_IMPL_CANDIDATES, got {inherits}"
    );
    assert!(
        inherits > 0,
        "some Inherits edges must still be produced under the cap"
    );
}

/// 4.4: the depth guard constant is exposed and is a sane, non-trivial bound.
#[test]
fn test_max_tree_depth_is_exposed() {
    assert!(
        std::hint::black_box(MAX_TREE_DEPTH) >= 128,
        "depth cap must be deep enough for real source"
    );
}

/// 4.4: a pathologically deep parse tree parses and walks without exhausting
/// the stack (the guard stops descent past `MAX_TREE_DEPTH`).
#[test]
fn test_deeply_nested_source_parses_without_stack_overflow() {
    let depth = 1500usize; // > MAX_TREE_DEPTH
    let mut source = String::from("fn deep() {");
    source.push_str(&"{".repeat(depth));
    source.push_str(&"}".repeat(depth));
    source.push('}');

    let parser = RustParser::new();
    let parsed = parser
        .parse(source.as_bytes())
        .expect("deeply nested source must still parse");
    // The function itself is shallow and still extracted.
    assert!(parsed.symbols.iter().any(|s| s.name == "deep"));
}

/// 4.4: normal source is unaffected by the depth guard.
#[test]
fn test_shallow_source_parses_normally() {
    let parser = RustParser::new();
    let parsed = parser
        .parse(b"fn a() { b(); }\nfn b() {}")
        .expect("shallow source must parse");
    assert_eq!(
        parsed
            .symbols
            .iter()
            .filter(|s| s.kind == SymbolKind::Function)
            .count(),
        2
    );
}

/// 4.6: `rollback_transaction` discards an open transaction's writes.
#[test]
fn test_rollback_transaction_discards_open_transaction() {
    let store = IndexStore::open_in_memory().unwrap();
    assert_eq!(store.file_count().unwrap(), 0);

    store.begin_transaction().unwrap();
    store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    store.rollback_transaction();

    assert_eq!(
        store.file_count().unwrap(),
        0,
        "rollback must discard the uncommitted insert"
    );
}

/// 4.6: calling `rollback_transaction` with no open transaction logs (never
/// panics), so the graph call sites can use it unconditionally.
#[test]
fn test_rollback_transaction_without_open_transaction_does_not_panic() {
    let store = IndexStore::open_in_memory().unwrap();
    // No `begin_transaction` - SQLite reports "no transaction is active".
    // The method must swallow-and-log rather than panic or propagate.
    store.rollback_transaction();
    // Store is still usable afterwards.
    assert_eq!(store.file_count().unwrap(), 0);
}
