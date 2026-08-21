//! Tests for the disabled-index guard (spec graphCI, T-025, FR-016).
//!
//! FR-016: While the code index is disabled (not active), all graph
//! sub-commands shall print the same "not active" message as existing
//! `/codeindex` sub-commands and return without performing any work.

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

/// The core "not active" message text (without the emoji prefix, which varies
/// between literal ⚠️ and \u{26a0}\u{fe0f} escape sequences).
const NOT_ACTIVE_MARKER: &str = "Code index is not active. Enable it first with `/codeindex on`";

#[test]
fn test_all_graph_query_subcommands_have_not_active_guard() {
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    for subcmd in &["godnodes", "explain", "path", "communities"] {
        let pattern = format!("\"{subcmd}\" =>");
        let pos = source
            .find(&pattern)
            .unwrap_or_else(|| panic!("found `{subcmd}` sub-command"));

        // Search within a generous window for the guard (the sub-command
        // implementation can be quite long before the else branch).
        let window = &source[pos..pos + 6000];
        assert!(
            window.contains(NOT_ACTIVE_MARKER),
            "Graph sub-command `{subcmd}` must have the disabled-index guard message"
        );
        assert!(
            window.contains("\"codeindex: not active\""),
            "Graph sub-command `{subcmd}` must set status to \"codeindex: not active\""
        );
    }
}

#[test]
fn test_graph_build_has_not_active_guard() {
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    let pos = source
        .find("\"graph\" =>")
        .expect("found `graph` sub-command");

    // The graph block should contain the "not active" guard for the build
    // sub-command.
    let window = &source[pos..pos + 6000];
    assert!(
        window.contains(NOT_ACTIVE_MARKER),
        "Graph build sub-command must have the disabled-index guard message"
    );
}

#[test]
fn test_not_active_message_matches_existing_subcommands() {
    // FR-016: the "not active" message must be the SAME as the one used by
    // existing codeindex sub-commands (e.g. reindex, rebuild).
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    // Count occurrences of the marker — there should be at least 6
    // (reindex, graph build, godnodes, explain, path, communities, rebuild).
    let count = source.matches(NOT_ACTIVE_MARKER).count();
    assert!(
        count >= 6,
        "Expected at least 6 occurrences of the not-active message, found {count}"
    );
}

#[test]
fn test_not_active_guard_sets_status_correctly() {
    // All graph sub-commands must set self.status to "codeindex: not active"
    // when the index is disabled.
    let source = std::fs::read_to_string(slash_rs_path()).expect("read slash.rs");

    for subcmd in &["godnodes", "explain", "path", "communities"] {
        let pattern = format!("\"{subcmd}\" =>");
        let pos = source
            .find(&pattern)
            .unwrap_or_else(|| panic!("found `{subcmd}` sub-command"));

        let window = &source[pos..pos + 6000];
        assert!(
            window.contains("\"codeindex: not active\".to_string()"),
            "Graph sub-command `{subcmd}` must set status to \"codeindex: not active\""
        );
    }
}
