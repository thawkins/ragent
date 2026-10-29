//! Legal task-transition table tests (spec `teamssetup` FR-003, FR-004, D3).
//!
//! [`TaskStatus::can_transition_to`] is the single function that decides whether
//! a task transition is legal. These tests pin the whole D3 table - every
//! `(from, to)` pair of the four statuses - so a future enum variant or a
//! loosened arm cannot silently widen what a write path may do.

use ragent_agent::team::TaskStatus;

/// Every status in the enum, in declaration order.
const ALL_TASK_STATUSES: [TaskStatus; 4] = [
    TaskStatus::Pending,
    TaskStatus::InProgress,
    TaskStatus::Completed,
    TaskStatus::Cancelled,
];

/// The D3 table from `specs/teamssetup/PLAN.md`, transcribed row by row.
///
/// Each row is `(from, to, allowed)`; ownership restrictions are actor checks
/// applied on top of this table and are covered by the claim/complete tests.
const D3_TABLE: [(TaskStatus, TaskStatus, bool); 16] = [
    // Pending
    (TaskStatus::Pending, TaskStatus::Pending, false),
    (TaskStatus::Pending, TaskStatus::InProgress, true),
    (TaskStatus::Pending, TaskStatus::Completed, false),
    (TaskStatus::Pending, TaskStatus::Cancelled, true),
    // InProgress
    (TaskStatus::InProgress, TaskStatus::Pending, true),
    (TaskStatus::InProgress, TaskStatus::InProgress, false),
    (TaskStatus::InProgress, TaskStatus::Completed, true),
    (TaskStatus::InProgress, TaskStatus::Cancelled, true),
    // Completed is terminal
    (TaskStatus::Completed, TaskStatus::Pending, false),
    (TaskStatus::Completed, TaskStatus::InProgress, false),
    (TaskStatus::Completed, TaskStatus::Completed, false),
    (TaskStatus::Completed, TaskStatus::Cancelled, false),
    // Cancelled is terminal
    (TaskStatus::Cancelled, TaskStatus::Pending, false),
    (TaskStatus::Cancelled, TaskStatus::InProgress, false),
    (TaskStatus::Cancelled, TaskStatus::Completed, false),
    (TaskStatus::Cancelled, TaskStatus::Cancelled, false),
];

#[test]
fn test_task_status_can_transition_to_matches_d3_table() {
    for (from, to, expected) in D3_TABLE {
        assert_eq!(
            from.can_transition_to(to),
            expected,
            "D3 says {from:?} -> {to:?} should be allowed={expected}"
        );
    }
}

#[test]
fn test_task_status_d3_table_covers_every_pair() {
    // Every pair appears exactly once, so the table above is both complete and
    // free of duplicates that could mask a wrong row.
    for from in ALL_TASK_STATUSES {
        for to in ALL_TASK_STATUSES {
            let rows = D3_TABLE
                .iter()
                .filter(|(f, t, _)| *f == from && *t == to)
                .count();
            assert_eq!(
                rows, 1,
                "expected exactly one D3 row for {from:?} -> {to:?}"
            );
        }
    }
    assert_eq!(D3_TABLE.len(), ALL_TASK_STATUSES.len().pow(2));
}

#[test]
fn test_task_status_self_transition_is_never_legal() {
    for from in ALL_TASK_STATUSES {
        assert!(
            !from.can_transition_to(from),
            "re-asserting {from:?} is not a state change and must be rejected"
        );
    }
}

#[test]
fn test_task_status_terminal_states_have_no_outgoing_edge() {
    for from in ALL_TASK_STATUSES
        .into_iter()
        .filter(TaskStatus::is_terminal)
    {
        for to in ALL_TASK_STATUSES {
            assert!(
                !from.can_transition_to(to),
                "terminal state {from:?} must not reach {to:?}"
            );
        }
    }
}

#[test]
fn test_task_status_non_terminal_states_have_an_exit() {
    // No reachable-and-unleaveable state: every non-terminal status can reach at
    // least one other status.
    for from in ALL_TASK_STATUSES.into_iter().filter(|s| !s.is_terminal()) {
        let exits = ALL_TASK_STATUSES
            .into_iter()
            .filter(|to| from.can_transition_to(*to))
            .count();
        assert!(exits > 0, "{from:?} must have at least one legal exit");
    }
}

#[test]
fn test_task_status_is_terminal_matches_status() {
    assert!(!TaskStatus::Pending.is_terminal());
    assert!(!TaskStatus::InProgress.is_terminal());
    assert!(TaskStatus::Completed.is_terminal());
    assert!(TaskStatus::Cancelled.is_terminal());
}

#[test]
fn test_task_status_claim_and_complete_are_legal() {
    // The two moves the happy path depends on, asserted by name so a table edit
    // that breaks claiming or completion fails with an obvious message.
    assert!(
        TaskStatus::Pending.can_transition_to(TaskStatus::InProgress),
        "claiming a Pending task must be legal"
    );
    assert!(
        TaskStatus::InProgress.can_transition_to(TaskStatus::Completed),
        "completing an InProgress task must be legal"
    );
}

#[test]
fn test_task_status_terminal_regression_pairs_are_illegal() {
    // F-18 named these three pairs explicitly; they must stay refused.
    assert!(!TaskStatus::Completed.can_transition_to(TaskStatus::InProgress));
    assert!(!TaskStatus::Cancelled.can_transition_to(TaskStatus::Completed));
    assert!(!TaskStatus::Completed.can_transition_to(TaskStatus::Cancelled));
}
