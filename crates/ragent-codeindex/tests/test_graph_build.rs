#![allow(clippy::assert_is_empty)]
//! Tests for `CodeIndex::build_graph()` (spec graphCI, T-016).
//!
//! Covers FR-009 (graph build reports edge counts distinguishing EXTRACTED
//! from INFERRED) and FR-021 (no existing sub-command is altered — this
//! test verifies the new method is additive).

use ragent_codeindex::CodeIndex;
use ragent_codeindex::graph::BuildResult;
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

/// Write a small Rust file with two functions where one calls the other.
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
fn test_build_graph_returns_build_result() {
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let config = make_config(&dir);
    let idx = CodeIndex::open(&config).unwrap();

    // Index the files first so there are symbols to derive edges from.
    idx.full_reindex().unwrap();

    // Now build the graph explicitly.
    let result = idx.build_graph().unwrap();
    assert!(result.edges_total > 0, "should have at least one edge");
}

#[test]
fn test_build_result_has_extracted_and_inferred_counts() {
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let config = make_config(&dir);
    let idx = CodeIndex::open(&config).unwrap();
    idx.full_reindex().unwrap();

    let result = idx.build_graph().unwrap();
    // The sum of extracted + inferred should equal total.
    assert_eq!(
        result.edges_extracted + result.edges_inferred,
        result.edges_total,
        "edges_extracted + edges_inferred should equal edges_total"
    );
}

#[test]
fn test_build_graph_on_empty_index() {
    let dir = TempDir::new().unwrap();
    let config = make_config(&dir);
    let idx = CodeIndex::open(&config).unwrap();

    // No files indexed — build should return zero edges.
    let result = idx.build_graph().unwrap();
    assert_eq!(result.edges_total, 0);
    assert_eq!(result.edges_extracted, 0);
    assert_eq!(result.edges_inferred, 0);
}

#[test]
fn test_build_graph_is_idempotent() {
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let config = make_config(&dir);
    let idx = CodeIndex::open(&config).unwrap();
    idx.full_reindex().unwrap();

    let first = idx.build_graph().unwrap();
    let second = idx.build_graph().unwrap();
    // Building twice should not duplicate edges.
    assert_eq!(first.edges_total, second.edges_total);
}

#[test]
fn test_build_result_display_shows_counts() {
    let result = BuildResult {
        edges_total: 10,
        edges_extracted: 7,
        edges_inferred: 3,
        elapsed_ms: 42,
    };
    let s = format!("{result}");
    assert!(s.contains("10 edges"));
    assert!(s.contains("7 EXTRACTED"));
    assert!(s.contains("3 INFERRED"));
}

#[test]
fn test_build_graph_does_not_alter_existing_subcommands() {
    // FR-021: existing sub-commands must continue to work.
    // Verify that status, search, and symbols still work after build_graph.
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let config = make_config(&dir);
    let idx = CodeIndex::open(&config).unwrap();
    idx.full_reindex().unwrap();

    // Build the graph.
    idx.build_graph().unwrap();

    // Status should still work.
    let stats = idx.status().unwrap();
    assert!(stats.files_indexed > 0);

    // Search should still work.
    use ragent_codeindex::types::SearchQuery;
    let hits = idx.search(&SearchQuery::new("callee")).unwrap();
    assert!(!hits.is_empty(), "search should find 'callee'");

    // Symbols should still work.
    use ragent_codeindex::types::SymbolFilter;
    let symbols = idx.symbols(&SymbolFilter::default()).unwrap();
    assert!(!symbols.is_empty(), "symbols query should return results");
}
