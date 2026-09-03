#![allow(clippy::assert_is_empty)]
//! todo2tasks T-004: tests for the dependency DAG computation
//! (`compute_task_dag`, `get_task_view`, `list_task_views`).
//!
//! These tests verify the derived DAG fields:
//! - `blocks` (inverse edges): if B lists A in `blocked_by`, then A
//!   blocks B
//! - `is_blocked` (FR-005): status is "pending" and at least one
//!   `blocked_by` ID is not "completed"
//! - `is_available`: status is "pending", owner is empty, and all
//!   `blocked_by` IDs are "completed" (or `blocked_by` is empty)
//!
//! Tests cover both the pure `compute_task_dag` function (no DB needed)
//! and the `Storage` convenience methods (`get_task_view`,
//! `list_task_views`).

use ragent_storage::storage::{Storage, TaskRow, compute_task_dag};

// ── Helper: build a TaskRow with sensible defaults ─────────────────

fn make_task(id: &str, status: &str, blocked_by: &[&str]) -> TaskRow {
    TaskRow {
        id: id.to_string(),
        session_id: "sess".to_string(),
        title: format!("Task {id}"),
        status: status.to_string(),
        description: String::new(),
        created_at: "2025-01-01T00:00:00Z".to_string(),
        updated_at: "2025-01-01T00:00:00Z".to_string(),
        active_form: None,
        owner: None,
        metadata: "{}".to_string(),
        blocked_by: blocked_by.iter().map(|s| s.to_string()).collect(),
    }
}

fn make_task_with_owner(id: &str, status: &str, blocked_by: &[&str], owner: &str) -> TaskRow {
    let mut t = make_task(id, status, blocked_by);
    t.owner = Some(owner.to_string());
    t
}

// ── compute_task_dag: blocks (inverse edges) ───────────────────────

/// Verify that `blocks` is correctly derived as the inverse of
/// `blocked_by`.
#[test]
fn test_dag_blocks_inverse_edges() {
    // A has no deps. B is blocked_by A. C is blocked_by A and B.
    let tasks = vec![
        make_task("a", "pending", &[]),
        make_task("b", "pending", &["a"]),
        make_task("c", "pending", &["a", "b"]),
    ];
    let dag = compute_task_dag(&tasks);

    // A blocks B and C.
    let a = dag.get("a").unwrap();
    assert_eq!(a.blocks, vec!["b", "c"]);

    // B blocks C.
    let b = dag.get("b").unwrap();
    assert_eq!(b.blocks, vec!["c"]);

    // C blocks nothing.
    let c = dag.get("c").unwrap();
    assert!(c.blocks.is_empty());
}

/// Verify that `blocks` is sorted for deterministic output.
#[test]
fn test_dag_blocks_sorted() {
    let tasks = vec![
        make_task("x", "pending", &[]),
        make_task("d", "pending", &["x"]),
        make_task("a", "pending", &["x"]),
        make_task("c", "pending", &["x"]),
        make_task("b", "pending", &["x"]),
    ];
    let dag = compute_task_dag(&tasks);
    let x = dag.get("x").unwrap();
    assert_eq!(x.blocks, vec!["a", "b", "c", "d"]);
}

/// Verify that `blocks` does not contain duplicates even if multiple
/// tasks list the same dep (shouldn't happen, but be safe).
#[test]
fn test_dag_blocks_no_duplicates() {
    let tasks = vec![
        make_task("a", "pending", &[]),
        make_task("b", "pending", &["a"]),
        make_task("c", "pending", &["a"]),
    ];
    let dag = compute_task_dag(&tasks);
    let a = dag.get("a").unwrap();
    assert_eq!(a.blocks, vec!["b", "c"]);
    assert_eq!(a.blocks.len(), 2);
}

// ── compute_task_dag: is_blocked (FR-005) ──────────────────────────

/// A pending task with no deps is NOT blocked.
#[test]
fn test_dag_pending_no_deps_not_blocked() {
    let tasks = vec![make_task("a", "pending", &[])];
    let dag = compute_task_dag(&tasks);
    let a = dag.get("a").unwrap();
    assert!(!a.is_blocked);
}

/// A pending task whose only blocker is also pending IS blocked.
#[test]
fn test_dag_pending_blocker_pending_is_blocked() {
    let tasks = vec![
        make_task("a", "pending", &[]),
        make_task("b", "pending", &["a"]),
    ];
    let dag = compute_task_dag(&tasks);
    let b = dag.get("b").unwrap();
    assert!(b.is_blocked);
    let a = dag.get("a").unwrap();
    assert!(!a.is_blocked);
}

/// A pending task whose only blocker is in_progress IS blocked.
#[test]
fn test_dag_pending_blocker_in_progress_is_blocked() {
    let tasks = vec![
        make_task("a", "in_progress", &[]),
        make_task("b", "pending", &["a"]),
    ];
    let dag = compute_task_dag(&tasks);
    let b = dag.get("b").unwrap();
    assert!(b.is_blocked);
}

/// A pending task whose only blocker is completed is NOT blocked.
#[test]
fn test_dag_pending_blocker_completed_not_blocked() {
    let tasks = vec![
        make_task("a", "completed", &[]),
        make_task("b", "pending", &["a"]),
    ];
    let dag = compute_task_dag(&tasks);
    let b = dag.get("b").unwrap();
    assert!(!b.is_blocked);
}

/// A pending task with multiple blockers, some completed, some not, IS
/// blocked.
#[test]
fn test_dag_partial_completion_still_blocked() {
    let tasks = vec![
        make_task("a", "completed", &[]),
        make_task("b", "pending", &[]),
        make_task("c", "pending", &["a", "b"]),
    ];
    let dag = compute_task_dag(&tasks);
    let c = dag.get("c").unwrap();
    assert!(c.is_blocked);
}

/// A pending task with all blockers completed is NOT blocked.
#[test]
fn test_dag_all_blockers_completed_not_blocked() {
    let tasks = vec![
        make_task("a", "completed", &[]),
        make_task("b", "completed", &[]),
        make_task("c", "pending", &["a", "b"]),
    ];
    let dag = compute_task_dag(&tasks);
    let c = dag.get("c").unwrap();
    assert!(!c.is_blocked);
}

/// An in_progress task is never blocked regardless of deps.
#[test]
fn test_dag_in_progress_never_blocked() {
    let tasks = vec![
        make_task("a", "pending", &[]),
        make_task("b", "in_progress", &["a"]),
    ];
    let dag = compute_task_dag(&tasks);
    let b = dag.get("b").unwrap();
    assert!(!b.is_blocked);
}

/// A completed task is never blocked regardless of deps.
#[test]
fn test_dag_completed_never_blocked() {
    let tasks = vec![
        make_task("a", "pending", &[]),
        make_task("b", "completed", &["a"]),
    ];
    let dag = compute_task_dag(&tasks);
    let b = dag.get("b").unwrap();
    assert!(!b.is_blocked);
}

// ── compute_task_dag: is_available ─────────────────────────────────

/// A pending task with no deps and no owner is available.
#[test]
fn test_dag_available_no_deps_no_owner() {
    let tasks = vec![make_task("a", "pending", &[])];
    let dag = compute_task_dag(&tasks);
    let a = dag.get("a").unwrap();
    assert!(a.is_available);
}

/// A pending task with no deps but with an owner is NOT available.
#[test]
fn test_dag_available_no_deps_with_owner() {
    let tasks = vec![make_task_with_owner("a", "pending", &[], "coder")];
    let dag = compute_task_dag(&tasks);
    let a = dag.get("a").unwrap();
    assert!(!a.is_available);
}

/// A pending task with all blockers completed and no owner is available.
#[test]
fn test_dag_available_all_blockers_done() {
    let tasks = vec![
        make_task("a", "completed", &[]),
        make_task("b", "completed", &[]),
        make_task("c", "pending", &["a", "b"]),
    ];
    let dag = compute_task_dag(&tasks);
    let c = dag.get("c").unwrap();
    assert!(c.is_available);
    assert!(!c.is_blocked);
}

/// A pending task with all blockers completed but with an owner is NOT
/// available.
#[test]
fn test_dag_not_available_all_blockers_done_with_owner() {
    let tasks = vec![
        make_task("a", "completed", &[]),
        make_task_with_owner("b", "pending", &["a"], "agent"),
    ];
    let dag = compute_task_dag(&tasks);
    let b = dag.get("b").unwrap();
    assert!(!b.is_available);
}

/// A pending task with some blockers not completed is NOT available
/// (and IS blocked).
#[test]
fn test_dag_not_available_some_blockers_pending() {
    let tasks = vec![
        make_task("a", "completed", &[]),
        make_task("b", "pending", &[]),
        make_task("c", "pending", &["a", "b"]),
    ];
    let dag = compute_task_dag(&tasks);
    let c = dag.get("c").unwrap();
    assert!(!c.is_available);
    assert!(c.is_blocked);
}

/// An in_progress task is never available.
#[test]
fn test_dag_in_progress_not_available() {
    let tasks = vec![make_task("a", "in_progress", &[])];
    let dag = compute_task_dag(&tasks);
    let a = dag.get("a").unwrap();
    assert!(!a.is_available);
}

/// A completed task is never available.
#[test]
fn test_dag_completed_not_available() {
    let tasks = vec![make_task("a", "completed", &[])];
    let dag = compute_task_dag(&tasks);
    let a = dag.get("a").unwrap();
    assert!(!a.is_available);
}

// ── compute_task_dag: edge cases ───────────────────────────────────

/// A blocked_by reference to a non-existent task ID is treated as
/// "not completed" — the task stays blocked.
#[test]
fn test_dag_dangling_blocked_by_reference_treated_as_blocked() {
    let tasks = vec![make_task("a", "pending", &["nonexistent"])];
    let dag = compute_task_dag(&tasks);
    let a = dag.get("a").unwrap();
    assert!(a.is_blocked);
    assert!(!a.is_available);
}

/// Empty task list produces empty DAG.
#[test]
fn test_dag_empty_task_list() {
    let tasks: Vec<TaskRow> = vec![];
    let dag = compute_task_dag(&tasks);
    assert!(dag.is_empty());
}

/// Single task with no deps: not blocked, available, blocks nothing.
#[test]
fn test_dag_single_task_no_deps() {
    let tasks = vec![make_task("solo", "pending", &[])];
    let dag = compute_task_dag(&tasks);
    let solo = dag.get("solo").unwrap();
    assert!(solo.blocks.is_empty());
    assert!(!solo.is_blocked);
    assert!(solo.is_available);
}

/// Chain A→B→C (C blocked_by B, B blocked_by A):
/// - A is available, not blocked, blocks B
/// - B is blocked, not available, blocks C
/// - C is blocked, not available, blocks nothing
#[test]
fn test_dag_chain_a_b_c() {
    let tasks = vec![
        make_task("a", "pending", &[]),
        make_task("b", "pending", &["a"]),
        make_task("c", "pending", &["b"]),
    ];
    let dag = compute_task_dag(&tasks);

    let a = dag.get("a").unwrap();
    assert!(!a.is_blocked);
    assert!(a.is_available);
    assert_eq!(a.blocks, vec!["b"]);

    let b = dag.get("b").unwrap();
    assert!(b.is_blocked);
    assert!(!b.is_available);
    assert_eq!(b.blocks, vec!["c"]);

    let c = dag.get("c").unwrap();
    assert!(c.is_blocked);
    assert!(!c.is_available);
    assert!(c.blocks.is_empty());
}

/// Diamond dependency: A→B, A→C, B→D, C→D.
/// When A is completed, B and C become available.
/// When B and C are also completed, D becomes available.
#[test]
fn test_dag_diamond_dependency() {
    let tasks = vec![
        make_task("a", "pending", &[]),
        make_task("b", "pending", &["a"]),
        make_task("c", "pending", &["a"]),
        make_task("d", "pending", &["b", "c"]),
    ];
    let dag = compute_task_dag(&tasks);

    // Initially only A is available.
    assert!(dag.get("a").unwrap().is_available);
    assert!(!dag.get("b").unwrap().is_available);
    assert!(!dag.get("c").unwrap().is_available);
    assert!(!dag.get("d").unwrap().is_available);

    // A blocks B and C.
    assert_eq!(dag.get("a").unwrap().blocks, vec!["b", "c"]);
    // B and C both block D.
    assert_eq!(dag.get("b").unwrap().blocks, vec!["d"]);
    assert_eq!(dag.get("c").unwrap().blocks, vec!["d"]);
}

// ── Storage::get_task_view ─────────────────────────────────────────

/// Verify `get_task_view` returns the task with derived DAG fields.
#[test]
fn test_get_task_view_with_derived_fields() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-gv", "/tmp/get-view")
        .expect("create session");

    storage
        .create_task(
            "a",
            "sess-gv",
            "Task A",
            "desc",
            "completed",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create a");
    storage
        .create_task(
            "b",
            "sess-gv",
            "Task B",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &["a".to_string()],
        )
        .expect("create b");

    let view = storage
        .get_task_view("b", "sess-gv")
        .expect("get view")
        .expect("task b exists");

    assert_eq!(view.task.id, "b");
    assert_eq!(view.task.blocked_by, vec!["a"]);
    // B's blocker A is completed, so B is not blocked and is available.
    assert!(!view.derived.is_blocked);
    assert!(view.derived.is_available);
    // B blocks nothing.
    assert!(view.derived.blocks.is_empty());

    // Check A's view — A should block B.
    let view_a = storage
        .get_task_view("a", "sess-gv")
        .expect("get view")
        .expect("task a exists");
    assert_eq!(view_a.derived.blocks, vec!["b"]);
    // A is completed, not blocked, not available.
    assert!(!view_a.derived.is_blocked);
    assert!(!view_a.derived.is_available);
}

/// Verify `get_task_view` returns None for non-existent task.
#[test]
fn test_get_task_view_nonexistent() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-ne3", "/tmp/nonexistent3")
        .expect("create session");

    let view = storage
        .get_task_view("no-such", "sess-ne3")
        .expect("get view");
    assert!(view.is_none());
}

// ── Storage::list_task_views ───────────────────────────────────────

/// Verify `list_task_views` returns all tasks with derived fields.
#[test]
fn test_list_task_views_all_with_derived() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-lv", "/tmp/list-views")
        .expect("create session");

    storage
        .create_task(
            "a",
            "sess-lv",
            "A",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create a");
    storage
        .create_task(
            "b",
            "sess-lv",
            "B",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &["a".to_string()],
        )
        .expect("create b");

    let views = storage
        .list_task_views("sess-lv", None)
        .expect("list views");
    assert_eq!(views.len(), 2);

    // Ordered by created_at: a first, then b.
    assert_eq!(views[0].task.id, "a");
    assert!(views[0].derived.is_available);
    assert!(!views[0].derived.is_blocked);
    assert_eq!(views[0].derived.blocks, vec!["b"]);

    assert_eq!(views[1].task.id, "b");
    assert!(!views[1].derived.is_available);
    assert!(views[1].derived.is_blocked);
}

/// Verify `list_task_views` with a status filter still computes DAG
/// from the full task set.
#[test]
fn test_list_task_views_status_filter_dag_from_full_set() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-filt", "/tmp/filter-dag")
        .expect("create session");

    // A is completed, B is pending and blocked_by A.
    storage
        .create_task(
            "a",
            "sess-filt",
            "A",
            "desc",
            "completed",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create a");
    storage
        .create_task(
            "b",
            "sess-filt",
            "B",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &["a".to_string()],
        )
        .expect("create b");

    // Filter to only pending — B should still show correct DAG (A's
    // completion status is known even though A is filtered out).
    let pending = storage
        .list_task_views("sess-filt", Some("pending"))
        .expect("list pending");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].task.id, "b");
    // B's blocker A is completed, so B is not blocked.
    assert!(!pending[0].derived.is_blocked);
    assert!(pending[0].derived.is_available);
    // B's blocks should be empty (no task lists B in blocked_by).
    assert!(pending[0].derived.blocks.is_empty());
}

/// Verify `list_task_views` with "all" filter returns everything.
#[test]
fn test_list_task_views_all_filter() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-all2", "/tmp/all2")
        .expect("create session");

    storage
        .create_task(
            "a",
            "sess-all2",
            "A",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create a");
    storage
        .create_task(
            "b",
            "sess-all2",
            "B",
            "desc",
            "completed",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create b");

    let all = storage
        .list_task_views("sess-all2", Some("all"))
        .expect("list all");
    assert_eq!(all.len(), 2);
}

/// Verify `list_task_views` on empty session returns empty vec.
#[test]
fn test_list_task_views_empty_session() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-empty", "/tmp/empty")
        .expect("create session");

    let views = storage
        .list_task_views("sess-empty", None)
        .expect("list views");
    assert!(views.is_empty());
}

// ── Integration: create → update → re-derive ───────────────────────

/// Verify that completing a blocker updates the derived state of
/// dependent tasks when re-read.
#[test]
fn test_dag_state_change_blocker_completed() {
    let storage = Storage::open_in_memory().expect("in-memory storage");
    storage
        .create_session("sess-sc", "/tmp/state-change")
        .expect("create session");

    storage
        .create_task(
            "a",
            "sess-sc",
            "A",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &[],
        )
        .expect("create a");
    storage
        .create_task(
            "b",
            "sess-sc",
            "B",
            "desc",
            "pending",
            None,
            None,
            "{}",
            &["a".to_string()],
        )
        .expect("create b");

    // Initially B is blocked by pending A.
    let views = storage.list_task_views("sess-sc", None).expect("list");
    let b = views.iter().find(|v| v.task.id == "b").unwrap();
    assert!(b.derived.is_blocked);
    assert!(!b.derived.is_available);

    // Complete A.
    use ragent_storage::storage::TaskUpdateParams;
    storage
        .update_task(
            "a",
            "sess-sc",
            &TaskUpdateParams {
                status: Some("completed"),
                ..Default::default()
            },
        )
        .expect("complete a");

    // Now B should be not blocked and available.
    let views = storage.list_task_views("sess-sc", None).expect("list");
    let b = views.iter().find(|v| v.task.id == "b").unwrap();
    assert!(!b.derived.is_blocked);
    assert!(b.derived.is_available);
}
