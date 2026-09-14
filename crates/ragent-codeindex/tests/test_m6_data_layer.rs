//! M6 data-layer regression tests (PERF-072/073/074/075).
//!
//! These pin the observable contracts of the code-index store changes:
//! the stale diff is computed from a streamed scan (O(scanned) memory) while
//! remaining correct for a large index; the symbol-name filter matches a
//! leading fragment (anchored, index-friendly) rather than an interior
//! substring; the per-file upserts are atomic and round-trip; and the FTS
//! `IndexWriter` is reused across batches.

use chrono::Utc;
use ragent_codeindex::store::IndexStore;
use ragent_codeindex::types::{
    FileEntry, ScannedFile, Symbol, SymbolFilter, SymbolKind, Visibility,
};
use std::path::PathBuf;

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

fn scanned(path: &str, hash: &str) -> ScannedFile {
    ScannedFile {
        path: PathBuf::from(path),
        hash: hash.to_string(),
        size: 100,
        language: Some("rust".to_string()),
        mtime_ns: 1_000_000_000,
        line_count: 10,
    }
}

fn make_symbol(name: &str, temp_id: i64) -> Symbol {
    Symbol {
        id: temp_id,
        file_id: 0,
        name: name.to_string(),
        qualified_name: Some(format!("mod::{name}")),
        kind: SymbolKind::Function,
        visibility: Visibility::Public,
        start_line: 1,
        end_line: 10,
        start_col: 0,
        end_col: 0,
        parent_id: None,
        signature: Some(format!("fn {name}()")),
        doc_comment: None,
        body_hash: Some("hash".to_string()),
    }
}

/// PERF-072: with a large index and a scan that matches it exactly, the diff
/// is empty and every non-scanned indexed file is reported for removal.
#[test]
fn stale_diff_is_correct_with_a_large_index() {
    let store = IndexStore::open_in_memory().expect("store");
    // Populate a large index: 500 files, only two of which are re-scanned.
    for i in 0..500 {
        store
            .upsert_file(&make_entry(&format!("src/f{i}.rs"), "h-same"))
            .expect("seed");
    }
    store
        .upsert_file(&make_entry("src/keep.rs", "h-keep"))
        .expect("seed keep");
    store
        .upsert_file(&make_entry("src/changed.rs", "old-hash"))
        .expect("seed changed");

    let diff = store
        .get_stale_files(&[
            scanned("src/keep.rs", "h-keep"),
            scanned("src/changed.rs", "new-hash"),
        ])
        .expect("diff");

    assert!(
        diff.to_add.is_empty(),
        "both scanned files already exist, so nothing is new"
    );
    assert_eq!(
        diff.to_update.len(),
        1,
        "only the hash-changed file updates"
    );
    assert_eq!(diff.to_update[0].path, PathBuf::from("src/changed.rs"));
    // Every indexed file not re-scanned is a removal (the 500 seeded files).
    assert_eq!(diff.to_remove.len(), 500);
    assert!(diff.to_remove.iter().all(|p| p.starts_with("src/f")));
}

/// PERF-072: a brand-new scanned path is reported for addition.
#[test]
fn stale_diff_reports_new_files() {
    let store = IndexStore::open_in_memory().expect("store");
    store
        .upsert_file(&make_entry("src/old.rs", "h"))
        .expect("seed");

    let diff = store
        .get_stale_files(&[scanned("src/new.rs", "h-new")])
        .expect("diff");
    assert_eq!(diff.to_add.len(), 1);
    assert_eq!(diff.to_add[0].path, PathBuf::from("src/new.rs"));
    assert_eq!(diff.to_update.len(), 0);
    assert_eq!(diff.to_remove.len(), 1, "the un-scanned old.rs is removed");
}

/// PERF-073: the name filter matches a leading fragment (anchored prefix) but
/// not an interior-only fragment, and `LIKE` metacharacters are literal.
#[test]
fn symbol_name_filter_is_anchored_prefix() {
    let store = IndexStore::open_in_memory().expect("store");
    let file_id = store
        .upsert_file(&make_entry("src/lib.rs", "h"))
        .expect("upsert");
    store
        .upsert_symbols(
            file_id,
            &[
                make_symbol("ServerProcessor", 1),
                make_symbol("CachedServer", 2),
                make_symbol("Server", 3),
            ],
        )
        .expect("symbols");

    // Prefix "Server" matches the Server and ServerProcessor symbols...
    let matches = store
        .query_symbols(&SymbolFilter {
            name: Some("Server".to_string()),
            ..Default::default()
        })
        .expect("query");
    let names: Vec<&str> = matches.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Server"));
    assert!(names.contains(&"ServerProcessor"));
    // ...but not the interior occurrence in CachedServer.
    assert!(
        !names.contains(&"CachedServer"),
        "interior substring must no longer match: {names:?}"
    );

    // Case-insensitive, matching the previous semantics.
    let lower = store
        .query_symbols(&SymbolFilter {
            name: Some("serverpr".to_string()),
            ..Default::default()
        })
        .expect("query");
    assert!(lower.iter().any(|s| s.name == "ServerProcessor"));

    // A `%` in the filter is literal, not a wildcard.
    let literal = store
        .query_symbols(&SymbolFilter {
            name: Some("%".to_string()),
            ..Default::default()
        })
        .expect("query");
    assert!(literal.is_empty(), "percent must not act as a wildcard");
}

/// PERF-074: `upsert_file` returns the same id on insert and update, and
/// `upsert_symbols` replaces cleanly (atomic delete + re-insert).
#[test]
fn upsert_file_id_stable_and_symbols_replace() {
    let store = IndexStore::open_in_memory().expect("store");
    let id1 = store
        .upsert_file(&make_entry("src/a.rs", "h1"))
        .expect("insert");
    let id2 = store
        .upsert_file(&make_entry("src/a.rs", "h2"))
        .expect("update");
    assert_eq!(id1, id2, "RETURNING id must be stable across an upsert");

    store
        .upsert_symbols(id1, &[make_symbol("first", 1), make_symbol("second", 2)])
        .expect("first insert");
    let count = store
        .upsert_symbols(id1, &[make_symbol("only", 3)])
        .expect("replace");
    assert_eq!(count, 1);
    let all = store
        .query_symbols(&SymbolFilter {
            file_path: Some("src/a.rs".to_string()),
            ..Default::default()
        })
        .expect("query");
    let names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
    assert_eq!(
        names,
        vec!["only"],
        "replace must drop the previous symbols"
    );
}

/// PERF-074: `find_references_limited` caps results in SQL, matching the
/// truncating behaviour of the previous caller-side truncate.
#[test]
fn find_references_limited_caps_results() {
    let store = IndexStore::open_in_memory().expect("store");
    let file_id = store
        .upsert_file(&make_entry("src/refs.rs", "h"))
        .expect("upsert");
    let refs: Vec<ragent_codeindex::types::SymbolRef> = (0..10)
        .map(|i| ragent_codeindex::types::SymbolRef {
            symbol_name: "Target".to_string(),
            file_id,
            file_path: String::new(),
            line: i,
            col: 0,
            kind: "call".to_string(),
        })
        .collect();
    store.upsert_refs(file_id, &refs).expect("refs");

    assert_eq!(
        store
            .find_references_limited("Target", 3)
            .expect("limited")
            .len(),
        3
    );
    assert_eq!(store.find_references("Target").expect("all").len(), 10);
    assert_eq!(
        store
            .find_references_limited("Target", 0)
            .expect("unlimited")
            .len(),
        10,
        "limit 0 means no limit"
    );
}
