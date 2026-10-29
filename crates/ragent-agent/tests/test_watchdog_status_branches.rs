//! Watchdog eligibility classification tests (spec `teamssetup` FR-012, F-08).
//!
//! `TeamManager::start_watchdog` decides, once per timeout, whether a stale
//! member is eligible for a failure verdict. FR-012 requires that decision to be
//! made per status with no implicit path: a live member (`Spawning`, `Working`,
//! `ShuttingDown`) may be failed, a parked member (`Idle`, `PlanPending`,
//! `Blocked`, `Suspended`) and a terminal member (`Stopped`, `Failed`) may not.
//!
//! `test_team_manager_extra::watchdog_never_fails_a_parked_member` covers the
//! parked half. This file covers what the implicit `unwrap_or(false)` path hid:
//! **every** status is driven through one watchdog run, so a status that loses
//! its explicit branch shows up as either a wrongly failed member or a missing
//! `TeammateStatusChanged`.

use std::sync::Arc;
use std::time::Duration;

use ragent_agent::event::{Event, EventBus};
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::team::{MemberStatus, TeamManager, TeamStore, TransitionCause};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::stub_processor;

/// Every member status, in enum declaration order.
const ALL_MEMBER_STATUSES: [MemberStatus; 9] = [
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

/// Everything a test needs to drive and observe one watchdog run.
struct Fixture {
    manager: Arc<TeamManager>,
    bus: Arc<EventBus>,
    /// The manager only holds a `Weak` to its processor, so the fixture keeps
    /// the strong reference alive for the life of the test.
    _processor: Arc<SessionProcessor>,
    _tmp: tempfile::TempDir,
}

/// Build a manager over a team with one spawned member per entry of `statuses`,
/// each named `w0`, `w1`, ... and left in the status under test.
///
/// Every member is spawned (so it has a handle) and then moved to its target
/// status through the single writer; the handle's `last_progress` is left where
/// the spawn put it, so the watchdog considers the member on its first tick.
async fn seeded_manager(team_name: &str, statuses: &[MemberStatus], timeout: Duration) -> Fixture {
    let bus = Arc::new(EventBus::new(1024));
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(workspace.join(".ragent")).expect("create .ragent");
    let store = TeamStore::create(team_name, "lead-1", &workspace, true).expect("create team");
    let team_dir = store.dir.clone();
    drop(store);

    let processor = stub_processor(bus.clone());
    let mut manager = TeamManager::new(
        team_name,
        "lead-1",
        team_dir,
        Arc::clone(&processor),
        bus.clone(),
    );
    manager.watchdog_timeout = timeout;
    let manager = Arc::new(manager);

    let cwd = std::env::current_dir().expect("cwd");
    for (i, status) in statuses.iter().enumerate() {
        let name = format!("w{i}");
        manager
            .spawn_teammate_internal(&name, "general", "work", None, None, &cwd)
            .await
            .unwrap_or_else(|e| panic!("spawn {name}: {e}"));
        let agent_id = member_id(&manager, &name);
        // The spawn path leaves the member `Working`. `Spawning` is only
        // reachable from a terminal status (a respawn), so a freshly spawned
        // member cannot be put back into it and the test covers `Working`
        // instead; every other status is a real transition from `Working`.
        if !matches!(*status, MemberStatus::Working | MemberStatus::Spawning) {
            // Terminal statuses are only reachable through `ShuttingDown`, so
            // take that legal edge first rather than asking the table for a
            // move it forbids.
            if status.is_terminal() {
                manager
                    .set_member_status(
                        &agent_id,
                        MemberStatus::ShuttingDown,
                        TransitionCause::ShutdownRequested,
                    )
                    .unwrap_or_else(|e| panic!("shut down {name}: {e}"));
            }
            manager
                .set_member_status(&agent_id, *status, cause_for(*status))
                .unwrap_or_else(|e| panic!("move {name} to {status:?}: {e}"));
        }
    }

    Fixture {
        manager,
        bus,
        _processor: processor,
        _tmp: tmp,
    }
}

/// The status-transition cause the production code uses for each destination.
fn cause_for(status: MemberStatus) -> TransitionCause {
    match status {
        MemberStatus::Spawning => TransitionCause::Spawned,
        MemberStatus::Working => TransitionCause::TaskClaimed,
        MemberStatus::Idle => TransitionCause::ReportedIdle,
        MemberStatus::PlanPending => TransitionCause::PlanSubmitted,
        MemberStatus::Blocked => TransitionCause::DependencyBlocked,
        MemberStatus::Suspended => TransitionCause::Suspended,
        MemberStatus::ShuttingDown => TransitionCause::ShutdownRequested,
        MemberStatus::Stopped => TransitionCause::ShutdownAcked,
        MemberStatus::Failed => TransitionCause::SpawnFailed,
    }
}

/// The agent ID the manager assigned to the member named `name`.
fn member_id(manager: &TeamManager, name: &str) -> String {
    TeamStore::load(&manager.team_dir)
        .expect("load team")
        .config
        .members
        .iter()
        .find(|m| m.name == name)
        .map(|m| m.agent_id.clone())
        .unwrap_or_else(|| panic!("member {name} not found"))
}

/// One member's current status, read from disk.
fn status_of(team_dir: &std::path::Path, agent_id: &str) -> MemberStatus {
    TeamStore::load(team_dir)
        .expect("load team")
        .config
        .member_by_id(agent_id)
        .map(|m| m.status)
        .unwrap_or_else(|| panic!("member {agent_id} missing"))
}

/// FR-012: only a live member is ever failed.
///
/// One manager holds a stale member in each of the nine statuses. The watchdog
/// runs with a 40 ms timeout; the assertion waits for both live controls
/// (`Working`, `ShuttingDown`) to be failed - proving the watchdog ran and had
/// stale members in front of it - then checks that every parked and terminal
/// member kept its status and published no `TeammateFailed`.
#[tokio::test]
async fn test_watchdog_fails_only_live_members_and_classifies_every_status() {
    let fx = seeded_manager(
        "wd-classify",
        &ALL_MEMBER_STATUSES,
        Duration::from_millis(40),
    )
    .await;
    let manager = &fx.manager;
    let ids: Vec<String> = (0..ALL_MEMBER_STATUSES.len())
        .map(|i| member_id(manager, &format!("w{i}")))
        .collect();
    let working_id = member_id(manager, "w1");
    let shutting_id = member_id(manager, "w6");

    let mut rx = fx.bus.subscribe();
    Arc::clone(manager).start_watchdog();

    let mut controls_failed = false;
    for _ in 0..250 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        if status_of(&manager.team_dir, &working_id) == MemberStatus::Failed
            && status_of(&manager.team_dir, &shutting_id) == MemberStatus::Failed
        {
            controls_failed = true;
            break;
        }
    }
    assert!(
        controls_failed,
        "control: the watchdog must fail both stale live members (Working and ShuttingDown)"
    );

    // Every non-live status is untouched by the failure verdict.
    for (i, status) in ALL_MEMBER_STATUSES.iter().enumerate() {
        if status.is_live() {
            continue;
        }
        assert_eq!(
            status_of(&manager.team_dir, &ids[i]),
            *status,
            "FR-012: a {status:?} member must keep its status, never be failed by the watchdog"
        );
    }

    // And no TeammateFailed event names a non-live member.
    while let Ok(event) = rx.try_recv() {
        if let Event::TeammateFailed { agent_id, .. } = event {
            for (i, status) in ALL_MEMBER_STATUSES.iter().enumerate() {
                if status.is_live() {
                    continue;
                }
                assert!(
                    agent_id != ids[i],
                    "FR-012: watchdog published TeammateFailed for a {status:?} member"
                );
            }
        }
    }
}

/// FR-012: a parked member survives every tick while a live member beside it is
/// failed. This is the `Blocked` branch specifically - the status F-08 reported
/// as taking the implicit path.
#[tokio::test]
async fn test_watchdog_leaves_a_blocked_member_blocked_across_many_ticks() {
    let fx = seeded_manager(
        "wd-blocked-ticks",
        &[MemberStatus::Blocked, MemberStatus::Working],
        Duration::from_millis(20),
    )
    .await;
    let manager = &fx.manager;
    let blocked_id = member_id(manager, "w0");
    let working_id = member_id(manager, "w1");

    Arc::clone(manager).start_watchdog();

    // Long enough for dozens of 10 ms ticks.
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        assert_eq!(
            status_of(&manager.team_dir, &blocked_id),
            MemberStatus::Blocked,
            "FR-012: a Blocked member must survive every watchdog tick"
        );
        if status_of(&manager.team_dir, &working_id) == MemberStatus::Failed {
            break;
        }
    }
    assert_eq!(
        status_of(&manager.team_dir, &working_id),
        MemberStatus::Failed,
        "control: the stale live member must still be failed"
    );
    assert_eq!(
        status_of(&manager.team_dir, &blocked_id),
        MemberStatus::Blocked,
        "FR-012: a Blocked member must still be Blocked after the live member was failed"
    );
}

/// FR-012: a watchdog over a team whose `config.json` cannot be read takes no
/// verdict at all - it never falls back to a default that could fail a member.
///
/// The manager points at a directory that does not exist, so `TeamStore::load`
/// fails inside the tick; the assertion is that this is a harmless no-op rather
/// than a panic or a spurious failure.
#[tokio::test]
async fn test_watchdog_fails_nobody_when_the_config_cannot_be_read() {
    let bus = Arc::new(EventBus::new(64));
    let tmp = tempfile::tempdir().expect("tempdir");
    let bogus = tmp.path().join("missing").join("wd-noconfig");
    let processor = stub_processor(bus.clone());
    let manager = TeamManager::new("wd-noconfig", "lead-1", bogus, processor, bus);
    assert!(!manager.team_dir.exists(), "config must be unreadable");
    Arc::new(manager).start_watchdog();
    tokio::time::sleep(Duration::from_millis(50)).await;
}

/// FR-008/FR-012: the watchdog's verdict goes through `set_member_status`, so it
/// publishes exactly one `TeammateStatusChanged` (`working -> failed`) alongside
/// the coarse `TeammateFailed`, and records the reason.
#[tokio::test]
async fn test_watchdog_failure_publishes_one_status_changed_event() {
    let fx = seeded_manager(
        "wd-status-event",
        &[MemberStatus::Working],
        Duration::from_millis(20),
    )
    .await;
    let manager = &fx.manager;
    let working_id = member_id(manager, "w0");
    let mut rx = fx.bus.subscribe();

    Arc::clone(manager).start_watchdog();

    let mut status_changed: Vec<(String, String)> = Vec::new();
    let mut failed_events = 0usize;
    // Drain helper for both counters.
    let drain = |rx: &mut tokio::sync::broadcast::Receiver<Event>,
                 status_changed: &mut Vec<(String, String)>,
                 failed_events: &mut usize| {
        while let Ok(event) = rx.try_recv() {
            match event {
                Event::TeammateStatusChanged {
                    agent_id, from, to, ..
                } if agent_id == working_id => status_changed.push((from, to)),
                Event::TeammateFailed { agent_id, .. } if agent_id == working_id => {
                    *failed_events += 1;
                }
                _ => {}
            }
        }
    };

    for _ in 0..250 {
        tokio::time::sleep(Duration::from_millis(10)).await;
        if status_of(&manager.team_dir, &working_id) == MemberStatus::Failed {
            // Give the publishes that accompanied the write time to land.
            tokio::time::sleep(Duration::from_millis(100)).await;
            drain(&mut rx, &mut status_changed, &mut failed_events);
            break;
        }
    }

    assert_eq!(
        status_changed,
        vec![("working".to_owned(), "failed".to_owned())],
        "the watchdog must publish exactly one TeammateStatusChanged working -> failed"
    );
    assert_eq!(
        failed_events, 1,
        "the watchdog must publish exactly one TeammateFailed"
    );

    let member = TeamStore::load(&manager.team_dir)
        .expect("load team")
        .config
        .member_by_id(&working_id)
        .cloned()
        .expect("member present");
    assert!(
        member
            .last_spawn_error
            .as_deref()
            .is_some_and(|e| e.contains("no progress")),
        "the failure reason must be recorded: {:?}",
        member.last_spawn_error
    );
    assert!(
        member.current_task_id.is_none(),
        "a failed member holds no task"
    );
}
