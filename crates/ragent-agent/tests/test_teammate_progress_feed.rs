//! A live turn is progress, whoever is watching (spec `teamssetup` FR-012).
//!
//! The watchdog fails a `Working` member whose `last_progress` is older than
//! `watchdog_timeout`. That marker used to be refreshed only by the TUI, from
//! the coarse team events it consumed (`TeammateIdle`, `TeamTaskClaimed`, …).
//! A teammate driven headlessly - a server-hosted team, an automation run, a
//! lead with no panel open - published none of those, so its marker never moved:
//! the watchdog failed it after the window while its agent loop was still in a
//! turn, and the teammate then finished the work it had been declared dead in
//! the middle of.
//!
//! The runtime now supplies the evidence itself:
//! [`TeamManager::watch_teammate_progress`] subscribes to the bus for the length
//! of each turn and calls [`TeamManager::record_progress`] for every event that
//! carries the teammate's child session id. These tests pin that contract
//! without a model, a provider, or a turn:
//!
//! 1. a teammate's own session events keep it alive across a timeout it would
//!    otherwise trip;
//! 2. another session's events do not - the feed is scoped to the child session,
//!    so a busy lead cannot mask a wedged teammate;
//! 3. the feed stops when the turn does, so a finished teammate is not kept
//!    alive by a listener that outlives its loop.
//!
//! Every test uses its own `tempfile::tempdir()`; nothing is written into the
//! repository.

use std::sync::Arc;
use std::time::Duration;

use ragent_agent::event::{Event, EventBus};
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::team::{MemberStatus, TeamManager, TeamStore};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::stub_processor;

/// A spawned, `Working` teammate whose loop never runs an agent turn.
///
/// The handle the spawn registered carries no agent-loop task (the fixture
/// starts none), which is enough for the watchdog: it judges a member by
/// `last_progress` and its on-disk status, and it is those two the feed under
/// test moves.
struct Fixture {
    manager: Arc<TeamManager>,
    bus: Arc<EventBus>,
    agent_id: String,
    session_id: String,
    /// The manager only holds a `Weak` to its processor.
    _processor: Arc<SessionProcessor>,
    _tmp: tempfile::TempDir,
}

async fn fixture(team_name: &str, timeout: Duration) -> Fixture {
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
    let agent_id = manager
        .spawn_teammate_internal("worker", "general", "work", None, None, &cwd)
        .await
        .expect("spawn the teammate");
    let session_id = TeamStore::load(&manager.team_dir)
        .expect("load team")
        .config
        .member_by_id(&agent_id)
        .and_then(|m| m.session_id.clone())
        .expect("the spawn records the child session");

    Fixture {
        manager,
        bus,
        agent_id,
        session_id,
        _processor: processor,
        _tmp: tmp,
    }
}

/// One member's current status, read from disk.
fn status_of(manager: &TeamManager, agent_id: &str) -> MemberStatus {
    TeamStore::load(&manager.team_dir)
        .expect("load team")
        .config
        .member_by_id(agent_id)
        .map(|m| m.status)
        .expect("member present")
}

/// Whether the manager's `status-history.jsonl` records a watchdog verdict for
/// `agent_id`.
fn watchdog_failed(manager: &TeamManager, agent_id: &str) -> bool {
    ragent_agent::team::status_history::read_status_history(&manager.team_dir)
        .iter()
        .any(|e| e.agent_id == agent_id && e.cause == "watchdog_timeout")
}

/// Poll until `done()` or a generous bound; returns whether it became true.
async fn wait_until<F: Fn() -> bool>(done: F, bound: Duration) -> bool {
    let deadline = tokio::time::Instant::now() + bound;
    while tokio::time::Instant::now() < deadline {
        if done() {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    done()
}

/// FR-012: a teammate's own session events keep it out of the watchdog's reach.
///
/// The timeout is short enough that a member whose marker never moved is failed
/// within the bound; the test keeps publishing `TextDelta` / `ToolCallStart`
/// events for the child session across many timeout windows and asserts the
/// member is still `Working` and was never failed. The tool event is included
/// because a long single-tool step publishes exactly two of them, at its start
/// and at its end - a feed that only listened to streaming text would lose a
/// teammate to its own build.
#[tokio::test]
async fn test_a_teammates_own_events_keep_it_alive_past_the_watchdog_window() {
    let fx = fixture("prog-lived", Duration::from_millis(60)).await;
    let _feed = fx
        .manager
        .watch_teammate_progress_for_test(&fx.agent_id, &fx.session_id);
    Arc::clone(&fx.manager).start_watchdog();

    // Five windows' worth of work, one event per 20 ms.
    for step in 0..15 {
        match step % 3 {
            0 => fx.bus.publish(Event::TextDelta {
                session_id: fx.session_id.clone(),
                text: format!("chunk {step}"),
            }),
            1 => fx.bus.publish(Event::ToolCallStart {
                session_id: fx.session_id.clone(),
                call_id: format!("call-{step}"),
                tool: "Bash".to_string(),
            }),
            _ => fx.bus.publish(Event::ReasoningDelta {
                session_id: fx.session_id.clone(),
                text: "thinking".to_string(),
            }),
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }

    assert_eq!(
        status_of(&fx.manager, &fx.agent_id),
        MemberStatus::Working,
        "FR-012: a teammate publishing its turn's events is making progress"
    );
    assert!(
        !watchdog_failed(&fx.manager, &fx.agent_id),
        "FR-012: the watchdog must never have failed a member that was working"
    );
}

/// FR-012: the feed is scoped to the teammate's own child session.
///
/// A busy lead (or a sibling) publishes constantly while this teammate is
/// silent. The feed must not take that as the teammate's progress, or a wedged
/// teammate would be kept alive indefinitely by unrelated traffic; the control
/// is the member being failed anyway.
#[tokio::test]
async fn test_another_sessions_events_do_not_count_as_the_teammates_progress() {
    let fx = fixture("prog-scoped", Duration::from_millis(40)).await;
    let _feed = fx
        .manager
        .watch_teammate_progress_for_test(&fx.agent_id, &fx.session_id);
    Arc::clone(&fx.manager).start_watchdog();

    for step in 0..30 {
        fx.bus.publish(Event::TextDelta {
            session_id: "lead-1".to_string(),
            text: format!("lead chatter {step}"),
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    assert!(
        wait_until(
            || watchdog_failed(&fx.manager, &fx.agent_id),
            Duration::from_secs(5)
        )
        .await,
        "FR-012: traffic for another session must not mask a wedged teammate"
    );
    assert_eq!(
        status_of(&fx.manager, &fx.agent_id),
        MemberStatus::Failed,
        "the silent teammate is the one the watchdog fails"
    );
}

/// FR-012: the feed ends with the turn it watches.
///
/// A listener that outlived its teammate's loop would keep a finished member's
/// marker fresh forever, and the watchdog would never report a wedged teammate
/// again. Dropping the guard (what the spawn loop does when its turn returns)
/// must stop the feed: events published after the drop no longer hold the
/// member out of the failure verdict.
#[tokio::test]
async fn test_the_progress_feed_stops_with_the_turn() {
    let fx = fixture("prog-stops", Duration::from_millis(40)).await;
    let feed = fx
        .manager
        .watch_teammate_progress_for_test(&fx.agent_id, &fx.session_id);
    drop(feed);
    Arc::clone(&fx.manager).start_watchdog();

    // The member's session is still chattering, but nothing is listening.
    for step in 0..30 {
        fx.bus.publish(Event::TextDelta {
            session_id: fx.session_id.clone(),
            text: format!("post-turn {step}"),
        });
        tokio::time::sleep(Duration::from_millis(10)).await;
    }

    assert!(
        wait_until(
            || watchdog_failed(&fx.manager, &fx.agent_id),
            Duration::from_secs(5)
        )
        .await,
        "FR-012: once the turn's guard is dropped the watchdog judges the member again"
    );
}

/// FR-012: the spawn loop wires the feed itself - the tests above only prove the
/// helper works when a caller installs it.
///
/// The three tests before this one hand
/// [`TeamManager::watch_teammate_progress_for_test`] a feed and never let the
/// spawn loop run, so each of them would still pass if the loop forgot to
/// subscribe at all - which is exactly the state that produced the original
/// bug. This one spawns a real teammate through
/// [`TeamManager::spawn_teammate_internal`] and publishes nothing by hand: the
/// subscription the spawn installed is the only thing that can move the marker,
/// so a spawn path that does not subscribe fails the member inside the bound.
#[tokio::test]
async fn test_the_spawned_turns_own_feed_keeps_the_member_alive() {
    let fx = fixture("prog-spawned", Duration::from_millis(150)).await;
    let statuses = TeamStore::load(&fx.manager.team_dir)
        .expect("load team")
        .config
        .member_by_id(&fx.agent_id)
        .map(|m| m.status)
        .expect("member present");
    assert_eq!(
        statuses,
        MemberStatus::Working,
        "the spawn marks its teammate Working"
    );

    // The spawn loop's stub turn returns almost immediately; keep the turn (and
    // so its subscription) alive by holding the handle the spawn registered.
    Arc::clone(&fx.manager).start_watchdog();

    // Publish three windows' worth of the child session's own events. Nothing
    // here touches the manager directly: only the subscription the spawn
    // installed can see them.
    for step in 0..15 {
        fx.bus.publish(Event::ToolCallStart {
            session_id: fx.session_id.clone(),
            call_id: format!("spawn-call-{step}"),
            tool: "Bash".to_string(),
        });
        tokio::time::sleep(Duration::from_millis(60)).await;
    }

    assert!(
        !watchdog_failed(&fx.manager, &fx.agent_id),
        "FR-012: the spawn loop must subscribe to its own turn's events"
    );
    assert_ne!(
        status_of(&fx.manager, &fx.agent_id),
        MemberStatus::Failed,
        "FR-012: a spawned teammate publishing turn events is never failed"
    );
}
