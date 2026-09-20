//! Integration tests for the background task service (M3, T-021/T-023).
//!
//! These exercise [`ragent_agent::background::BackgroundTaskService`]:
//! spawn → persist → wait → drain_completed, plus the wake/notify hook and
//! the cleanup operation. Storage is an in-memory SQLite database.

use std::sync::Arc;

use ragent_agent::background::BackgroundTaskService;
use ragent_storage::Storage;
use ragent_types::event::EventBus;

/// Create a session row so the `background_tasks` foreign key is satisfied.
fn ensure_session(storage: &Storage, session_id: &str) {
    // `create_session` errors if the row already exists; ignore that case.
    let _ = storage.create_session(session_id, ".");
}

/// Build a service backed by in-memory storage with a single session row.
async fn make_service(session_id: &str) -> Arc<BackgroundTaskService> {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    ensure_session(&storage, session_id);
    let storage = Arc::new(storage);
    let event_bus = Arc::new(EventBus::new(64));
    Arc::new(BackgroundTaskService::new(storage, event_bus))
}

#[tokio::test]
async fn test_bg_service_spawn_wait_drain() {
    let session = "sess-drain";
    let service = make_service(session).await;
    let task_id = service
        .spawn(
            session,
            "echo hello-service",
            &std::env::current_dir().unwrap(),
        )
        .await
        .expect("spawn");

    let row = service.wait(&task_id, 10).await.expect("wait");
    assert_eq!(row.status, "completed");
    assert_eq!(row.exit_code, Some(0));

    // Drain should surface the completion once.
    let completed = service.drain_completed(session).await;
    assert_eq!(completed.len(), 1);
    assert_eq!(completed[0].task_id, task_id);
    assert_eq!(completed[0].status, "completed");
    assert!(
        completed[0].tail.contains("hello-service"),
        "tail should contain output: {}",
        completed[0].tail
    );

    // Second drain should be empty (already drained).
    let again = service.drain_completed(session).await;
    assert!(again.is_empty(), "second drain should be empty");
    assert!(!service.has_pending_completions());
}

#[tokio::test]
async fn test_bg_service_notify_fires_on_completion() {
    let session = "sess-notify";
    let service = make_service(session).await;
    let notify = service.completion_notify();

    let task_id = service
        .spawn(
            session,
            "echo notify-test",
            &std::env::current_dir().unwrap(),
        )
        .await
        .expect("spawn");

    // The notify should fire once the task completes. Await with a timeout.
    tokio::time::timeout(tokio::time::Duration::from_secs(10), notify.notified())
        .await
        .expect("notify should fire before timeout");

    // Ensure the task is actually done.
    let _ = service.wait(&task_id, 10).await.expect("wait");
    assert!(service.has_pending_completions());
}

#[tokio::test]
async fn test_bg_service_drain_filters_by_session() {
    let sess_a = "sess-a";
    let sess_b = "sess-b";
    // A single service instance with both sessions present.
    let storage = Storage::open_in_memory().expect("in-memory storage");
    ensure_session(&storage, sess_a);
    ensure_session(&storage, sess_b);
    let storage = Arc::new(storage);
    let event_bus = Arc::new(EventBus::new(64));
    let service = Arc::new(BackgroundTaskService::new(storage, event_bus));

    let id_a = service
        .spawn(sess_a, "echo a", &std::env::current_dir().unwrap())
        .await
        .expect("spawn a");
    let id_b = service
        .spawn(sess_b, "echo b", &std::env::current_dir().unwrap())
        .await
        .expect("spawn b");

    let _ = service.wait(&id_a, 10).await.expect("wait a");
    let _ = service.wait(&id_b, 10).await.expect("wait b");

    // Drain for session A should only return A's task.
    let drained_a = service.drain_completed(sess_a).await;
    assert_eq!(drained_a.len(), 1);
    assert_eq!(drained_a[0].task_id, id_a);

    // Drain for session B should return B's task.
    let drained_b = service.drain_completed(sess_b).await;
    assert_eq!(drained_b.len(), 1);
    assert_eq!(drained_b[0].task_id, id_b);
}

#[tokio::test]
async fn test_bg_service_list_and_status() {
    let session = "sess-list";
    let service = make_service(session).await;

    let task_id = service
        .spawn(session, "echo list-test", &std::env::current_dir().unwrap())
        .await
        .expect("spawn");
    let _ = service.wait(&task_id, 10).await.expect("wait");

    let rows = service.list(Some(session), None, 50).await.expect("list");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].id, task_id);

    let status_row = service.status(&task_id).await.expect("status");
    assert_eq!(status_row.status, "completed");
}

#[tokio::test]
async fn test_bg_service_cancel() {
    let session = "sess-cancel";
    let service = make_service(session).await;

    let task_id = service
        .spawn(session, "sleep 30", &std::env::current_dir().unwrap())
        .await
        .expect("spawn");

    tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    service.cancel(&task_id).await.expect("cancel");

    let row = service.wait(&task_id, 10).await.expect("wait after cancel");
    assert_eq!(
        row.status, "cancelled",
        "task should be cancelled after cancel"
    );
}

#[tokio::test]
async fn test_bg_service_cleanup_removes_done_tasks() {
    let session = "sess-cleanup";
    let service = make_service(session).await;

    let task_id = service
        .spawn(
            session,
            "echo cleanup-test",
            &std::env::current_dir().unwrap(),
        )
        .await
        .expect("spawn");
    let _ = service.wait(&task_id, 10).await.expect("wait");

    // Wait for the final flush to persist the completed status in storage,
    // since `cleanup` queries storage directly (not the in-memory overlay).
    // We use `list` (which does NOT overlay in-memory state) so that we only
    // proceed once the storage row itself reflects "completed".
    let mut storage_completed = false;
    for _ in 0..50 {
        if let Ok(rows) = service.list(Some(session), None, 50).await {
            if rows
                .iter()
                .any(|r| r.id == task_id && r.status == "completed")
            {
                storage_completed = true;
                break;
            }
        }
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }
    assert!(
        storage_completed,
        "task should be persisted as completed in storage before cleanup"
    );

    // Cleanup with older_than_minutes=0 should remove the just-finished task.
    let count = service
        .cleanup(Some(session), 0, true)
        .await
        .expect("cleanup");
    assert!(count >= 1, "cleanup should remove at least one task");

    let rows = service.list(Some(session), None, 50).await.expect("list");
    assert!(
        rows.is_empty(),
        "no tasks should remain after cleanup, got {}",
        rows.len()
    );
}
