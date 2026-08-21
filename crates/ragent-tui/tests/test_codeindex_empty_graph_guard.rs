//! Tests for the empty-graph guard message (spec graphCI, T-024, FR-015).
//!
//! FR-015: While the `graph_edges` table is empty (no graph has been built),
//! any graph query sub-command (`explain`, `path`, `communities`, `godnodes`)
//! shall print a message instructing the user to run `/codeindex graph build`
//! first.
//!
//! This test verifies that the slash.rs source contains the empty-graph
//! guard for all four graph query sub-commands, and that the guard message
//! instructs the user to run `/codeindex graph build`.

use std::path::PathBuf;

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from(".."))
}

fn slash_rs_path() -> PathBuf {
    workspace_root().join("crates/ragent-tui/src/app/slash.rs")
}

/// The empty-graph guard message must instruct the user to run
/// `/codeindex graph build` (FR-015).
const GUARD_MESSAGE_MARKER: &str = "/codeindex graph build";

#[test]
fn test_all_four_graph_query_subcommands_exist() {
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    for subcmd in &["godnodes", "explain", "path", "communities"] {
        let pattern = format!("\"{subcmd}\" =>");
        assert!(
            source.contains(&pattern),
            "Graph query sub-command `{subcmd}` not found in slash.rs"
        );
    }
}

#[test]
fn test_godnodes_has_empty_graph_guard() {
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    // Find the godnodes block and verify it contains the guard.
    let godnodes_pos = source
        .find("\"godnodes\" =>")
        .expect("found godnodes sub-command");

    // Search within a reasonable window for the guard.
    let window = &source[godnodes_pos..godnodes_pos + 2000];
    assert!(
        window.contains("graph_edge_count"),
        "godnodes sub-command must check edge count for empty-graph guard"
    );
    assert!(
        window.contains(GUARD_MESSAGE_MARKER),
        "godnodes empty-graph guard must mention `/codeindex graph build`"
    );
}

#[test]
fn test_explain_has_empty_graph_guard() {
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    let explain_pos = source
        .find("\"explain\" =>")
        .expect("found explain sub-command");

    let window = &source[explain_pos..explain_pos + 2000];
    assert!(
        window.contains("graph_edge_count"),
        "explain sub-command must check edge count for empty-graph guard"
    );
    assert!(
        window.contains(GUARD_MESSAGE_MARKER),
        "explain empty-graph guard must mention `/codeindex graph build`"
    );
}

#[test]
fn test_path_has_empty_graph_guard() {
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    let path_pos = source.find("\"path\" =>").expect("found path sub-command");

    let window = &source[path_pos..path_pos + 2000];
    assert!(
        window.contains("graph_edge_count"),
        "path sub-command must check edge count for empty-graph guard"
    );
    assert!(
        window.contains(GUARD_MESSAGE_MARKER),
        "path empty-graph guard must mention `/codeindex graph build`"
    );
}

#[test]
fn test_communities_has_empty_graph_guard() {
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    let communities_pos = source
        .find("\"communities\" =>")
        .expect("found communities sub-command");

    let window = &source[communities_pos..communities_pos + 2000];
    assert!(
        window.contains("graph_edge_count"),
        "communities sub-command must check edge count for empty-graph guard"
    );
    assert!(
        window.contains(GUARD_MESSAGE_MARKER),
        "communities empty-graph guard must mention `/codeindex graph build`"
    );
}

#[test]
fn test_empty_graph_guard_uses_edge_count_check() {
    // FR-015: the guard should check edge_count == 0 before running the query.
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    // The guard should use graph_edge_count() == 0 as the condition.
    assert!(
        source.contains("graph_edge_count().unwrap_or(0) == 0"),
        "Empty-graph guard must check graph_edge_count() == 0"
    );
}

#[test]
fn test_graph_edge_count_method_exists_on_codeindex() {
    // The CodeIndex struct must expose a graph_edge_count() method for the
    // TUI guard to use.
    let lib_path = workspace_root().join("crates/ragent-codeindex/src/lib.rs");
    let source = std::fs::read_to_string(lib_path).expect("read lib.rs");
    assert!(
        source.contains("pub fn graph_edge_count"),
        "CodeIndex must expose graph_edge_count() method"
    );
}
