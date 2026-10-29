//! Tests-only coverage for the `AgentManager` task-registry bookkeeping paths
//! that do not require spawning a real sub-agent: `get_task`, `list_agents`,
//! `running_background_count`, `cancel_all`/`cancel_agent`, the waiter-count
//! protocol, the background concurrency limit, and `drain_completed` reaping.
//!
//! Entries are seeded directly through the `#[doc(hidden)]` test helpers.

use std::sync::Arc;

use ragent_agent::task::TaskStatus;

#[path = "task_support/mod.rs"]
mod task_support;
use task_support::{entry_full, manager, test_processor};

#[tokio::test]
async fn get_task_returns_and_misses() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full(
        "t1",
        TaskStatus::Completed,
        "parent",
        true,
        false,
    ))
    .await;

    let found = mgr.get_task("t1").await.expect("task present");
    assert_eq!(found.id, "t1");
    assert!(mgr.get_task("nope").await.is_none());
}

#[tokio::test]
async fn list_agents_filters_by_parent_and_excludes_detached() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full("keep", TaskStatus::Running, "p1", false, false))
        .await;
    mgr.seed_completed_for_test(entry_full("other", TaskStatus::Running, "p2", false, false))
        .await;
    mgr.seed_completed_for_test(entry_full(
        "detached",
        TaskStatus::Running,
        "p1",
        true,
        true,
    ))
    .await;

    let listed = mgr.list_agents("p1").await;
    let ids: Vec<&str> = listed.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(ids, vec!["keep"], "only p1 non-detached entries: {ids:?}");
}

#[tokio::test]
async fn running_background_count_counts_only_running_background() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full("bg-run", TaskStatus::Running, "p", true, false))
        .await;
    mgr.seed_completed_for_test(entry_full(
        "bg-done",
        TaskStatus::Completed,
        "p",
        true,
        false,
    ))
    .await;
    mgr.seed_completed_for_test(entry_full("fg-run", TaskStatus::Running, "p", false, false))
        .await;
    mgr.seed_completed_for_test(entry_full(
        "bg-detached",
        TaskStatus::Running,
        "p",
        true,
        true,
    ))
    .await;

    assert_eq!(mgr.running_background_count().await, 1);
}

#[tokio::test]
async fn cancel_agent_unknown_id_errors() {
    let mgr = manager(4);
    let err = mgr
        .cancel_agent("missing")
        .await
        .expect_err("unknown task must error");
    assert!(err.to_string().contains("missing"), "err: {err}");
}

#[tokio::test]
async fn cancel_all_is_a_noop_with_no_running_tasks() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full("done", TaskStatus::Completed, "p", true, false))
        .await;
    // Nothing running for the parent - must not panic.
    mgr.cancel_all("p").await;
}

#[tokio::test]
async fn increment_waiter_only_succeeds_for_running_tasks() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full("run", TaskStatus::Running, "p", true, false))
        .await;
    mgr.seed_completed_for_test(entry_full("done", TaskStatus::Completed, "p", true, false))
        .await;

    assert!(mgr.increment_waiter("run").await, "running task increments");
    assert!(
        !mgr.increment_waiter("done").await,
        "completed task must not increment"
    );
    assert!(
        !mgr.increment_waiter("absent").await,
        "absent task must not increment"
    );
}

#[tokio::test]
async fn decrement_waiter_is_bounded_at_zero() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full("run", TaskStatus::Running, "p", true, false))
        .await;

    // Decrementing a zero count is a no-op; incrementing then decrementing
    // returns to zero.
    mgr.decrement_waiter("run").await;
    assert_eq!(mgr.get_task("run").await.expect("task").waiter_count, 0);
    assert!(mgr.increment_waiter("run").await);
    assert_eq!(mgr.get_task("run").await.expect("task").waiter_count, 1);
    mgr.decrement_waiter("run").await;
    assert_eq!(mgr.get_task("run").await.expect("task").waiter_count, 0);
    // Decrementing a missing task id is a no-op (no panic).
    mgr.decrement_waiter("absent").await;
}

#[tokio::test]
async fn drain_completed_returns_nothing_without_the_pending_flag() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full("done", TaskStatus::Completed, "p", true, false))
        .await;
    // The P-11 fast path returns early when the flag is unset.
    assert!(!mgr.has_pending_background());
    assert!(mgr.drain_completed("p").await.is_empty());
}

#[tokio::test]
async fn drain_completed_reports_and_reaps_completed_background_tasks() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full("done", TaskStatus::Completed, "p", true, false))
        .await;
    mgr.seed_completed_for_test(entry_full("running", TaskStatus::Running, "p", true, false))
        .await;
    mgr.set_pending_background_for_test();
    assert!(mgr.has_pending_background());

    let drained = mgr.drain_completed("p").await;
    let ids: Vec<&str> = drained.iter().map(|e| e.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["done"],
        "only the completed task is reported: {ids:?}"
    );

    // The reported completed task was reaped; the running one remains.
    assert!(
        mgr.get_task("done").await.is_none(),
        "completed task reaped"
    );
    assert!(mgr.get_task("running").await.is_some());
    // The flag cleared because no unreported completed background tasks remain.
    assert!(!mgr.has_pending_background());
}

#[tokio::test]
async fn drain_completed_skips_tasks_with_active_waiters() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full(
        "waited",
        TaskStatus::Completed,
        "p",
        true,
        false,
    ))
    .await;
    // Simulate a live waiter: waiter_count is bumped while the task runs, but
    // here we set it via the field directly through a re-seed.
    let mut e = entry_full("waited", TaskStatus::Completed, "p", true, false);
    e.waiter_count = 1;
    mgr.seed_completed_for_test(e).await;

    mgr.set_pending_background_for_test();
    let drained = mgr.drain_completed("p").await;
    assert!(
        drained.is_empty(),
        "a task with an active waiter must not be injected"
    );
}

#[tokio::test]
async fn drain_completed_skips_detached_tasks_but_reaps_them() {
    let mgr = manager(4);
    mgr.seed_completed_for_test(entry_full("fire", TaskStatus::Completed, "p", true, true))
        .await;
    mgr.set_pending_background_for_test();

    let drained = mgr.drain_completed("p").await;
    assert!(drained.is_empty(), "detached tasks are never injected");
    assert!(mgr.get_task("fire").await.is_none(), "detached task reaped");
}

#[tokio::test]
async fn spawn_background_rejects_when_the_concurrency_limit_is_reached() {
    // A manager capped at one background task: seed a running background entry
    // so the limit check trips before any real sub-agent is spawned.
    let mgr = Arc::new(ragent_agent::task::AgentManager::new(
        Arc::new(ragent_agent::event::EventBus::new(8)),
        test_processor(),
        1,
        300,
    ));
    mgr.seed_completed_for_test(entry_full("busy", TaskStatus::Running, "p", true, false))
        .await;

    let dir = tempfile::tempdir().expect("tempdir");
    let err = mgr
        .spawn_background("p", "explore", "do it", None, dir.path(), None)
        .await
        .expect_err("the concurrency limit must reject the spawn");
    assert!(
        err.to_string()
            .contains("Maximum concurrent background tasks"),
        "err: {err}"
    );
}

/// FR-011: the registry reports the limit, its occupancy, the resulting
/// headroom and the ids occupying the limit as one consistent snapshot.
#[tokio::test]
async fn limit_report_counts_running_and_suspended_background_tasks() {
    let mgr = manager(2);
    assert_eq!(mgr.max_background(), 2);

    let empty = mgr.limit_report().await;
    assert_eq!((empty.limit, empty.occupancy, empty.headroom), (2, 0, 2));
    assert!(empty.occupying_ids.is_empty());

    mgr.seed_completed_for_test(entry_full("bg-run", TaskStatus::Running, "p", true, false))
        .await;
    let one = mgr.limit_report().await;
    assert_eq!((one.limit, one.occupancy, one.headroom), (2, 1, 1));
    assert_eq!(one.occupying_ids, vec!["bg-run".to_string()]);

    // FR-030: a suspended task still holds its slot; a detached one never does,
    // and neither does a foreground or a finished task.
    mgr.seed_completed_for_test(entry_full(
        "bg-suspended",
        TaskStatus::Suspended,
        "p",
        true,
        false,
    ))
    .await;
    mgr.seed_completed_for_test(entry_full(
        "bg-detached",
        TaskStatus::Running,
        "p",
        true,
        true,
    ))
    .await;
    mgr.seed_completed_for_test(entry_full("fg-run", TaskStatus::Running, "p", false, false))
        .await;
    mgr.seed_completed_for_test(entry_full(
        "bg-done",
        TaskStatus::Completed,
        "p",
        true,
        false,
    ))
    .await;

    let full = mgr.limit_report().await;
    assert_eq!((full.limit, full.occupancy, full.headroom), (2, 2, 0));
    let mut ids = full.occupying_ids.clone();
    ids.sort();
    assert_eq!(ids, vec!["bg-run".to_string(), "bg-suspended".to_string()]);
    assert_eq!(mgr.running_background_count().await, full.occupancy);

    // Headroom saturates at zero: a limit lowered below the occupancy must not
    // report a negative number of free slots.
    mgr.seed_completed_for_test(entry_full(
        "bg-run-2",
        TaskStatus::Running,
        "p",
        true,
        false,
    ))
    .await;
    let over = mgr.limit_report().await;
    assert_eq!((over.occupancy, over.headroom), (3, 0));
}

/// FR-029: the spawn refusal names the limit, the occupancy and the tasks
/// occupying it, and still carries the `limit_reached` code (FR-010).
#[tokio::test]
async fn spawn_refusal_names_limit_occupancy_and_occupying_tasks() {
    use ragent_agent::tool::param_schema::reason_in;

    let mgr = manager(2);
    mgr.seed_completed_for_test(entry_full("busy-a", TaskStatus::Running, "p", true, false))
        .await;
    mgr.seed_completed_for_test(entry_full(
        "busy-b",
        TaskStatus::Suspended,
        "p",
        true,
        false,
    ))
    .await;

    let dir = tempfile::tempdir().expect("tempdir");
    let err = mgr
        .spawn_background("p", "explore", "do it", None, dir.path(), None)
        .await
        .expect_err("the concurrency limit must reject the spawn");

    let message = err.to_string();
    assert_eq!(reason_in(&message), Some("limit_reached"), "{message}");
    assert!(
        message.contains("(2)"),
        "the limit must be named: {message}"
    );
    assert!(
        message.contains("2 in use"),
        "the occupancy must be named: {message}"
    );
    assert!(
        message.contains("busy-a") && message.contains("busy-b"),
        "the occupying task ids must be named: {message}"
    );
    assert!(
        message.contains("headroom"),
        "the headroom must be named: {message}"
    );
}
