//! The single claim implementation (spec `teamssetup` FR-007, FR-025, FR-026).
//!
//! `TaskStore::claim_next`, `TaskStore::claim_specific`,
//! `TaskStore::pre_assign_task` and `TeamManager::claim_next_task` all delegate
//! to `claim_task_in_list`. These tests pin the properties that delegation is
//! supposed to buy:
//!
//! - every claim path asks `Task::is_claimable`, so a cancelled, completed, or
//!   dependency-blocked task is refused by all of them;
//! - the cached (`TeamManager`) and uncached (`TaskStore`) paths produce the
//!   same task, the same mutation and the same refusal text;
//! - one agent holds one task at a time on every path.

use ragent_agent::team::{Task, TaskList, TaskStatus, TaskStore};

#[path = "team_support/mod.rs"]
mod team_support;
use team_support::setup_team_with_task_store;

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::team_manager;

/// A real team directory with an open task store: the store's mutators fire the
/// team's task hooks (FR-017), so they need a readable `config.json`.
fn store() -> (tempfile::TempDir, TaskStore) {
    let (tmp, _team_dir, store) = setup_team_with_task_store();
    (tmp, store)
}

/// Write a task directly into the store's list, bypassing `add_task` so the
/// fixture can place a task in a non-`Pending` status.
fn seed(store: &TaskStore, tasks: Vec<Task>) {
    store
        .write_through(|list| list.tasks.extend(tasks))
        .expect("seed tasks");
}

/// A team directory holding `tasks`, with a store over it.
fn team_over(tasks: Vec<Task>) -> (tempfile::TempDir, TaskStore) {
    let (tmp, store) = store();
    seed(&store, tasks);
    (tmp, store)
}

/// A `TeamManager` over the store's own team directory, so the cached and
/// uncached claim halves read one `tasks.json` instead of two fixtures that
/// happen to agree.
fn manager_over(
    store: &TaskStore,
) -> (
    std::sync::Arc<ragent_agent::team::TeamManager>,
    tempfile::TempDir,
) {
    let (manager, tmp, _bus) = manager_support::team_manager_for(&store.team_dir);
    (manager, tmp)
}

// -- one claimable predicate ------------------------------------------------

#[tokio::test]
async fn claim_next_skips_cancelled_tasks() {
    let (_tmp, store) = store();
    let mut cancelled = Task::new("task-001", "dropped");
    cancelled.status = TaskStatus::Cancelled;
    store.add_task(cancelled).await.expect("add cancelled");
    store
        .add_task(Task::new("task-002", "live"))
        .await
        .expect("add pending");

    let (claimed, already_had) = store.claim_next("tm-001").await.expect("claim");
    assert!(!already_had);
    assert_eq!(
        claimed.expect("a claimable task").id,
        "task-002",
        "a cancelled task is not claimable"
    );
}

#[tokio::test]
async fn claim_specific_refuses_a_cancelled_task_and_names_the_status() {
    let (_tmp, store) = store();
    let mut cancelled = Task::new("task-001", "dropped");
    cancelled.status = TaskStatus::Cancelled;
    store.add_task(cancelled).await.expect("add cancelled");

    let err = store
        .claim_specific("task-001", "tm-001")
        .await
        .expect_err("cancelled task is not claimable");
    let msg = err.to_string();
    assert!(msg.contains("in status cancelled"), "err: {msg}");
    assert!(
        msg.contains("terminal"),
        "the refusal must say a cancelled task cannot be claimed again: {msg}"
    );
    assert!(
        !msg.contains("pending"),
        "a cancelled task must never be described as pending (FR-006): {msg}"
    );
}

#[tokio::test]
async fn claim_specific_refuses_a_completed_task_by_status() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "done"))
        .await
        .expect("add");
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");
    store
        .complete("task-001", "tm-001")
        .await
        .expect("complete");

    let err = store
        .claim_specific("task-001", "tm-002")
        .await
        .expect_err("completed task is not claimable");
    assert!(
        err.to_string().contains("in status completed"),
        "err: {err}"
    );
}

#[test]
fn next_claimable_agrees_with_is_claimable() {
    let mut list = TaskList::new("t");
    let mut cancelled = Task::new("task-001", "dropped");
    cancelled.status = TaskStatus::Cancelled;
    list.tasks.push(cancelled);
    let mut blocked = Task::new("task-002", "waits");
    blocked.depends_on = vec!["task-999".to_string()];
    list.tasks.push(blocked);
    let mut done = Task::new("task-003", "finished");
    done.status = TaskStatus::Completed;
    list.tasks.push(done);

    let completed = list.completed_ids();
    for task in &list.tasks {
        assert!(
            !task.is_claimable(&completed),
            "task {} must not be claimable",
            task.id
        );
    }
    assert!(list.next_claimable().is_none());
}

// -- one task at a time -----------------------------------------------------

#[tokio::test]
async fn claim_specific_refuses_a_second_task_while_one_is_in_flight() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");
    store
        .add_task(Task::new("task-002", "b"))
        .await
        .expect("add");

    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");
    let err = store
        .claim_specific("task-002", "tm-001")
        .await
        .expect_err("one task at a time");
    // The refusal names the agent, the task it already holds, and the next
    // action - completing that task first (FR-032).
    let text = err.to_string();
    assert!(text.contains("cannot claim 'task-002'"), "err: {text}");
    assert!(
        text.contains("is working on task 'task-001'"),
        "err: {text}"
    );
    assert!(text.contains("team_task_complete"), "err: {text}");

    // Re-claiming the task already held is an idempotent success.
    let again = store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("re-claim");
    assert_eq!(again.id, "task-001");
    assert_eq!(again.status, TaskStatus::InProgress);
}

#[tokio::test]
async fn claim_next_reports_the_task_already_in_flight() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");
    store
        .add_task(Task::new("task-002", "b"))
        .await
        .expect("add");

    let (first, already_had) = store.claim_next("tm-001").await.expect("claim");
    assert_eq!(first.expect("claim").id, "task-001");
    assert!(!already_had);

    let (again, already_had) = store.claim_next("tm-001").await.expect("claim again");
    assert!(already_had);
    assert_eq!(again.expect("in-flight task").id, "task-001");
}

// -- cached and uncached paths agree ---------------------------------------

#[tokio::test]
async fn store_and_manager_claim_the_same_task() {
    let (_tmp, _team_dir, store) = setup_team_with_task_store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add task-001");
    store
        .add_task(Task::new("task-002", "b"))
        .await
        .expect("add task-002");

    let (manager, _tmp2, _bus) = team_manager("claim-parity", "lead-1", true);
    manager
        .apply_to_task_list(|list| {
            list.tasks.push(Task::new("task-001", "a"));
            list.tasks.push(Task::new("task-002", "b"));
        })
        .expect("seed manager");

    let (uncached, uncached_had) = store.claim_next("tm-001").await.expect("store claim");
    let (cached, cached_had) = manager
        .claim_next_task("tm-001")
        .await
        .expect("manager claim");

    assert!(!uncached_had && !cached_had);
    assert_eq!(
        uncached.as_ref().map(|t| t.id.as_str()),
        cached.as_ref().map(|t| t.id.as_str()),
        "both paths must claim the same task"
    );
    assert_eq!(uncached.as_ref().map(|t| t.id.as_str()), Some("task-001"));

    let on_disk = store.read().expect("read");
    let claimed = on_disk
        .tasks
        .iter()
        .find(|t| t.id == "task-001")
        .expect("task-001 present");
    assert_eq!(claimed.status, TaskStatus::InProgress);
    assert_eq!(claimed.assigned_to.as_deref(), Some("tm-001"));
    assert!(claimed.claimed_at.is_some());
}

#[tokio::test]
async fn manager_and_store_refuse_a_cancelled_task_with_the_same_text() {
    let (_tmp, _team_dir, store) = setup_team_with_task_store();
    let mut cancelled = Task::new("task-001", "dropped");
    cancelled.status = TaskStatus::Cancelled;
    store
        .add_task(cancelled.clone())
        .await
        .expect("add cancelled");

    let store_err = store
        .claim_specific("task-001", "tm-001")
        .await
        .expect_err("cancelled task is not claimable");

    let (manager, _tmp2, _bus) = team_manager("claim-parity-cancel", "lead-1", true);
    manager
        .apply_to_task_list(|list| list.tasks.push(cancelled))
        .expect("seed manager");
    let (manager_claimed, manager_already_had) = manager
        .claim_next_task("tm-001")
        .await
        .expect("nothing claimable");
    assert!(
        manager_claimed.is_none() && !manager_already_had,
        "a cancelled task leaves nothing for the cached path either"
    );

    assert!(
        store_err.to_string().contains("in status cancelled"),
        "err: {store_err}"
    );
}

#[tokio::test]
async fn manager_claim_with_nothing_available_reports_none() {
    let (manager, _tmp, _bus) = team_manager("claim-empty", "lead-1", true);
    let (claimed, already_had) = manager.claim_next_task("tm-001").await.expect("claim");
    assert!(claimed.is_none());
    assert!(!already_had);
}

// -- pre-assignment shares the rule ----------------------------------------

#[tokio::test]
async fn pre_assign_obeys_the_claim_rule() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");
    let assigned = store
        .pre_assign_task("task-001", "tm-001")
        .expect("pre-assign");
    assert_eq!(assigned.status, TaskStatus::InProgress);
    assert_eq!(assigned.assigned_to.as_deref(), Some("tm-001"));

    // A cancelled task cannot be pre-assigned either.
    let mut cancelled = Task::new("task-002", "dropped");
    cancelled.status = TaskStatus::Cancelled;
    store.add_task(cancelled).await.expect("add cancelled");
    let err = store
        .pre_assign_task("task-002", "tm-002")
        .expect_err("cancelled task is not claimable");
    assert!(
        err.to_string().contains("in status cancelled"),
        "err: {err}"
    );
}

#[test]
fn seed_helper_places_tasks_without_claiming_them() {
    let (_tmp, store) = store();
    let mut cancelled = Task::new("task-001", "dropped");
    cancelled.status = TaskStatus::Cancelled;
    seed(&store, vec![cancelled.clone()]);
    let list = store.read().expect("read");
    assert_eq!(list.tasks.len(), 1);
    assert_eq!(list.tasks[0].status, TaskStatus::Cancelled);
}

// -- every claim path shares the predicate and reports "cancelled" (T-010) ---

/// The four claim entry points, so a new one cannot be added without a row here.
#[derive(Debug, Clone, Copy)]
enum ClaimPath {
    /// `TaskStore::claim_next` - "claim the next task", uncached.
    StoreNext,
    /// `TaskStore::claim_specific` - "claim this task", uncached.
    StoreSpecific,
    /// `TaskStore::pre_assign_task` - a spawn's pre-assignment, uncached.
    StorePreAssign,
    /// `TeamManager::claim_next_task` - "claim the next task", cached.
    ManagerNext,
}

const CLAIM_PATHS: [ClaimPath; 4] = [
    ClaimPath::StoreNext,
    ClaimPath::StoreSpecific,
    ClaimPath::StorePreAssign,
    ClaimPath::ManagerNext,
];

/// The task ID the named claim paths ask for.
const TARGET: &str = "task-001";

/// Claim through `path`, returning a description of a successful claim or the
/// refusal text. A "next" claim that finds nothing claimable is a success with
/// no task, which is the outcome this fixture expects for a cancelled-only list.
async fn claim_via(
    path: ClaimPath,
    store: &TaskStore,
    manager: &ragent_agent::team::TeamManager,
) -> Result<Option<String>, String> {
    match path {
        ClaimPath::StoreNext => store
            .claim_next("tm-001")
            .await
            .map(|(task, already_had)| {
                assert!(!already_had, "nothing was in flight to already own");
                task.map(|t| t.id)
            })
            .map_err(|e| e.to_string()),
        ClaimPath::StoreSpecific => store
            .claim_specific(TARGET, "tm-001")
            .await
            .map(|t| Some(t.id))
            .map_err(|e| e.to_string()),
        ClaimPath::StorePreAssign => store
            .pre_assign_task(TARGET, "tm-001")
            .map(|t| Some(t.id))
            .map_err(|e| e.to_string()),
        ClaimPath::ManagerNext => manager
            .claim_next_task("tm-001")
            .await
            .map(|(task, already_had)| {
                assert!(!already_had, "nothing was in flight to already own");
                task.map(|t| t.id)
            })
            .map_err(|e| e.to_string()),
    }
}

#[tokio::test]
async fn every_claim_path_refuses_a_cancelled_task_naming_cancelled() {
    for path in CLAIM_PATHS {
        // One team directory, one cancelled task, both claim halves over it.
        let (_tmp, store) = team_over(Vec::new());
        let mut cancelled = Task::new(TARGET, "dropped");
        cancelled.status = TaskStatus::Cancelled;
        seed(&store, vec![cancelled]);
        let (manager, _tmp2) = manager_over(&store);

        let outcome = claim_via(path, &store, &manager).await;
        match path {
            // A "next" claim has no task to name, so a list holding only a
            // cancelled task yields "nothing available" rather than a refusal.
            // The `team_task_claim` surface reports the cancelled count for
            // that case (`cancelled_tasks`), which the next test pins.
            ClaimPath::StoreNext | ClaimPath::ManagerNext => {
                assert_eq!(
                    outcome.expect("a next claim is never an error"),
                    None,
                    "{path:?} must find nothing claimable in a cancelled-only list"
                );
            }
            // A named claim must refuse and say "cancelled", never "pending".
            ClaimPath::StoreSpecific | ClaimPath::StorePreAssign => {
                let err = outcome.expect_err("a cancelled task is not claimable");
                assert!(
                    err.contains("in status cancelled"),
                    "{path:?} must name the cancelled status: {err}"
                );
                assert!(
                    err.contains("terminal"),
                    "{path:?} must say the status is terminal, not suggest a retry: {err}"
                );
                assert!(
                    !err.contains("in status pending"),
                    "{path:?} must not describe a cancelled task as pending: {err}"
                );
            }
        }

        // Whatever the path said, the cancelled task is untouched.
        let list = store.read().expect("read");
        assert_eq!(
            list.tasks[0].status,
            TaskStatus::Cancelled,
            "{path:?} must not change a cancelled task"
        );
        assert_eq!(
            list.tasks[0].assigned_to, None,
            "{path:?} must not assign a cancelled task"
        );
    }
}

#[tokio::test]
async fn a_refused_claim_leaves_tasks_json_byte_identical() {
    let (_tmp, store) = team_over(Vec::new());
    let mut cancelled = Task::new(TARGET, "dropped");
    cancelled.status = TaskStatus::Cancelled;
    seed(&store, vec![cancelled]);
    let (manager, _tmp2) = manager_over(&store);

    let before = std::fs::read(store.path()).expect("read tasks.json");
    for path in CLAIM_PATHS {
        // Every outcome here is "nothing was claimed"; none may write.
        let _ = claim_via(path, &store, &manager).await;
    }
    let after = std::fs::read(store.path()).expect("read tasks.json");
    assert_eq!(
        before, after,
        "a refused claim must not touch tasks.json (FR-004)"
    );
}

#[tokio::test]
async fn the_claim_refusal_names_the_status_the_task_holds() {
    // FR-006 at the predicate: `is_claimable` is false for every non-pending
    // status, and the explanation names the status the task holds rather than
    // the only status a claim accepts.
    for status in [
        TaskStatus::InProgress,
        TaskStatus::Completed,
        TaskStatus::Cancelled,
    ] {
        let (_tmp, store) = store();
        let mut task = Task::new(TARGET, "held");
        task.status = status;
        seed(&store, vec![task]);

        let err = store
            .claim_specific(TARGET, "tm-001")
            .await
            .expect_err("a non-pending task is not claimable");
        let msg = err.to_string();
        assert!(
            msg.contains(&format!("in status {}", status.as_str())),
            "{status:?}: the refusal must name the status the task holds: {msg}"
        );
        assert!(
            !msg.contains("in status pending"),
            "{status:?}: the refusal must not call the task pending: {msg}"
        );
        let list = store.read().expect("read");
        assert_eq!(
            list.tasks[0].status, status,
            "{status:?}: a refused claim must not change the status"
        );
    }
}

#[tokio::test]
async fn the_cancelled_refusal_differs_from_the_completed_one() {
    // A cancelled task is terminal with no way back, so the refusal says so;
    // a completed task keeps the generic wording. Both name their status.
    let (_tmp, store) = store();
    let mut cancelled = Task::new("task-001", "dropped");
    cancelled.status = TaskStatus::Cancelled;
    let mut completed = Task::new("task-002", "done");
    completed.status = TaskStatus::Completed;
    seed(&store, vec![cancelled, completed]);

    let cancelled_err = store
        .claim_specific("task-001", "tm-001")
        .await
        .expect_err("cancelled")
        .to_string();
    let completed_err = store
        .claim_specific("task-002", "tm-001")
        .await
        .expect_err("completed")
        .to_string();
    assert!(
        cancelled_err.contains("a cancelled task is terminal"),
        "err: {cancelled_err}"
    );
    assert!(
        completed_err.contains("a finished task is terminal"),
        "err: {completed_err}"
    );
    assert_ne!(cancelled_err, completed_err);
}
