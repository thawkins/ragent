//! Integration tests for `MemoryReplaceTool` whitespace tolerance (WSPLAN M4-T2).
//!
//! Exercises the tool end-to-end against real temp memory blocks containing
//! CRLF line endings and trailing spaces, confirming that `memory_replace`
//! uses the shared seven-pass matcher and no longer fails on the same
//! whitespace quirks that `edit` handles.

use std::sync::Arc;

use ragent_tools_extended::memory_replace::MemoryReplaceTool;
use ragent_tools_extended::{Tool, ToolContext};
use serde_json::json;
use tempfile::TempDir;

fn ctx(working_dir: &std::path::Path) -> ToolContext {
    ToolContext {
        session_id: "test".to_string(),
        working_dir: working_dir.to_path_buf(),
        event_bus: Arc::new(ragent_types::event::EventBus::new(64)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    }
}

/// Create a memory block file under `<working_dir>/.ragent/memory/<label>.md`
/// with the given raw content (no frontmatter required — `from_markdown`
/// treats bare text as block content).
fn write_block(working_dir: &std::path::Path, label: &str, content: &str) -> std::path::PathBuf {
    let dir = working_dir.join(".ragent").join("memory");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join(format!("{label}.md"));
    // Write the block using the canonical MemoryBlock::to_markdown round-trip
    // so from_markdown parses it back identically (frontmatter + body).
    use ragent_tools_extended::memory::block::{BlockScope, MemoryBlock};
    let block = MemoryBlock::new(label, BlockScope::Project).with_content(content.to_string());
    std::fs::write(&path, block.to_markdown()).unwrap();
    path
}

/// Read a block's content via the storage layer (round-trips frontmatter).
fn read_block_content(working_dir: &std::path::Path, label: &str) -> String {
    use ragent_tools_extended::memory::block::BlockScope;
    use ragent_tools_extended::memory::storage::{BlockStorage, FileBlockStorage};
    let storage = FileBlockStorage::new();
    storage
        .load(label, &BlockScope::Project, working_dir)
        .unwrap()
        .unwrap()
        .content
}

// ── CRLF tolerance ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_replace_crlf() {
    let tmp = TempDir::new().unwrap();
    // Block content has CRLF; needle uses LF only.
    write_block(
        tmp.path(),
        "patterns",
        "# Patterns\n\nUse `Result<T, E>` for errors.\r\nPrefer `?` over match.\r\n",
    );

    let input = json!({
        "label": "patterns",
        "old_str": "Use `Result<T, E>` for errors.\nPrefer `?` over match.\n",
        "new_str": "Use `Result<T, E>` for fallible ops.\nPrefer `?` over match.\n",
    });

    let _out = MemoryReplaceTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect("CRLF block should match via CRLF pass");

    let result = read_block_content(tmp.path(), "patterns");
    assert!(
        result.contains("fallible ops"),
        "replacement should apply: {result}"
    );
}

// ── Trailing-space tolerance ──────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_replace_trailing_spaces() {
    let tmp = TempDir::new().unwrap();
    // Block content has trailing spaces the needle omits.
    write_block(
        tmp.path(),
        "patterns",
        "# Patterns\n\nAlways clamp values.  \nNever panic.  \n",
    );

    let input = json!({
        "label": "patterns",
        "old_str": "Always clamp values.\nNever panic.\n",
        "new_str": "Always clamp values.\nNever panic on input.\n",
    });

    let _out = MemoryReplaceTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect("trailing-space block should match via trailing-WS pass");

    let result = read_block_content(tmp.path(), "patterns");
    assert!(
        result.contains("Never panic on input."),
        "replacement should apply: {result}"
    );
}

// ── Dropped leading-indentation tolerance ─────────────────────────────────────

#[tokio::test]
async fn test_memory_replace_dropped_leading_indent() {
    let tmp = TempDir::new().unwrap();
    // Block content is indented; needle drops the leading whitespace.
    write_block(
        tmp.path(),
        "patterns",
        "Notes:\n    - Prefer Result over Option for fallible ops.\n    - Use thiserror for libs.\n",
    );

    let input = json!({
        "label": "patterns",
        "old_str": "- Prefer Result over Option for fallible ops.\n- Use thiserror for libs.\n",
        "new_str": "- Prefer Result for fallible ops.\n- Use anyhow for apps.\n",
    });

    let _out = MemoryReplaceTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect("indented block should match via leading-WS pass");

    let result = read_block_content(tmp.path(), "patterns");
    assert!(
        result.contains("- Prefer Result for fallible ops."),
        "replacement should apply with re-applied indent: {result}"
    );
    assert!(
        result.contains("    - Use anyhow for apps."),
        "common indent should be re-applied: {result}"
    );
}

// ── Final-newline mismatch tolerance ───────────────���──────────────────────────

#[tokio::test]
async fn test_memory_replace_final_newline_mismatch() {
    let tmp = TempDir::new().unwrap();
    // Block content has a trailing newline; needle lacks it.
    write_block(tmp.path(), "patterns", "Remember to run `cargo fmt`.\n");

    let input = json!({
        "label": "patterns",
        "old_str": "Remember to run `cargo fmt`.",
        "new_str": "Remember to run `cargo fmt` and `cargo clippy`.",
    });

    let _out = MemoryReplaceTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect("final-newline mismatch should match via final-newline pass");

    let result = read_block_content(tmp.path(), "patterns");
    assert!(
        result.contains("cargo clippy"),
        "replacement should apply: {result}"
    );
}

// ── Multiple matches still errors ─────────────────────────────────────────────

#[tokio::test]
async fn test_memory_replace_multiple_matches_errors() {
    let tmp = TempDir::new().unwrap();
    write_block(tmp.path(), "patterns", "foo\nfoo\n");

    let input = json!({
        "label": "patterns",
        "old_str": "foo",
        "new_str": "bar",
    });

    let err = MemoryReplaceTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("multiple matches must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("2 times"),
        "error should report the match count: {msg}"
    );
}

// ── Not found still errors ────────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_replace_not_found_errors() {
    let tmp = TempDir::new().unwrap();
    write_block(tmp.path(), "patterns", "bar\n");

    let input = json!({
        "label": "patterns",
        "old_str": "baz",
        "new_str": "qux",
    });

    let err = MemoryReplaceTool
        .execute(input, &ctx(tmp.path()))
        .await
        .expect_err("not found must error");
    let msg = format!("{err}");
    assert!(
        msg.contains("old_str not found"),
        "error should mention not found: {msg}"
    );
}
