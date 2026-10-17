//! Tests-only coverage for the file-locked team `TaskStore` and the `Task` /
//! `TaskList` value types (`ragent_agent::team::task`).
//!
//! Exercises the claim / complete / pre-assign / update / remove state machine,
//! the idempotency and dependency guards, and the list-level helpers. All state
//! lives in a temp team directory.

use ragent_agent::team::task::{TASK_LIST_SCHEMA_VERSION, TaskList};
use ragent_agent::team::{Task, TaskStatus, TaskStore};

#[path = "team_support/mod.rs"]
mod team_support;
use team_support::setup_workspace;

fn store() -> (tempfile::TempDir, TaskStore) {
    let (tmp, dir) = setup_workspace();
    let store = TaskStore::open(&dir).expect("open task store");
    (tmp, store)
}

#[test]
fn task_defaults_and_list_helpers() {
    let mut t = Task::new("task-001", "do it");
    assert_eq!(t.status, TaskStatus::Pending);
    assert_eq!(t.depends_on.len(), 0);
    t.validate().expect("valid task");

    t.depends_on = vec!["task-001".to_string()];
    assert!(t.validate().is_err(), "self-dependency must be rejected");

    let list = TaskList::new("team-a");
    assert_eq!(list.schema_version, TASK_LIST_SCHEMA_VERSION);
    assert_eq!(list.team_name, "team-a");
    assert!(list.validate().is_ok());
}

#[test]
fn task_list_migrate_stamps_version_and_timestamp() {
    let mut list = TaskList {
        schema_version: 0,
        team_name: "t".to_string(),
        tasks: Vec::new(),
        updated_at: None,
    };
    list.migrate();
    assert_eq!(list.schema_version, TASK_LIST_SCHEMA_VERSION);
    assert!(list.updated_at.is_some());
}

#[test]
fn read_on_missing_file_returns_default_list() {
    let (_tmp, store) = store();
    let list = store.read().expect("read");
    assert!(list.tasks.is_empty());
    assert_eq!(list.schema_version, 0, "an empty file reads as the Default");
}

#[test]
fn add_task_persists_and_rejects_duplicates() {
    let (_tmp, store) = store();
    store.add_task(Task::new("task-001", "first")).expect("add");
    let list = store.read().expect("read");
    assert_eq!(list.tasks.len(), 1);
    assert!(list.updated_at.is_some(), "write stamps updated_at");

    let err = store
        .add_task(Task::new("task-001", "dup"))
        .expect_err("duplicate id must error");
    assert!(err.to_string().contains("already exists"), "err: {err}");
}

#[test]
fn write_through_applies_the_mutation() {
    let (_tmp, store) = store();
    let written = store
        .write_through(|list| list.tasks.push(Task::new("task-001", "t")))
        .expect("write_through");
    assert_eq!(written.tasks.len(), 1);
    assert_eq!(store.read().expect("read").tasks.len(), 1);
}

#[test]
fn claim_next_respects_dependencies() {
    let (_tmp, store) = store();

    // task-002 depends on task-001.
    let mut dep = Task::new("task-002", "dependent");
    dep.depends_on = vec!["task-001".to_string()];
    store.add_task(dep).expect("add dep");
    store
        .add_task(Task::new("task-001", "prereq"))
        .expect("add prereq");

    // The prereq is the only claimable task first.
    let (first, already_had) = store.claim_next("tm-001").expect("claim");
    assert!(!already_had);
    assert_eq!(first.expect("task").id, "task-001");

    // task-002 is blocked until task-001 completes.
    let (blocked, _) = store.claim_next("tm-002").expect("claim again");
    assert!(blocked.is_none(), "dependency blocks task-002");

    store
        .complete("task-001", "tm-001")
        .expect("complete prereq");
    let (now, _) = store.claim_next("tm-002").expect("claim after completion");
    assert_eq!(now.expect("claimable").id, "task-002");
}

#[test]
fn claim_next_returns_the_agents_existing_in_progress_task() {
    let (_tmp, store) = store();
    store.add_task(Task::new("task-001", "only")).expect("add");

    let (first, already_had) = store.claim_next("tm-001").expect("claim");
    assert!(!already_had);
    assert_eq!(first.expect("task").id, "task-001");

    let (again, already_had) = store.claim_next("tm-001").expect("claim again");
    assert!(already_had, "second claim reports already owned");
    assert_eq!(again.expect("task").id, "task-001");
}

#[test]
fn claim_next_with_nothing_available_returns_none() {
    let (_tmp, store) = store();
    let (task, already_had) = store.claim_next("tm-001").expect("claim");
    assert!(task.is_none());
    assert!(!already_had);
}

#[test]
fn claim_specific_guards_and_success() {
    let (_tmp, store) = store();
    store.add_task(Task::new("task-001", "a")).expect("add");
    store.add_task(Task::new("task-002", "b")).expect("add");

    // Unknown task.
    assert!(store.claim_specific("task-999", "tm-001").is_err());

    // Claim task-001, then a different in-progress task for the same agent is
    // rejected while task-001 is still in progress.
    store.claim_specific("task-001", "tm-001").expect("claim 1");
    let err = store
        .claim_specific("task-002", "tm-001")
        .expect_err("must complete first");
    assert!(
        err.to_string().contains("must complete it before"),
        "err: {err}"
    );

    // Idempotent re-claim of the same task.
    let again = store
        .claim_specific("task-001", "tm-001")
        .expect("re-claim");
    assert_eq!(again.id, "task-001");

    // Another agent cannot claim a task already assigned to tm-001.
    store.complete("task-001", "tm-001").expect("complete");
    // task-002 is unassigned; pre-assign it to tm-002 then try tm-003.
    store
        .pre_assign_task("task-002", "tm-002")
        .expect("pre-assign");
    let err = store
        .claim_specific("task-002", "tm-003")
        .expect_err("assigned to another agent");
    assert!(
        err.to_string().contains("already assigned to tm-002"),
        "err: {err}"
    );
}

#[test]
fn claim_specific_rejects_non_pending_and_unsatisfied() {
    let (_tmp, store) = store();
    store.add_task(Task::new("task-001", "done")).expect("add");
    // Claim and complete as the same agent so the assigned_to guard is skipped
    // and the *status* guard is the one exercised.
    store.claim_specific("task-001", "tm-001").expect("claim");
    store.complete("task-001", "tm-001").expect("complete");

    let err = store
        .claim_specific("task-001", "tm-001")
        .expect_err("completed task not claimable");
    assert!(err.to_string().contains("cannot be claimed"), "err: {err}");

    // A pending task with an unsatisfied dependency cannot be claimed.
    let mut dep = Task::new("task-002", "needs a missing prereq");
    dep.depends_on = vec!["task-000".to_string()];
    store.add_task(dep).expect("add dep");
    let err = store
        .claim_specific("task-002", "tm-002")
        .expect_err("unsatisfied dependency");
    assert!(
        err.to_string().contains("unsatisfied dependencies"),
        "err: {err}"
    );
}

#[test]
fn complete_is_idempotent_per_agent_and_rejects_other_agent() {
    let (_tmp, store) = store();
    store.add_task(Task::new("task-001", "a")).expect("add");

    let done = store.complete("task-001", "tm-001").expect("complete");
    assert_eq!(done.status, TaskStatus::Completed);
    assert_eq!(done.completed_by.as_deref(), Some("tm-001"));

    let again = store.complete("task-001", "tm-001").expect("re-complete");
    assert_eq!(again.id, "task-001");

    let err = store
        .complete("task-001", "tm-002")
        .expect_err("another agent cannot re-complete");
    assert!(
        err.to_string().contains("already completed by 'tm-001'"),
        "err: {err}"
    );
}

#[test]
fn complete_missing_task_lists_available() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "present"))
        .expect("add");
    let err = store.complete("task-404", "tm-001").expect_err("not found");
    let msg = err.to_string();
    assert!(msg.contains("task 'task-404' not found"), "err: {msg}");
    assert!(
        msg.contains("task-001 (present)"),
        "err should list available: {msg}"
    );
}

#[test]
fn pre_assign_guards_and_success() {
    let (_tmp, store) = store();
    store.add_task(Task::new("task-001", "a")).expect("add");

    let assigned = store
        .pre_assign_task("task-001", "tm-001")
        .expect("pre-assign");
    assert_eq!(assigned.status, TaskStatus::InProgress);
    assert_eq!(assigned.assigned_to.as_deref(), Some("tm-001"));

    // Unknown task.
    assert!(store.pre_assign_task("task-999", "tm-002").is_err());
    // Not pending (already in progress).
    let err = store
        .pre_assign_task("task-001", "tm-002")
        .expect_err("not pending");
    assert!(err.to_string().contains("not pending"), "err: {err}");
}

#[test]
fn update_and_remove_task() {
    let (_tmp, store) = store();
    store.add_task(Task::new("task-001", "a")).expect("add");

    let updated = store
        .update_task("task-001", |t| t.title = "renamed".to_string())
        .expect("update");
    assert_eq!(updated.title, "renamed");

    let err = store
        .update_task("task-404", |_| {})
        .expect_err("missing task");
    assert!(err.to_string().contains("not found"), "err: {err}");

    let removed = store.remove_task("task-001").expect("remove");
    assert_eq!(removed.id, "task-001");
    assert!(store.read().expect("read").tasks.is_empty());
    assert!(
        store.remove_task("task-001").is_err(),
        "second remove fails"
    );
}

#[test]
fn task_list_completed_ids_next_claimable_and_in_progress() {
    let mut list = TaskList::new("t");
    let mut a = Task::new("task-001", "a");
    a.status = TaskStatus::Completed;
    let mut b = Task::new("task-002", "b");
    b.status = TaskStatus::InProgress;
    b.assigned_to = Some("tm-001".to_string());
    b.depends_on = vec!["task-001".to_string()];
    list.tasks.push(a);
    list.tasks.push(b);

    let done = list.completed_ids();
    assert!(done.contains("task-001"));
    assert!(!done.contains("task-002"));

    // task-002 is InProgress, not Pending, so no claimable task remains.
    assert!(list.next_claimable().is_none());
    assert_eq!(
        list.in_progress_for("tm-001").map(|t| t.id.as_str()),
        Some("task-002")
    );
    assert!(list.in_progress_for("tm-999").is_none());
    assert!(list.get_mut("task-001").is_some());
    assert!(list.get_mut("nope").is_none());
}

#[tokio::test]
async fn blocking_wrappers_round_trip() {
    let (_tmp, store) = store();
    store
        .add_task_blocking(Task::new("task-001", "a"))
        .await
        .expect("add blocking");

    let list = store.read_blocking().await.expect("read blocking");
    assert_eq!(list.tasks.len(), 1);

    let (claimed, _) = store
        .claim_next_blocking("tm-001".to_string())
        .await
        .expect("claim blocking");
    assert_eq!(claimed.expect("task").id, "task-001");

    let completed = store
        .complete_blocking("task-001".to_string(), "tm-001".to_string())
        .await
        .expect("complete blocking");
    assert_eq!(completed.status, TaskStatus::Completed);

    let updated = store
        .update_task_blocking("task-001".to_string(), |t| {
            t.status = TaskStatus::Cancelled;
        })
        .await
        .expect("update blocking");
    assert_eq!(updated.status, TaskStatus::Cancelled);
}
