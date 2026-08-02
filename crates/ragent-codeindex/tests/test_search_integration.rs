//! External tests for `integration_tests` from `crates/ragent-codeindex/src/search.rs`
//!
//! Relocated from the inline `#[cfg(test)]` module.

use ragent_codeindex::search::{FtsIndex, FtsSymbol};

/// Mimic how `full_reindex` adds symbols: `remove_file` + `add_symbols` per file
#[test]
fn test_incremental_add_many_files() {
    let dir = tempfile::tempdir().unwrap();
    let fts_path = dir.path().join("fts");
    let fts = FtsIndex::open(&fts_path).unwrap();

    // Simulate 50 files, each with a few symbols
    for i in 0..50 {
        let file_path = format!("src/file_{i}.rs");
        fts.remove_file(&file_path).unwrap();
        let name = format!("func_{i}");
        let qname = format!("crate::mod_{i}::func_{i}");
        let syms = vec![FtsSymbol {
            name: &name,
            qualified_name: Some(&qname),
            kind: "function",
            file_path: &file_path,
            signature: Some("fn func() -> bool"),
            doc_comment: Some("A test function."),
            body_snippet: Some("let x = 42; return true;"),
            start_line: 10,
            end_line: 20,
        }];
        fts.add_symbols(&syms).unwrap();
    }

    let count = fts.doc_count().unwrap();
    eprintln!("doc_count after 50 files: {count}");
    assert_eq!(count, 50, "should have 50 docs");

    let results = fts.search("func_25", 10).unwrap();
    eprintln!("search for func_25: {} results", results.len());
    assert!(!results.is_empty(), "should find func_25");
    assert_eq!(results[0].symbol_name, "func_25");

    // Now drop and reopen to test persistence
    drop(fts);
    let fts2 = FtsIndex::open(&fts_path).unwrap();
    let count2 = fts2.doc_count().unwrap();
    eprintln!("doc_count after reopen: {count2}");
    assert_eq!(count2, 50, "should still have 50 docs after reopen");

    let results2 = fts2.search("func_25", 10).unwrap();
    eprintln!("search after reopen: {} results", results2.len());
    assert!(!results2.is_empty(), "should find func_25 after reopen");
}
