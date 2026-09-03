#![allow(clippy::assert_is_empty)]
//! todo2tasks T-006: integration tests for auto-unblock evaluation
//! (FR-003, FR-005).
//!
//! Verifies that when a task transitions to `completed`, dependent
//! tasks that list it in `blocked_by` are re-evaluated at read time:
//! those whose `blocked_by` is now fully `completed` are flagged as
//! "available" in `task_get` / `task_list` output, while those with
//! remaining unfinished blockers are flagged as "blocked".

use std::sync::Arc;

use ragent_tools_extended::storage::StorageBackend;
use ragent_tools_extended::task::{TaskGetTool, TaskListTool};
use ragent_tools_extended::{Tool, ToolContext};
use ragent_types::event::EventBus;
use serde_json::json;

#[path = "support/mock_storage.rs"]
mod mock_storage;
use mock_storage::MockStorage;

/// Build a `ToolContext` backed by the given storage.
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

// ── task_get: is_available / is_blocked metadata ────────────────────

/// A pending task with no blocked_by and no owner should be available.
#[tokio::test]
async fn test_task_get_available_no_deps() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Standalone task", "pending");
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t1"}), &ctx)
        .await
        .expect("task_get should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["is_available"], true);
    assert_eq!(meta["is_blocked"], false);
}

/// A pending task whose sole blocker is completed should be available
/// (FR-003: auto-unblock on completion).
#[tokio::test]
async fn test_task_get_available_after_blocker_completed() {
    let storage = MockStorage::new();
    // t1 is completed.
    storage.seed("t1", "test-session", "Setup", "completed");
    // t2 depends on t1 and is still pending.
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("task_get should succeed");

    let meta = out.metadata.expect("metadata present");
    // t2's only blocker (t1) is completed → available.
    assert_eq!(meta["is_available"], true, "t2 should be available");
    assert_eq!(meta["is_blocked"], false, "t2 should not be blocked");
}

/// A pending task with an unfinished blocker should be blocked.
#[tokio::test]
async fn test_task_get_blocked_when_blocker_pending() {
    let storage = MockStorage::new();
    // t1 is still pending.
    storage.seed("t1", "test-session", "Setup", "pending");
    // t2 depends on t1.
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("task_get should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["is_available"], false, "t2 should not be available");
    assert_eq!(meta["is_blocked"], true, "t2 should be blocked");
}

/// A pending task with multiple blockers where one is completed and
/// the other is not should still be blocked.
#[tokio::test]
async fn test_task_get_blocked_partial_completion() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Setup", "completed");
    storage.seed("t2", "test-session", "Config", "pending");
    // t3 depends on both t1 and t2.
    storage.seed_task(
        "t3",
        "test-session",
        "Deploy",
        "pending",
        "",
        None,
        None,
        &["t1", "t2"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t3"}), &ctx)
        .await
        .expect("task_get should succeed");

    let meta = out.metadata.expect("metadata present");
    // t2 is still pending → t3 is blocked.
    assert_eq!(meta["is_available"], false);
    assert_eq!(meta["is_blocked"], true);
}

/// A pending task with all blockers completed but has an owner should
/// NOT be available (owner prevents availability).
#[tokio::test]
async fn test_task_get_owner_prevents_available() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Setup", "completed");
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "pending",
        "",
        None,
        Some("coder-agent"),
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("task_get should succeed");

    let meta = out.metadata.expect("metadata present");
    // All blockers done but owner is set → not available, not blocked.
    assert_eq!(meta["is_available"], false, "owner prevents available");
    assert_eq!(
        meta["is_blocked"], false,
        "not blocked since all blockers done"
    );
}

/// An in_progress task should never be available or blocked.
#[tokio::test]
async fn test_task_get_in_progress_not_available() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Setup", "completed");
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "in_progress",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("task_get should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["is_available"], false);
    assert_eq!(meta["is_blocked"], false);
}

/// A completed task should never be available or blocked.
#[tokio::test]
async fn test_task_get_completed_not_available() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Setup", "completed");
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "completed",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("task_get should succeed");

    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["is_available"], false);
    assert_eq!(meta["is_blocked"], false);
}

// ── task_get: content annotations ───────────────────────────────────

/// task_get content should show [available] for an available task.
#[tokio::test]
async fn test_task_get_content_available_annotation() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Setup", "completed");
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("task_get should succeed");

    assert!(
        out.content.contains("[available]"),
        "content should contain [available]: {}",
        out.content
    );
}

/// task_get content should show [blocked by #id] for a blocked task.
#[tokio::test]
async fn test_task_get_content_blocked_annotation() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Setup", "pending");
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("task_get should succeed");

    assert!(
        out.content.contains("[blocked by"),
        "content should contain [blocked by: {}",
        out.content
    );
    assert!(
        out.content.contains("#t1"),
        "content should contain #t1: {}",
        out.content
    );
}

/// task_get content should show [blocked by #id, #id] for multiple
/// blockers.
#[tokio::test]
async fn test_task_get_content_blocked_multiple_annotation() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "A", "pending");
    storage.seed("t2", "test-session", "B", "pending");
    storage.seed_task(
        "t3",
        "test-session",
        "C",
        "pending",
        "",
        None,
        None,
        &["t1", "t2"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskGetTool;

    let out = tool
        .execute(json!({"task_id": "t3"}), &ctx)
        .await
        .expect("task_get should succeed");

    assert!(
        out.content.contains("[blocked by"),
        "content should contain [blocked by: {}",
        out.content
    );
    assert!(
        out.content.contains("#t1"),
        "content should contain #t1: {}",
        out.content
    );
    assert!(
        out.content.contains("#t2"),
        "content should contain #t2: {}",
        out.content
    );
}

// ── task_list: is_available / is_blocked metadata ───────────────────

/// task_list should include is_available=true for a task whose
/// blockers are all completed (FR-003).
#[tokio::test]
async fn test_task_list_available_after_unblock() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Setup", "completed");
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    let meta = out.metadata.expect("metadata present");
    let tasks = meta["tasks"].as_array().expect("tasks array");
    // Find t2.
    let t2 = tasks
        .iter()
        .find(|t| t["id"] == "t2")
        .expect("t2 should be in list");
    assert_eq!(t2["is_available"], true);
    assert_eq!(t2["is_blocked"], false);
}

/// task_list should include is_blocked=true for a task with unfinished
/// blockers (FR-005).
#[tokio::test]
async fn test_task_list_blocked_flag() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Setup", "pending");
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    let meta = out.metadata.expect("metadata present");
    let tasks = meta["tasks"].as_array().expect("tasks array");
    let t2 = tasks
        .iter()
        .find(|t| t["id"] == "t2")
        .expect("t2 should be in list");
    assert_eq!(t2["is_available"], false);
    assert_eq!(t2["is_blocked"], true);
}

/// task_list with status=pending filter should still compute correct
/// is_blocked/is_available from the full task set (not just filtered).
#[tokio::test]
async fn test_task_list_filtered_dag_correct() {
    let storage = MockStorage::new();
    // t1 completed — won't appear in pending filter, but t2's
    // is_available depends on knowing t1's status.
    storage.seed("t1", "test-session", "Setup", "completed");
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({"status": "pending"}), &ctx)
        .await
        .expect("task_list should succeed");

    let meta = out.metadata.expect("metadata present");
    // Only t2 should be in the filtered list.
    let tasks = meta["tasks"].as_array().expect("tasks array");
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0]["id"], "t2");
    // t2 should be available even though t1 is not in the filtered list.
    assert_eq!(tasks[0]["is_available"], true, "t2 should be available");
    assert_eq!(tasks[0]["is_blocked"], false);
}

// ── task_list: content annotations ──────────────────────────────────

/// task_list content should show [available] for unblocked tasks.
#[tokio::test]
async fn test_task_list_content_available() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Setup", "completed");
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    assert!(
        out.content.contains("[available]"),
        "content should contain [available]: {}",
        out.content
    );
}

/// task_list content should show [blocked by #id] for blocked tasks.
#[tokio::test]
async fn test_task_list_content_blocked() {
    let storage = MockStorage::new();
    storage.seed("t1", "test-session", "Setup", "pending");
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    let ctx = test_ctx(Arc::new(storage));
    let tool = TaskListTool;

    let out = tool
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");

    assert!(
        out.content.contains("[blocked by"),
        "content should contain [blocked by: {}",
        out.content
    );
    assert!(
        out.content.contains("#t1"),
        "content should contain #t1: {}",
        out.content
    );
}

// ── Auto-unblock scenario: multi-step ───────────────────────────────

/// Simulate the full auto-unblock lifecycle: t1 blocks t2; t1 is
/// completed; task_get/task_list now show t2 as available.
#[tokio::test]
async fn test_auto_unblock_lifecycle() {
    let storage = MockStorage::new();

    // Initial state: t1 is pending, t2 depends on t1.
    storage.seed("t1", "test-session", "Setup", "pending");
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );

    let ctx = test_ctx(Arc::new(storage.clone()));
    let get = TaskGetTool;
    let list = TaskListTool;

    // Step 1: before t1 completes, t2 should be blocked.
    let out = get
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("task_get should succeed");
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["is_blocked"], true, "t2 should be blocked initially");
    assert_eq!(meta["is_available"], false);

    // Step 2: simulate t1 transitioning to completed (via update_task_simple
    // in the mock storage).
    storage.set_status("t1", "completed");

    // Step 3: after t1 completes, t2 should be available (FR-003).
    let out = get
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("task_get should succeed");
    let meta = out.metadata.expect("metadata present");
    assert_eq!(
        meta["is_available"], true,
        "t2 should be available after unblock"
    );
    assert_eq!(meta["is_blocked"], false);

    // Also verify via task_list.
    let out = list
        .execute(json!({}), &ctx)
        .await
        .expect("task_list should succeed");
    let meta = out.metadata.expect("metadata present");
    let tasks = meta["tasks"].as_array().expect("tasks array");
    let t2 = tasks
        .iter()
        .find(|t| t["id"] == "t2")
        .expect("t2 should be in list");
    assert_eq!(t2["is_available"], true);
    assert_eq!(t2["is_blocked"], false);
}

/// Auto-unblock with a chain: t1 → t2 → t3.  Completing t1 unblocks t2
/// (which can start), but t3 remains blocked until t2 is also completed.
#[tokio::test]
async fn test_auto_unblock_chain_partial() {
    let storage = MockStorage::new();

    storage.seed("t1", "test-session", "Setup", "completed");
    storage.seed_task(
        "t2",
        "test-session",
        "Build",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    storage.seed_task(
        "t3",
        "test-session",
        "Deploy",
        "pending",
        "",
        None,
        None,
        &["t2"],
    );

    let ctx = test_ctx(Arc::new(storage));
    let get = TaskGetTool;

    // t2 should be available (t1 is completed).
    let out = get
        .execute(json!({"task_id": "t2"}), &ctx)
        .await
        .expect("task_get should succeed");
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["is_available"], true, "t2 should be available");

    // t3 should still be blocked (t2 is pending).
    let out = get
        .execute(json!({"task_id": "t3"}), &ctx)
        .await
        .expect("task_get should succeed");
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["is_blocked"], true, "t3 should be blocked");
    assert_eq!(meta["is_available"], false);
}

/// Auto-unblock with diamond: t1 → {t2, t3} → t4.  Completing t1
/// unblocks t2 and t3, but t4 remains blocked until both are done.
#[tokio::test]
async fn test_auto_unblock_diamond() {
    let storage = MockStorage::new();

    storage.seed("t1", "test-session", "Setup", "completed");
    storage.seed_task(
        "t2",
        "test-session",
        "Build A",
        "completed",
        "",
        None,
        None,
        &["t1"],
    );
    storage.seed_task(
        "t3",
        "test-session",
        "Build B",
        "pending",
        "",
        None,
        None,
        &["t1"],
    );
    storage.seed_task(
        "t4",
        "test-session",
        "Integrate",
        "pending",
        "",
        None,
        None,
        &["t2", "t3"],
    );

    let ctx = test_ctx(Arc::new(storage));
    let get = TaskGetTool;

    // t3 should be available (t1 is completed).
    let out = get
        .execute(json!({"task_id": "t3"}), &ctx)
        .await
        .expect("task_get should succeed");
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["is_available"], true, "t3 should be available");

    // t4 should be blocked (t3 is still pending).
    let out = get
        .execute(json!({"task_id": "t4"}), &ctx)
        .await
        .expect("task_get should succeed");
    let meta = out.metadata.expect("metadata present");
    assert_eq!(meta["is_blocked"], true, "t4 should be blocked");
    assert_eq!(meta["is_available"], false);
}
