#![allow(clippy::assert_is_empty)]
//! Tests for the `graph_edges` and `communities` SQLite tables (spec graphCI, T-001).

use chrono::Utc;
use ragent_codeindex::store::IndexStore;
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
fn test_graph_edges_table_exists() {
    let store = IndexStore::open_in_memory().unwrap();
    // The table should exist and be empty.
    assert_eq!(store.edge_count().unwrap(), 0);
}

#[test]
fn test_communities_table_exists() {
    let store = IndexStore::open_in_memory().unwrap();
    assert_eq!(store.community_count().unwrap(), 0);
    assert!(store.query_all_communities().unwrap().is_empty());
}

#[test]
fn test_upsert_and_query_edge() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let symbols = vec![
        make_symbol("caller", SymbolKind::Function, 0, file_id),
        make_symbol("callee", SymbolKind::Function, 0, file_id),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    assert_eq!(stored.len(), 2);
    let caller_id = stored.iter().find(|s| s.name == "caller").unwrap().id;
    let callee_id = stored.iter().find(|s| s.name == "callee").unwrap().id;

    store
        .upsert_edge(
            caller_id,
            callee_id,
            "calls",
            "EXTRACTED",
            Some(file_id),
            Some(5),
        )
        .unwrap();

    assert_eq!(store.edge_count().unwrap(), 1);
    assert_eq!(store.edge_count_by_confidence("EXTRACTED").unwrap(), 1);
    assert_eq!(store.edge_count_by_confidence("INFERRED").unwrap(), 0);

    let edges = store.query_edges_for_symbol(caller_id).unwrap();
    assert_eq!(edges.len(), 1);
    assert_eq!(edges[0].0, caller_id);
    assert_eq!(edges[0].1, callee_id);
    assert_eq!(edges[0].2, "calls");
    assert_eq!(edges[0].3, "EXTRACTED");
}

#[test]
fn test_upsert_edge_replaces_on_conflict() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let symbols = vec![
        make_symbol("foo", SymbolKind::Function, 0, file_id),
        make_symbol("bar", SymbolKind::Function, 0, file_id),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let foo_id = stored.iter().find(|s| s.name == "foo").unwrap().id;
    let bar_id = stored.iter().find(|s| s.name == "bar").unwrap().id;

    // Insert with INFERRED.
    store
        .upsert_edge(foo_id, bar_id, "calls", "INFERRED", None, None)
        .unwrap();
    assert_eq!(store.edge_count().unwrap(), 1);
    assert_eq!(store.edge_count_by_confidence("INFERRED").unwrap(), 1);

    // Upsert with EXTRACTED — should replace, not duplicate.
    store
        .upsert_edge(foo_id, bar_id, "calls", "EXTRACTED", Some(file_id), Some(3))
        .unwrap();
    assert_eq!(store.edge_count().unwrap(), 1);
    assert_eq!(store.edge_count_by_confidence("EXTRACTED").unwrap(), 1);
    assert_eq!(store.edge_count_by_confidence("INFERRED").unwrap(), 0);
}

#[test]
fn test_query_all_edges() {
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
        .upsert_edge(a_id, b_id, "calls", "EXTRACTED", None, None)
        .unwrap();
    store
        .upsert_edge(b_id, c_id, "references", "INFERRED", None, None)
        .unwrap();

    let all = store.query_all_edges().unwrap();
    assert_eq!(all.len(), 2);
}

#[test]
fn test_delete_edges_for_symbols() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let symbols = vec![
        make_symbol("a", SymbolKind::Function, 0, file_id),
        make_symbol("b", SymbolKind::Function, 0, file_id),
        make_symbol("c", SymbolKind::Function, 0, file_id),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let a_id = stored.iter().find(|s| s.name == "a").unwrap().id;
    let b_id = stored.iter().find(|s| s.name == "b").unwrap().id;
    let c_id = stored.iter().find(|s| s.name == "c").unwrap().id;

    store
        .upsert_edge(a_id, b_id, "calls", "EXTRACTED", None, None)
        .unwrap();
    store
        .upsert_edge(b_id, c_id, "calls", "EXTRACTED", None, None)
        .unwrap();
    store
        .upsert_edge(a_id, c_id, "references", "INFERRED", None, None)
        .unwrap();
    assert_eq!(store.edge_count().unwrap(), 3);

    // Delete edges involving a_id.
    store.delete_edges_for_symbols(&[a_id]).unwrap();
    assert_eq!(store.edge_count().unwrap(), 1);

    // The remaining edge should be b->c.
    let remaining = store.query_all_edges().unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].0, b_id);
    assert_eq!(remaining[0].1, c_id);
}

#[test]
fn test_delete_edges_for_empty_slice() {
    let store = IndexStore::open_in_memory().unwrap();
    // Should be a no-op.
    store.delete_edges_for_symbols(&[]).unwrap();
    assert_eq!(store.edge_count().unwrap(), 0);
}

#[test]
fn test_cascade_delete_edges_on_symbol_delete() {
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
        .upsert_edge(
            caller_id,
            callee_id,
            "calls",
            "EXTRACTED",
            Some(file_id),
            Some(5),
        )
        .unwrap();
    assert_eq!(store.edge_count().unwrap(), 1);

    // Deleting the file cascades to symbols, which cascades to graph_edges.
    store.delete_file("a.rs").unwrap();
    assert_eq!(store.edge_count().unwrap(), 0);
}

#[test]
fn test_upsert_and_query_community() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let symbols = vec![
        make_symbol("foo", SymbolKind::Function, 0, file_id),
        make_symbol("bar", SymbolKind::Function, 0, file_id),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let foo_id = stored.iter().find(|s| s.name == "foo").unwrap().id;
    let bar_id = stored.iter().find(|s| s.name == "bar").unwrap().id;

    store.upsert_community(foo_id, 1, Some("core")).unwrap();
    store.upsert_community(bar_id, 1, Some("core")).unwrap();

    assert_eq!(store.community_count().unwrap(), 1);

    let members = store.query_community_members(1).unwrap();
    assert_eq!(members.len(), 2);

    let all = store.query_all_communities().unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.iter().all(|(_, c, _)| *c == 1));
}

#[test]
fn test_upsert_community_replaces() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![make_symbol("foo", SymbolKind::Function, 0, file_id)];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let foo_id = store.get_file_symbols(file_id).unwrap()[0].id;

    store.upsert_community(foo_id, 1, Some("old")).unwrap();
    assert_eq!(store.community_count().unwrap(), 1);

    // Re-assign to a different community.
    store.upsert_community(foo_id, 2, Some("new")).unwrap();
    assert_eq!(store.community_count().unwrap(), 1);

    let all = store.query_all_communities().unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].1, 2);
    assert_eq!(all[0].2.as_deref(), Some("new"));
}

#[test]
fn test_clear_communities() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![
        make_symbol("a", SymbolKind::Function, 0, file_id),
        make_symbol("b", SymbolKind::Function, 0, file_id),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let stored = store.get_file_symbols(file_id).unwrap();

    store.upsert_community(stored[0].id, 1, Some("c1")).unwrap();
    store.upsert_community(stored[1].id, 2, Some("c2")).unwrap();
    assert_eq!(store.community_count().unwrap(), 2);

    store.clear_communities().unwrap();
    assert_eq!(store.community_count().unwrap(), 0);
    assert!(store.query_all_communities().unwrap().is_empty());
}

#[test]
fn test_cascade_delete_communities_on_symbol_delete() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    let symbols = vec![make_symbol("foo", SymbolKind::Function, 0, file_id)];
    store.upsert_symbols(file_id, &symbols).unwrap();
    let foo_id = store.get_file_symbols(file_id).unwrap()[0].id;

    store.upsert_community(foo_id, 1, Some("core")).unwrap();
    assert_eq!(store.community_count().unwrap(), 1);

    // Deleting the file cascades to symbols, which cascades to communities.
    store.delete_file("a.rs").unwrap();
    assert_eq!(store.community_count().unwrap(), 0);
}

#[test]
fn test_schema_version_is_three() {
    // Verify the schema version was bumped to 3 by checking that the new
    // tables exist and are queryable.  We cannot access the private `conn`
    // field from an external test, so we use the public `edge_count` and
    // `community_count` methods as indirect proof.
    let store = IndexStore::open_in_memory().unwrap();
    // These would panic if the tables didn't exist.
    assert_eq!(store.edge_count().unwrap(), 0);
    assert_eq!(store.community_count().unwrap(), 0);
}

#[test]
fn test_existing_tables_unchanged() {
    // Verify that the existing tables still work as before (FR-025).
    let store = IndexStore::open_in_memory().unwrap();

    // indexed_files
    store.upsert_file(&make_entry("a.rs", "h1")).unwrap();
    assert_eq!(store.file_count().unwrap(), 1);

    // symbols
    let file_id = store.get_file_id("a.rs").unwrap().unwrap();
    let symbols = vec![make_symbol("foo", SymbolKind::Function, 0, file_id)];
    store.upsert_symbols(file_id, &symbols).unwrap();
    assert_eq!(store.symbol_count().unwrap(), 1);

    // imports
    use ragent_codeindex::types::ImportEntry;
    store
        .upsert_imports(
            file_id,
            &[ImportEntry {
                file_id,
                imported_name: "Bar".to_string(),
                source_module: "baz".to_string(),
                alias: None,
                line: 1,
                kind: "use".to_string(),
            }],
        )
        .unwrap();
    assert_eq!(store.get_file_imports(file_id).unwrap().len(), 1);

    // symbol_refs
    use ragent_codeindex::types::SymbolRef;
    store
        .upsert_refs(
            file_id,
            &[SymbolRef {
                symbol_name: "Bar".to_string(),
                file_id,
                file_path: String::new(),
                line: 2,
                col: 0,
                kind: "call".to_string(),
            }],
        )
        .unwrap();
    assert_eq!(store.reference_count().unwrap(), 1);

    // file_deps
    store
        .set_file_deps(file_id, &[("b.rs".to_string(), "imports".to_string())])
        .unwrap();
    assert!(!store.get_dependents("b.rs").unwrap().is_empty());
}
// ── edge_count_by_kind / graph_node_count (graph status helpers) ─────────────

#[test]
fn test_edge_count_by_kind() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let symbols = vec![
        make_symbol("caller", SymbolKind::Function, 0, file_id),
        make_symbol("callee", SymbolKind::Function, 0, file_id),
        make_symbol("imported", SymbolKind::Function, 0, file_id),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let caller = stored.iter().find(|s| s.name == "caller").unwrap().id;
    let callee = stored.iter().find(|s| s.name == "callee").unwrap().id;
    let imported = stored.iter().find(|s| s.name == "imported").unwrap().id;

    store
        .upsert_edge(caller, callee, "calls", "EXTRACTED", Some(file_id), Some(5))
        .unwrap();
    store
        .upsert_edge(
            caller,
            imported,
            "imports",
            "EXTRACTED",
            Some(file_id),
            Some(1),
        )
        .unwrap();

    assert_eq!(store.edge_count_by_kind("calls").unwrap(), 1);
    assert_eq!(store.edge_count_by_kind("imports").unwrap(), 1);
    assert_eq!(store.edge_count_by_kind("references").unwrap(), 0);
}

#[test]
fn test_graph_node_count_distinct() {
    let store = IndexStore::open_in_memory().unwrap();
    let file_id = store.upsert_file(&make_entry("a.rs", "h1")).unwrap();

    let symbols = vec![
        make_symbol("a", SymbolKind::Function, 0, file_id),
        make_symbol("b", SymbolKind::Function, 0, file_id),
        make_symbol("c", SymbolKind::Function, 0, file_id),
        // "lonely" never appears in an edge — must NOT be counted as a node.
        make_symbol("lonely", SymbolKind::Function, 0, file_id),
    ];
    store.upsert_symbols(file_id, &symbols).unwrap();

    let stored = store.get_file_symbols(file_id).unwrap();
    let a = stored.iter().find(|s| s.name == "a").unwrap().id;
    let b = stored.iter().find(|s| s.name == "b").unwrap().id;
    let c = stored.iter().find(|s| s.name == "c").unwrap().id;

    // a -> b, b -> c  => 3 distinct nodes (a, b, c); "lonely" excluded.
    store
        .upsert_edge(a, b, "calls", "EXTRACTED", Some(file_id), Some(1))
        .unwrap();
    store
        .upsert_edge(b, c, "calls", "INFERRED", Some(file_id), Some(2))
        .unwrap();

    assert_eq!(store.graph_node_count().unwrap(), 3);
}
