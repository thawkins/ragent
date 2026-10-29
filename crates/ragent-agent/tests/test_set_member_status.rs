//! Integration tests for [`TeamManager::set_member_status`], the single member
//! status writer (spec `teamssetup` FR-008, FR-035).
//!
//! FR-008 requires that every write to `TeamMember::status` go through one
//! function that validates the transition, writes the status, and publishes the
//! corresponding event, so no code path can set a status without both. FR-035
//! requires exactly one event per transition, carrying the agent ID, the
//! previous status, the new status, and the cause.
//!
//! These tests pin the writer's four obligations:
//!
//! 1. an illegal transition is refused, naming the member and both statuses,
//!    and leaves the roster untouched;
//! 2. a legal transition persists the new status to `config.json`;
//! 3. a legal transition publishes exactly one `TeammateStatusChanged`, carrying
//!    the real `from`/`to`/`cause`;
//! 4. a write that does not change the status is not a transition: the roster
//!    is not rewritten and no coarse lifecycle event is published, but the
//!    write is still announced as one `TeammateStatusChanged` and still lands in
//!    `status-history.jsonl`, so a member that re-asserts where it is (a
//!    watchdog-failed teammate finishing its turn) is visible.
//!
//! A manager built over a stub `SessionProcessor` (see `manager_support`) is
//! enough: the writer touches only on-disk team state.

use ragent_agent::Event;
use ragent_agent::team::{MemberStatus, TeamMember, TeamStore, TransitionCause};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::team_manager;

/// Read one member's on-disk status straight from `config.json`.
///
/// Read from disk rather than through the manager so the assertion fails if the
/// writer forgot to persist.
fn on_disk_status(manager: &ragent_agent::team::TeamManager, agent_id: &str) -> MemberStatus {
    TeamStore::load(&manager.team_dir)
        .expect("load team store")
        .config
        .member_by_id(agent_id)
        .expect("member exists")
        .status
}

/// Rewrite one member's on-disk record, for tests that need a member parked in
/// a status the shared fixture does not produce.
fn set_on_disk_status(
    manager: &ragent_agent::team::TeamManager,
    agent_id: &str,
    status: MemberStatus,
) {
    let mut store = TeamStore::load(&manager.team_dir).expect("load team store");
    store
        .config
        .member_by_id_mut(agent_id)
        .expect("member exists")
        .status = status;
    store.save().expect("save team store");
}

/// Drain the bus without blocking, returning every `TeammateStatusChanged` it
/// carried.
fn status_events(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> Vec<Event> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if matches!(event, Event::TeammateStatusChanged { .. }) {
            out.push(event);
        }
    }
    out
}

// -- Validation --------------------------------------------------------------

#[test]
fn illegal_transition_is_refused_naming_member_and_both_statuses() {
    let (manager, _tmp, _bus) = team_manager("guard-team", "lead-1", true);
    // The shared fixture records `alice` as `Working`.
    assert_eq!(on_disk_status(&manager, "tm-001"), MemberStatus::Working);

    let err = manager
        .set_member_status(
            "tm-001",
            MemberStatus::Stopped,
            TransitionCause::ShutdownAcked,
        )
        .expect_err("Working -> Stopped is not a legal transition");

    let msg = err.to_string();
    assert!(msg.contains("tm-001"), "must name the member: {msg}");
    assert!(
        msg.contains("working"),
        "must name the current status: {msg}"
    );
    assert!(
        msg.contains("stopped"),
        "must name the requested status: {msg}"
    );
    assert!(
        msg.contains("shutdown_acked"),
        "must name the cause it refused: {msg}"
    );

    // A refusal must not have moved the member.
    assert_eq!(on_disk_status(&manager, "tm-001"), MemberStatus::Working);
}

#[test]
fn unknown_agent_id_is_an_error_not_a_silent_noop() {
    let (manager, _tmp, _bus) = team_manager("unknown-team", "lead-1", true);

    let err = manager
        .set_member_status(
            "tm-999",
            MemberStatus::Working,
            TransitionCause::TaskClaimed,
        )
        .expect_err("an unknown member cannot be moved");

    let msg = err.to_string();
    assert!(msg.contains("tm-999"), "must name the member: {msg}");
    assert!(msg.contains("unknown-team"), "must name the team: {msg}");
}

#[test]
fn terminal_status_has_no_exit_except_respawn() {
    let (manager, _tmp, _bus) = team_manager("terminal-team", "lead-1", true);
    set_on_disk_status(&manager, "tm-001", MemberStatus::Stopped);

    // A stopped member cannot be resumed; only a respawn re-enters at Spawning.
    let err = manager
        .set_member_status("tm-001", MemberStatus::Idle, TransitionCause::Resumed)
        .expect_err("Stopped -> Idle is not legal");
    assert!(err.to_string().contains("stopped"), "err: {err}");

    // The respawn edge is legal.
    let applied = manager
        .set_member_status("tm-001", MemberStatus::Spawning, TransitionCause::Spawned)
        .expect("Stopped -> Spawning is the respawn edge")
        .expect("a transition happened");
    assert_eq!(applied.from, MemberStatus::Stopped);
    assert_eq!(applied.to, MemberStatus::Spawning);
}

// -- Persistence -------------------------------------------------------------

#[test]
fn legal_transition_persists_the_new_status() {
    let (manager, _tmp, _bus) = team_manager("persist-team", "lead-1", true);

    // The shared fixture starts the member `Working`, so the persisted move
    // under test is the parked one an idle teammate makes.
    let applied = manager
        .set_member_status("tm-001", MemberStatus::Idle, TransitionCause::ReportedIdle)
        .expect("Working -> Idle is legal")
        .expect("a transition happened");

    assert_eq!(
        applied,
        ragent_agent::team::StatusTransition {
            from: MemberStatus::Working,
            to: MemberStatus::Idle,
            cause: TransitionCause::ReportedIdle,
        }
    );
    assert_eq!(on_disk_status(&manager, "tm-001"), MemberStatus::Idle);
}

#[test]
fn leaving_working_clears_the_held_task() {
    let (manager, _tmp, _bus) = team_manager("taskclear-team", "lead-1", true);
    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Working,
            TransitionCause::TaskClaimed,
        )
        .expect("to Working");
    manager
        .update_member("tm-001", |m| {
            m.current_task_id = Some("task-001".to_string())
        })
        .expect("record a held task");

    manager
        .set_member_status("tm-001", MemberStatus::Idle, TransitionCause::TaskCompleted)
        .expect("Working -> Idle is legal");

    let member = TeamStore::load(&manager.team_dir)
        .expect("load")
        .config
        .member_by_id("tm-001")
        .expect("member")
        .clone();
    assert_eq!(member.status, MemberStatus::Idle);
    assert_eq!(
        member.current_task_id, None,
        "an idle member must not still claim a task"
    );
}

#[test]
fn leaving_plan_pending_clears_the_plan_request_id() {
    let (manager, _tmp, _bus) = team_manager("planclear-team", "lead-1", true);
    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Working,
            TransitionCause::TaskClaimed,
        )
        .expect("to Working");
    manager
        .set_member_status(
            "tm-001",
            MemberStatus::PlanPending,
            TransitionCause::PlanSubmitted,
        )
        .expect("to PlanPending");
    manager
        .update_member("tm-001", |m| {
            m.plan_request_id = Some("plan-42".to_string())
        })
        .expect("record a plan request");

    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Working,
            TransitionCause::PlanApproved,
        )
        .expect("PlanPending -> Working is legal");

    let member = TeamStore::load(&manager.team_dir)
        .expect("load")
        .config
        .member_by_id("tm-001")
        .expect("member")
        .clone();
    assert_eq!(
        member.plan_request_id, None,
        "an answered plan request id must not survive the verdict"
    );
}

#[test]
fn reaching_stopped_clears_the_shutdown_request_id() {
    let (manager, _tmp, _bus) = team_manager("shutdownclear-team", "lead-1", true);
    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Working,
            TransitionCause::TaskClaimed,
        )
        .expect("to Working");
    manager
        .set_member_status(
            "tm-001",
            MemberStatus::ShuttingDown,
            TransitionCause::ShutdownRequested,
        )
        .expect("to ShuttingDown");
    manager
        .update_member("tm-001", |m| {
            m.shutdown_request_id = Some("shut-7".to_string());
        })
        .expect("record a shutdown request");

    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Stopped,
            TransitionCause::ShutdownAcked,
        )
        .expect("ShuttingDown -> Stopped is legal");

    let member = TeamStore::load(&manager.team_dir)
        .expect("load")
        .config
        .member_by_id("tm-001")
        .expect("member")
        .clone();
    assert_eq!(
        member.shutdown_request_id, None,
        "an answered shutdown request id must not survive the ack"
    );
}

// -- Event emission ----------------------------------------------------------

#[tokio::test]
async fn legal_transition_publishes_exactly_one_event_with_the_full_record() {
    let (manager, _tmp, bus) = team_manager("event-team", "lead-1", true);
    let mut rx = bus.subscribe();

    // The shared fixture starts the member `Working`; `-> Idle` is a legal
    // transition from there.
    manager
        .set_member_status("tm-001", MemberStatus::Idle, TransitionCause::ReportedIdle)
        .expect("legal transition");

    let events = status_events(&mut rx);
    assert_eq!(events.len(), 1, "exactly one event per transition");

    match &events[0] {
        Event::TeammateStatusChanged {
            session_id,
            team_name,
            agent_id,
            from,
            to,
            cause,
        } => {
            assert_eq!(session_id, "lead-1");
            assert_eq!(team_name, "event-team");
            assert_eq!(agent_id, "tm-001");
            assert_eq!(from, "working");
            assert_eq!(to, "idle");
            assert_eq!(cause, "reported_idle");
        }
        other => panic!("expected TeammateStatusChanged, got {other:?}"),
    }
}

#[tokio::test]
async fn refused_transition_publishes_nothing() {
    let (manager, _tmp, bus) = team_manager("noevent-team", "lead-1", true);
    let mut rx = bus.subscribe();

    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Working,
            TransitionCause::TaskClaimed,
        )
        .expect("prime to Working");
    let _ = status_events(&mut rx); // discard the priming transition

    let _ = manager
        .set_member_status("tm-001", MemberStatus::Spawning, TransitionCause::Spawned)
        .expect_err("Working -> Spawning is not legal");

    assert!(
        status_events(&mut rx).is_empty(),
        "a refused transition must not publish a status-change event"
    );
}

#[tokio::test]
async fn reasserting_the_current_status_is_not_a_transition() {
    let (manager, _tmp, bus) = team_manager("noop-team", "lead-1", true);
    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Working,
            TransitionCause::TaskClaimed,
        )
        .expect("prime to Working");
    let mut rx = bus.subscribe();

    let applied = manager
        .set_member_status(
            "tm-001",
            MemberStatus::Working,
            TransitionCause::ReportedIdle,
        )
        .expect("re-asserting the status a member holds is not an error");

    assert_eq!(
        applied, None,
        "a non-move must report that no transition happened"
    );
    // A re-asserted write is still *published*: it is how a teammate the
    // watchdog declared dead makes its continued life visible. What it is not
    // is a move, so `Ok(None)` and the unchanged status stand.
    let events = status_events(&mut rx);
    assert_eq!(events.len(), 1, "a re-assertion is announced exactly once");
    match &events[0] {
        Event::TeammateStatusChanged {
            agent_id,
            from,
            to,
            cause,
            ..
        } => {
            assert_eq!(agent_id, "tm-001");
            assert_eq!((from.as_str(), to.as_str()), ("working", "working"));
            assert_eq!(cause, "reported_idle");
        }
        other => panic!("expected TeammateStatusChanged, got {other:?}"),
    }
    assert_eq!(
        on_disk_status(&manager, "tm-001"),
        MemberStatus::Working,
        "a re-asserted status writes no roster change"
    );
    // FR-036: the history is the record `team_status` reads back after the
    // event has gone past, so the re-assertion has to be in it too. The priming
    // write is the first line; the re-assertion is the second.
    let history = ragent_agent::team::status_history::read_status_history(&manager.team_dir);
    let last = history.last().expect("the re-assertion is recorded");
    assert_eq!(last.agent_id, "tm-001");
    assert_eq!(
        (last.from.as_str(), last.to.as_str()),
        ("working", "working")
    );
    assert_eq!(last.cause, "reported_idle");
}

// -- The writer is the only way in -------------------------------------------

#[test]
fn every_legal_edge_in_the_table_is_accepted_by_the_writer() {
    // The writer must not add a rule the table does not have: walk every legal
    // edge and confirm each is applied. The starting status is primed by writing
    // it directly to disk (which the writer itself may not do) so each edge is
    // tested in isolation.
    let (manager, _tmp, _bus) = team_manager("edge-team", "lead-1", true);

    let all = [
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

    for from in all {
        for to in all {
            if !from.can_transition_to(to) {
                continue;
            }
            let mut store = TeamStore::load(&manager.team_dir).expect("load");
            let member = store.config.member_by_id_mut("tm-001").expect("member");
            member.status = from;
            store.save().expect("save");

            let applied = manager
                .set_member_status("tm-001", to, from.transition_cause(to))
                .unwrap_or_else(|e| {
                    panic!("{from:?} -> {to:?} is legal but the writer refused it: {e}")
                });

            assert_eq!(
                applied.map(|t| t.to),
                Some(to),
                "{from:?} -> {to:?} must be applied"
            );
            assert_eq!(on_disk_status(&manager, "tm-001"), to);
        }
    }
}

#[test]
fn update_member_refuses_an_unknown_agent_id() {
    let (manager, _tmp, _bus) = team_manager("upd-team", "lead-1", true);

    let err = manager
        .update_member("tm-999", |_| {})
        .expect_err("an unknown member cannot be updated");
    assert!(err.to_string().contains("tm-999"), "err: {err}");
}

#[test]
fn update_member_persists_the_change() {
    let (manager, _tmp, _bus) = team_manager("upd2-team", "lead-1", true);

    manager
        .update_member("tm-001", |m| {
            m.session_id = Some("sess-42".to_string());
            m.last_spawn_error = Some("boom".to_string());
        })
        .expect("update");

    let member = TeamStore::load(&manager.team_dir)
        .expect("load")
        .config
        .member_by_id("tm-001")
        .expect("member")
        .clone();
    assert_eq!(member.session_id.as_deref(), Some("sess-42"));
    assert_eq!(member.last_spawn_error.as_deref(), Some("boom"));
    // The accompanying fields land without moving the status.
    assert_eq!(member.status, MemberStatus::Working);
}

#[test]
fn fixture_member_record_is_unchanged_by_the_shared_fixture() {
    // Guard against a future edit to `manager_support` that would silently
    // change the starting status these tests assume.
    let (manager, _tmp, _bus) = team_manager("fixture-team", "lead-1", true);
    let member: TeamMember = TeamStore::load(&manager.team_dir)
        .expect("load")
        .config
        .member_by_id("tm-001")
        .expect("member")
        .clone();
    assert_eq!(member.agent_id, "tm-001");
    // `Working`, not `Spawning`: a freshly created team has nothing spawning
    // its member, and `Spawning` has no edge to the parked or shutdown
    // statuses this suite's callers move members through.
    assert_eq!(member.status, MemberStatus::Working);
}
