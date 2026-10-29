//! Task cancellation (spec `teamssetup` FR-005, FR-006).
//!
//! `TaskStatus::Cancelled` had no producer before T-011, so a lead could not
//! abandon a task except by editing `tasks.json`. These tests pin the new
//! producer and the properties the spec attaches to it:
//!
//! - FR-005 - a `Pending` or `InProgress` task becomes `Cancelled`, and an
//!   `InProgress` task's owner is told through the mailbox;
//! - FR-006 - a cancelled task is refused by every claim and completion path, and
//!   the refusal names `cancelled` rather than `pending`;
//! - FR-003/FR-004 - the status move goes through `transition_task`, so a
//!   terminal task cannot be cancelled again and a refusal leaves `tasks.json`
//!   byte-identical;
//! - FR-025/FR-026 - the cached (`TeamManager`) and uncached (`TaskStore`) paths
//!   share one implementation and one refusal text.

use ragent_agent::team::{Mailbox, MessageType, Task, TaskStatus, TaskStore};

#[path = "team_support/mod.rs"]
mod team_support;
use team_support::setup_team_with_task_store;

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::team_manager;

/// A real team directory with an open task store: the store's mutators fire the
/// team's task hooks (FR-017), so they need a readable `config.json` beside
/// `tasks.json`.
fn store() -> (tempfile::TempDir, TaskStore) {
    let (tmp, _team_dir, store) = setup_team_with_task_store();
    (tmp, store)
}

/// Seed a task straight into the store's list so a fixture can start in a
/// non-`Pending` status.
fn seed(store: &TaskStore, task: Task) {
    store
        .write_through(|list| list.tasks.push(task))
        .expect("seed task");
}

fn task_in(id: &str, status: TaskStatus, owner: Option<&str>) -> Task {
    let mut task = Task::new(id, format!("work {id}"));
    task.status = status;
    task.assigned_to = owner.map(str::to_owned);
    task
}

fn status_of(store: &TaskStore, id: &str) -> TaskStatus {
    store
        .read()
        .expect("read tasks")
        .tasks
        .iter()
        .find(|t| t.id == id)
        .expect("task present")
        .status
}

// -- FR-005: the producer ---------------------------------------------------

#[tokio::test]
async fn store_cancels_a_pending_task_without_notifying_anyone() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");

    let cancelled = store.cancel("task-001", "lead").expect("cancel");

    assert_eq!(cancelled.task.status, TaskStatus::Cancelled);
    assert_eq!(cancelled.from, TaskStatus::Pending);
    assert_eq!(cancelled.transition.from, TaskStatus::Pending);
    assert_eq!(cancelled.transition.to, TaskStatus::Cancelled);
    assert_eq!(cancelled.transition.actor, "lead");
    assert_eq!(status_of(&store, "task-001"), TaskStatus::Cancelled);
    assert!(
        cancelled.notify_target().is_none(),
        "a pending task has nobody working on it, so nobody is notified"
    );
}

#[test]
fn store_cancels_an_in_progress_task_and_keeps_the_owner_on_the_record() {
    let (_tmp, store) = store();
    seed(
        &store,
        task_in("task-001", TaskStatus::InProgress, Some("tm-001")),
    );

    let cancelled = store.cancel("task-001", "lead").expect("cancel");

    assert_eq!(cancelled.task.status, TaskStatus::Cancelled);
    assert_eq!(cancelled.from, TaskStatus::InProgress);
    assert_eq!(cancelled.notify_target(), Some("tm-001"));
    // The owner stays on the record: the lead can still see who was working on
    // the abandoned task.
    assert_eq!(cancelled.task.assigned_to.as_deref(), Some("tm-001"));
    assert_eq!(cancelled.owner.as_deref(), Some("tm-001"));
}

#[test]
fn store_refuses_to_cancel_a_completed_task_naming_completed() {
    let (_tmp, store) = store();
    seed(
        &store,
        task_in("task-001", TaskStatus::Completed, Some("tm-001")),
    );

    let err = store.cancel("task-001", "lead").expect_err("refused");

    assert!(
        err.to_string().contains("in status completed"),
        "the refusal must name the actual status: {err}"
    );
    assert_eq!(status_of(&store, "task-001"), TaskStatus::Completed);
}

#[test]
fn store_refuses_to_cancel_a_cancelled_task_naming_cancelled() {
    let (_tmp, store) = store();
    seed(&store, task_in("task-001", TaskStatus::Cancelled, None));

    let err = store.cancel("task-001", "lead").expect_err("refused");

    assert!(
        err.to_string().contains("in status cancelled"),
        "re-cancelling must report the status it holds: {err}"
    );
}

#[tokio::test]
async fn store_cancel_of_an_unknown_task_names_the_available_ids() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");

    let err = store.cancel("task-999", "lead").expect_err("not found");

    assert!(err.to_string().contains("task-999"), "err: {err}");
    assert!(err.to_string().contains("task-001"), "err: {err}");
}

#[test]
fn refused_cancellation_leaves_tasks_json_byte_identical() {
    let (_tmp, store) = store();
    seed(
        &store,
        task_in("task-001", TaskStatus::Completed, Some("tm-001")),
    );
    let path = store.path().to_path_buf();
    let before = std::fs::read(&path).expect("read before");

    store.cancel("task-001", "lead").expect_err("refused");

    let after = std::fs::read(&path).expect("read after");
    assert_eq!(before, after, "a refused cancellation must not touch disk");
}

// -- FR-005: the manager entry point and its mailbox notification ----------

#[test]
fn manager_cancel_notifies_the_owning_teammate_through_the_mailbox() {
    let (manager, _tmp, _bus) = team_manager("tcancel-owner", "lead-1", true);
    manager
        .apply_to_task_list(|list| {
            list.tasks
                .push(task_in("task-001", TaskStatus::InProgress, Some("tm-001")));
        })
        .expect("seed");

    let cancelled = manager
        .cancel_task("task-001", "lead", Some("not needed"))
        .expect("cancel");

    assert_eq!(cancelled.task.status, TaskStatus::Cancelled);
    assert_eq!(
        cancelled.notified.as_deref(),
        Some("tm-001"),
        "the owning teammate must be told"
    );

    let mailbox = Mailbox::open(&manager.team_dir, "tm-001").expect("open mailbox");
    let messages = mailbox.drain_unread().expect("drain");
    let notice = messages
        .iter()
        .find(|m| m.message_type == MessageType::TaskCancelled)
        .expect("a TaskCancelled message");
    assert_eq!(notice.from, "lead");
    assert_eq!(notice.to, "tm-001");
    assert!(
        notice.content.contains("task-001"),
        "the notice names the task: {}",
        notice.content
    );
    assert!(
        notice.content.contains("not needed"),
        "the notice carries the lead's reason: {}",
        notice.content
    );
}

#[test]
fn manager_cancel_of_a_pending_task_sends_no_message() {
    let (manager, _tmp, _bus) = team_manager("tcancel-pending", "lead-1", true);
    manager
        .apply_to_task_list(|list| list.tasks.push(Task::new("task-001", "a")))
        .expect("seed");

    let cancelled = manager
        .cancel_task("task-001", "lead", None)
        .expect("cancel");

    assert!(
        cancelled.notified.is_none(),
        "a pending task has no owner to notify"
    );
    let mailbox = Mailbox::open(&manager.team_dir, "tm-001").expect("open mailbox");
    assert!(
        mailbox.drain_unread().expect("drain").is_empty(),
        "no message must be written for an unowned task"
    );
}

#[test]
fn manager_cancel_is_lead_only() {
    let (manager, _tmp, _bus) = team_manager("tcancel-actor", "lead-1", true);
    manager
        .apply_to_task_list(|list| {
            list.tasks
                .push(task_in("task-001", TaskStatus::InProgress, Some("tm-002")));
        })
        .expect("seed");

    let err = manager
        .cancel_task("task-001", "tm-002", None)
        .expect_err("a teammate cannot cancel");

    assert!(err.to_string().contains("only the lead"), "err: {err}");
    let on_disk = manager.task_list().expect("read");
    let task = on_disk
        .tasks
        .iter()
        .find(|t| t.id == "task-001")
        .expect("present");
    assert_eq!(task.status, TaskStatus::InProgress, "nothing was mutated");
}

#[tokio::test]
async fn manager_and_store_share_one_refusal_text() {
    let (_tmp, _team_dir, store) = setup_team_with_task_store();
    let completed = task_in("task-001", TaskStatus::Completed, Some("tm-001"));
    store.add_task(completed.clone()).await.expect("add");

    let store_err = store
        .cancel("task-001", "lead")
        .expect_err("terminal task is not cancellable");

    let (manager, _tmp2, _bus) = team_manager("tcancel-parity", "lead-1", true);
    manager
        .apply_to_task_list(|list| list.tasks.push(completed))
        .expect("seed");
    let manager_err = manager
        .cancel_task("task-001", "lead", None)
        .expect_err("terminal task is not cancellable");

    assert_eq!(
        store_err.to_string(),
        manager_err.to_string(),
        "the two entry points must refuse in the same words (FR-026)"
    );
}

#[tokio::test]
async fn manager_cancel_of_an_in_progress_task_leaves_it_unclaimable() {
    let (manager, _tmp, _bus) = team_manager("tcancel-claim", "lead-1", true);
    manager
        .apply_to_task_list(|list| {
            list.tasks
                .push(task_in("task-001", TaskStatus::InProgress, Some("tm-001")));
        })
        .expect("seed");

    manager
        .cancel_task("task-001", "lead", None)
        .expect("cancel");

    // FR-006: the cancelled task is refused by name on the specific path and
    // skipped by the next-task scan.
    let store = TaskStore::open(&manager.team_dir).expect("open store");
    let err = store
        .claim_specific("task-001", "tm-002")
        .await
        .expect_err("cancelled task is not claimable");
    assert!(
        err.to_string().contains("in status cancelled"),
        "err: {err}"
    );
    let (next, already_had) = manager.claim_next_task("tm-002").await.expect("claim next");
    assert!(
        next.is_none() && !already_had,
        "a cancelled task is skipped by the next-task scan"
    );
}

// -- FR-006: completion of a cancelled task is refused ----------------------

#[tokio::test]
async fn completing_a_cancelled_task_is_refused_and_mutates_nothing() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");
    store.cancel("task-001", "lead").expect("cancel");

    let err = store
        .complete("task-001", "lead")
        .await
        .expect_err("cancelled is terminal");

    assert!(
        err.to_string().contains("cancelled"),
        "the refusal names the cancelled status: {err}"
    );
    let on_disk = store.read().expect("read");
    let task = on_disk
        .tasks
        .iter()
        .find(|t| t.id == "task-001")
        .expect("present");
    assert_eq!(task.status, TaskStatus::Cancelled);
    assert!(task.completed_by.is_none());
    assert!(task.completed_at.is_none());
}
