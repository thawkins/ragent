//! Tests for the file-based `memory_write` and `memory_read` tools.
//!
//! These tests verify that content is actually persisted to `.ragent/memory/`
//! and that the legacy/block paths both return success metadata. They also
//! confirm that `memory_write` ignores unsupported parameters such as `category`
//! and `tags` rather than failing or misattributing them.

use ragent_tools_extended::event::EventBus;
use ragent_tools_extended::memory_write::{MemoryReadTool, MemoryWriteTool};
use ragent_tools_extended::{Tool, ToolContext};
use serde_json::json;
use std::sync::Arc;
use tempfile::TempDir;

fn ctx_for(dir: &TempDir) -> ToolContext {
    ToolContext {
        session_id: "test-session".to_string(),
        working_dir: dir.path().to_path_buf(),
        event_bus: Arc::new(EventBus::new(16)),
        storage: None,
        code_index: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
    }
}

#[tokio::test]
async fn test_memory_write_legacy_appends_to_memory_md() {
    let tmp = TempDir::new().unwrap();
    let tool = MemoryWriteTool;
    let ctx = ctx_for(&tmp);

    let out = tool
        .execute(
            json!({
                "content": "First legacy note",
                "scope": "project"
            }),
            &ctx,
        )
        .await
        .expect("execute");

    assert!(
        out.content.contains("Memory written"),
        "output should report success: {}",
        out.content
    );

    let file = tmp.path().join(".ragent").join("memory").join("MEMORY.md");
    assert!(
        file.exists(),
        "MEMORY.md should exist at {}",
        file.display()
    );
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("First legacy note"),
        "file should contain the note: {text}"
    );
}

#[tokio::test]
async fn test_memory_write_block_creates_labelled_file() {
    let tmp = TempDir::new().unwrap();
    let tool = MemoryWriteTool;
    let ctx = ctx_for(&tmp);

    let out = tool
        .execute(
            json!({
                "content": "Prefer Result<T, E>",
                "scope": "project",
                "label": "patterns",
                "description": "Rust error-handling patterns",
                "mode": "overwrite"
            }),
            &ctx,
        )
        .await
        .expect("execute");

    assert!(
        out.content.contains("Memory written"),
        "output should report success: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["label"], "patterns");
    assert_eq!(meta["mode"], "overwrite");

    let file = tmp
        .path()
        .join(".ragent")
        .join("memory")
        .join("patterns.md");
    assert!(
        file.exists(),
        "patterns.md should exist at {}",
        file.display()
    );
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("Prefer Result<T, E>"),
        "file should contain the content: {text}"
    );
    assert!(
        text.contains("Rust error-handling patterns"),
        "file should contain the description in frontmatter: {text}"
    );
    assert!(
        text.starts_with("---"),
        "block file should start with YAML frontmatter: {text}"
    );
}

#[tokio::test]
async fn test_memory_write_ignores_category_and_tags() {
    let tmp = TempDir::new().unwrap();
    let tool = MemoryWriteTool;
    let ctx = ctx_for(&tmp);

    let out = tool
        .execute(
            json!({
                "content": "A plain note",
                "label": "notes",
                "mode": "overwrite",
                "category": "pattern",
                "tags": ["rust"],
                "confidence": 0.9
            }),
            &ctx,
        )
        .await
        .expect("execute should ignore unsupported params");

    assert!(
        out.content.contains("Memory written"),
        "output should report success: {}",
        out.content
    );

    let file = tmp.path().join(".ragent").join("memory").join("notes.md");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("A plain note"),
        "file should still contain the content: {text}"
    );
    assert!(
        !text.contains("category"),
        "file should not contain unsupported category frontmatter: {text}"
    );
    assert!(
        !text.contains("tags"),
        "file should not contain unsupported tags frontmatter: {text}"
    );
}

#[tokio::test]
async fn test_memory_read_block_reads_back_written_content() {
    let tmp = TempDir::new().unwrap();
    let write_tool = MemoryWriteTool;
    let read_tool = MemoryReadTool;
    let ctx = ctx_for(&tmp);

    write_tool
        .execute(
            json!({
                "content": "Readback test",
                "label": "readback",
                "mode": "overwrite"
            }),
            &ctx,
        )
        .await
        .expect("write");

    let out = read_tool
        .execute(json!({"label": "readback"}), &ctx)
        .await
        .expect("read");

    assert!(
        out.content.contains("Readback test"),
        "read output should contain the note: {}",
        out.content
    );
}

#[tokio::test]
async fn test_memory_write_block_appends_with_timestamp() {
    let tmp = TempDir::new().unwrap();
    let tool = MemoryWriteTool;
    let ctx = ctx_for(&tmp);

    tool.execute(
        json!({
            "content": "First entry",
            "label": "journal",
            "mode": "overwrite"
        }),
        &ctx,
    )
    .await
    .unwrap();

    tool.execute(
        json!({
            "content": "Second entry",
            "label": "journal",
            "mode": "append"
        }),
        &ctx,
    )
    .await
    .unwrap();

    let file = tmp.path().join(".ragent").join("memory").join("journal.md");
    let text = std::fs::read_to_string(&file).unwrap();
    assert!(
        text.contains("First entry"),
        "file should contain first entry: {text}"
    );
    assert!(
        text.contains("Second entry"),
        "file should contain second entry: {text}"
    );
    assert!(
        text.matches("!-- ").count() >= 1,
        "append should add a timestamp marker: {text}"
    );
}
