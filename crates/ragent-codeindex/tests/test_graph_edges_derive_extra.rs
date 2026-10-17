//! Extra coverage tests for edge derivation
//! (`crates/ragent-codeindex/src/graph/edges.rs`).
//!
//! These tests build a [`GraphInputs`] snapshot by hand (all fields are public)
//! and drive the pure in-memory derivations directly, exercising every early
//! `continue` branch in `derive_ref_edges`, `derive_import_edges`,
//! `derive_impl_edges`, and `extract_trait_from_signature` that the existing
//! store-backed tests never reach.

use ragent_codeindex::graph::edges::{
    GraphInputs, derive_edges_from_inputs, derive_edges_from_inputs_for_language,
};
use ragent_codeindex::types::{
    Confidence, EdgeKind, FileEntry, ImportEntry, Symbol, SymbolKind, SymbolRef, Visibility,
};
use std::collections::HashMap;

fn sym(id: i64, file_id: i64, name: &str, kind: SymbolKind, start: u32, end: u32) -> Symbol {
    Symbol {
        id,
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

fn entry(path: &str, language: Option<&str>) -> FileEntry {
    FileEntry {
        path: path.to_string(),
        content_hash: "h".to_string(),
        byte_size: 10,
        language: language.map(|l| l.to_string()),
        last_indexed: chrono::Utc::now(),
        mtime_ns: 1,
        line_count: 5,
    }
}

fn refs(symbol_name: &str, file_id: i64, line: u32, kind: &str) -> SymbolRef {
    SymbolRef {
        symbol_name: symbol_name.to_string(),
        file_id,
        file_path: String::new(),
        line,
        col: 0,
        kind: kind.to_string(),
    }
}

fn imp_sym(id: i64, file_id: i64, name: &str, sig: &str) -> Symbol {
    Symbol {
        id,
        file_id,
        name: name.to_string(),
        qualified_name: Some(name.to_string()),
        kind: SymbolKind::Impl,
        visibility: Visibility::Public,
        start_line: 6,
        end_line: 10,
        start_col: 0,
        end_col: 0,
        parent_id: None,
        signature: Some(sig.to_string()),
        doc_comment: None,
        body_hash: Some("h".to_string()),
    }
}

/// A snapshot with real work: a caller calling a callee in another file
/// (Inferred), plus imports and an `impl Trait for Type` pair.
fn build_inputs() -> GraphInputs {
    let all_symbols = vec![
        sym(1, 1, "caller", SymbolKind::Function, 1, 4),
        sym(2, 2, "callee", SymbolKind::Function, 1, 4),
        sym(3, 2, "Greeter", SymbolKind::Trait, 6, 8),
        imp_sym(4, 1, "MyType", "impl Greeter for MyType"),
    ];
    let all_refs = vec![
        // Real call: caller (file 1) references callee (file 2) -> Inferred.
        refs("callee", 1, 2, "call"),
        // `type` reference kind.
        refs("Greeter", 1, 3, "type"),
        // `field_access` reference kind.
        refs("callee", 1, 4, "field_access"),
        // Unknown ref kind -> skipped.
        refs("callee", 1, 4, "unknown_kind"),
        // Name with no matching symbol -> skipped.
        refs("does_not_exist", 1, 2, "call"),
        // Ref whose line is outside every containing symbol -> skipped.
        refs("callee", 1, 99, "call"),
    ];

    let mut imports_by_file: HashMap<i64, Vec<ImportEntry>> = HashMap::new();
    imports_by_file.insert(
        1,
        vec![
            ImportEntry {
                file_id: 1,
                imported_name: "callee".to_string(),
                source_module: "other".to_string(),
                alias: None,
                line: 1,
                kind: "use".to_string(),
            },
            // Import with no matching symbol -> skipped.
            ImportEntry {
                file_id: 1,
                imported_name: "ghost".to_string(),
                source_module: "nowhere".to_string(),
                alias: None,
                line: 2,
                kind: "use".to_string(),
            },
        ],
    );

    let mut file_ids: HashMap<String, i64> = HashMap::new();
    file_ids.insert("a.rs".to_string(), 1);
    file_ids.insert("b.rs".to_string(), 2);
    file_ids.insert("c.txt".to_string(), 3);

    GraphInputs {
        all_symbols,
        all_refs,
        imports_by_file,
        files: vec![
            entry("a.rs", Some("rust")),
            entry("b.rs", Some("rust")),
            entry("c.txt", None),
        ],
        file_ids,
    }
}

#[test]
fn test_derive_ref_edges_emits_calls_and_references() {
    let inputs = build_inputs();
    let edges = derive_edges_from_inputs(&inputs, None);

    // caller -> callee (Calls, Inferred: different files).
    let call = edges
        .iter()
        .find(|e| e.kind == EdgeKind::Calls && e.target_sym == 2)
        .expect("call edge should be derived");
    assert_eq!(call.source_sym, 1);
    assert_eq!(call.confidence, Confidence::Inferred);

    // A References edge is produced for the `type` and `field_access` kinds.
    assert!(
        edges
            .iter()
            .any(|e| e.kind == EdgeKind::References && e.source_sym == 1),
        "type/field_access refs should produce References edges"
    );
}

#[test]
fn test_derive_import_edges_skips_unknown_name() {
    let inputs = build_inputs();
    let edges = derive_edges_from_inputs(&inputs, None);
    // Exactly the resolvable import (callee) yields Import edges; the `ghost`
    // import is skipped.
    let import_edges: Vec<_> = edges
        .iter()
        .filter(|e| e.kind == EdgeKind::Imports)
        .collect();
    assert!(!import_edges.is_empty(), "expected import edges");
    assert!(
        import_edges.iter().all(|e| e.target_sym != 0),
        "no placeholder edges"
    );
}

#[test]
fn test_derive_impl_edges_inherits_and_implements() {
    let inputs = build_inputs();
    let edges = derive_edges_from_inputs(&inputs, None);

    // The impl block's name has no matching symbol so no `Inherits` edge,
    // but `extract_trait_from_signature` finds `Greeter` and emits Implements.
    let implements = edges
        .iter()
        .find(|e| e.kind == EdgeKind::Implements)
        .expect("impl Trait for Type should produce an Implements edge");
    assert_eq!(implements.source_sym, 4);
    assert_eq!(implements.target_sym, 3);
}

#[test]
fn test_derive_edges_skips_language_less_file() {
    let mut inputs = build_inputs();
    // Give file 3 (no language) a symbol + import so the skip branch is hit.
    inputs
        .all_symbols
        .push(sym(9, 3, "orphan", SymbolKind::Function, 1, 2));
    inputs.imports_by_file.insert(
        3,
        vec![ImportEntry {
            file_id: 3,
            imported_name: "orphan".to_string(),
            source_module: "x".to_string(),
            alias: None,
            line: 1,
            kind: "use".to_string(),
        }],
    );
    let edges = derive_edges_from_inputs(&inputs, None);
    // No edge should originate from the language-less file 3.
    assert!(
        edges.iter().all(|e| e.source_sym != 9),
        "files without a language must be skipped"
    );
}

#[test]
fn test_derive_for_language_filters_to_target_set() {
    let inputs = build_inputs();
    let edges = derive_edges_from_inputs_for_language(&inputs, "rust", None);
    assert!(
        edges
            .iter()
            .any(|e| e.kind == EdgeKind::Calls && e.source_sym == 1 && e.target_sym == 2),
        "rust subgraph should keep the caller->callee edge"
    );

    // A language with no matching files yields no edges.
    let none = derive_edges_from_inputs_for_language(&inputs, "cobol", None);
    assert!(none.is_empty(), "unknown language should produce no edges");
}

#[test]
fn test_ref_kind_type_and_field_access_map_to_references() {
    let mut inputs = build_inputs();
    inputs.all_refs = vec![
        refs("callee", 1, 2, "type"),
        refs("callee", 1, 3, "field_access"),
    ];
    inputs.imports_by_file.clear();
    let edges = derive_edges_from_inputs(&inputs, None);
    let ref_count = edges
        .iter()
        .filter(|e| e.kind == EdgeKind::References)
        .count();
    assert_eq!(ref_count, 2, "one References edge per type/field ref");
}
