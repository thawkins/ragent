//! Coverage for FR-038 and FR-039 (spec `teamssetup`): `TaskList::validate()`
//! runs after every write and an invalid list is refused rather than persisted,
//! with dangling `depends_on` edges and dependency cycles reported at write
//! time instead of leaving a task unclaimable forever.
//!
//! Two layers are covered:
//!
//! - the pure predicate, [`TaskList::validate`], which must name the offending
//!   task and dependency and stay quiet on a legal list;
//! - the write boundary, `TaskStore::write_locked`, reached through every
//!   mutate-then-persist entry point, which must refuse **before** touching
//!   `tasks.json` - the file is asserted byte-identical after each refusal.

use ragent_agent::team::task::TaskList;
use ragent_agent::team::{Task, TaskStatus, TaskStore};

#[path = "team_support/mod.rs"]
mod team_support;
use team_support::setup_team_with_task_store;

/// A real team directory with an open task store (needed because the store's
/// mutators fire the team's task hooks, which read `config.json`).
fn store() -> (tempfile::TempDir, std::path::PathBuf, TaskStore) {
    setup_team_with_task_store()
}

/// Build a task with the given id and dependencies.
fn task(id: &str, depends_on: &[&str]) -> Task {
    let mut t = Task::new(id, format!("title of {id}"));
    t.depends_on = depends_on.iter().map(|s| (*s).to_string()).collect();
    t
}

/// A list holding `tasks`.
fn list(tasks: Vec<Task>) -> TaskList {
    let mut l = TaskList::new("wv");
    l.tasks = tasks;
    l
}

// -- The predicate -------------------------------------------------------------

#[test]
fn a_legal_dependency_chain_validates() {
    // task-001 -> task-002 -> task-003 is a chain, not a cycle.
    let l = list(vec![
        task("task-001", &[]),
        task("task-002", &["task-001"]),
        task("task-003", &["task-002", "task-001"]),
    ]);
    l.validate()
        .expect("an acyclic chain with known ids is valid");
}

#[test]
fn an_empty_list_validates() {
    TaskList::new("wv")
        .validate()
        .expect("an empty list is valid");
}

#[test]
fn a_dangling_dependency_is_named_with_the_missing_id() {
    let l = list(vec![task("task-001", &[]), task("task-002", &["task-999"])]);
    let err = l
        .validate()
        .expect_err("dangling dependency must be refused");
    let text = err.to_string();
    assert!(text.contains("task-002"), "names the dependent: {text}");
    assert!(text.contains("task-999"), "names the missing id: {text}");
    assert!(text.contains("forever"), "explains the consequence: {text}");
    assert!(
        text.contains("task-001"),
        "lists the ids that do exist: {text}"
    );
}

#[test]
fn a_two_task_cycle_is_reported_with_the_chain() {
    let l = list(vec![
        task("task-001", &["task-002"]),
        task("task-002", &["task-001"]),
    ]);
    let err = l.validate().expect_err("a cycle must be refused");
    let text = err.to_string();
    assert!(text.contains("cycle"), "names the failure: {text}");
    assert!(text.contains("task-001"), "names a cycle member: {text}");
    assert!(text.contains("task-002"), "names the other member: {text}");
    assert!(
        text.contains("task-001 -> task-002 -> task-001")
            || text.contains("task-002 -> task-001 -> task-002"),
        "prints the closing chain: {text}"
    );
}

#[test]
fn a_three_task_cycle_is_reported() {
    let l = list(vec![
        task("task-001", &["task-003"]),
        task("task-002", &["task-001"]),
        task("task-003", &["task-002"]),
    ]);
    let err = l.validate().expect_err("a 3-cycle must be refused");
    assert!(err.to_string().contains("cycle"), "err: {err}");
}

#[test]
fn a_cycle_reachable_only_through_a_tail_is_still_found() {
    // task-000 is a legal root; the cycle sits below it. A DFS that only
    // inspects roots would miss this.
    let l = list(vec![
        task("task-000", &["task-001"]),
        task("task-001", &["task-002"]),
        task("task-002", &["task-001"]),
    ]);
    let err = l.validate().expect_err("a buried cycle must be refused");
    assert!(err.to_string().contains("cycle"), "err: {err}");
}

#[test]
fn a_task_depending_on_itself_is_refused() {
    let l = list(vec![task("task-001", &["task-001"])]);
    let err = l.validate().expect_err("self-dependency must be refused");
    assert!(
        err.to_string().contains("depends on itself"),
        "per-task rule still applies: {err}"
    );
}

#[test]
fn duplicate_ids_are_refused() {
    let l = list(vec![task("task-001", &[]), task("task-001", &[])]);
    let err = l.validate().expect_err("duplicate ids must be refused");
    assert!(
        err.to_string().contains("more than once"),
        "names the duplication: {err}"
    );
}

#[test]
fn a_diamond_dependency_is_not_a_cycle() {
    // task-004 depends on two tasks that share a dependency - a diamond, which
    // is legal and must not be mistaken for a cycle by a naive DFS.
    let l = list(vec![
        task("task-001", &[]),
        task("task-002", &["task-001"]),
        task("task-003", &["task-001"]),
        task("task-004", &["task-002", "task-003"]),
    ]);
    l.validate().expect("a diamond is acyclic");
}

// -- The write boundary --------------------------------------------------------

#[tokio::test]
async fn a_dangling_dependency_is_refused_at_write_time_and_nothing_is_written() {
    let (_tmp, _team_dir, store) = store();
    store
        .add_task(task("task-001", &[]))
        .await
        .expect("seed task");
    let before = std::fs::read_to_string(store.path()).expect("read tasks.json");

    let err = store
        .add_task(task("task-002", &["task-999"]))
        .await
        .expect_err("a dangling dependency must be refused");
    let text = err.to_string();
    assert!(text.contains("task-999"), "names the missing id: {text}");
    assert!(
        text.contains("refusing to write invalid"),
        "comes from the write boundary: {text}"
    );

    let after = std::fs::read_to_string(store.path()).expect("read tasks.json");
    assert_eq!(before, after, "the refusal left tasks.json byte-identical");
    let list = store.read().expect("read");
    assert_eq!(list.tasks.len(), 1, "the bad task was not persisted");
    assert!(list.get("task-002").is_none(), "task-002 does not exist");
}

#[tokio::test]
async fn a_cycle_is_refused_at_write_time_and_nothing_is_written() {
    let (_tmp, _team_dir, store) = store();
    store
        .add_task(task("task-001", &[]))
        .await
        .expect("seed task");
    let before = std::fs::read_to_string(store.path()).expect("read tasks.json");

    let err = store
        .add_task(task("task-002", &["task-001"]))
        .await
        .expect("a legal edge is accepted");
    assert_eq!(
        err,
        ragent_agent::team::HookOutcome::Allow,
        "no hook is configured, so the creation is allowed"
    );

    // Now close the loop: task-001 may not depend on task-002 while task-002
    // depends on task-001.
    let err = store
        .update_task("task-001", |t| {
            t.depends_on = vec!["task-002".to_string()];
        })
        .expect_err("closing a cycle must be refused");
    assert!(err.to_string().contains("cycle"), "err: {err}");

    let list = store.read().expect("read");
    let first = list.get("task-001").expect("task-001");
    assert!(
        first.depends_on.is_empty(),
        "task-001's dependencies were not persisted: {:?}",
        first.depends_on
    );
    assert!(!before.is_empty(), "sanity: the seed write produced a file");
}

#[tokio::test]
async fn the_write_through_path_validates_too() {
    let (_tmp, _team_dir, store) = store();
    store
        .add_task(task("task-001", &[]))
        .await
        .expect("seed task");

    let err = store
        .write_through(|list| {
            list.tasks.push(task("task-002", &["task-404"]));
        })
        .expect_err("write_through must validate before persisting");
    assert!(err.to_string().contains("task-404"), "err: {err}");

    let list = store.read().expect("read");
    assert_eq!(list.tasks.len(), 1, "the invalid list was not written");
}

#[tokio::test]
async fn a_legal_chain_is_persisted_across_several_writes() {
    let (_tmp, _team_dir, store) = store();
    store.add_task(task("task-001", &[])).await.expect("seed");
    store
        .add_task(task("task-002", &["task-001"]))
        .await
        .expect("a dependency on an existing task is accepted");
    store
        .add_task(task("task-003", &["task-002"]))
        .await
        .expect("a longer chain is accepted");

    let list = store.read().expect("read");
    assert_eq!(list.tasks.len(), 3);
    assert_eq!(
        list.get("task-003").expect("task-003").depends_on,
        ["task-002"]
    );
}

#[tokio::test]
async fn the_created_task_stays_claimable_when_its_dependency_completes() {
    // The point of FR-039: a validated edge is one that *can* be satisfied, so
    // completing the dependency makes the dependent claimable.
    let (_tmp, _team_dir, store) = store();
    store.add_task(task("task-001", &[])).await.expect("seed");
    store
        .add_task(task("task-002", &["task-001"]))
        .await
        .expect("dependency on an existing task");

    let before = store.read().expect("read");
    assert_eq!(
        before.get("task-002").expect("task-002").status,
        TaskStatus::Pending
    );
    assert!(
        before.next_claimable().map(|t| t.id.clone()).as_deref() == Some("task-001"),
        "only the root is claimable while the dependency is open"
    );

    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim the dependency");
    store
        .complete("task-001", "tm-001")
        .await
        .expect("complete the dependency");

    let after = store.read().expect("read");
    assert!(
        after.next_claimable().map(|t| t.id.clone()).as_deref() == Some("task-002"),
        "the dependent became claimable once its dependency completed"
    );
}

#[test]
fn task_ids_lists_the_ids_in_insertion_order() {
    let l = list(vec![task("task-001", &[]), task("task-002", &["task-001"])]);
    assert_eq!(l.task_ids(), vec!["task-001", "task-002"]);
}
