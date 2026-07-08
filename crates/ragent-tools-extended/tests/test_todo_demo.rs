//! Demonstration: exercise todo_read / todo_write through a full lifecycle.
//! Run with: cargo test -p ragent-tools-extended --test test_todo_demo -- --nocapture

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
fn ctx(storage: Arc<dyn StorageBackend>) -> ToolContext {
    ToolContext {
        storage: Some(storage),
        event_bus: Arc::new(EventBus::new(128)),
        session_id: "demo-session".to_string(),
        working_dir: std::env::current_dir().unwrap(),
        code_index: None,
        config: None,
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
    }
}

#[tokio::test]
async fn demo_todo_lifecycle() {
    let storage: Arc<dyn StorageBackend> = Arc::new(MockStorage::new());
    let read_tool = TodoReadTool;
    let write_tool = TodoWriteTool;

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║           TODO TOOL DEMONSTRATION — FULL LIFECYCLE           ║");
    println!("╚══════════════════════════════════════════════════════════════╝\n");

    // ── Step 1: READ empty list ───────────────────────────────────
    println!("[STEP 1] todo_read (empty list)");
    let out = read_tool
        .execute(json!({}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 2: ADD a new todo ────────────────────────────────────
    println!("[STEP 2] todo_write action=add (default status: pending)");
    let out = write_tool
        .execute(
            json!({"action": "add", "id": "task-001", "title": "Implement OAuth2 login", "description": "Add Google and GitHub providers"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 3: ADD another with explicit status ──────────────────
    println!("[STEP 3] todo_write action=add status=blocked");
    let out = write_tool
        .execute(
            json!({"action": "add", "id": "task-002", "title": "Refactor database layer", "status": "blocked"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 4: READ all ──────────────────────────────────────────
    println!("[STEP 4] todo_read status=all");
    let out = read_tool
        .execute(json!({"status": "all"}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 5: UPDATE status (pending → in_progress) ─────────────
    println!("[STEP 5] todo_write action=update status=in_progress");
    let out = write_tool
        .execute(
            json!({"action": "update", "id": "task-001", "status": "in_progress"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 6: READ filtered by in_progress ──────────────────────
    println!("[STEP 6] todo_read status=in_progress");
    let out = read_tool
        .execute(json!({"status": "in_progress"}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 7: UPDATE title only (no status change) ──────────────
    println!("[STEP 7] todo_write action=update title only");
    let out = write_tool
        .execute(
            json!({"action": "update", "id": "task-001", "title": "Implement OAuth2 login (with refresh tokens)"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 8: COMPLETE (in_progress → done) ─────────────────────
    println!("[STEP 8] todo_write action=complete");
    let out = write_tool
        .execute(
            json!({"action": "complete", "id": "task-001"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 9: READ filtered by done ─────────────────────────────
    println!("[STEP 9] todo_read status=done");
    let out = read_tool
        .execute(json!({"status": "done"}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 10: COMPLETE via alias "completed" ──────────────────
    println!("[STEP 10] todo_write action=completed (alias for complete)");
    let out = write_tool
        .execute(
            json!({"action": "completed", "id": "task-002"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 11: READ all (both done) ─────────────────────────────
    println!("[STEP 11] todo_read status=all (final state)");
    let out = read_tool
        .execute(json!({"status": "all"}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 12: REMOVE one ──────────────────────────────────────
    println!("[STEP 12] todo_write action=remove");
    let out = write_tool
        .execute(
            json!({"action": "remove", "id": "task-001"}),
            &ctx(storage.clone()),
        )
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 13: CLEAR remaining ──────────────────────────────────
    println!("[STEP 13] todo_write action=clear");
    let out = write_tool
        .execute(json!({"action": "clear"}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    // ── Step 14: READ empty ───────────────────────────────────────
    println!("[STEP 14] todo_read (verify empty)");
    let out = read_tool
        .execute(json!({}), &ctx(storage.clone()))
        .await
        .unwrap();
    println!("{}", out.content);
    println!();

    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║                    LIFECYCLE COMPLETE ✅                      ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
}
