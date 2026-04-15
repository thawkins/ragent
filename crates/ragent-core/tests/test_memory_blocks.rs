//! Integration tests for Milestone 1 — Memory Block System.
//!
//! Tests the memory tools (memory_write, memory_read, memory_replace,
//! memory_migrate) and their interaction with the block storage system.

use std::path::PathBuf;
use std::sync::Arc;

use ragent_core::tool::{Tool, ToolContext};

/// Helper: create a ToolContext with a temp working directory.
fn make_ctx(tmp: &tempfile::TempDir) -> ToolContext {
    ToolContext {
        session_id: "test-session".to_string(),
        working_dir: PathBuf::from(tmp.path()),
        event_bus: Arc::new(ragent_core::event::EventBus::new(100)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    }
}

// ── Legacy backward compatibility ────────────────────────────────────────────

#[tokio::test]
async fn test_memory_write_legacy_no_label() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = make_ctx(&tmp);

    let write_tool = ragent_core::tool::memory_write::MemoryWriteTool;
    let input = serde_json::json!({
        "content": "Hello from legacy mode",
        "scope": "project"
    });

    let result = write_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("Hello from legacy mode"));
    assert!(result.content.contains("scope: project"));

    // Verify the file was written.
    let mem_path = tmp.path().join(".ragent/memory/MEMORY.md");
    assert!(mem_path.exists());
    let content = std::fs::read_to_string(&mem_path).unwrap();
    assert!(content.contains("Hello from legacy mode"));
}

#[tokio::test]
async fn test_memory_read_legacy_no_label() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = make_ctx(&tmp);

    // Write a legacy file first.
    let mem_dir = tmp.path().join(".ragent/memory");
    std::fs::create_dir_all(&mem_dir).unwrap();
    std::fs::write(mem_dir.join("MEMORY.md"), "Legacy content here.").unwrap();

    let read_tool = ragent_core::tool::memory_write::MemoryReadTool;
    let input = serde_json::json!({
        "scope": "project"
    });

    let result = read_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("Legacy content here."));
}

// ── Block-based write and read ────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_write_block_append() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = make_ctx(&tmp);

    let write_tool = ragent_core::tool::memory_write::MemoryWriteTool;

    // Write a new block.
    let input = serde_json::json!({
        "content": "First entry",
        "label": "patterns",
        "description": "Coding patterns",
        "scope": "project"
    });
    let result = write_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("label: patterns"));
    assert!(result.content.contains("mode: append"));

    // Append to the block.
    let input2 = serde_json::json!({
        "content": "Second entry",
        "label": "patterns",
        "scope": "project"
    });
    let result2 = write_tool.execute(input2, &ctx).await.unwrap();
    assert!(result2.content.contains("label: patterns"));

    // Read the block.
    let read_tool = ragent_core::tool::memory_write::MemoryReadTool;
    let read_input = serde_json::json!({
        "label": "patterns",
        "scope": "project"
    });
    let read_result = read_tool.execute(read_input, &ctx).await.unwrap();
    assert!(read_result.content.contains("First entry"));
    assert!(read_result.content.contains("Second entry"));
}

#[tokio::test]
async fn test_memory_write_block_overwrite() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = make_ctx(&tmp);

    let write_tool = ragent_core::tool::memory_write::MemoryWriteTool;

    // Write initial content.
    let input = serde_json::json!({
        "content": "Old content",
        "label": "notes",
        "scope": "project"
    });
    write_tool.execute(input, &ctx).await.unwrap();

    // Overwrite.
    let input2 = serde_json::json!({
        "content": "New content only",
        "label": "notes",
        "scope": "project",
        "mode": "overwrite"
    });
    write_tool.execute(input2, &ctx).await.unwrap();

    // Read and verify only new content.
    let read_tool = ragent_core::tool::memory_write::MemoryReadTool;
    let read_input = serde_json::json!({
        "label": "notes",
        "scope": "project"
    });
    let read_result = read_tool.execute(read_input, &ctx).await.unwrap();
    assert!(read_result.content.contains("New content only"));
    assert!(!read_result.content.contains("Old content"));
}

#[tokio::test]
async fn test_memory_read_block_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = make_ctx(&tmp);

    let read_tool = ragent_core::tool::memory_write::MemoryReadTool;
    let input = serde_json::json!({
        "label": "nonexistent",
        "scope": "project"
    });

    let result = read_tool.execute(input, &ctx).await.unwrap();
    assert!(
        result
            .content
            .contains("No memory block 'nonexistent' found")
    );
}

// ── memory_replace ───────────────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_replace_basic() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = make_ctx(&tmp);

    let write_tool = ragent_core::tool::memory_write::MemoryWriteTool;
    let replace_tool = ragent_core::tool::memory_replace::MemoryReplaceTool;

    // Create a block.
    let input = serde_json::json!({
        "content": "Use 2-space indent for JavaScript.",
        "label": "conventions",
        "scope": "project",
        "mode": "overwrite"
    });
    write_tool.execute(input, &ctx).await.unwrap();

    // Replace within the block.
    let replace_input = serde_json::json!({
        "label": "conventions",
        "old_str": "2-space indent",
        "new_str": "4-space indent",
        "scope": "project"
    });
    let result = replace_tool.execute(replace_input, &ctx).await.unwrap();
    assert!(result.content.contains("Replaced 1 occurrence"));

    // Verify.
    let read_tool = ragent_core::tool::memory_write::MemoryReadTool;
    let read_input = serde_json::json!({
        "label": "conventions",
        "scope": "project"
    });
    let read_result = read_tool.execute(read_input, &ctx).await.unwrap();
    assert!(read_result.content.contains("4-space indent"));
    assert!(!read_result.content.contains("2-space indent"));
}

#[tokio::test]
async fn test_memory_replace_not_found() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = make_ctx(&tmp);

    let write_tool = ragent_core::tool::memory_write::MemoryWriteTool;
    let replace_tool = ragent_core::tool::memory_replace::MemoryReplaceTool;

    // Create a block.
    let input = serde_json::json!({
        "content": "Some content",
        "label": "myblock",
        "scope": "project",
        "mode": "overwrite"
    });
    write_tool.execute(input, &ctx).await.unwrap();

    // Try to replace non-existent text.
    let replace_input = serde_json::json!({
        "label": "myblock",
        "old_str": "does not exist",
        "new_str": "replacement",
        "scope": "project"
    });
    let result = replace_tool.execute(replace_input, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

#[tokio::test]
async fn test_memory_replace_ambiguous() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = make_ctx(&tmp);

    let write_tool = ragent_core::tool::memory_write::MemoryWriteTool;
    let replace_tool = ragent_core::tool::memory_replace::MemoryReplaceTool;

    // Create a block with duplicate text.
    let input = serde_json::json!({
        "content": "foo bar foo",
        "label": "dupe",
        "scope": "project",
        "mode": "overwrite"
    });
    write_tool.execute(input, &ctx).await.unwrap();

    // Try to replace "foo" which appears twice.
    let replace_input = serde_json::json!({
        "label": "dupe",
        "old_str": "foo",
        "new_str": "baz",
        "scope": "project"
    });
    let result = replace_tool.execute(replace_input, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("found 2 times"));
}

// ── memory_migrate ───────────────────────────────────���───────────────────────

#[tokio::test]
async fn test_memory_migrate_dry_run() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = make_ctx(&tmp);

    // Create a MEMORY.md with headings.
    let mem_dir = tmp.path().join(".ragent/memory");
    std::fs::create_dir_all(&mem_dir).unwrap();
    std::fs::write(
        mem_dir.join("MEMORY.md"),
        "# Memory\n\n## Patterns\n\nUse Result.\n\n## Conventions\n\n4 spaces.\n",
    )
    .unwrap();

    let migrate_tool = ragent_core::tool::memory_migrate::MemoryMigrateTool;
    let input = serde_json::json!({
        "scope": "project",
        "execute": false
    });

    let result = migrate_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("dry-run"));
    assert!(result.content.contains("patterns"));
    assert!(result.content.contains("conventions"));

    // Verify no blocks were created.
    assert!(!mem_dir.join("patterns.md").exists());
    assert!(!mem_dir.join("conventions.md").exists());
}

#[tokio::test]
async fn test_memory_migrate_execute() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = make_ctx(&tmp);

    // Create a MEMORY.md with headings.
    let mem_dir = tmp.path().join(".ragent/memory");
    std::fs::create_dir_all(&mem_dir).unwrap();
    std::fs::write(
        mem_dir.join("MEMORY.md"),
        "# Memory\n\n## Alpha\n\nAlpha content.\n\n## Beta\n\nBeta content.\n",
    )
    .unwrap();

    let migrate_tool = ragent_core::tool::memory_migrate::MemoryMigrateTool;
    let input = serde_json::json!({
        "scope": "project",
        "execute": true
    });

    let result = migrate_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("executed"));

    // Verify blocks were created.
    assert!(mem_dir.join("alpha.md").exists());
    assert!(mem_dir.join("beta.md").exists());

    // Original MEMORY.md should still exist.
    assert!(mem_dir.join("MEMORY.md").exists());
}

#[tokio::test]
async fn test_memory_migrate_no_file() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = make_ctx(&tmp);

    let migrate_tool = ragent_core::tool::memory_migrate::MemoryMigrateTool;
    let input = serde_json::json!({
        "scope": "project"
    });

    let result = migrate_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("No MEMORY.md found"));
}

// ── Content limit enforcement ──────────────────────────────────────────���─────

#[tokio::test]
async fn test_memory_write_block_limit_exceeded() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = make_ctx(&tmp);

    let write_tool = ragent_core::tool::memory_write::MemoryWriteTool;

    let input = serde_json::json!({
        "content": "This is more than ten bytes of content",
        "label": "limited",
        "scope": "project",
        "limit": 10,
        "mode": "overwrite"
    });

    let result = write_tool.execute(input, &ctx).await;
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("exceeds block limit")
    );
}

// ── Label validation ─────────────────────────────────────────────────────────

#[tokio::test]
async fn test_memory_write_invalid_label() {
    let tmp = tempfile::tempdir().unwrap();
    let ctx = make_ctx(&tmp);

    let write_tool = ragent_core::tool::memory_write::MemoryWriteTool;

    let input = serde_json::json!({
        "content": "test",
        "label": "1invalid",
        "scope": "project"
    });

    let result = write_tool.execute(input, &ctx).await;
    assert!(result.is_err());
}

// ── Read-only block protection ────���──────────────────────────────────────────

#[tokio::test]
async fn test_read_only_block_cannot_be_written() {
    let tmp = tempfile::tempdir().unwrap();
    let wd = PathBuf::from(tmp.path());

    // Create a read-only block directly via storage.
    use ragent_core::memory::block::{BlockScope, MemoryBlock};
    use ragent_core::memory::storage::{BlockStorage, FileBlockStorage};

    let storage = FileBlockStorage::new();
    let block = MemoryBlock::new("readonly-test", BlockScope::Project)
        .with_content("Original content".to_string())
        .with_read_only(true);
    storage.save(&block, &wd).unwrap();

    // Try to write via the tool.
    let ctx = ToolContext {
        session_id: "test-session".to_string(),
        working_dir: wd,
        event_bus: Arc::new(ragent_core::event::EventBus::new(100)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };

    let write_tool = ragent_core::tool::memory_write::MemoryWriteTool;
    let input = serde_json::json!({
        "content": "Modified content",
        "label": "readonly-test",
        "scope": "project",
        "mode": "overwrite"
    });

    let result = write_tool.execute(input, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("read-only"));
}

#[tokio::test]
async fn test_read_only_block_cannot_be_replaced() {
    let tmp = tempfile::tempdir().unwrap();
    let wd = PathBuf::from(tmp.path());

    use ragent_core::memory::block::{BlockScope, MemoryBlock};
    use ragent_core::memory::storage::{BlockStorage, FileBlockStorage};

    let storage = FileBlockStorage::new();
    let block = MemoryBlock::new("readonly-replace", BlockScope::Project)
        .with_content("Original content here".to_string())
        .with_read_only(true);
    storage.save(&block, &wd).unwrap();

    let ctx = ToolContext {
        session_id: "test-session".to_string(),
        working_dir: wd,
        event_bus: Arc::new(ragent_core::event::EventBus::new(100)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };

    let replace_tool = ragent_core::tool::memory_replace::MemoryReplaceTool;
    let input = serde_json::json!({
        "label": "readonly-replace",
        "old_str": "Original",
        "new_str": "Modified",
        "scope": "project"
    });

    let result = replace_tool.execute(input, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("read-only"));
}

// ── Prompt auto-loading ─────────────────────────────────────────────────────

#[test]
fn test_prompt_includes_memory_blocks() {
    let tmp = tempfile::tempdir().unwrap();
    let wd = PathBuf::from(tmp.path());

    // Create a memory block.
    use ragent_core::memory::block::{BlockScope, MemoryBlock};
    use ragent_core::memory::storage::{BlockStorage, FileBlockStorage};

    let storage = FileBlockStorage::new();
    let block = MemoryBlock::new("test-block", BlockScope::Project)
        .with_content("This is test block content.".to_string())
        .with_description("A test block".to_string());
    storage.save(&block, &wd).unwrap();

    // Test that the prompt includes the block.
    use ragent_core::memory::storage::load_all_blocks;
    let blocks = load_all_blocks(&storage, &wd);
    assert!(!blocks.is_empty());

    let (_, loaded) = blocks
        .iter()
        .find(|(_, b)| b.label == "test-block")
        .unwrap();
    assert_eq!(loaded.content, "This is test block content.");
}
