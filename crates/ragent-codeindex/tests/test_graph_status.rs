//! Tests for `CodeIndex::graph_status()` (spec graphCI).
//!
//! Verifies that the graph status aggregation used by the
//! `/codeindex status` TUI sub-command reports correct edge/node/community
//! counts after a graph build, and reports an empty (zeroed) graph before
//! the graph has been built.

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
fn test_graph_status_empty_index() {
    let dir = TempDir::new().unwrap();
    // No source files written — reindex finds nothing, so the graph is empty.
    let config = make_config(&dir);
    let idx = CodeIndex::open(&config).unwrap();
    idx.full_reindex().unwrap();

    // With no symbols there are no edges and no nodes.
    let gs = idx.graph_status().unwrap();
    assert_eq!(gs.total_edges, 0);
    assert_eq!(gs.edges_extracted, 0);
    assert_eq!(gs.edges_inferred, 0);
    assert_eq!(gs.nodes, 0);
    assert_eq!(gs.communities, 0);
    assert_eq!(gs.edges_calls, 0);
    assert_eq!(gs.edges_imports, 0);
}

#[test]
fn test_graph_status_after_build() {
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let config = make_config(&dir);
    let idx = CodeIndex::open(&config).unwrap();
    idx.full_reindex().unwrap();
    let build = idx.build_graph().unwrap();
    assert!(build.edges_total > 0, "graph should have edges");

    let gs = idx.graph_status().unwrap();
    assert_eq!(gs.total_edges, build.edges_total as u64);
    assert_eq!(
        gs.edges_extracted + gs.edges_inferred,
        gs.total_edges,
        "extracted + inferred should equal total"
    );
    // The sample file has one caller->callee call, so at least one node and
    // at least one "calls" edge must be present.
    assert!(
        gs.nodes >= 2,
        "should have at least 2 nodes, got {}",
        gs.nodes
    );
    assert!(
        gs.edges_calls >= 1,
        "should have at least one calls edge, got {}",
        gs.edges_calls
    );
}

#[test]
fn test_graph_status_display() {
    let dir = TempDir::new().unwrap();
    write_rust_file(&dir);
    let config = make_config(&dir);
    let idx = CodeIndex::open(&config).unwrap();
    idx.full_reindex().unwrap();
    idx.build_graph().unwrap();

    let gs = idx.graph_status().unwrap();
    let rendered = format!("{gs}");
    assert!(
        rendered.contains("Total edges:"),
        "display missing total edges"
    );
    assert!(rendered.contains("Nodes:"), "display missing nodes");
    assert!(rendered.contains("EXTRACTED:"), "display missing extracted");
    assert!(rendered.contains("INFERRED:"), "display missing inferred");
    assert!(
        rendered.contains("Communities:"),
        "display missing communities"
    );
    assert!(rendered.contains("calls:"), "display missing calls kind");
}
