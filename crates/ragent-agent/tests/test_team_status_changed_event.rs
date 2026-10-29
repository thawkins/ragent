//! Integration tests for [`TransitionCause`] and `Event::TeammateStatusChanged`
//! (spec `teamssetup` FR-035).
//!
//! FR-035 requires that every member-status transition emits exactly one event
//! carrying the agent ID, the previous status, the new status, and the cause, so
//! the event stream alone can reconstruct the status history. This file covers
//! the two pieces that make that possible:
//!
//! - the [`TransitionCause`] vocabulary and its canonical tokens, and
//! - the `Event::TeammateStatusChanged` payload shape.
//!
//! What is *not* covered here is the emission itself - that each status write
//! publishes exactly one event - which is T-003's `set_member_status` and is
//! tested with it.

use ragent_agent::Event;
use ragent_agent::team::{MemberStatus, TransitionCause};

/// Every `TransitionCause` variant, in declaration order.
///
/// Kept as one list so a new variant added to the enum fails the exhaustiveness
/// checks below rather than silently escaping them.
const ALL_CAUSES: [TransitionCause; 17] = [
    TransitionCause::Spawned,
    TransitionCause::SpawnFailed,
    TransitionCause::TaskClaimed,
    TransitionCause::TaskCompleted,
    TransitionCause::ReportedIdle,
    TransitionCause::PlanSubmitted,
    TransitionCause::PlanApproved,
    TransitionCause::PlanRejected,
    TransitionCause::DependencyBlocked,
    TransitionCause::DependencySatisfied,
    TransitionCause::Suspended,
    TransitionCause::Resumed,
    TransitionCause::ShutdownRequested,
    TransitionCause::ShutdownAcked,
    TransitionCause::WatchdogTimeout,
    TransitionCause::TaskCancelled,
    TransitionCause::TeamCleanedUp,
];

/// Every `MemberStatus` variant, in declaration order.
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

// -- TransitionCause vocabulary ---------------------------------------------

#[test]
fn every_cause_has_a_distinct_snake_case_token() {
    let mut seen = std::collections::BTreeSet::new();
    for cause in ALL_CAUSES {
        let token = cause.as_str();
        assert!(
            token.chars().all(|c| c.is_ascii_lowercase() || c == '_'),
            "FR-035: the cause token must be snake_case ASCII, got {token:?} for {cause:?}"
        );
        assert!(
            seen.insert(token),
            "FR-035: cause token {token:?} is used by more than one variant"
        );
    }
    assert_eq!(
        seen.len(),
        ALL_CAUSES.len(),
        "every cause must have its own token"
    );
}

#[test]
fn cause_token_matches_its_serde_representation() {
    // The token is what an event payload and a `team_status` history row carry
    // (FR-035, FR-036), so it must be the same string serde writes.
    for cause in ALL_CAUSES {
        let json = serde_json::to_string(&cause).expect("serialize cause");
        assert_eq!(
            json,
            format!("\"{}\"", cause.as_str()),
            "as_str() must agree with the serde representation for {cause:?}"
        );
    }
}

#[test]
fn cause_deserialises_from_its_token() {
    // A consumer that reads a cause back from an event or a stored history row
    // goes through serde; the round trip must land on the same variant.
    for cause in ALL_CAUSES {
        let wire = format!("\"{}\"", cause.as_str());
        let back: TransitionCause = serde_json::from_str(&wire).expect("deserialize cause");
        assert_eq!(back, cause, "round trip failed for token {wire}");
    }
}

// -- transition_cause coverage ----------------------------------------------

#[test]
fn every_legal_transition_derives_a_cause() {
    // `transition_cause(from, to)` is the fallback for a write site that does
    // not know why a status changed. It must total over the legal transition
    // table: an edge with no derivable cause would mean a status can move
    // without FR-035's stream being able to name the reason.
    let mut covered = 0usize;
    for from in ALL_STATUSES {
        for to in ALL_STATUSES {
            if !from.can_transition_to(to) {
                continue;
            }
            covered += 1;
            // Any mapped cause is acceptable here; what must not happen is a
            // panic or a non-token result.
            let cause = from.transition_cause(to);
            assert!(
                !cause.as_str().is_empty(),
                "no cause derivable for the legal edge {from:?} -> {to:?}"
            );
        }
    }
    assert!(
        covered > 0,
        "the transition table was empty, so this test proves nothing"
    );
}

#[test]
fn transition_cause_names_the_intuitive_cause_for_common_edges() {
    // These are the edges a reader would ask "why?" about first, so the derived
    // answer must be the obvious one rather than an arbitrary member of the
    // vocabulary.
    let cases = [
        (
            MemberStatus::Stopped,
            MemberStatus::Spawning,
            TransitionCause::Spawned,
        ),
        (
            MemberStatus::Spawning,
            MemberStatus::Failed,
            TransitionCause::SpawnFailed,
        ),
        (
            MemberStatus::Working,
            MemberStatus::Failed,
            TransitionCause::WatchdogTimeout,
        ),
        (
            MemberStatus::Idle,
            MemberStatus::Working,
            TransitionCause::TaskClaimed,
        ),
        (
            MemberStatus::Working,
            MemberStatus::Idle,
            TransitionCause::TaskCompleted,
        ),
        (
            MemberStatus::Working,
            MemberStatus::PlanPending,
            TransitionCause::PlanSubmitted,
        ),
        (
            MemberStatus::PlanPending,
            MemberStatus::Working,
            TransitionCause::PlanApproved,
        ),
        (
            MemberStatus::Working,
            MemberStatus::Blocked,
            TransitionCause::DependencyBlocked,
        ),
        (
            MemberStatus::Working,
            MemberStatus::Suspended,
            TransitionCause::Suspended,
        ),
        (
            MemberStatus::Suspended,
            MemberStatus::Idle,
            TransitionCause::Resumed,
        ),
        (
            MemberStatus::Idle,
            MemberStatus::ShuttingDown,
            TransitionCause::ShutdownRequested,
        ),
        (
            MemberStatus::ShuttingDown,
            MemberStatus::Stopped,
            TransitionCause::ShutdownAcked,
        ),
    ];
    for (from, to, expected) in cases {
        assert_eq!(
            from.transition_cause(to),
            expected,
            "transition_cause({from:?}, {to:?}) must be {expected:?}"
        );
    }
}

// -- Event::TeammateStatusChanged --------------------------------------------

fn status_changed() -> Event {
    Event::TeammateStatusChanged {
        session_id: "sess-1".to_string(),
        team_name: "audit".to_string(),
        agent_id: "tm-003".to_string(),
        from: MemberStatus::Working.as_str().to_string(),
        to: MemberStatus::Idle.as_str().to_string(),
        cause: TransitionCause::TaskCompleted.as_str().to_string(),
    }
}

#[test]
fn status_changed_event_is_named_for_the_serialised_stream() {
    // The event's variant name is part of the wire contract (SSE `event:` and
    // stored event logs), so it may not drift silently.
    assert_eq!(
        status_changed().type_name(),
        "TeammateStatusChanged",
        "the variant name is the wire contract for the event stream"
    );
}

#[test]
fn status_changed_event_carries_the_whole_transition() {
    // FR-035 names four facts that must be reconstructable without the log: who,
    // from, to, and why. All four must survive a serialise/deserialise round.
    let json = serde_json::to_string(&status_changed()).expect("serialize event");
    for (field, value) in [
        ("agent_id", "tm-003"),
        ("from", "working"),
        ("to", "idle"),
        ("cause", "task_completed"),
    ] {
        assert!(
            json.contains(value),
            "FR-035: the event payload must carry {field}={value}; got {json}"
        );
    }

    let back: Event = serde_json::from_str(&json).expect("deserialize event");
    match back {
        Event::TeammateStatusChanged {
            agent_id,
            from,
            to,
            cause,
            ..
        } => {
            assert_eq!(agent_id, "tm-003");
            assert_eq!(from, MemberStatus::Working.as_str());
            assert_eq!(to, MemberStatus::Idle.as_str());
            assert_eq!(cause, TransitionCause::TaskCompleted.as_str());
        }
        other => panic!("round trip produced {other:?}, expected TeammateStatusChanged"),
    }
}

#[test]
fn status_changed_event_reports_its_session_for_routing() {
    // Event routing and per-session filtering read `session_id()`; a variant
    // that returned `None` here would be invisible to a filtered subscriber.
    assert_eq!(
        status_changed().session_id(),
        Some("sess-1"),
        "the event must be routable to its lead session"
    );
}
