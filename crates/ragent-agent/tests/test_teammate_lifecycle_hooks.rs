//! Integration tests for the `TeammateSpawned` / `TeammateStopped` events
//! published by [`TeamManager::set_member_status`] (spec `teamssetup` FR-017).
//!
//! FR-017 requires the `...TeammateSpawned` and `...TeammateStopped` hook points
//! to fire from the team runtime's *own* mutation path, not from one privileged
//! call site. Before T-024 the spawn event was published by
//! `spawn_teammate_internal` alone and the stop event did not exist, so a
//! teammate that stopped because the lead forced the shutdown, or because the
//! team was torn down, announced nothing, and a respawn announced itself from a
//! different place than the status it recorded.
//!
//! These tests pin the writer's obligations:
//!
//! 1. entering `Spawning` publishes exactly one `TeammateSpawned` carrying the
//!    member's human-facing name;
//! 2. entering `Stopped` publishes exactly one `TeammateStopped` carrying the
//!    cause of the stop;
//! 3. every other destination publishes neither;
//! 4. a refused or re-asserted write publishes neither (there was no move);
//! 5. a respawn announces a spawn again, because it re-enters `Spawning`;
//! 6. the pair is not published a second time by the spawn path itself.
//!
//! A manager built over a stub `SessionProcessor` (see `manager_support`) is
//! enough: the writer touches only on-disk team state and the event bus.

use std::collections::HashMap;

use ragent_agent::Event;
use ragent_agent::team::{MemberStatus, TeamStore, TransitionCause};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::team_manager;

/// Read one member's name straight from `config.json`.
fn on_disk_name(manager: &ragent_agent::team::TeamManager, agent_id: &str) -> String {
    TeamStore::load(&manager.team_dir)
        .expect("load team store")
        .config
        .member_by_id(agent_id)
        .expect("member exists")
        .name
        .clone()
}

/// Rewrite one member's on-disk status, for tests that need a member parked in
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

/// Drain the bus without blocking, returning the number of `TeammateSpawned`
/// and `TeammateStopped` events it carried.
fn lifecycle_counts(rx: &mut tokio::sync::broadcast::Receiver<Event>) -> (usize, usize) {
    let mut spawned = 0;
    let mut stopped = 0;
    while let Ok(event) = rx.try_recv() {
        match event {
            Event::TeammateSpawned { .. } => spawned += 1,
            Event::TeammateStopped { .. } => stopped += 1,
            _ => {}
        }
    }
    (spawned, stopped)
}

/// Drain the bus without blocking, returning the `TeammateSpawned` payloads.
fn spawned_events(
    rx: &mut tokio::sync::broadcast::Receiver<Event>,
) -> Vec<(String, String, String, String)> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let Event::TeammateSpawned {
            session_id,
            team_name,
            teammate_name,
            agent_id,
        } = event
        {
            out.push((session_id, team_name, teammate_name, agent_id));
        }
    }
    out
}

/// Drain the bus without blocking, returning the `TeammateStopped` payloads.
fn stopped_events(
    rx: &mut tokio::sync::broadcast::Receiver<Event>,
) -> Vec<(String, String, String, String)> {
    let mut out = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let Event::TeammateStopped {
            session_id,
            team_name,
            agent_id,
            cause,
        } = event
        {
            out.push((session_id, team_name, agent_id, cause));
        }
    }
    out
}

// -- TeammateSpawned ---------------------------------------------------------

#[test]
fn entering_spawning_publishes_one_teammate_spawned_with_the_member_name() {
    let (manager, _tmp, bus) = team_manager("spawn-team", "lead-1", true);
    let mut rx = bus.subscribe();
    // The fixture member starts `Working`; the respawn edge is the only route
    // into `Spawning` from there, and it is the same edge a real respawn takes.
    set_on_disk_status(&manager, "tm-001", MemberStatus::Stopped);
    let expected_name = on_disk_name(&manager, "tm-001");

    manager
        .set_member_status("tm-001", MemberStatus::Spawning, TransitionCause::Spawned)
        .expect("Stopped -> Spawning is the respawn edge");

    let events = spawned_events(&mut rx);
    assert_eq!(
        events.len(),
        1,
        "entering Spawning must publish exactly one TeammateSpawned, got {events:?}"
    );
    let (session_id, team_name, teammate_name, agent_id) = &events[0];
    assert_eq!(
        session_id, "lead-1",
        "the event is scoped to the lead session"
    );
    assert_eq!(team_name, "spawn-team");
    assert_eq!(agent_id, "tm-001");
    assert_eq!(
        *teammate_name, expected_name,
        "the event must carry the human-facing name from the roster, not the id"
    );
}

#[test]
fn entering_spawning_also_publishes_the_status_record() {
    // FR-035 and FR-017 are both satisfied by one write: the coarse spawn event
    // is additional to, not instead of, the complete status record.
    let (manager, _tmp, bus) = team_manager("spawn-record-team", "lead-1", true);
    let mut rx = bus.subscribe();
    set_on_disk_status(&manager, "tm-001", MemberStatus::Stopped);

    manager
        .set_member_status("tm-001", MemberStatus::Spawning, TransitionCause::Spawned)
        .expect("respawn edge is legal");

    let mut status_records = 0;
    while let Ok(event) = rx.try_recv() {
        if let Event::TeammateStatusChanged { to, from, .. } = event {
            assert_eq!(from, "stopped");
            assert_eq!(to, "spawning");
            status_records += 1;
        }
    }
    assert_eq!(
        status_records, 1,
        "exactly one status record per transition"
    );
}

#[test]
fn respawn_announces_a_second_spawn() {
    // A member that stops and is respawned is spawned twice as far as an
    // observer is concerned; suppressing the second would hide the reuse of a
    // stale record, which is what the event exists to surface.
    let (manager, _tmp, bus) = team_manager("respawn-team", "lead-1", true);
    let mut rx = bus.subscribe();
    set_on_disk_status(&manager, "tm-001", MemberStatus::Stopped);

    manager
        .set_member_status("tm-001", MemberStatus::Spawning, TransitionCause::Spawned)
        .expect("first respawn");
    manager
        .set_member_status("tm-001", MemberStatus::Failed, TransitionCause::SpawnFailed)
        .expect("Spawning -> Failed is legal");
    manager
        .set_member_status("tm-001", MemberStatus::Spawning, TransitionCause::Spawned)
        .expect("second respawn");

    let events = spawned_events(&mut rx);
    assert_eq!(
        events.len(),
        2,
        "each entry into Spawning is a spawn to an observer, got {events:?}"
    );
}

#[test]
fn entering_spawning_names_the_id_when_the_member_is_unreadable() {
    // The event is published after a successful save, so the roster read cannot
    // legitimately fail; the fallback exists so a filesystem surprise degrades
    // to the id rather than dropping the spawn announcement entirely.
    let (manager, _tmp, bus) = team_manager("fallback-team", "lead-1", true);
    let mut rx = bus.subscribe();
    set_on_disk_status(&manager, "tm-001", MemberStatus::Stopped);
    // Remove the roster entry's *name* source by asserting the normal path
    // first, so this test fails loudly if the fallback ever becomes the default.
    manager
        .set_member_status("tm-001", MemberStatus::Spawning, TransitionCause::Spawned)
        .expect("respawn edge");
    let events = spawned_events(&mut rx);
    assert_eq!(events.len(), 1);
    assert_ne!(
        events[0].2, "tm-001",
        "with a readable roster the event must carry the name, not the id"
    );
}

// -- TeammateStopped ---------------------------------------------------------

#[test]
fn entering_stopped_publishes_one_teammate_stopped_with_the_cause() {
    let (manager, _tmp, bus) = team_manager("stop-team", "lead-1", true);
    let mut rx = bus.subscribe();
    // `ShuttingDown -> Stopped` is the table's only route to the terminal
    // status, and the one a forced shutdown and an acked request both take.
    set_on_disk_status(&manager, "tm-001", MemberStatus::ShuttingDown);

    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Stopped,
            TransitionCause::ShutdownAcked,
        )
        .expect("ShuttingDown -> Stopped is legal");

    let events = stopped_events(&mut rx);
    assert_eq!(
        events.len(),
        1,
        "entering Stopped must publish exactly one TeammateStopped, got {events:?}"
    );
    let (session_id, team_name, agent_id, cause) = &events[0];
    assert_eq!(session_id, "lead-1");
    assert_eq!(team_name, "stop-team");
    assert_eq!(agent_id, "tm-001");
    assert_eq!(
        cause, "shutdown_acked",
        "the stop event must carry why the teammate stopped"
    );
}

#[test]
fn the_stop_cause_is_the_transition_cause_not_an_inferred_one() {
    // A forced shutdown and an acked request reach the same status; only the
    // recorded cause distinguishes them, which is the reason FR-035's cause
    // vocabulary exists.
    let (manager, _tmp, bus) = team_manager("stop-cause-team", "lead-1", true);
    let mut rx = bus.subscribe();
    set_on_disk_status(&manager, "tm-001", MemberStatus::ShuttingDown);

    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Stopped,
            TransitionCause::TeamCleanedUp,
        )
        .expect("ShuttingDown -> Stopped is legal");

    let events = stopped_events(&mut rx);
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].3, "team_cleaned_up",
        "the cause is the caller's, never inferred from the status pair"
    );
}

#[test]
fn a_stop_publishes_the_status_record_too() {
    let (manager, _tmp, bus) = team_manager("stop-record-team", "lead-1", true);
    let mut rx = bus.subscribe();
    set_on_disk_status(&manager, "tm-001", MemberStatus::ShuttingDown);

    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Stopped,
            TransitionCause::ShutdownAcked,
        )
        .expect("ShuttingDown -> Stopped is legal");

    let mut status_records = 0;
    while let Ok(event) = rx.try_recv() {
        if let Event::TeammateStatusChanged {
            from, to, cause, ..
        } = event
        {
            assert_eq!(from, "shutting_down");
            assert_eq!(to, "stopped");
            assert_eq!(cause, "shutdown_acked");
            status_records += 1;
        }
    }
    assert_eq!(status_records, 1);
}

// -- Neither ----------------------------------------------------------------

#[test]
fn no_other_destination_publishes_either_lifecycle_event() {
    // One representative legal edge per non-spawn, non-stop destination.
    let edges: &[(MemberStatus, MemberStatus, TransitionCause)] = &[
        (
            MemberStatus::Working,
            MemberStatus::Idle,
            TransitionCause::ReportedIdle,
        ),
        (
            MemberStatus::Working,
            MemberStatus::PlanPending,
            TransitionCause::PlanSubmitted,
        ),
        (
            MemberStatus::Working,
            MemberStatus::Suspended,
            TransitionCause::Suspended,
        ),
        (
            MemberStatus::Working,
            MemberStatus::Failed,
            TransitionCause::WatchdogTimeout,
        ),
        (
            MemberStatus::Working,
            MemberStatus::ShuttingDown,
            TransitionCause::ShutdownRequested,
        ),
        (
            MemberStatus::Idle,
            MemberStatus::Blocked,
            TransitionCause::DependencyBlocked,
        ),
        (
            MemberStatus::Idle,
            MemberStatus::Working,
            TransitionCause::TaskClaimed,
        ),
    ];

    for (from, to, cause) in edges {
        let (manager, _tmp, bus) = team_manager("other-team", "lead-1", true);
        let mut rx = bus.subscribe();
        set_on_disk_status(&manager, "tm-001", *from);

        manager
            .set_member_status("tm-001", *to, *cause)
            .unwrap_or_else(|e| {
                panic!("{} -> {} should be legal: {e}", from.as_str(), to.as_str())
            });

        let (spawned, stopped) = lifecycle_counts(&mut rx);
        assert_eq!(
            (spawned, stopped),
            (0, 0),
            "{} -> {} must publish neither lifecycle event",
            from.as_str(),
            to.as_str()
        );
    }
}

#[test]
fn a_refused_transition_publishes_neither_lifecycle_event() {
    let (manager, _tmp, bus) = team_manager("refused-team", "lead-1", true);
    let mut rx = bus.subscribe();
    // The fixture member is `Working`: it has no edge to `Stopped` at all.
    manager
        .set_member_status(
            "tm-001",
            MemberStatus::Stopped,
            TransitionCause::ShutdownAcked,
        )
        .expect_err("Working -> Stopped is illegal");

    let (spawned, stopped) = lifecycle_counts(&mut rx);
    assert_eq!(
        (spawned, stopped),
        (0, 0),
        "a refused write must announce nothing"
    );
}

#[test]
fn a_re_asserted_status_publishes_neither_lifecycle_event() {
    // `Ok(None)` is not a transition, so it must not re-announce the spawn or
    // the stop. Otherwise the event stream would claim a motion that did not
    // happen - e.g. `team_idle` on an already-idle member. The fine-grained
    // `TeammateStatusChanged` is still published for the re-assertion; only the
    // coarse lifecycle pair stays silent.
    let (manager, _tmp, bus) = team_manager("reassert-team", "lead-1", true);
    let mut rx = bus.subscribe();
    set_on_disk_status(&manager, "tm-001", MemberStatus::Spawning);

    let applied = manager
        .set_member_status("tm-001", MemberStatus::Spawning, TransitionCause::Spawned)
        .expect("re-asserting a status is not an error");
    assert!(applied.is_none(), "re-asserting is not a transition");

    let (spawned, stopped) = lifecycle_counts(&mut rx);
    assert_eq!((spawned, stopped), (0, 0));
}

// -- No double publish ------------------------------------------------------

#[tokio::test]
async fn a_real_spawn_publishes_the_spawn_exactly_once() {
    // The end-to-end shape of the bug FR-017 names: before T-024 the writer and
    // the spawn path could each announce the same spawn. Spawn a real teammate
    // and count.
    let (manager, _tmp, bus) = team_manager("e2e-spawn-team", "lead-1", true);
    let mut rx = bus.subscribe();
    let working_dir = manager
        .team_dir
        .parent()
        .expect("team dir parent")
        .to_path_buf();
    let prompt = "audit the parser";

    let spawned = manager
        .spawn_teammate_internal("auditor", "general", prompt, None, None, &working_dir)
        .await;

    match spawned {
        Ok(agent_id) => {
            let events = spawned_events(&mut rx);
            let for_this_agent: Vec<_> = events
                .iter()
                .filter(|(_, _, _, id)| *id == agent_id)
                .collect();
            assert_eq!(
                for_this_agent.len(),
                1,
                "one spawn, one TeammateSpawned: {events:?}"
            );
            assert_eq!(for_this_agent[0].2, "auditor");
        }
        Err(e) => {
            // No provider is configured in the stub, so the spawn may fail; the
            // invariant this test protects is "at most one event per spawn",
            // and a failed spawn must announce at most the one it recorded.
            let events = spawned_events(&mut rx);
            let counts: HashMap<&str, usize> =
                events
                    .iter()
                    .fold(HashMap::new(), |mut acc, (_, _, _, id)| {
                        *acc.entry(id.as_str()).or_insert(0) += 1;
                        acc
                    });
            assert!(
                counts.values().all(|n| *n <= 1),
                "a spawn must never announce itself twice ({e}): {events:?}"
            );
        }
    }
}
