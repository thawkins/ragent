//! Integration tests for `MultiEditTool` (WSPLAN Milestone 3).
//!
//! Covers: two edits in one file, edits across two files, overlap detection,
//! JSON-order independence (edits applied highest-offset-first), and
//! whitespace-tolerant batch edits via the shared seven-pass matcher.

use std::sync::Arc;

use ragent_tools_core::multiedit::MultiEditTool;
use ragent_tools_core::{Tool, ToolContext};
use serde_json::json;
use tempfile::TempDir;

/// Build a `ToolContext` rooted at the given working directory.
fn ctx(working_dir: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: working_dir.to_path_buf(),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
    }
}

/// Write a file relative to `dir` with the given content.
fn write_file(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    path
}

// ── Two edits in one file ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_two_edits_one_file() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(
        tmp.path(),
        "a.rs",
        "fn a() { 1 }\nfn b() { 2 }\nfn c() { 3 }\n",
    );

    let input = json!({
        "edits": [
            { "path": "a.rs", "old_str": "fn a() { 1 }", "new_str": "fn a() { 10 }" },
            { "path": "a.rs", "old_str": "fn c() { 3 }", "new_str": "fn c() { 30 }" }
        ]
    });

    let out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 2 edits across 1 file");

    let result = std::fs::read_to_string(&path).unwrap();
    assert_eq!(result, "fn a() { 10 }\nfn b() { 2 }\nfn c() { 30 }\n");
}

// ── Edits across two files ────────────────────────────────────────────────────

#[tokio::test]
async fn test_edits_across_two_files() {
    let tmp = TempDir::new().unwrap();
    let p1 = write_file(tmp.path(), "a.rs", "alpha\n");
    let p2 = write_file(tmp.path(), "b.rs", "beta\n");

    let input = json!({
        "edits": [
            { "path": "a.rs", "old_str": "alpha", "new_str": "ALPHA" },
            { "path": "b.rs", "old_str": "beta", "new_str": "BETA" }
        ]
    });

    let out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 2 edits across 2 files");
    assert_eq!(std::fs::read_to_string(&p1).unwrap(), "ALPHA\n");
    assert_eq!(std::fs::read_to_string(&p2).unwrap(), "BETA\n");
}

// ── Overlap detection ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_overlap_detection_rejects() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "a.rs", "fn foo() { let x = 1; let y = 2; }\n");

    // Edit A replaces "let x = 1;" (inside the line). Edit B replaces the whole
    // line containing it. Their original-content byte ranges overlap.
    let input = json!({
        "edits": [
            { "path": "a.rs", "old_str": "let x = 1;", "new_str": "let x = 10;" },
            { "path": "a.rs", "old_str": "fn foo() { let x = 1; let y = 2; }", "new_str": "fn foo() { let x = 10; let y = 20; }" }
        ]
    });

    let err = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("overlapping edits must be rejected");

    let msg = format!("{err}");
    assert!(
        msg.contains("overlap"),
        "error should mention overlap: {msg}"
    );
    assert!(msg.contains("a.rs"), "error should name the file: {msg}");
    assert!(
        msg.contains("Edits 0 and 1") || msg.contains("Edits 1 and 0"),
        "error should name the edit indices: {msg}"
    );
}

// ── JSON-order independence ───────────────────────────────────────────────────

#[tokio::test]
async fn test_json_order_independence() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "line1\nline2\nline3\n");

    // Supply edits in reverse order (highest-offset first). The tool should
    // still produce the same result as forward order because it sorts
    // end-to-start internally.
    let input_reverse = json!({
        "edits": [
            { "path": "a.rs", "old_str": "line3", "new_str": "LINE3" },
            { "path": "a.rs", "old_str": "line1", "new_str": "LINE1" }
        ]
    });

    let out = MultiEditTool
        .execute(input_reverse, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 2 edits across 1 file");
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "LINE1\nline2\nLINE3\n"
    );

    // Now reset and supply in forward order — same result.
    std::fs::write(&path, "line1\nline2\nline3\n").unwrap();
    let input_forward = json!({
        "edits": [
            { "path": "a.rs", "old_str": "line1", "new_str": "LINE1" },
            { "path": "a.rs", "old_str": "line3", "new_str": "LINE3" }
        ]
    });

    let _out = MultiEditTool
        .execute(input_forward, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(
        std::fs::read_to_string(&path).unwrap(),
        "LINE1\nline2\nLINE3\n"
    );
}

// ── Whitespace-tolerant batch edits ───────────────────────────────────────────

#[tokio::test]
async fn test_whitespace_tolerant_batch_edits() {
    let tmp = TempDir::new().unwrap();
    // File has trailing spaces the needle omits, and uses CRLF for one line.
    let path = write_file(tmp.path(), "a.rs", "fn a() {  \r\n    bar  \n}\n");

    let input = json!({
        "edits": [
            { "path": "a.rs", "old_str": "fn a() {\n    bar\n}\n", "new_str": "fn a() {\n    baz\n}\n" }
        ]
    });

    let out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 1 edit across 1 file");
    let result = std::fs::read_to_string(&path).unwrap();
    assert!(
        result.contains("baz"),
        "replacement should apply via trailing-WS pass: {result}"
    );
}

// ── Non-overlapping adjacent edits (touching ranges allowed) ──────────────────

#[tokio::test]
async fn test_adjacent_touching_edits_allowed() {
    let tmp = TempDir::new().unwrap();
    let path = write_file(tmp.path(), "a.rs", "AABB\n");

    // Edit 0 replaces "AA" (bytes 0..2). Edit 1 replaces "BB" (bytes 2..4).
    // They touch but do not overlap.
    let input = json!({
        "edits": [
            { "path": "a.rs", "old_str": "AA", "new_str": "XX" },
            { "path": "a.rs", "old_str": "BB", "new_str": "YY" }
        ]
    });

    let out = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .unwrap();
    assert_eq!(out.content, "Applied 2 edits across 1 file");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "XXYY\n");
}

// ── NotFound surfaces pass + closest line ─────────────────────────────────────

#[tokio::test]
async fn test_not_found_error_includes_pass() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "a.rs", "fn a() { 1 }\n");

    let input = json!({
        "edits": [
            { "path": "a.rs", "old_str": "nonexistent code here", "new_str": "x" }
        ]
    });

    let err = MultiEditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("missing old_str must error");

    let msg = format!("{err}");
    assert!(
        msg.contains("Edit 0"),
        "error should name the edit index: {msg}"
    );
    assert!(
        msg.contains("matching pass"),
        "error should mention the matching pass: {msg}"
    );
}
