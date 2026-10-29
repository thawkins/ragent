//! Integration tests for the `MemberStatus` state machine (spec `teamssetup`
//! FR-003, FR-009).
//!
//! Covers the three status-class predicates (`is_terminal`, `is_parked`,
//! `is_live`) and the single legal-transition table
//! [`MemberStatus::can_transition_to`]. The tested items are public via
//! `ragent_agent::team::MemberStatus`, so no `#[path]` re-import is needed.

use ragent_agent::team::MemberStatus;

/// Every `MemberStatus` variant, in declaration order.
///
/// Kept as one list so a new variant added to the enum fails the exhaustiveness
/// checks below rather than silently escaping them.
const ALL_STATUSES: [MemberStatus; 9] = [
    MemberStatus::Spawning,
    MemberStatus::Working,
    MemberStatus::Idle,
    MemberStatus::PlanPending,
    MemberStatus::Blocked,
    MemberStatus::Suspended,
    MemberStatus::ShuttingDown,
    MemberStatus::Stopped,
    MemberStatus::Failed,
];

/// The D2 classification of each status: is it terminal, parked, or live.
///
/// This is the table the predicates must reproduce, written out independently
/// of `config.rs` so a change to the definition is caught here.
const CLASSES: [(MemberStatus, bool, bool, bool); 9] = [
    // (status, is_terminal, is_parked, is_live)
    (MemberStatus::Spawning, false, false, true),
    (MemberStatus::Working, false, false, true),
    (MemberStatus::Idle, false, true, false),
    (MemberStatus::PlanPending, false, true, false),
    (MemberStatus::Blocked, false, true, false),
    (MemberStatus::Suspended, false, true, false),
    (MemberStatus::ShuttingDown, false, false, true),
    (MemberStatus::Stopped, true, false, false),
    (MemberStatus::Failed, true, false, false),
];

/// The FR-009 / PLAN.md M1 transition table.
///
/// Each row lists the statuses a member may move to from the key status.
const LEGAL_TARGETS: [(MemberStatus, &[MemberStatus]); 9] = [
    (
        MemberStatus::Spawning,
        &[MemberStatus::Working, MemberStatus::Failed],
    ),
    (
        MemberStatus::Working,
        &[
            MemberStatus::Idle,
            MemberStatus::PlanPending,
            MemberStatus::Blocked,
            MemberStatus::Suspended,
            MemberStatus::ShuttingDown,
            MemberStatus::Failed,
        ],
    ),
    (
        MemberStatus::Idle,
        &[
            MemberStatus::Working,
            MemberStatus::Blocked,
            MemberStatus::Suspended,
            MemberStatus::ShuttingDown,
            MemberStatus::Failed,
        ],
    ),
    (
        MemberStatus::PlanPending,
        &[
            MemberStatus::Working,
            MemberStatus::Idle,
            MemberStatus::Failed,
        ],
    ),
    (
        MemberStatus::Blocked,
        &[
            MemberStatus::Idle,
            MemberStatus::Suspended,
            MemberStatus::ShuttingDown,
            MemberStatus::Failed,
        ],
    ),
    (
        MemberStatus::Suspended,
        &[
            MemberStatus::Idle,
            MemberStatus::ShuttingDown,
            MemberStatus::Failed,
        ],
    ),
    (
        MemberStatus::ShuttingDown,
        &[MemberStatus::Stopped, MemberStatus::Failed],
    ),
    (MemberStatus::Stopped, &[MemberStatus::Spawning]),
    (MemberStatus::Failed, &[MemberStatus::Spawning]),
];

/// Look up the expected classification row for `status`.
fn expected_class(status: MemberStatus) -> (bool, bool, bool) {
    CLASSES
        .iter()
        .find(|(s, _, _, _)| *s == status)
        .map(|(_, terminal, parked, live)| (*terminal, *parked, *live))
        .expect("CLASSES covers every MemberStatus variant")
}

#[test]
fn test_member_status_predicates_match_classification_table() {
    for status in ALL_STATUSES {
        let (terminal, parked, live) = expected_class(status);
        let label = status.as_str();
        assert_eq!(
            status.is_terminal(),
            terminal,
            "is_terminal() wrong for {label}"
        );
        assert_eq!(status.is_parked(), parked, "is_parked() wrong for {label}");
        assert_eq!(status.is_live(), live, "is_live() wrong for {label}");
    }
}

#[test]
fn test_member_status_predicates_are_mutually_exclusive() {
    // FR-009: every status is exactly one of terminal, parked, or live. The
    // writer relies on this to pick a guard without re-listing statuses.
    for status in ALL_STATUSES {
        let count = [status.is_terminal(), status.is_parked(), status.is_live()]
            .iter()
            .filter(|flag| **flag)
            .count();
        assert_eq!(
            count,
            1,
            "{} must satisfy exactly one status predicate, matched {count}",
            status.as_str()
        );
    }
}

#[test]
fn test_member_status_terminal_excludes_shutting_down() {
    // The regression that matters most: `ShuttingDown` still has a live session
    // waiting on `team_shutdown_ack`, so treating it as terminal would let a
    // caller drop a teammate that has not finished stopping.
    assert!(!MemberStatus::ShuttingDown.is_terminal());
    assert!(MemberStatus::Stopped.is_terminal());
    assert!(MemberStatus::Failed.is_terminal());
}

#[test]
fn test_member_status_parked_covers_blocked_plan_pending_suspended() {
    // FR-012 needs exactly these three to be skipped by the watchdog, and
    // `Idle` joins them because a member with no work cannot be wedged.
    assert!(MemberStatus::Blocked.is_parked());
    assert!(MemberStatus::PlanPending.is_parked());
    assert!(MemberStatus::Suspended.is_parked());
    assert!(MemberStatus::Idle.is_parked());
    assert!(!MemberStatus::Working.is_parked());
    assert!(!MemberStatus::Spawning.is_parked());
}

#[test]
fn test_member_status_live_is_only_watchdog_eligible_set() {
    // FR-012: only a live member may be judged failed.
    assert!(MemberStatus::Spawning.is_live());
    assert!(MemberStatus::Working.is_live());
    assert!(MemberStatus::ShuttingDown.is_live());
    assert!(!MemberStatus::Stopped.is_live());
    assert!(!MemberStatus::Failed.is_live());
}

#[test]
fn test_member_status_can_transition_to_matches_table() {
    // Table-driven check of all 81 (from, to) pairs against the declared table.
    for (from, targets) in LEGAL_TARGETS {
        for to in ALL_STATUSES {
            let allowed = to != from && targets.contains(&to);
            assert_eq!(
                from.can_transition_to(to),
                allowed,
                "can_transition_to({} -> {}) should be {allowed}",
                from.as_str(),
                to.as_str()
            );
        }
    }
}

#[test]
fn test_member_status_self_transition_is_never_legal() {
    for status in ALL_STATUSES {
        assert!(
            !status.can_transition_to(status),
            "{} -> {} must be illegal (a self-transition is not a state change)",
            status.as_str(),
            status.as_str()
        );
    }
}

#[test]
fn test_member_status_blocked_is_not_a_terminal_sink() {
    // F-06: `Blocked` used to be enterable but un-leaveable. It must now reach
    // every operator verb the `Suspended` state accepts.
    assert!(MemberStatus::Blocked.can_transition_to(MemberStatus::Idle));
    assert!(MemberStatus::Blocked.can_transition_to(MemberStatus::Suspended));
    assert!(MemberStatus::Blocked.can_transition_to(MemberStatus::ShuttingDown));
}

#[test]
fn test_member_status_respawn_from_terminal_states_only() {
    // A respawn is the only exit from a terminal status, and it re-enters at
    // `Spawning`; no live or parked status may be sent back to `Spawning`.
    for status in ALL_STATUSES {
        assert_eq!(
            status.can_transition_to(MemberStatus::Spawning),
            status.is_terminal(),
            "{} -> spawning must be legal only from a terminal status",
            status.as_str()
        );
    }
}

#[test]
fn test_member_status_stopped_only_reached_through_shutting_down() {
    // `Stopped` is never entered directly: a member stops by being asked to
    // shut down, acknowledging, and only then becoming `Stopped`.
    for status in ALL_STATUSES {
        let expected = status == MemberStatus::ShuttingDown;
        assert_eq!(
            status.can_transition_to(MemberStatus::Stopped),
            expected,
            "{} -> stopped must be legal only from shutting_down",
            status.as_str()
        );
    }
}
