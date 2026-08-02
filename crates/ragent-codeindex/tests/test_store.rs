//! External integration tests for the code-index SQLite store.

use chrono::Utc;
use ragent_codeindex::store::IndexStore;
use ragent_codeindex::types::{
    FileEntry, ImportEntry, ScannedFile, StaleDiff, Symbol, SymbolFilter, SymbolKind, SymbolRef,
    Visibility,
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

fn make_symbol(name: &str, kind: SymbolKind, parent_id: Option<i64>, temp_id: i64) -> Symbol {
    Symbol {
        id: temp_id,
        file_id: 0,
        name: name.to_string(),
        qualified_name: Some(name.to_string()),
        kind,
        visibility: Visibility::Public,
        start_line: 1,
        end_line: 10,
        start_col: 0,
        end_col: 0,
        parent_id,
        signature: Some(format!("fn {name}()")),
        doc_comment: None,
        body_hash: Some("hash123".to_string()),
    }
}

#[test]
fn test_open_in_memory() {
    let store = IndexStore::open_in_memory().unwrap();
    assert_eq!(store.file_count().unwrap(), 0);
}

#[test]
fn test_upsert_and_get() {
    let store = IndexStore::open_in_memory().unwrap();
    let entry = make_entry("src/main.rs", "abc123");
    store.upsert_file(&entry).unwrap();

    let got = store.get_file("src/main.rs").unwrap().unwrap();
    assert_eq!(got.path, "src/main.rs");
    assert_eq!(got.content_hash, "abc123");
    assert_eq!(got.byte_size, 100);
}

#[test]
fn test_upsert_updates_existing() {
    let store = IndexStore::open_in_memory().unwrap();
    let entry1 = make_entry("src/main.rs", "hash_v1");
    store.upsert_file(&entry1).unwrap();

    let entry2 = make_entry("src/main.rs", "hash_v2");
    store.upsert_file(&entry2).unwrap();

    assert_eq!(store.file_count().unwrap(), 1);
    let got = store.get_file("src/main.rs").unwrap().unwrap();
    assert_eq!(got.content_hash, "hash_v2");
}

#[test]
fn test_list_files() {
    let store = IndexStore::open_in_memory().unwrap();
    store.upsert_file(&make_entry("b.rs", "h2")).unwrap();
    store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let files = store.list_files().unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(files[0].path, "a.rs"); // sorted by path
    assert_eq!(files[1].path, "b.rs");
}

#[test]
fn test_delete_file() {
    let store = IndexStore::open_in_memory().unwrap();
    store
        .upsert_file(&make_entry("src/main.rs", "abc"))
        .unwrap();
    assert_eq!(store.file_count().unwrap(), 1);

    store.delete_file("src/main.rs").unwrap();
    assert_eq!(store.file_count().unwrap(), 0);
    assert!(store.get_file("src/main.rs").unwrap().is_none());
}

#[test]
fn test_stale_detection() {
    let store = IndexStore::open_in_memory().unwrap();

    // Index two files.
    store.upsert_file(&make_entry("a.rs", "hash_a")).unwrap();
    store.upsert_file(&make_entry("b.rs", "hash_b")).unwrap();

    // Simulate a re-scan: a.rs unchanged, b.rs changed, c.rs is new, d.rs removed.
    store.upsert_file(&make_entry("d.rs", "hash_d")).unwrap();

    let scanned = vec![
        ScannedFile {
            path: "a.rs".into(),
            hash: "hash_a".to_string(), // unchanged
            size: 100,
            language: Some("rust".to_string()),
            mtime_ns: 1_000_000_000,
            line_count: 10,
        },
        ScannedFile {
            path: "b.rs".into(),
            hash: "hash_b_v2".to_string(), // changed
            size: 200,
            language: Some("rust".to_string()),
            mtime_ns: 2_000_000_000,
            line_count: 20,
        },
        ScannedFile {
            path: "c.rs".into(),
            hash: "hash_c".to_string(), // new
            size: 50,
            language: Some("rust".to_string()),
            mtime_ns: 3_000_000_000,
            line_count: 5,
        },
    ];

    let diff = store.get_stale_files(&scanned).unwrap();

    assert_eq!(diff.to_add.len(), 1);
    assert_eq!(diff.to_add[0].path.to_string_lossy(), "c.rs");

    assert_eq!(diff.to_update.len(), 1);
    assert_eq!(diff.to_update[0].path.to_string_lossy(), "b.rs");

    assert_eq!(diff.to_remove.len(), 1);
    assert_eq!(diff.to_remove[0], "d.rs");
}

#[test]
fn test_apply_diff() {
    let store = IndexStore::open_in_memory().unwrap();
    store
        .upsert_file(&make_entry("old.rs", "hash_old"))
        .unwrap();

    let diff = StaleDiff {
        to_add: vec![ScannedFile {
            path: "new.rs".into(),
            hash: "hash_new".to_string(),
            size: 100,
            language: Some("rust".to_string()),
            mtime_ns: 1_000_000_000,
            line_count: 10,
        }],
        to_update: vec![],
        to_remove: vec!["old.rs".to_string()],
    };

    store.apply_diff(&diff).unwrap();

    assert!(store.get_file("old.rs").unwrap().is_none());
    assert!(store.get_file("new.rs").unwrap().is_some());
    assert_eq!(store.file_count().unwrap(), 1);
}

#[test]
fn test_language_counts() {
    let store = IndexStore::open_in_memory().unwrap();
    store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    store.upsert_file(&make_entry("b.rs", "h2")).unwrap();

    let mut entry_py = make_entry("c.py", "h3");
    entry_py.language = Some("python".to_string());
    store.upsert_file(&entry_py).unwrap();

    let counts = store.language_counts().unwrap();
    assert_eq!(counts.len(), 2);
    // rust=2 should come first (sorted by count DESC)
    assert_eq!(counts[0], ("rust".to_string(), 2));
    assert_eq!(counts[1], ("python".to_string(), 1));
}

#[test]
fn test_upsert_and_query_symbols() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store
        .upsert_file(&make_entry("src/main.rs", "abc"))
        .unwrap();

    let symbols = vec![
        make_symbol("main", SymbolKind::Function, None, 0),
        make_symbol("Config", SymbolKind::Struct, None, 1),
        make_symbol("new", SymbolKind::Method, Some(1), 2),
    ];

    let count = store.upsert_symbols(file_id, &symbols).unwrap();
    assert_eq!(count, 3);
    assert_eq!(store.symbol_count().unwrap(), 3);

    // Query all
    let all = store.query_symbols(&SymbolFilter::default()).unwrap();
    assert_eq!(all.len(), 3);

    // Query by name
    let by_name = store
        .query_symbols(&SymbolFilter {
            name: Some("main".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(by_name.len(), 1);
    assert_eq!(by_name[0].name, "main");

    // Query by kind
    let methods = store
        .query_symbols(&SymbolFilter {
            kind: Some(SymbolKind::Method),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(methods.len(), 1);
    assert_eq!(methods[0].name, "new");
}

#[test]
fn test_symbol_parent_id_mapping() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("src/lib.rs", "def")).unwrap();

    let symbols = vec![
        make_symbol("Foo", SymbolKind::Struct, None, 0),
        make_symbol("bar", SymbolKind::Method, Some(0), 1),
    ];

    store.upsert_symbols(file_id, &symbols).unwrap();

    let all = store.query_symbols(&SymbolFilter::default()).unwrap();
    let foo = all.iter().find(|s| s.name == "Foo").unwrap();
    let bar = all.iter().find(|s| s.name == "bar").unwrap();

    // bar's parent_id should be the real DB id of Foo
    assert_eq!(bar.parent_id, Some(foo.id));
}

#[test]
fn test_upsert_symbols_replaces() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let v1 = vec![make_symbol("old_fn", SymbolKind::Function, None, 0)];
    store.upsert_symbols(file_id, &v1).unwrap();
    assert_eq!(store.symbol_count().unwrap(), 1);

    let v2 = vec![
        make_symbol("new_fn", SymbolKind::Function, None, 0),
        make_symbol("another", SymbolKind::Function, None, 1),
    ];
    store.upsert_symbols(file_id, &v2).unwrap();
    assert_eq!(store.symbol_count().unwrap(), 2);

    let all = store.query_symbols(&SymbolFilter::default()).unwrap();
    let names: Vec<&str> = all.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"new_fn"));
    assert!(names.contains(&"another"));
    assert!(!names.contains(&"old_fn"));
}

#[test]
fn test_upsert_and_query_imports() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store
        .upsert_file(&make_entry("src/main.rs", "abc"))
        .unwrap();

    let imports = vec![
        ImportEntry {
            file_id,
            imported_name: "HashMap".to_string(),
            source_module: "std::collections".to_string(),
            alias: None,
            line: 1,
            kind: "use".to_string(),
        },
        ImportEntry {
            file_id,
            imported_name: "Result".to_string(),
            source_module: "anyhow".to_string(),
            alias: None,
            line: 2,
            kind: "use".to_string(),
        },
    ];

    let count = store.upsert_imports(file_id, &imports).unwrap();
    assert_eq!(count, 2);

    let got = store.get_file_imports(file_id).unwrap();
    assert_eq!(got.len(), 2);
    assert_eq!(got[0].imported_name, "HashMap");

    let searched = store.query_imports("Hash").unwrap();
    assert_eq!(searched.len(), 1);
    assert_eq!(searched[0].imported_name, "HashMap");
}

#[test]
fn test_upsert_and_find_refs() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store
        .upsert_file(&make_entry("src/main.rs", "abc"))
        .unwrap();

    let refs = vec![
        SymbolRef {
            symbol_name: "Config".to_string(),
            file_id,
            file_path: String::new(),
            line: 10,
            col: 5,
            kind: "type_ref".to_string(),
        },
        SymbolRef {
            symbol_name: "Config".to_string(),
            file_id,
            file_path: String::new(),
            line: 20,
            col: 8,
            kind: "call".to_string(),
        },
    ];

    store.upsert_refs(file_id, &refs).unwrap();

    let found = store.find_references("Config").unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].line, 10);
    assert_eq!(found[1].line, 20);
}

#[test]
fn test_file_deps() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store
        .upsert_file(&make_entry("src/main.rs", "abc"))
        .unwrap();

    let deps = vec![
        ("src/config.rs".to_string(), "use".to_string()),
        ("src/utils.rs".to_string(), "mod".to_string()),
    ];

    store.set_file_deps(file_id, &deps).unwrap();

    let dependents = store.get_dependents("src/config.rs").unwrap();
    assert_eq!(dependents.len(), 1);
    assert_eq!(dependents[0], file_id);

    let no_deps = store.get_dependents("nonexistent.rs").unwrap();
    assert!(no_deps.is_empty());
}

#[test]
fn test_get_stats() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![make_symbol("foo", SymbolKind::Function, None, 0)];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stats = store.get_stats().unwrap();
    assert_eq!(stats.files_indexed, 1);
    assert_eq!(stats.total_symbols, 1);
    assert_eq!(stats.total_bytes, 100);
}

#[test]
fn test_cascade_delete() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let symbols = vec![make_symbol("foo", SymbolKind::Function, None, 0)];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let imports = vec![ImportEntry {
        file_id,
        imported_name: "Bar".to_string(),
        source_module: "baz".to_string(),
        alias: None,
        line: 1,
        kind: "use".to_string(),
    }];
    store.upsert_imports(file_id, &imports).unwrap();

    assert_eq!(store.symbol_count().unwrap(), 1);

    // Deleting the file should cascade-delete symbols and imports.
    store.delete_file("a.rs").unwrap();
    assert_eq!(store.symbol_count().unwrap(), 0);
    assert!(store.get_file_imports(file_id).unwrap().is_empty());
}

#[test]
fn test_get_file_id() {
    let store = IndexStore::open_in_memory().unwrap();
    assert!(store.get_file_id("nonexistent.rs").unwrap().is_none());

    store.upsert_file(&make_entry("src/lib.rs", "h1")).unwrap();
    let id = store.get_file_id("src/lib.rs").unwrap();
    assert!(id.is_some());
}
