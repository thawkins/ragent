//! Tests for the graph readiness fields added to [`IndexStats`]
//! (`graph_total_edges`, `graph_nodes`, `graph_communities`).
//!
//! `CodeIndex::status()` and `try_status()` must report the semantic edge
//! graph dataset alongside the FTS/SQLite stats so `codeindex_status` can
//! tell the model whether the graph has been built.

use ragent_codeindex::CodeIndex;
use ragent_codeindex::types::CodeIndexConfig;
use std::fs;
use tempfile::TempDir;

fn make_config(dir: &TempDir) -> CodeIndexConfig {
    CodeIndexConfig {
        enabled: true,
        project_root: dir.path().to_path_buf(),
        index_dir: dir.path().join(".ragent/codeindex"),
        scan_config: ragent_codeindex::types::ScanConfig::default(),
    }
}

fn write_rust_file(dir: &TempDir) {
    let src_dir = dir.path().join("src");
    fs::create_dir_all(&src_dir).unwrap();
    fs::write(
        src_dir.join("main.rs"),
        r#"fn caller() {
    callee();
}

fn callee() {
    println!("hello");
}
"#,
    )
    .unwrap();
}

#[test]
fn test_status_graph_zero_before_graph_build() {
    let dir = TempDir::new().unwrap();
    // No source files written — reindex finds nothing, so the graph is empty.
    let idx = CodeIndex::open(&make_config(&dir)).unwrap();
    idx.full_reindex().unwrap();

    let stats = idx.status().unwrap();
    assert_eq!(stats.files_indexed, 0, "nothing indexed");
    assert_eq!(stats.graph_total_edges, 0, "graph not built yet");
    assert_eq!(stats.graph_nodes, 0);
    assert_eq!(stats.graph_communities, 0);

    let stats = idx.try_status().unwrap();
    assert_eq!(stats.graph_total_edges, 0);
    assert_eq!(stats.graph_nodes, 0);
    assert_eq!(stats.graph_communities, 0);
}

#[test]
fn test_status_graph_counts_after_graph_build() {
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let idx = CodeIndex::open(&make_config(&dir)).unwrap();
    idx.full_reindex().unwrap();

    let build = idx.build_graph().unwrap();
    assert!(build.edges_total > 0, "graph should have edges");

    let gs = idx.graph_status().unwrap();

    let stats = idx.status().unwrap();
    assert_eq!(stats.graph_total_edges, gs.total_edges);
    assert_eq!(stats.graph_nodes, gs.nodes);
    assert_eq!(stats.graph_communities, gs.communities);

    let stats = idx.try_status().unwrap();
    assert_eq!(stats.graph_total_edges, gs.total_edges);
    assert_eq!(stats.graph_nodes, gs.nodes);
    assert_eq!(stats.graph_communities, gs.communities);
}

#[test]
fn test_status_fts_doc_count_present() {
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let idx = CodeIndex::open(&make_config(&dir)).unwrap();
    idx.full_reindex().unwrap();

    let stats = idx.status().unwrap();
    assert!(stats.files_indexed > 0);
    assert!(
        stats.fts_doc_count > 0,
        "FTS should be populated after a full reindex"
    );
}
// ── try_graph_status: non-blocking graph stats (FR-017) ─────────────────────

#[test]
fn test_try_graph_status_matches_blocking_variant() {
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let idx = CodeIndex::open(&make_config(&dir)).unwrap();
    idx.full_reindex().unwrap();

    let build = idx.build_graph().unwrap();
    assert!(build.edges_total > 0, "graph should have edges");

    let blocking = idx.graph_status().unwrap();
    let non_blocking = idx.try_graph_status().expect("store lock is free");
    assert_eq!(non_blocking.total_edges, blocking.total_edges);
    assert_eq!(non_blocking.nodes, blocking.nodes);
    assert_eq!(non_blocking.communities, blocking.communities);
    assert_eq!(non_blocking.edges_calls, blocking.edges_calls);
    assert_eq!(non_blocking.edges_extracted, blocking.edges_extracted);
    assert_eq!(non_blocking.edges_inferred, blocking.edges_inferred);
}

#[test]
fn test_try_graph_status_returns_none_when_store_locked() {
    let dir = TempDir::new().unwrap();
    let idx = CodeIndex::open(&make_config(&dir)).unwrap();

    // Hold the store mutex and verify the non-blocking probe degrades to None
    // immediately instead of blocking.
    let _guard = idx.try_lock_store_for_test().expect("store lock");
    assert!(
        idx.try_graph_status().is_none(),
        "try_graph_status must return None while the store lock is held"
    );
}

#[test]
fn test_try_status_returns_none_when_store_locked() {
    let dir = TempDir::new().unwrap();
    let idx = CodeIndex::open(&make_config(&dir)).unwrap();

    let _guard = idx.try_lock_store_for_test().expect("store lock");
    assert!(
        idx.try_status().is_none(),
        "try_status must return None while the store lock is held"
    );
}
