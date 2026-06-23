//! Integration tests for `EditTool` on real temp files (WSPLAN M4-T1).
//!
//! Exercises the shared seven-pass matcher end-to-end through the `edit` tool
//! against temp files containing: CRLF line endings, tab indentation, trailing
//! spaces, missing final newline, and blank-line differences. All cases must
//! succeed without `old_str not found`.

use std::sync::Arc;

use ragent_tools_core::edit::EditTool;
use ragent_tools_core::{Tool, ToolContext};
use serde_json::json;
use tempfile::TempDir;

fn ctx(working_dir: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: working_dir.to_path_buf(),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
    }
}

fn write_file(dir: &std::path::Path, name: &str, content: &str) -> std::path::PathBuf {
    let path = dir.join(name);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::write(&path, content).unwrap();
    path
}

/// Helper: run an edit and assert the resulting file content.
async fn assert_edit(
    dir: &std::path::Path,
    file: &str,
    initial: &str,
    old_str: &str,
    new_str: &str,
    expected: &str,
) {
    let path = write_file(dir, file, initial);
    let input = json!({
        "path": file,
        "old_str": old_str,
        "new_str": new_str,
    });
    let _out = EditTool
        .execute(input, &ctx(dir))
        .await
        .expect("edit should succeed");
    let result = std::fs::read_to_string(&path).unwrap();
    assert_eq!(result, expected, "file content after edit");
}

// ── CRLF ──────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_edit_crlf_file_lf_needle() {
    let tmp = TempDir::new().unwrap();
    // File has CRLF; needle uses LF only (as the `read` tool would produce).
    // The matcher's CRLF pass replaces the matched bytes with `new_str` verbatim,
    // so the trailing CRLF on the last matched line is replaced by the needle's
    // LF-only new_str. The file's final newline becomes LF.
    assert_edit(
        tmp.path(),
        "a.rs",
        "fn foo() {\r\n    bar\r\n}\r\n",
        "fn foo() {\n    bar\n}\n",
        "fn foo() {\n    baz\n}\n",
        "fn foo() {\n    baz\n}\n",
    )
    .await;
}

// ── Tab indentation ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_edit_tab_indent_file_space_needle() {
    let tmp = TempDir::new().unwrap();
    // File uses tab indentation; needle drops leading whitespace entirely
    // (leading-WS pass) and the matcher re-applies the tab indent.
    assert_edit(
        tmp.path(),
        "a.rs",
        "\tfn foo() {\n\t\tlet x = 1;\n\t}\n",
        "fn foo() {\n    let x = 1;\n}\n",
        "fn foo() {\n    let x = 2;\n}\n",
        "\tfn foo() {\n\t    let x = 2;\n\t}\n",
    )
    .await;
}

// ── Trailing spaces ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_edit_trailing_spaces_file() {
    let tmp = TempDir::new().unwrap();
    // File has trailing spaces the needle omits (trailing-WS pass).
    assert_edit(
        tmp.path(),
        "a.rs",
        "fn foo() {  \n    bar  \n}\n",
        "fn foo() {\n    bar\n}\n",
        "fn foo() {\n    baz\n}\n",
        "fn foo() {\n    baz\n}\n",
    )
    .await;
}

// ── Missing final newline ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_edit_missing_final_newline_file() {
    let tmp = TempDir::new().unwrap();
    // File lacks a trailing newline; needle includes one (final-newline pass).
    // The matcher's final-newline pass replaces the matched core with `new_str`,
    // so `new_str` (which ends with `\n`) is spliced in place of the core,
    // yielding a file that now ends with a newline.
    assert_edit(
        tmp.path(),
        "a.rs",
        "fn foo() {\n    bar\n}",
        "fn foo() {\n    bar\n}\n",
        "fn foo() {\n    baz\n}\n",
        "fn foo() {\n    baz\n}\n",
    )
    .await;
}

#[tokio::test]
async fn test_edit_extra_final_newline_needle() {
    let tmp = TempDir::new().unwrap();
    // File has trailing newline; needle lacks it (final-newline pass, other dir).
    assert_edit(
        tmp.path(),
        "a.rs",
        "fn foo() {\n    bar\n}\n",
        "fn foo() {\n    bar\n}",
        "fn foo() {\n    baz\n}",
        "fn foo() {\n    baz\n}\n",
    )
    .await;
}

// ── Blank-line differences ────────────────────────────────────────────────────

#[tokio::test]
async fn test_edit_blank_line_in_needle() {
    let tmp = TempDir::new().unwrap();
    // Needle has a leading blank line the file lacks (blank-line pass).
    assert_edit(
        tmp.path(),
        "a.rs",
        "fn foo() {\n    bar\n}\n",
        "\nfn foo() {\n    bar\n}\n",
        "fn foo() {\n    baz\n}\n",
        "fn foo() {\n    baz\n}\n",
    )
    .await;
}

#[tokio::test]
async fn test_edit_blank_line_in_file() {
    let tmp = TempDir::new().unwrap();
    // File has a leading blank line the needle lacks (blank-line pass).
    assert_edit(
        tmp.path(),
        "a.rs",
        "\nfn foo() {\n    bar\n}\n",
        "fn foo() {\n    bar\n}\n",
        "fn foo() {\n    baz\n}\n",
        "\nfn foo() {\n    baz\n}\n",
    )
    .await;
}

// ── Collapsed whitespace (tabs + extra internal spaces) ───────────────────────

#[tokio::test]
async fn test_edit_collapsed_whitespace() {
    let tmp = TempDir::new().unwrap();
    // File uses tab indentation AND extra internal spaces; needle uses spaces
    // for indent and single internal spaces (collapsed pass).
    assert_edit(
        tmp.path(),
        "a.rs",
        "\tlet  x  =  1;\n\tlet  y  =  2;\n",
        "let x = 1;\nlet y = 2;\n",
        "let x = 1;\nlet y = 99;\n",
        "\tlet x = 1;\n\tlet y = 99;\n",
    )
    .await;
}

// ── Exact match (baseline) ────────────────���───────────────────────────────────

#[tokio::test]
async fn test_edit_exact_match_baseline() {
    let tmp = TempDir::new().unwrap();
    assert_edit(
        tmp.path(),
        "a.rs",
        "fn foo() {\n    bar\n}\n",
        "    bar\n",
        "    baz\n",
        "fn foo() {\n    baz\n}\n",
    )
    .await;
}

// ── NotFound surfaces a clear error ───────────────────────────────────────────

#[tokio::test]
async fn test_edit_not_found_errors() {
    let tmp = TempDir::new().unwrap();
    write_file(tmp.path(), "a.rs", "fn foo() { 1 }\n");
    let input = json!({
        "path": "a.rs",
        "old_str": "nonexistent code here",
        "new_str": "x",
    });
    let err = EditTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("missing old_str must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("old_str not found"),
        "error should mention not found: {msg}"
    );
}
