//! `TeamManager::resume_teammate` accepts `Blocked` as well as `Suspended`
//! (spec `teamssetup` FR-010, F-06, F-25).
//!
//! Before T-016 the resume verb bailed unless the member was exactly
//! `Suspended`, so a `Blocked` member had no operator escape: nothing in
//! `ragent-agent` unblocked it, and `team_cleanup` refuses a live member (F-22),
//! so a blocked member wedged its team for good.
//!
//! Two properties are pinned here:
//!
//! 1. **The verb covers both parked statuses.** `Suspended -> Idle` and
//!    `Blocked -> Idle` both succeed. `Blocked` carries no reason field, so the
//!    move to `Idle` is the clearing of the block; the test asserts the member
//!    is parked-as-blocked before the call and idle-with-no-task after it.
//! 2. **The move goes through the single status writer.** Each resume publishes
//!    exactly one `TeammateStatusChanged` carrying `from`, `to` and the cause
//!    `resumed`, plus one coarse `TeammateResumed`. A site that reverted to a
//!    direct `member.status = ...` write would lose the status-change event.
//!
//! The fixture is [`manager_support::team_manager`], whose member `alice`
//! (`tm-001`) starts `Working`; each test moves her into the status under test
//! through the writer, so the setup itself cannot fake the outcome.

use ragent_agent::event::Event;
use ragent_agent::team::TeamStore;
use ragent_agent::team::config::{MemberStatus, TransitionCause};

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

/// Read `agent_id`'s persisted `current_task_id` through the team store.
fn task_of(manager: &ragent_agent::team::TeamManager, agent_id: &str) -> Option<String> {
    TeamStore::load(&manager.team_dir)
        .expect("load team store")
        .config
        .member_by_id(agent_id)
        .expect("member present")
        .current_task_id
        .clone()
}

/// Drain the bus into `(status_changes, coarse_resumed_count)`.
///
/// `status_changes` is every `TeammateStatusChanged` as
/// `(agent_id, from, to, cause)` in publish order.
fn drain_events(
    rx: &mut tokio::sync::broadcast::Receiver<Event>,
) -> (Vec<(String, String, String, String)>, usize) {
    let mut changes = Vec::new();
    let mut resumed = 0usize;
    while let Ok(event) = rx.try_recv() {
        match event {
            Event::TeammateStatusChanged {
                agent_id,
                from,
                to,
                cause,
                ..
            } => changes.push((agent_id, from, to, cause)),
            Event::TeammateResumed { .. } => resumed += 1,
            _ => {}
        }
    }
    (changes, resumed)
}

// -- Blocked is resumable ----------------------------------------------------

#[tokio::test]
async fn resume_of_a_blocked_member_returns_it_to_idle_through_the_writer() {
    let (manager, _tmp, bus) = team_manager("t016-blocked", "lead-1", true);
    // Park the member on a dependency the way the runtime does: a claimed task
    // stays recorded while the member is blocked, and leaving `Working` for
    // `Blocked` keeps it (only `Idle`/`Stopped`/`Failed` clear it).
    manager
        .update_member("tm-001", |m| {
            m.current_task_id = Some("task-007".to_string());
        })
        .expect("record a claimed task");
    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Blocked,
            TransitionCause::DependencyBlocked,
        )
        .expect("block the member")
        .expect("working -> blocked is a move");

    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Blocked);
    assert_eq!(
        task_of(&manager, "tm-001").as_deref(),
        Some("task-007"),
        "a blocked member still holds the task it was parked on"
    );

    let mut rx = bus.subscribe();
    manager
        .resume_teammate("tm-001")
        .await
        .expect("a blocked member must be resumable (FR-010)");

    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Idle);
    assert_eq!(
        task_of(&manager, "tm-001"),
        None,
        "resuming releases the block's task: idle members hold no work"
    );

    let (changes, resumed) = drain_events(&mut rx);
    assert_eq!(
        changes,
        vec![(
            "tm-001".to_string(),
            "blocked".to_string(),
            "idle".to_string(),
            "resumed".to_string(),
        )],
        "the block exit must be one writer transition carrying blocked -> idle"
    );
    assert_eq!(resumed, 1, "exactly one coarse TeammateResumed event");
}

// -- Suspended still works ---------------------------------------------------

#[tokio::test]
async fn resume_of_a_suspended_member_still_returns_it_to_idle() {
    let (manager, _tmp, bus) = team_manager("t016-suspended", "lead-1", true);
    manager
        .suspend_teammate("tm-001")
        .await
        .expect("suspend the member");
    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Suspended);

    let mut rx = bus.subscribe();
    manager
        .resume_teammate("tm-001")
        .await
        .expect("a suspended member must stay resumable");

    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Idle);
    let (changes, resumed) = drain_events(&mut rx);
    assert_eq!(
        changes,
        vec![(
            "tm-001".to_string(),
            "suspended".to_string(),
            "idle".to_string(),
            "resumed".to_string(),
        )],
        "the suspend/resume symmetry must not regress while Blocked is added"
    );
    assert_eq!(resumed, 1);
}

// -- Both parked statuses reach the same destination -------------------------

#[tokio::test]
async fn blocked_and_suspended_resume_to_the_same_idle_destination() {
    let (manager, _tmp, _bus) = team_manager("t016-destination", "lead-1", true);

    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Blocked,
            TransitionCause::DependencyBlocked,
        )
        .expect("block");
    manager
        .resume_teammate("tm-001")
        .await
        .expect("resume blocked");
    let after_blocked = status_of(&manager, "tm-001");

    manager
        .suspend_teammate("tm-001")
        .await
        .expect("suspend from idle");
    manager
        .resume_teammate("tm-001")
        .await
        .expect("resume suspended");
    let after_suspended = status_of(&manager, "tm-001");

    assert_eq!(after_blocked, MemberStatus::Idle);
    assert_eq!(after_suspended, MemberStatus::Idle);
}

// -- Every other status is refused -------------------------------------------

#[tokio::test]
async fn resume_of_a_working_member_is_refused_and_changes_nothing() {
    let (manager, _tmp, bus) = team_manager("t016-working", "lead-1", true);
    // The fixture member starts `Working`.
    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Working);

    let mut rx = bus.subscribe();
    let err = manager
        .resume_teammate("tm-001")
        .await
        .expect_err("a working member has nothing to resume");

    let msg = err.to_string();
    assert!(
        msg.contains("tm-001"),
        "the refusal names the member: {msg}"
    );
    assert!(
        msg.contains("working"),
        "the refusal names the current status: {msg}"
    );
    assert!(
        msg.contains("suspended or blocked"),
        "the refusal names the statuses resume accepts: {msg}"
    );

    assert_eq!(
        status_of(&manager, "tm-001"),
        MemberStatus::Working,
        "a refused resume must not move the member"
    );
    let (changes, resumed) = drain_events(&mut rx);
    assert!(changes.is_empty(), "a refused resume publishes nothing");
    assert_eq!(resumed, 0);
}

#[tokio::test]
async fn resume_of_a_stopped_member_is_refused() {
    let (manager, _tmp, _bus) = team_manager("t016-stopped", "lead-1", true);
    manager
        .shutdown_teammate("tm-001", false)
        .await
        .expect("immediate shutdown");
    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Stopped);

    let err = manager
        .resume_teammate("tm-001")
        .await
        .expect_err("a stopped member is revived by a respawn, not a resume");
    assert!(
        err.to_string().contains("stopped"),
        "the refusal names the terminal status: {err}"
    );
    assert_eq!(status_of(&manager, "tm-001"), MemberStatus::Stopped);
}

#[tokio::test]
async fn resume_of_an_unknown_member_is_refused() {
    let (manager, _tmp, _bus) = team_manager("t016-unknown", "lead-1", true);

    let err = manager
        .resume_teammate("tm-999")
        .await
        .expect_err("an unknown member has nothing to resume");
    assert!(
        err.to_string().contains("no teammate 'tm-999'"),
        "the refusal names the unknown id: {err}"
    );
}

// -- The resume verb is not a second block producer --------------------------

#[tokio::test]
async fn resume_does_not_re_block_a_member_that_still_has_dependencies() {
    // FR-010 is the operator verb, not the auto-unblock rule (FR-011, T-017):
    // resuming a blocked member moves it to `Idle` unconditionally, and it is
    // the dependency machinery that may park it again. Pinned so T-017 cannot
    // quietly move the decision into `resume_teammate`.
    let (manager, _tmp, _bus) = team_manager("t016-no-reblock", "lead-1", true);
    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Blocked,
            TransitionCause::DependencyBlocked,
        )
        .expect("block");

    manager
        .resume_teammate("tm-001")
        .await
        .expect("operator resume");
    assert_eq!(
        status_of(&manager, "tm-001"),
        MemberStatus::Idle,
        "resume leaves the member idle; re-blocking is the dependency rule's job"
    );
}
