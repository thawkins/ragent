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
use team_support::setup_team_with_task_store;

/// A real team directory with an open task store: the store's mutators fire the
/// team's task hooks (FR-017), so they need a readable `config.json` beside
/// `tasks.json`.
fn store() -> (tempfile::TempDir, TaskStore) {
    let (tmp, _team_dir, store) = setup_team_with_task_store();
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

#[tokio::test]
async fn add_task_persists_and_rejects_duplicates() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "first"))
        .await
        .expect("add");
    let list = store.read().expect("read");
    assert_eq!(list.tasks.len(), 1);
    assert!(list.updated_at.is_some(), "write stamps updated_at");

    let err = store
        .add_task(Task::new("task-001", "dup"))
        .await
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

#[tokio::test]
async fn claim_next_respects_dependencies() {
    let (_tmp, store) = store();

    // task-002 depends on task-001. FR-039 validates `depends_on` at write
    // time, so the prerequisite is created first - the guard under test here is
    // the *claim* one (an unsatisfied dependency blocks a claim), not the
    // write-time one.
    store
        .add_task(Task::new("task-001", "prereq"))
        .await
        .expect("add prereq");
    let mut dep = Task::new("task-002", "dependent");
    dep.depends_on = vec!["task-001".to_string()];
    store.add_task(dep).await.expect("add dep");

    // The prereq is the only claimable task first.
    let (first, already_had) = store.claim_next("tm-001").await.expect("claim");
    assert!(!already_had);
    assert_eq!(first.expect("task").id, "task-001");

    // task-002 is blocked until task-001 completes.
    let (blocked, _) = store.claim_next("tm-002").await.expect("claim again");
    assert!(blocked.is_none(), "dependency blocks task-002");

    store
        .complete("task-001", "tm-001")
        .await
        .expect("complete prereq");
    let (now, _) = store
        .claim_next("tm-002")
        .await
        .expect("claim after completion");
    assert_eq!(now.expect("claimable").id, "task-002");
}

#[tokio::test]
async fn claim_next_returns_the_agents_existing_in_progress_task() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "only"))
        .await
        .expect("add");

    let (first, already_had) = store.claim_next("tm-001").await.expect("claim");
    assert!(!already_had);
    assert_eq!(first.expect("task").id, "task-001");

    let (again, already_had) = store.claim_next("tm-001").await.expect("claim again");
    assert!(already_had, "second claim reports already owned");
    assert_eq!(again.expect("task").id, "task-001");
}

#[tokio::test]
async fn claim_next_with_nothing_available_returns_none() {
    let (_tmp, store) = store();
    let (task, already_had) = store.claim_next("tm-001").await.expect("claim");
    assert!(task.is_none());
    assert!(!already_had);
}

#[tokio::test]
async fn claim_specific_guards_and_success() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");
    store
        .add_task(Task::new("task-002", "b"))
        .await
        .expect("add");

    // Unknown task.
    assert!(store.claim_specific("task-999", "tm-001").await.is_err());

    // Claim task-001, then a different in-progress task for the same agent is
    // rejected while task-001 is still in progress.
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim 1");
    let err = store
        .claim_specific("task-002", "tm-001")
        .await
        .expect_err("must complete first");
    let text = err.to_string();
    assert!(text.contains("cannot claim 'task-002'"), "err: {text}");
    assert!(
        text.contains("is working on task 'task-001'"),
        "err: {text}"
    );
    assert!(text.contains("team_task_complete"), "err: {text}");

    // Idempotent re-claim of the same task.
    let again = store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("re-claim");
    assert_eq!(again.id, "task-001");

    // Another agent cannot claim a task already assigned to tm-002.
    store
        .complete("task-001", "tm-001")
        .await
        .expect("complete");
    // A pending task pre-assigned to tm-003 (who has nothing in flight) is
    // refused to tm-002, naming the owner.
    let mut owned = Task::new("task-003", "c");
    owned.assigned_to = Some("tm-003".to_string());
    store.add_task(owned).await.expect("add owned");
    let err = store
        .claim_specific("task-003", "tm-002")
        .await
        .expect_err("assigned to another agent");
    assert!(
        err.to_string()
            .contains("is in status pending and pre-assigned to tm-003"),
        "err: {err}"
    );
}

#[tokio::test]
async fn claim_specific_rejects_non_pending_and_unsatisfied() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "done"))
        .await
        .expect("add");
    // Claim and complete as the same agent so the assigned_to guard is skipped
    // and the *status* guard is the one exercised.
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");
    store
        .complete("task-001", "tm-001")
        .await
        .expect("complete");

    let err = store
        .claim_specific("task-001", "tm-001")
        .await
        .expect_err("completed task not claimable");
    assert!(err.to_string().contains("cannot be claimed"), "err: {err}");

    // A pending task with an unsatisfied dependency cannot be claimed. FR-039
    // validates `depends_on` at write time, so the dependency has to name a real
    // task: it is created *in progress* so it is present and legally
    // unsatisfied, which is the state this assertion needs.
    let mut prereq = Task::new("task-000", "prereq held elsewhere");
    prereq.status = TaskStatus::InProgress;
    prereq.assigned_to = Some("tm-009".to_string());
    store.add_task(prereq).await.expect("add prereq");
    let mut dep = Task::new("task-002", "needs an open prereq");
    dep.depends_on = vec!["task-000".to_string()];
    store.add_task(dep).await.expect("add dep");
    let err = store
        .claim_specific("task-002", "tm-002")
        .await
        .expect_err("unsatisfied dependency");
    let text = err.to_string();
    assert!(text.contains("its dependencies are unmet"), "err: {text}");
    assert!(text.contains("depends on [task-000]"), "err: {text}");
}

#[tokio::test]
async fn complete_is_idempotent_per_agent_and_rejects_other_agent() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");

    // Completing is not claiming: an unclaimed task cannot be completed, so the
    // owner claims it first (FR-001, FR-003).
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");
    let (done, _hook) = store
        .complete("task-001", "tm-001")
        .await
        .expect("complete");
    assert_eq!(done.status, TaskStatus::Completed);
    assert_eq!(done.completed_by.as_deref(), Some("tm-001"));

    let (again, _hook) = store
        .complete("task-001", "tm-001")
        .await
        .expect("re-complete");
    assert_eq!(again.id, "task-001");

    let err = store
        .complete("task-001", "tm-002")
        .await
        .expect_err("another agent cannot re-complete");
    assert!(
        err.to_string()
            .contains("cannot be completed by tm-002: it is assigned to tm-001"),
        "the ownership refusal is the stronger claim and fires first: {err}"
    );
}

/// FR-001/FR-002: an agent that owns nothing gets a refusal naming the owner,
/// and the task is left exactly as it was - no reassignment, no completion.
#[tokio::test]
async fn complete_rejects_an_unowned_task_and_leaves_it_untouched() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");
    let before = store.read().expect("read");

    let err = store
        .complete("task-001", "tm-002")
        .await
        .expect_err("tm-002 owns nothing");
    let msg = err.to_string();
    assert!(
        msg.contains("cannot be completed by tm-002: it is assigned to tm-001"),
        "{msg}"
    );

    let after = store.read().expect("read");
    assert_eq!(
        serde_json::to_string(&after.tasks).expect("serialise after"),
        serde_json::to_string(&before.tasks).expect("serialise before"),
        "a refused completion mutates nothing"
    );
}

/// The auto-claim branch is gone: completing an unassigned `Pending` task is
/// refused instead of silently assigning it to the caller.
#[tokio::test]
async fn complete_refuses_to_auto_claim_an_unowned_pending_task() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");

    let err = store
        .complete("task-001", "tm-001")
        .await
        .expect_err("unowned pending task");
    let msg = err.to_string();
    assert!(
        msg.contains("it is unassigned, and completing is not claiming"),
        "{msg}"
    );
    assert!(
        msg.contains("unassigned"),
        "must name the absent owner: {msg}"
    );

    let after = store.read().expect("read");
    let task = after
        .tasks
        .iter()
        .find(|t| t.id == "task-001")
        .expect("task");
    assert_eq!(task.status, TaskStatus::Pending, "no implicit claim");
    assert!(task.assigned_to.is_none(), "no implicit reassignment");
    assert!(task.claimed_at.is_none(), "no claimed_at stamp");
}

/// The lead is exempt from the ownership rule (FR-001) but is still held to the
/// legal-transition model, so it must claim or assign before completing.
#[tokio::test]
async fn complete_accepts_the_lead_and_still_requires_a_legal_transition() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");

    let (done, _hook) = store
        .complete("task-001", "lead")
        .await
        .expect("lead completes on the owner's behalf");
    assert_eq!(done.status, TaskStatus::Completed);
    assert_eq!(done.completed_by.as_deref(), Some("lead"));

    // A second task, unclaimed: the lead's own completion still has no
    // Pending -> Completed edge.
    store
        .add_task(Task::new("task-002", "b"))
        .await
        .expect("add");
    let err = store
        .complete("task-002", "lead")
        .await
        .expect_err("pending task has no completion edge");
    assert!(
        err.to_string()
            .contains("is in status pending and has no owner"),
        "{err}"
    );
}

#[tokio::test]
async fn complete_missing_task_lists_available() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "present"))
        .await
        .expect("add");
    let err = store
        .complete("task-404", "tm-001")
        .await
        .expect_err("not found");
    let msg = err.to_string();
    assert!(msg.contains("task 'task-404' not found"), "err: {msg}");
    assert!(
        msg.contains("task-001 (present)"),
        "err should list available: {msg}"
    );
}

#[tokio::test]
async fn pre_assign_guards_and_success() {
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

    // Unknown task.
    assert!(store.pre_assign_task("task-999", "tm-002").is_err());

    // Not claimable (already in progress). Spec `teamssetup` FR-025 makes
    // pre-assignment share the one claim implementation, so the refusal names
    // the status the task actually holds rather than a pre-assign-specific
    // wording.
    let err = store
        .pre_assign_task("task-001", "tm-002")
        .expect_err("not claimable");
    assert!(
        err.to_string().contains("in status in_progress"),
        "err: {err}"
    );

    // A pending task with an unsatisfied dependency is not claimable either, so
    // it is refused rather than pinned to an agent that cannot work it. FR-039
    // requires the dependency to name a real task, so the prerequisite is
    // created *in progress* - present, open, and therefore unsatisfied.
    let mut prereq = Task::new("task-000", "prereq held elsewhere");
    prereq.status = TaskStatus::InProgress;
    prereq.assigned_to = Some("tm-009".to_string());
    store.add_task(prereq).await.expect("add prereq");
    let mut dep = Task::new("task-002", "needs an open prereq");
    dep.depends_on = vec!["task-000".to_string()];
    store.add_task(dep).await.expect("add dep");
    let err = store
        .pre_assign_task("task-002", "tm-002")
        .expect_err("unsatisfied dependency");
    let text = err.to_string();
    assert!(text.contains("its dependencies are unmet"), "err: {text}");
    assert!(text.contains("depends on [task-000]"), "err: {text}");
}

#[tokio::test]
async fn update_and_remove_task() {
    let (_tmp, store) = store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");

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
    // `add_task`/`claim_next`/`claim_specific`/`complete` are async on the store
    // themselves now (their hooks run on the mutation path), so they carry no
    // `_blocking` twin; `read_blocking` and `update_task_blocking` remain.
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");

    let list = store.read_blocking().await.expect("read blocking");
    assert_eq!(list.tasks.len(), 1);

    let (claimed, _) = store.claim_next("tm-001").await.expect("claim");
    assert_eq!(claimed.expect("task").id, "task-001");

    let (completed, _hook) = store
        .complete("task-001", "tm-001")
        .await
        .expect("complete");
    assert_eq!(completed.status, TaskStatus::Completed);

    let updated = store
        .update_task_blocking("task-001".to_string(), |t| {
            t.status = TaskStatus::Cancelled;
        })
        .await
        .expect("update blocking");
    assert_eq!(updated.status, TaskStatus::Cancelled);
}
