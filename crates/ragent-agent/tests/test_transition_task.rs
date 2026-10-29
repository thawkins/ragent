//! Shared `transition_task` helper tests (spec `teamssetup` FR-003, FR-004).
//!
//! `transition_task` is the single writer of `Task::status` and the only caller
//! of `TaskStatus::can_transition_to`. These tests pin the three things the
//! helper is responsible for:
//!
//! * FR-003 - it validates against the D3 table before mutating, so no caller
//!   can write an illegal pair;
//! * FR-004 - a refused transition returns an error naming the current status,
//!   the requested status, and the table row that forbids it, and leaves the
//!   task untouched;
//! * the idempotent re-assert path returns `Ok(None)` instead of writing, so the
//!   claim and complete paths need no "already there" special case.

use ragent_agent::team::{Task, TaskList, TaskStatus, TaskTransition, transition_task};

/// Every status in the enum, in declaration order.
const ALL_TASK_STATUSES: [TaskStatus; 4] = [
    TaskStatus::Pending,
    TaskStatus::InProgress,
    TaskStatus::Completed,
    TaskStatus::Cancelled,
];

/// The D3 table from `specs/teamssetup/PLAN.md`, transcribed row by row.
///
/// Identical to the table pinned by `test_task_status_transitions.rs`, repeated
/// here so this file proves the *helper* obeys it, not just the predicate.
const D3_TABLE: [(TaskStatus, TaskStatus, bool); 16] = [
    // Pending
    (TaskStatus::Pending, TaskStatus::Pending, true),
    (TaskStatus::Pending, TaskStatus::InProgress, true),
    (TaskStatus::Pending, TaskStatus::Completed, false),
    (TaskStatus::Pending, TaskStatus::Cancelled, true),
    // InProgress
    (TaskStatus::InProgress, TaskStatus::Pending, true),
    (TaskStatus::InProgress, TaskStatus::InProgress, true),
    (TaskStatus::InProgress, TaskStatus::Completed, true),
    (TaskStatus::InProgress, TaskStatus::Cancelled, true),
    // Completed is terminal
    (TaskStatus::Completed, TaskStatus::Pending, false),
    (TaskStatus::Completed, TaskStatus::InProgress, false),
    (TaskStatus::Completed, TaskStatus::Completed, true),
    (TaskStatus::Completed, TaskStatus::Cancelled, false),
    // Cancelled is terminal
    (TaskStatus::Cancelled, TaskStatus::Pending, false),
    (TaskStatus::Cancelled, TaskStatus::InProgress, false),
    (TaskStatus::Cancelled, TaskStatus::Completed, false),
    (TaskStatus::Cancelled, TaskStatus::Cancelled, true),
];

/// Build a one-task list whose single task holds `status`.
fn list_with(status: TaskStatus) -> TaskList {
    let mut list = TaskList::new("t");
    let mut task = Task::new("task-001", "Write the thing");
    task.status = status;
    list.tasks.push(task);
    list
}

#[test]
fn test_transition_task_applies_a_legal_transition() {
    let mut list = list_with(TaskStatus::Pending);
    let transition = transition_task(&mut list, "task-001", TaskStatus::InProgress, "tm-001")
        .expect("pending -> in_progress is legal");
    let transition = transition.expect("a real transition returns Some");
    assert_eq!(
        transition,
        TaskTransition {
            task_id: "task-001".to_owned(),
            from: TaskStatus::Pending,
            to: TaskStatus::InProgress,
            actor: "tm-001".to_owned(),
        }
    );
    assert_eq!(list.tasks[0].status, TaskStatus::InProgress);
}

#[test]
fn test_transition_task_records_the_actor_verbatim() {
    let mut list = list_with(TaskStatus::Pending);
    let transition = transition_task(&mut list, "task-001", TaskStatus::Cancelled, "lead")
        .expect("pending -> cancelled is legal")
        .expect("a real transition returns Some");
    assert_eq!(transition.actor, "lead");
}

#[test]
fn test_transition_task_refuses_an_illegal_transition_and_leaves_the_task_alone() {
    let mut list = list_with(TaskStatus::Pending);
    let err = transition_task(&mut list, "task-001", TaskStatus::Completed, "tm-001")
        .expect_err("pending -> completed skips in_progress and is illegal");
    let msg = err.to_string();
    assert!(
        msg.contains("task 'task-001'"),
        "refusal must name the task: {msg}"
    );
    assert!(
        msg.contains("from pending to completed"),
        "refusal must name both statuses: {msg}"
    );
    assert!(
        msg.contains("pending -> in_progress"),
        "refusal must name the table row that forbids it: {msg}"
    );
    assert_eq!(
        list.tasks[0].status,
        TaskStatus::Pending,
        "a refused transition must not mutate the task"
    );
}

#[test]
fn test_transition_task_refusal_names_the_terminal_row() {
    let mut list = list_with(TaskStatus::Completed);
    let err = transition_task(&mut list, "task-001", TaskStatus::InProgress, "tm-001")
        .expect_err("completed is terminal");
    let msg = err.to_string();
    assert!(msg.contains("from completed to in_progress"), "{msg}");
    assert!(
        msg.contains("completed is terminal"),
        "the completed row must say it is terminal: {msg}"
    );

    let mut list = list_with(TaskStatus::Cancelled);
    let err = transition_task(&mut list, "task-001", TaskStatus::Completed, "lead")
        .expect_err("cancelled is terminal");
    let msg = err.to_string();
    assert!(msg.contains("from cancelled to completed"), "{msg}");
    assert!(
        msg.contains("cancelled is terminal"),
        "the cancelled row must say it is terminal: {msg}"
    );
}

#[test]
fn test_transition_task_reasserting_the_current_status_is_a_no_op() {
    for status in ALL_TASK_STATUSES {
        let mut list = list_with(status);
        let result = transition_task(&mut list, "task-001", status, "tm-001")
            .expect("re-asserting the current status is never an error");
        assert!(
            result.is_none(),
            "re-asserting {status:?} must return None, never a transition"
        );
        assert_eq!(list.tasks[0].status, status);
    }
}

#[test]
fn test_transition_task_refuses_an_unknown_task_id() {
    let mut list = list_with(TaskStatus::Pending);
    let err = transition_task(&mut list, "task-999", TaskStatus::InProgress, "tm-001")
        .expect_err("an unknown task id is an error, not a no-op");
    let msg = err.to_string();
    assert!(msg.contains("task 'task-999' not found"), "{msg}");
    assert!(
        msg.contains("task-001"),
        "the refusal should name the tasks that do exist: {msg}"
    );
    assert_eq!(list.tasks[0].status, TaskStatus::Pending);
}

#[test]
fn test_transition_task_agrees_with_the_d3_table_on_every_pair() {
    for (from, to, allowed) in D3_TABLE {
        let mut list = list_with(from);
        let result = transition_task(&mut list, "task-001", to, "tm-001");
        if allowed {
            // Allowed pairs are either a real move (Some) or a re-assert
            // (Self -> Self, None); both are Ok.
            let applied = result.unwrap_or_else(|e| {
                panic!("D3 says {from:?} -> {to:?} is legal but the helper refused: {e}")
            });
            if from == to {
                assert!(applied.is_none(), "{from:?} -> {to:?} is a re-assert");
            } else {
                assert!(
                    applied.is_some(),
                    "{from:?} -> {to:?} is legal and must report a transition"
                );
            }
            assert_eq!(list.tasks[0].status, to);
        } else {
            let err = result.err().unwrap_or_else(|| {
                panic!("D3 says {from:?} -> {to:?} is illegal but the helper allowed it")
            });
            assert!(
                err.to_string()
                    .contains(&format!("from {} to {}", from.as_str(), to.as_str())),
                "refusal for {from:?} -> {to:?} must name both statuses: {err}"
            );
            assert_eq!(
                list.tasks[0].status, from,
                "a refused {from:?} -> {to:?} must not mutate the task"
            );
        }
    }
}

#[test]
fn test_transition_task_is_the_only_site_that_writes_task_status() {
    // FR-003's structural guarantee, checked by reading the source tree: every
    // `Task::status` write goes through `transition_task`, so the literal
    // `.status = TaskStatus::` assignment must not appear at all. T-008 routed
    // the two completion sites and T-009 routed the three claim sites
    // (`claim_next`, `claim_specific`, `pre_assign_task`, now one shared
    // `claim_task_in_list`); T-011 added `cancel_task_in_list` on the same
    // helper, so the ratchet stays at zero.
    let src = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/team/task.rs"))
        .expect("read task.rs");
    let direct_writes: Vec<usize> = src
        .lines()
        .enumerate()
        .filter(|(_, line)| {
            let line = line.trim_start();
            !line.starts_with("//") && line.contains(".status = TaskStatus::")
        })
        .map(|(i, _)| i + 1)
        .collect();
    assert_eq!(
        direct_writes,
        Vec::<usize>::new(),
        "task.rs must contain no direct `.status = TaskStatus::` assignment: route it through \
         transition_task instead (found at lines {direct_writes:?})."
    );
    // `transition_task` itself writes through its local `to` value, which is why
    // the helper's own assignment never matches the pattern guarded above.
    assert!(
        src.lines().any(|l| l.contains("task.status = to;")),
        "transition_task must remain the writer for validated transitions"
    );
}
