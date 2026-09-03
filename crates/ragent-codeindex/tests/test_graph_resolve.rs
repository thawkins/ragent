#![allow(clippy::assert_is_empty)]
//! Tests for cross-file symbol resolution (spec graphCI, T-005).

use chrono::Utc;
use ragent_codeindex::graph::resolve;
use ragent_codeindex::store::IndexStore;
use ragent_codeindex::types::{FileEntry, Symbol, SymbolKind, Visibility};

fn make_entry(path: &str, hash: &str, lang: Option<&str>) -> FileEntry {
    FileEntry {
        path: path.to_string(),
        content_hash: hash.to_string(),
        byte_size: 100,
        language: lang.map(|l| l.to_string()),
        last_indexed: Utc::now(),
        mtime_ns: 1_000_000_000,
        line_count: 20,
    }
}

fn make_symbol(name: &str, kind: SymbolKind, file_id: i64, vis: Visibility) -> Symbol {
    Symbol {
        id: 0,
        file_id,
        name: name.to_string(),
        qualified_name: Some(name.to_string()),
        kind,
        visibility: vis,
        start_line: 1,
        end_line: 10,
        start_col: 0,
        end_col: 0,
        parent_id: None,
        signature: Some(format!("fn {name}()")),
        doc_comment: None,
        body_hash: Some("h".to_string()),
    }
}

// ── Basic resolution ────────────────────────────────────────────────────

#[test]
fn test_resolve_symbol_not_found() {
    let store = IndexStore::open_in_memory().unwrap();
    let result = resolve::resolve_symbol("NonExistent", None, &store).unwrap();
    assert!(result.is_none());
}

#[test]
fn test_resolve_symbol_single_match() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store
        .upsert_file(&make_entry("a.rs", "h1", Some("rust")))
        .unwrap();
    let symbols = vec![make_symbol(
        "foo",
        SymbolKind::Function,
        file_id,
        Visibility::Public,
    )];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let result = resolve::resolve_symbol("foo", None, &store).unwrap();
    assert!(result.is_some());
    let resolved = result.unwrap();
    assert_eq!(resolved.file_id, file_id);
}

#[test]
fn test_resolve_prefers_same_file() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_a = store
        .upsert_file(&make_entry("a.rs", "h1", Some("rust")))
        .unwrap();
    let file_b = store
        .upsert_file(&make_entry("b.rs", "h2", Some("rust")))
        .unwrap();

    let symbols_a = vec![make_symbol(
        "foo",
        SymbolKind::Function,
        file_a,
        Visibility::Private,
    )];
    store.upsert_symbols(file_a, &symbols_a).unwrap();

    let symbols_b = vec![make_symbol(
        "foo",
        SymbolKind::Function,
        file_b,
        Visibility::Public,
    )];
    store.upsert_symbols(file_b, &symbols_b).unwrap();

    // Resolving from file_a should prefer the same-file definition (private)
    // over the cross-file one (public), because same-file > visibility.
    let result = resolve::resolve_symbol("foo", Some(file_a), &store).unwrap();
    let resolved = result.unwrap();
    assert_eq!(resolved.file_id, file_a);
    assert!(resolved.same_file);
}

#[test]
fn test_resolve_prefers_higher_visibility_cross_file() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_a = store
        .upsert_file(&make_entry("a.rs", "h1", Some("rust")))
        .unwrap();
    let file_b = store
        .upsert_file(&make_entry("b.rs", "h2", Some("rust")))
        .unwrap();
    let file_c = store
        .upsert_file(&make_entry("c.rs", "h3", Some("rust")))
        .unwrap();

    // All cross-file, different visibilities. Should prefer pub.
    store
        .upsert_symbols(
            file_b,
            &[make_symbol(
                "foo",
                SymbolKind::Function,
                file_b,
                Visibility::Private,
            )],
        )
        .unwrap();
    store
        .upsert_symbols(
            file_c,
            &[make_symbol(
                "foo",
                SymbolKind::Function,
                file_c,
                Visibility::Public,
            )],
        )
        .unwrap();

    // Source is file_a (no "foo" defined there), so all candidates are cross-file.
    let result = resolve::resolve_symbol("foo", Some(file_a), &store).unwrap();
    let resolved = result.unwrap();
    assert_eq!(resolved.file_id, file_c, "should prefer pub over private");
    assert!(!resolved.same_file);
}

#[test]
fn test_resolve_prefers_same_module() {
    let store = IndexStore::open_in_memory().unwrap();
    let src_file = store
        .upsert_file(&make_entry("src/main.rs", "h1", Some("rust")))
        .unwrap();
    let same_dir = store
        .upsert_file(&make_entry("src/lib.rs", "h2", Some("rust")))
        .unwrap();
    let diff_dir = store
        .upsert_file(&make_entry("tests/mod.rs", "h3", Some("rust")))
        .unwrap();

    // Both candidates have the same visibility; one is in the same directory
    // as the source, the other is in a different directory.
    store
        .upsert_symbols(
            same_dir,
            &[make_symbol(
                "foo",
                SymbolKind::Function,
                same_dir,
                Visibility::Public,
            )],
        )
        .unwrap();
    store
        .upsert_symbols(
            diff_dir,
            &[make_symbol(
                "foo",
                SymbolKind::Function,
                diff_dir,
                Visibility::Public,
            )],
        )
        .unwrap();

    let result = resolve::resolve_symbol("foo", Some(src_file), &store).unwrap();
    let resolved = result.unwrap();
    assert_eq!(
        resolved.file_id, same_dir,
        "should prefer same-module (directory)"
    );
}

#[test]
fn test_resolve_prefers_same_language() {
    let store = IndexStore::open_in_memory().unwrap();
    let src_file = store
        .upsert_file(&make_entry("src/main.rs", "h1", Some("rust")))
        .unwrap();
    let rust_file = store
        .upsert_file(&make_entry("other.rs", "h2", Some("rust")))
        .unwrap();
    let py_file = store
        .upsert_file(&make_entry("other.py", "h3", Some("python")))
        .unwrap();

    // Both cross-file, same visibility. One is rust, one is python.
    // Source is rust, so should prefer the rust candidate.
    store
        .upsert_symbols(
            rust_file,
            &[make_symbol(
                "foo",
                SymbolKind::Function,
                rust_file,
                Visibility::Public,
            )],
        )
        .unwrap();
    store
        .upsert_symbols(
            py_file,
            &[make_symbol(
                "foo",
                SymbolKind::Function,
                py_file,
                Visibility::Public,
            )],
        )
        .unwrap();

    let result = resolve::resolve_symbol("foo", Some(src_file), &store).unwrap();
    let resolved = result.unwrap();
    assert_eq!(resolved.file_id, rust_file, "should prefer same language");
}

// ── resolve_all_symbols ───────────────���────────────────────────────────

#[test]
fn test_resolve_all_symbols_empty() {
    let store = IndexStore::open_in_memory().unwrap();
    let result = resolve::resolve_all_symbols("NonExistent", None, &store).unwrap();
    assert!(result.is_empty());
}

#[test]
fn test_resolve_all_symbols_multiple() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_a = store
        .upsert_file(&make_entry("a.rs", "h1", Some("rust")))
        .unwrap();
    let file_b = store
        .upsert_file(&make_entry("b.rs", "h2", Some("rust")))
        .unwrap();

    store
        .upsert_symbols(
            file_a,
            &[make_symbol(
                "foo",
                SymbolKind::Function,
                file_a,
                Visibility::Public,
            )],
        )
        .unwrap();
    store
        .upsert_symbols(
            file_b,
            &[make_symbol(
                "foo",
                SymbolKind::Function,
                file_b,
                Visibility::Private,
            )],
        )
        .unwrap();

    let result = resolve::resolve_all_symbols("foo", Some(file_a), &store).unwrap();
    assert_eq!(result.len(), 2);
    // Best (same-file) should be first.
    assert!(result[0].same_file);
    assert!(!result[1].same_file);
}

#[test]
fn test_resolve_all_symbols_sorted_by_rank() {
    let store = IndexStore::open_in_memory().unwrap();
    let src_file = store
        .upsert_file(&make_entry("src/main.rs", "h0", Some("rust")))
        .unwrap();
    let same_dir = store
        .upsert_file(&make_entry("src/lib.rs", "h1", Some("rust")))
        .unwrap();
    let diff_dir = store
        .upsert_file(&make_entry("tests/mod.rs", "h2", Some("rust")))
        .unwrap();

    // same_dir has pub, diff_dir has pub — same_dir should rank higher due
    // to same module.
    store
        .upsert_symbols(
            same_dir,
            &[make_symbol(
                "foo",
                SymbolKind::Function,
                same_dir,
                Visibility::Public,
            )],
        )
        .unwrap();
    store
        .upsert_symbols(
            diff_dir,
            &[make_symbol(
                "foo",
                SymbolKind::Function,
                diff_dir,
                Visibility::Public,
            )],
        )
        .unwrap();

    let result = resolve::resolve_all_symbols("foo", Some(src_file), &store).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].file_id, same_dir, "same-module should rank first");
}

// ── is_definition_kind ────────────────────────────────────────────────

#[test]
fn test_is_definition_kind() {
    assert!(resolve::is_definition_kind(SymbolKind::Function));
    assert!(resolve::is_definition_kind(SymbolKind::Struct));
    assert!(resolve::is_definition_kind(SymbolKind::Trait));
    assert!(resolve::is_definition_kind(SymbolKind::Impl));
    assert!(!resolve::is_definition_kind(SymbolKind::Import));
    assert!(!resolve::is_definition_kind(SymbolKind::Unknown));
}

// ── ResolvedSymbol same_file flag ──────────────────────────────────────

#[test]
fn test_resolved_symbol_same_file_flag() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_a = store
        .upsert_file(&make_entry("a.rs", "h1", Some("rust")))
        .unwrap();

    store
        .upsert_symbols(
            file_a,
            &[make_symbol(
                "foo",
                SymbolKind::Function,
                file_a,
                Visibility::Public,
            )],
        )
        .unwrap();

    // Resolve with same source file.
    let result = resolve::resolve_symbol("foo", Some(file_a), &store)
        .unwrap()
        .unwrap();
    assert!(result.same_file);

    // Resolve with different source file.
    let file_b = store
        .upsert_file(&make_entry("b.rs", "h2", Some("rust")))
        .unwrap();
    let result = resolve::resolve_symbol("foo", Some(file_b), &store)
        .unwrap()
        .unwrap();
    assert!(!result.same_file);

    // Resolve with no source file.
    let result = resolve::resolve_symbol("foo", None, &store)
        .unwrap()
        .unwrap();
    assert!(!result.same_file);
}
