//! Tests for `todo_read` / `todo_write` full lifecycle.

use std::sync::Arc;

use ragent_tools_extended::storage::StorageBackend;
use ragent_tools_extended::todo::{TodoReadTool, TodoWriteTool};
use ragent_tools_extended::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

#[path = "support/mock_storage.rs"]
mod mock_storage;
use mock_storage::MockStorage;

/// In-memory mock storage for testing.
fn test_ctx(storage: Arc<dyn StorageBackend>) -> ToolContext {
    ToolContext {
        storage: Some(storage),
        event_bus: Arc::new(EventBus::new(128)),
        session_id: "test-session".to_string(),
        working_dir: std::env::current_dir().unwrap(),
        code_index: None,
        config: None,
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    }
}

#[tokio::test]
async fn test_todo_full_lifecycle() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage.clone());
    let read_tool = TodoReadTool;
    let write_tool = TodoWriteTool;

    // ── 1. READ (empty) ──────────────────────────────────��──────────
    let out = read_tool
        .execute(json!({}), &ctx)
        .await
        .expect("read should succeed");
    assert!(
        out.content.contains("No TODO items found"),
        "empty read: {:?}",
        out.content
    );

    // ── 2. ADD ──────────────────────────────────────────────────────
    let out = write_tool
        .execute(
            json!({"action": "add", "id": "td-01", "title": "First task", "status": "pending"}),
            &ctx,
        )
        .await
        .expect("add should succeed");
    assert!(out.content.contains("Added todo 'td-01'"));
    assert!(out.content.contains("pending"));
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["action"], "add");
    assert_eq!(meta["count"], 1);

    // ── 3. READ (one item, all statuses) ────────────────────────────
    let out = read_tool
        .execute(json!({"status": "all"}), &ctx)
        .await
        .expect("read should succeed");
    assert!(out.content.contains("## TODOs (1 items)"));
    assert!(out.content.contains("First task"));
    assert!(out.content.contains("⏳"));

    // ── 4. UPDATE: pending -> in_progress ──────────────────────────
    let out = write_tool
        .execute(
            json!({"action": "update", "id": "td-01", "status": "in_progress"}),
            &ctx,
        )
        .await
        .expect("update should succeed");
    assert!(
        out.content.contains("(pending -> in_progress)"),
        "status change: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["action"], "update");
    assert_eq!(meta["old_status"], "pending");
    assert_eq!(meta["new_status"], "in_progress");

    // ── 5. ADD another (default status = pending) ─────────────────
    let out = write_tool
        .execute(
            json!({"action": "add", "id": "td-02", "title": "Second task"}),
            &ctx,
        )
        .await
        .expect("add should succeed");
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["count"], 2);

    // ── 6. READ with status filter: blocked (should be empty) ───────
    let out = read_tool
        .execute(json!({"status": "blocked"}), &ctx)
        .await
        .expect("read should succeed");
    assert!(
        out.content
            .contains("No TODO items found with status 'blocked'")
    );

    // ── 7. READ with status filter: in_progress ────────────────────
    let out = read_tool
        .execute(json!({"status": "in_progress"}), &ctx)
        .await
        .expect("read should succeed");
    assert!(out.content.contains("First task"));
    assert!(!out.content.contains("Second task"));

    // ── 8. UPDATE title only (no status change metadata) ─────────────
    let out = write_tool
        .execute(
            json!({"action": "update", "id": "td-01", "title": "First task (renamed)"}),
            &ctx,
        )
        .await
        .expect("update title should succeed");
    assert!(!out.content.contains("->")); // no status arrow
    let meta = out.metadata.expect("metadata");
    assert!(meta.get("old_status").is_none());
    assert!(meta.get("new_status").is_none());

    // ── 9. COMPLETE (in_progress -> done) ───────────────────────────
    let out = write_tool
        .execute(json!({"action": "complete", "id": "td-01"}), &ctx)
        .await
        .expect("complete should succeed");
    assert!(
        out.content.contains("(in_progress -> done)"),
        "complete: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["action"], "complete");
    assert_eq!(meta["old_status"], "in_progress");
    assert_eq!(meta["new_status"], "done");

    // ── 10. COMPLETE via "completed" alias ─────────────────────────
    let out = write_tool
        .execute(json!({"action": "completed", "id": "td-02"}), &ctx)
        .await
        .expect("completed alias should succeed");
    assert!(out.content.contains("(pending -> done)"));

    // ── 11. READ (2 items, 2 done) ────────────────────────────────
    let out = read_tool
        .execute(json!({}), &ctx)
        .await
        .expect("read should succeed");
    assert!(out.content.contains("## TODOs (2 items)"));
    assert!(out.content.contains("✅"));

    // ── 12. REMOVE one ─────────────────────────────────────────────
    let out = write_tool
        .execute(json!({"action": "remove", "id": "td-01"}), &ctx)
        .await
        .expect("remove should succeed");
    assert!(out.content.contains("Removed todo 'td-01'"));
    assert!(out.content.contains("First task (renamed)"));
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["count"], 1);

    // ── 13. CLEAR the remaining ─────────────────────────────────────
    let out = write_tool
        .execute(json!({"action": "clear"}), &ctx)
        .await
        .expect("clear should succeed");
    assert!(out.content.contains("Cleared 1 todo item"));
    assert!(out.content.contains("No TODO items found"));

    // ── 14. READ (empty again) ─────────────────────────────────────
    let out = read_tool
        .execute(json!({}), &ctx)
        .await
        .expect("read should succeed");
    assert!(out.content.contains("No TODO items found"));
}

#[tokio::test]
async fn test_todo_add_blocked_status() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage);
    let write_tool = TodoWriteTool;

    let out = write_tool
        .execute(
            json!({"action": "add", "id": "td-blk", "title": "Blocked task", "status": "blocked"}),
            &ctx,
        )
        .await
        .expect("add blocked should succeed");
    assert!(out.content.contains("status 'blocked'"));
    assert!(out.content.contains("🚫"));
}

#[tokio::test]
async fn test_todo_update_invalid_status_errors() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage);
    let write_tool = TodoWriteTool;

    let err = write_tool
        .execute(json!({"action": "add", "id": "td-x", "title": "X"}), &ctx)
        .await
        .expect("seed add ok");
    // force ignore
    drop(err);

    let res = write_tool
        .execute(
            json!({"action": "update", "id": "td-x", "status": "bogus"}),
            &ctx,
        )
        .await;
    assert!(res.is_err(), "expected error for invalid status");
    let err = res.unwrap_err().to_string();
    assert!(err.contains("Invalid status 'bogus'"));
}

#[tokio::test]
async fn test_todo_read_invalid_status_errors() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage);
    let read_tool = TodoReadTool;

    let res = read_tool.execute(json!({"status": "bogus"}), &ctx).await;
    assert!(res.is_err(), "expected error for invalid status filter");
    let err = res.unwrap_err().to_string();
    assert!(err.contains("Invalid status filter 'bogus'"));
}

#[tokio::test]
async fn test_todo_complete_nonexistent_errors() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage);
    let write_tool = TodoWriteTool;

    let res = write_tool
        .execute(json!({"action": "complete", "id": "no-such-id"}), &ctx)
        .await;
    assert!(res.is_err(), "expected error for nonexistent todo");
    let err = res.unwrap_err().to_string();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn test_todo_remove_nonexistent_errors() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage);
    let write_tool = TodoWriteTool;

    let res = write_tool
        .execute(json!({"action": "remove", "id": "no-such-id"}), &ctx)
        .await;
    assert!(res.is_err(), "expected error for nonexistent todo");
    let err = res.unwrap_err().to_string();
    assert!(err.contains("not found"));
}

#[tokio::test]
async fn test_todo_update_no_fields_errors() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let ctx = test_ctx(storage);
    let write_tool = TodoWriteTool;

    // seed
    let _ = write_tool
        .execute(json!({"action": "add", "id": "td-y", "title": "Y"}), &ctx)
        .await;

    let res = write_tool
        .execute(json!({"action": "update", "id": "td-y"}), &ctx)
        .await;
    assert!(
        res.is_err(),
        "expected error when no fields provided for update"
    );
    let err = res.unwrap_err().to_string();
    assert!(err.contains("At least one of title, status, or description"));
}
