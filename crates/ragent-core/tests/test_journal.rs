//! Integration tests for Milestone 2 — Journal System.
//!
//! Tests the journal tools (journal_write, journal_search, journal_read)
//! and their interaction with the SQLite storage layer.

use std::path::PathBuf;
use std::sync::Arc;

use ragent_core::storage::Storage;
use ragent_core::tool::{Tool, ToolContext};

/// Helper: create a ToolContext with in-memory storage.
fn make_ctx() -> ToolContext {
    let storage = Arc::new(Storage::open_in_memory().unwrap());
    ToolContext {
        session_id: "test-session".to_string(),
        working_dir: PathBuf::from("/tmp/test-project"),
        event_bus: Arc::new(ragent_core::event::EventBus::new(100)),
        storage: Some(storage),
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    }
}

// ── journal_write ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_journal_write_basic() {
    let ctx = make_ctx();
    let tool = ragent_core::tool::journal::JournalWriteTool;

    let input = serde_json::json!({
        "title": "Bug fix insight",
        "content": "The off-by-one error in the parser was caused by incorrect index calculation.",
        "tags": ["bug", "parser"]
    });

    let result = tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("Journal entry recorded"));
    assert!(result.content.contains("Bug fix insight"));
    assert!(result.content.contains("bug, parser"));

    // Verify metadata.
    let meta = result.metadata.unwrap();
    assert_eq!(meta["title"], "Bug fix insight");
    assert!(!meta["id"].as_str().unwrap().is_empty());
}

#[tokio::test]
async fn test_journal_write_no_tags() {
    let ctx = make_ctx();
    let tool = ragent_core::tool::journal::JournalWriteTool;

    let input = serde_json::json!({
        "title": "Design decision",
        "content": "We chose SQLite over filesystem-only for structured queries."
    });

    let result = tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("Tags: none"));
}

#[tokio::test]
async fn test_journal_write_invalid_tag() {
    let ctx = make_ctx();
    let tool = ragent_core::tool::journal::JournalWriteTool;

    let input = serde_json::json!({
        "title": "Bad tag",
        "content": "This has an invalid tag",
        "tags": ["VALID", "invalid space"]
    });

    let result = tool.execute(input, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid tags"));
}

#[tokio::test]
async fn test_journal_write_no_storage() {
    let ctx = ToolContext {
        session_id: "test".to_string(),
        working_dir: PathBuf::from("/tmp"),
        event_bus: Arc::new(ragent_core::event::EventBus::new(100)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
        code_index: None,
    };
    let tool = ragent_core::tool::journal::JournalWriteTool;

    let input = serde_json::json!({
        "title": "Test",
        "content": "Content"
    });

    let result = tool.execute(input, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("storage"));
}

// ── journal_search ────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_journal_search_basic() {
    let ctx = make_ctx();

    // Write some entries first.
    let write_tool = ragent_core::tool::journal::JournalWriteTool;
    write_tool
        .execute(
            serde_json::json!({
                "title": "Parser bug fix",
                "content": "Fixed off-by-one error in the token parser.",
                "tags": ["bug", "parser"]
            }),
            &ctx,
        )
        .await
        .unwrap();

    write_tool
        .execute(
            serde_json::json!({
                "title": "Performance optimisation",
                "content": "Switched to DashMap for concurrent access patterns.",
                "tags": ["performance", "concurrency"]
            }),
            &ctx,
        )
        .await
        .unwrap();

    // Search for "parser".
    let search_tool = ragent_core::tool::journal::JournalSearchTool;
    let input = serde_json::json!({
        "query": "parser"
    });
    let result = search_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("Parser bug fix"));
    assert!(!result.content.contains("Performance optimisation"));
}

#[tokio::test]
async fn test_journal_search_with_tag_filter() {
    let ctx = make_ctx();

    let write_tool = ragent_core::tool::journal::JournalWriteTool;
    write_tool
        .execute(
            serde_json::json!({
                "title": "Rust error handling",
                "content": "Use Result<T, E> with thiserror for custom errors.",
                "tags": ["rust", "error-handling"]
            }),
            &ctx,
        )
        .await
        .unwrap();

    write_tool
        .execute(
            serde_json::json!({
                "title": "Python error handling",
                "content": "Use try/except with custom exception classes.",
                "tags": ["python", "error-handling"]
            }),
            &ctx,
        )
        .await
        .unwrap();

    // Search for "error" filtered by "rust" tag.
    let search_tool = ragent_core::tool::journal::JournalSearchTool;
    let input = serde_json::json!({
        "query": "error",
        "tags": ["rust"]
    });
    let result = search_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("Rust error handling"));
    assert!(!result.content.contains("Python error handling"));
}

#[tokio::test]
async fn test_journal_search_no_results() {
    let ctx = make_ctx();

    let search_tool = ragent_core::tool::journal::JournalSearchTool;
    let input = serde_json::json!({
        "query": "nonexistent_xyzzy"
    });

    let result = search_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("No journal entries found"));
}

#[tokio::test]
async fn test_journal_search_limit() {
    let ctx = make_ctx();

    let write_tool = ragent_core::tool::journal::JournalWriteTool;

    // Create 5 entries about "testing".
    for i in 0..5 {
        write_tool
            .execute(
                serde_json::json!({
                    "title": format!("Test entry {i}"),
                    "content": format!("Testing journal entry number {i} about testing patterns."),
                    "tags": ["testing"]
                }),
                &ctx,
            )
            .await
            .unwrap();
    }

    // Search with limit 2.
    let search_tool = ragent_core::tool::journal::JournalSearchTool;
    let input = serde_json::json!({
        "query": "testing",
        "limit": 2
    });

    let result = search_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("Found 2 journal entries"));
}

// ── journal_read ───────────────────────────────────────────────────────────────

#[tokio::test]
async fn test_journal_read_basic() {
    let ctx = make_ctx();

    // Write an entry.
    let write_tool = ragent_core::tool::journal::JournalWriteTool;
    let write_result = write_tool
        .execute(
            serde_json::json!({
                "title": "Read test entry",
                "content": "This is the full content of the journal entry.",
                "tags": ["test"]
            }),
            &ctx,
        )
        .await
        .unwrap();

    let entry_id = write_result.metadata.unwrap()["id"]
        .as_str()
        .unwrap()
        .to_string();

    // Read it back.
    let read_tool = ragent_core::tool::journal::JournalReadTool;
    let input = serde_json::json!({
        "id": entry_id
    });
    let result = read_tool.execute(input, &ctx).await.unwrap();
    assert!(result.content.contains("Read test entry"));
    assert!(
        result
            .content
            .contains("This is the full content of the journal entry.")
    );
    assert!(result.content.contains("test"));
}

#[tokio::test]
async fn test_journal_read_not_found() {
    let ctx = make_ctx();

    let read_tool = ragent_core::tool::journal::JournalReadTool;
    let input = serde_json::json!({
        "id": "nonexistent-id"
    });

    let result = read_tool.execute(input, &ctx).await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("not found"));
}

// ── Storage layer tests ──────────────────────────────────────────────────────

#[test]
fn test_storage_create_and_get_entry() {
    let storage = Storage::open_in_memory().unwrap();

    storage
        .create_journal_entry(
            "entry-1",
            "Test title",
            "Test content",
            "my-project",
            "sess-1",
            &["tag1".to_string(), "tag2".to_string()],
        )
        .unwrap();

    let entry = storage.get_journal_entry("entry-1").unwrap().unwrap();
    assert_eq!(entry.id, "entry-1");
    assert_eq!(entry.title, "Test title");
    assert_eq!(entry.content, "Test content");
    assert_eq!(entry.project, "my-project");
    assert_eq!(entry.session_id, "sess-1");
}

#[test]
fn test_storage_get_tags() {
    let storage = Storage::open_in_memory().unwrap();

    storage
        .create_journal_entry(
            "entry-2",
            "Tagged",
            "Content",
            "project",
            "sess-1",
            &["alpha".to_string(), "beta".to_string()],
        )
        .unwrap();

    let tags = storage.get_journal_tags("entry-2").unwrap();
    assert_eq!(tags, vec!["alpha", "beta"]);
}

#[test]
fn test_storage_search_entries() {
    let storage = Storage::open_in_memory().unwrap();

    storage
        .create_journal_entry(
            "e1",
            "Rust patterns",
            "Use Result for error handling in Rust.",
            "project",
            "sess-1",
            &[],
        )
        .unwrap();

    storage
        .create_journal_entry(
            "e2",
            "Python patterns",
            "Use exceptions for error handling in Python.",
            "project",
            "sess-1",
            &[],
        )
        .unwrap();

    let results = storage
        .search_journal_entries("Rust error", None, 10)
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "e1");
}

#[test]
fn test_storage_search_with_tag_filter() {
    let storage = Storage::open_in_memory().unwrap();

    storage
        .create_journal_entry(
            "e1",
            "Rust patterns",
            "Error handling patterns in Rust",
            "project",
            "sess-1",
            &["rust".to_string()],
        )
        .unwrap();

    storage
        .create_journal_entry(
            "e2",
            "Python patterns",
            "Error handling patterns in Python",
            "project",
            "sess-1",
            &["python".to_string()],
        )
        .unwrap();

    let results = storage
        .search_journal_entries("error handling", Some(&["rust".to_string()]), 10)
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, "e1");
}

#[test]
fn test_storage_list_entries() {
    let storage = Storage::open_in_memory().unwrap();

    for i in 1..=5 {
        storage
            .create_journal_entry(
                &format!("e{i}"),
                &format!("Entry {i}"),
                &format!("Content {i}"),
                "project",
                "sess-1",
                &[],
            )
            .unwrap();
    }

    let entries = storage.list_journal_entries(3).unwrap();
    assert_eq!(entries.len(), 3);
}

#[test]
fn test_storage_list_by_tag() {
    let storage = Storage::open_in_memory().unwrap();

    storage
        .create_journal_entry(
            "e1",
            "Tagged",
            "Has the bug tag",
            "project",
            "sess-1",
            &["bug".to_string()],
        )
        .unwrap();

    storage
        .create_journal_entry(
            "e2",
            "Not tagged",
            "No bug tag",
            "project",
            "sess-1",
            &["feature".to_string()],
        )
        .unwrap();

    let entries = storage.list_journal_entries_by_tag("bug", 10).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].id, "e1");
}

#[test]
fn test_storage_delete_entry() {
    let storage = Storage::open_in_memory().unwrap();

    storage
        .create_journal_entry(
            "e-del",
            "To delete",
            "Content",
            "project",
            "sess-1",
            &["tag".to_string()],
        )
        .unwrap();

    let deleted = storage.delete_journal_entry("e-del").unwrap();
    assert!(deleted);

    let entry = storage.get_journal_entry("e-del").unwrap();
    assert!(entry.is_none());

    // Tags should be gone too.
    let tags = storage.get_journal_tags("e-del").unwrap();
    assert!(tags.is_empty());
}

#[test]
fn test_storage_delete_nonexistent() {
    let storage = Storage::open_in_memory().unwrap();
    let deleted = storage.delete_journal_entry("ghost").unwrap();
    assert!(!deleted);
}

#[test]
fn test_storage_count_entries() {
    let storage = Storage::open_in_memory().unwrap();

    assert_eq!(storage.count_journal_entries().unwrap(), 0);

    storage
        .create_journal_entry("e1", "First", "Content", "project", "sess-1", &[])
        .unwrap();

    assert_eq!(storage.count_journal_entries().unwrap(), 1);

    storage
        .create_journal_entry("e2", "Second", "Content", "project", "sess-1", &[])
        .unwrap();

    assert_eq!(storage.count_journal_entries().unwrap(), 2);
}

#[test]
fn test_storage_idempotent_migration() {
    // Opening storage twice should not fail (schema is IF NOT EXISTS).
    let storage = Storage::open_in_memory().unwrap();
    storage
        .create_journal_entry("e1", "Test", "Content", "project", "sess-1", &[])
        .unwrap();

    // Re-open — tables already exist.
    let storage2 = Storage::open_in_memory().unwrap();
    assert_eq!(storage2.count_journal_entries().unwrap(), 0);
    // (In-memory databases are separate instances, so this just validates
    //  that migrate() doesn't panic on re-creation.)
}

// ── Event emission tests ─────────────────────────────────────────────────────

#[tokio::test]
async fn test_journal_write_emits_event() {
    let ctx = make_ctx();
    let mut rx = ctx.event_bus.subscribe();

    let tool = ragent_core::tool::journal::JournalWriteTool;
    let input = serde_json::json!({
        "title": "Event test",
        "content": "Testing event emission"
    });

    tool.execute(input, &ctx).await.unwrap();

    // Check that the event was emitted.
    let event = rx.try_recv().unwrap();
    match &event {
        ragent_core::event::Event::JournalEntryCreated { title, .. } => {
            assert_eq!(title, "Event test");
        }
        _ => panic!("Expected JournalEntryCreated event, got {:?}", event),
    }
}

#[tokio::test]
async fn test_journal_search_emits_event() {
    let ctx = make_ctx();

    // Write first so search finds something.
    let write_tool = ragent_core::tool::journal::JournalWriteTool;
    write_tool
        .execute(
            serde_json::json!({
                "title": "Search target",
                "content": "The content to search for"
            }),
            &ctx,
        )
        .await
        .unwrap();

    let mut rx = ctx.event_bus.subscribe();

    let search_tool = ragent_core::tool::journal::JournalSearchTool;
    let input = serde_json::json!({
        "query": "search target"
    });

    search_tool.execute(input, &ctx).await.unwrap();

    let event = rx.try_recv().unwrap();
    match &event {
        ragent_core::event::Event::JournalSearched {
            query,
            result_count,
            ..
        } => {
            assert_eq!(query, "search target");
            assert_eq!(*result_count, 1);
        }
        _ => panic!("Expected JournalSearched event, got {:?}", event),
    }
}
