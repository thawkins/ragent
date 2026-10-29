//! Every `TeamMember::status` write site in `manager.rs` goes through
//! [`TeamManager::set_member_status`] (spec `teamssetup` FR-008).
//!
//! Before T-004 four sites in `manager.rs` wrote `TeamMember::status` directly:
//! `suspend_teammate`, `resume_teammate`, `shutdown_teammate`, and
//! `approve_plan`. A direct write skips the transition table, skips the
//! destination's side effects, and - the observable difference this suite pins -
//! publishes no `TeammateStatusChanged` event.
//!
//! The behavioural signature of routing a site through the writer is therefore
//! the same for all four: the move happens **and** exactly one
//! `TeammateStatusChanged` carrying the full record is published. A site that
//! reverts to a direct write loses the event and a site that bypasses the table
//! can reach a status the table forbids; both fail here.
//!
//! Every write is announced, including one that re-asserts the status the member
//! already holds (FR-008): that announcement is how a teammate the watchdog
//! declared dead says it is still in its turn, so it carries `from == to` and
//! writes no roster row. [`status_moves`] is what a *transition* assertion wants
//! and leaves those self announcements out; [`status_changes`] keeps them so a
//! test can assert on the whole announcement sequence without losing one.
//!
//! The fixture is [`manager_support::team_manager`], whose member `alice`
//! (`tm-001`) starts `Working`.

use ragent_agent::event::Event;
use ragent_agent::team::TeamStore;
use ragent_agent::team::config::{MemberStatus, PlanStatus, TeamMember, TransitionCause};

#[path = "manager_support/mod.rs"]
mod manager_support;

use manager_support::team_manager;

/// Read `agent_id`'s persisted status through the team store.
fn status_of(manager: &ragent_agent::team::TeamManager, agent_id: &str) -> MemberStatus {
    TeamStore::load(&manager.team_dir)
        .expect("load team store")
        .config
        .member_by_id(agent_id)
        .expect("member present")
        .status
}

/// Drain the bus and return every `TeammateStatusChanged` as
/// `(agent_id, from, to, cause)`.
fn status_changes(
    rx: &mut tokio::sync::broadcast::Receiver<Event>,
) -> Vec<(String, String, String, String)> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let Event::TeammateStatusChanged {
            agent_id,
            from,
            to,
            cause,
            ..
        } = event
        {
            out.push((agent_id, from, to, cause));
        }
    }
    out
}

/// Drain the bus and return only the `TeammateStatusChanged` events that are
/// *transitions* - the ones whose `from` differs from their `to`.
///
/// A write that re-asserts the status a member already holds is announced too
/// (FR-008), but it moves nothing: it records no roster change and no history
/// row. Tests that pin "what moved" want this, not [`status_changes`], so the
/// self announcements that legitimately happen along the way do not have to be
/// spelled out in every expectation.
fn status_moves(
    rx: &mut tokio::sync::broadcast::Receiver<Event>,
) -> Vec<(String, String, String, String)> {
    drop_self_transitions(status_changes(rx))
}

/// Remove every `(agent_id, from, to, cause)` whose `from` equals its `to`.
fn drop_self_transitions(
    changes: Vec<(String, String, String, String)>,
) -> Vec<(String, String, String, String)> {
    changes
        .into_iter()
        .filter(|(_, from, to, _)| from != to)
        .collect()
}

// -- suspend ----------------------------------------------------------------

#[tokio::test]
async fn suspend_records_the_move_through_the_writer() {
    let (manager, _tmp, bus) = team_manager("t004-suspend", "lead-1", true);
    let mut rx = bus.subscribe();

    manager.suspend_teammate("tm-001").await.expect("suspend");

    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Suspended);
    let changes = status_changes(&mut rx);
    assert_eq!(
        changes,
        vec![(
            "tm-001".to_string(),
            "working".to_string(),
            "suspended".to_string(),
            "suspended".to_string(),
        )],
        "suspending must be a transition through the writer, not a direct write"
    );
}

// -- resume -----------------------------------------------------------------

#[tokio::test]
async fn resume_records_the_move_through_the_writer() {
    let (manager, _tmp, bus) = team_manager("t004-resume", "lead-1", true);
    manager.suspend_teammate("tm-001").await.expect("suspend");
    let mut rx = bus.subscribe();

    manager.resume_teammate("tm-001").await.expect("resume");

    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Idle);
    let changes = status_changes(&mut rx);
    assert_eq!(
        changes,
        vec![(
            "tm-001".to_string(),
            "suspended".to_string(),
            "idle".to_string(),
            "resumed".to_string(),
        )],
        "resuming must be a transition through the writer, not a direct write"
    );
}

// -- shutdown ---------------------------------------------------------------

#[tokio::test]
async fn immediate_shutdown_records_both_moves_through_the_writer() {
    let (manager, _tmp, bus) = team_manager("t004-shutdown-immediate", "lead-1", true);
    let mut rx = bus.subscribe();

    manager
        .shutdown_teammate("tm-001", false)
        .await
        .expect("immediate shutdown");

    // `Working -> ShuttingDown -> Stopped` is the table's only route to the
    // terminal state: the direct `Working -> Stopped` write the old code made
    // is refused by the writer, so the path makes both moves visibly.
    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Stopped);
    let changes = status_changes(&mut rx);
    assert_eq!(
        changes,
        vec![
            (
                "tm-001".to_string(),
                "working".to_string(),
                "shutting_down".to_string(),
                "shutdown_requested".to_string(),
            ),
            (
                "tm-001".to_string(),
                "shutting_down".to_string(),
                "stopped".to_string(),
                "shutdown_acked".to_string(),
            ),
        ],
        "both shutdown moves must be transitions through the writer"
    );
}

#[tokio::test]
async fn immediate_shutdown_of_a_shutting_down_member_stops_it() {
    let (manager, _tmp, bus) = team_manager("t004-shutdown-stop", "lead-1", true);
    // A graceful request first so the member is already `ShuttingDown`; the
    // immediate path then only has the request-to-ack move left to make.
    manager
        .shutdown_teammate("tm-001", true)
        .await
        .expect("graceful shutdown");
    let mut rx = bus.subscribe();

    manager
        .shutdown_teammate("tm-001", false)
        .await
        .expect("immediate shutdown");

    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Stopped);
    let changes = status_changes(&mut rx);
    // The immediate path re-asserts `ShuttingDown` while it pushes the request,
    // and that re-assertion is announced (FR-008) - it is simply not a move, so
    // it carries `from == to` and leaves the roster alone.
    assert_eq!(
        drop_self_transitions(changes.clone()),
        vec![(
            "tm-001".to_string(),
            "shutting_down".to_string(),
            "stopped".to_string(),
            "shutdown_acked".to_string(),
        )],
        "a member already shutting down makes only the ack move"
    );
    assert!(
        changes.iter().all(
            |(_, from, to, _)| (from == "shutting_down" && to == "shutting_down")
                || (from == "shutting_down" && to == "stopped")
        ),
        "every announcement is either the re-asserted request or the ack: {changes:?}"
    );
}

#[tokio::test]
async fn graceful_shutdown_twice_is_not_a_second_transition() {
    let (manager, _tmp, bus) = team_manager("t004-shutdown-twice", "lead-1", true);
    manager
        .shutdown_teammate("tm-001", true)
        .await
        .expect("first graceful shutdown");
    let mut rx = bus.subscribe();

    // Re-asserting `ShuttingDown` is not a move: the request is still pushed and
    // the write is still announced, but `from == to`, so the roster records no
    // transition and the status stays where the first call put it.
    manager
        .shutdown_teammate("tm-001", true)
        .await
        .expect("second graceful shutdown");

    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::ShuttingDown);
    assert_eq!(
        status_changes(&mut rx),
        vec![(
            "tm-001".to_string(),
            "shutting_down".to_string(),
            "shutting_down".to_string(),
            "shutdown_requested".to_string(),
        )],
        "a re-asserted status is announced as a self transition, not as a move"
    );
}

// -- approve_plan -----------------------------------------------------------

#[test]
fn plan_approval_records_the_move_through_the_writer() {
    let (manager, _tmp, bus) = team_manager("t004-approve", "lead-1", true);
    // A submitted plan parks the member in `PlanPending`, the only status with
    // an edge to `Working` on the plan path.
    prime_plan_pending(&manager);
    let mut rx = bus.subscribe();

    manager.approve_plan("tm-001", true).expect("approve");

    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Working);
    let changes = status_changes(&mut rx);
    assert_eq!(
        changes,
        vec![(
            "tm-001".to_string(),
            "plan_pending".to_string(),
            "working".to_string(),
            "plan_approved".to_string(),
        )],
        "approving a plan must be a transition through the writer"
    );
    assert_eq!(
        TeamStore::load(&manager.team_dir)
            .expect("load")
            .config
            .member_by_id("tm-001")
            .expect("member")
            .plan_status,
        PlanStatus::Approved
    );
}

#[test]
fn plan_rejection_records_the_verdict_without_moving_the_member() {
    let (manager, _tmp, bus) = team_manager("t004-reject", "lead-1", true);
    prime_plan_pending(&manager);
    let mut rx = bus.subscribe();

    manager.approve_plan("tm-001", false).expect("reject");

    let member = TeamStore::load(&manager.team_dir)
        .expect("load")
        .config
        .member_by_id("tm-001")
        .expect("member")
        .clone();
    assert_eq!(member.plan_status, PlanStatus::Rejected);
    assert_eq!(
        member.status,
        MemberStatus::PlanPending,
        "a rejected plan leaves the teammate still waiting on the lead"
    );
    assert!(
        status_changes(&mut rx).is_empty(),
        "a rejection moves no status, so it publishes no status-change event"
    );
}

#[test]
fn approving_a_plan_for_a_member_that_is_not_plan_pending_still_records_the_verdict() {
    // The fixture member is `Working`, which is the status an approval asks
    // for. The move is therefore a self transition - not a transition at all -
    // so nothing moves and the verdict is still recorded. The write is still
    // announced (FR-008), which is why the assertion is on the *moves*.
    let (manager, _tmp, bus) = team_manager("t004-approve-noop", "lead-1", true);
    let mut rx = bus.subscribe();

    manager.approve_plan("tm-001", true).expect("approve");

    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Working);
    assert!(
        status_moves(&mut rx).is_empty(),
        "approving into the status the member already holds is not a transition"
    );
    assert_eq!(
        TeamStore::load(&manager.team_dir)
            .expect("load")
            .config
            .member_by_id("tm-001")
            .expect("member")
            .plan_status,
        PlanStatus::Approved
    );
}

#[test]
fn approving_a_plan_for_a_terminal_member_is_refused() {
    // `Stopped -> Working` is not in the table: a member with no session cannot
    // be put back to work by a plan verdict.
    let (manager, _tmp, _bus) = team_manager("t004-approve-terminal", "lead-1", true);
    manager
        .set_member_status(
            "tm-001",
            MemberStatus::ShuttingDown,
            TransitionCause::ShutdownRequested,
        )
        .expect("to ShuttingDown");
    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Stopped,
            TransitionCause::ShutdownAcked,
        )
        .expect("to Stopped");

    let err = manager
        .approve_plan("tm-001", true)
        .expect_err("a stopped member cannot be approved into Working");
    assert!(
        err.to_string().contains("tm-001"),
        "the refusal must name the member: {err}"
    );
    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Stopped);
    assert_eq!(
        TeamStore::load(&manager.team_dir)
            .expect("load")
            .config
            .member_by_id("tm-001")
            .expect("member")
            .plan_status,
        PlanStatus::None,
        "a refused approval must not record a verdict the move would have carried"
    );
}

// -- helpers ----------------------------------------------------------------

/// Put `tm-001` into `PlanPending` with a submitted plan, which is the state
/// `approve_plan` acts on.
fn prime_plan_pending(manager: &ragent_agent::team::TeamManager) {
    manager
        .set_member_status(
            "tm-001",
            MemberStatus::PlanPending,
            TransitionCause::PlanSubmitted,
        )
        .expect("to PlanPending");
    manager
        .update_member("tm-001", |m: &mut TeamMember| {
            m.plan_status = PlanStatus::Pending;
        })
        .expect("record the submitted plan");
    manager.invalidate_plan_pending_cache("tm-001");
}
