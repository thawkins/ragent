//! Extra tests-only coverage for the `TeamManager` runtime (`team/manager.rs`)
//! plus the mailbox (`team/mailbox.rs`) and task-store (`team/task.rs`) branches
//! that the existing team test files do not reach.
//!
//! Targets the genuine gaps verified by prior investigation:
//! - `start_watchdog` (no test drives it),
//! - `reconcile_spawning_members` (zero coverage),
//! - the remaining exact error-string branches in the synchronous `TaskStore`
//!   API (multi-agent in-progress guard, unsatisfied deps, wrong-owner complete,
//!   pending-but-assigned pre-assign),
//! - `MailboxMessage::validate` empty id/from/to and `Mailbox::mark_read` for an
//!   unknown id,
//! - `run_team_hook` with a hook actually configured (the Feedback path),
//! - the mailbox `*_blocking` wrappers and the notifier free functions.
//!
//! A real `TeamManager` is built over a stub `SessionProcessor` (see
//! `manager_support`) with **no** provider. No network, no real LLM.

use std::sync::Arc;
use std::time::Duration;

use ragent_agent::event::{Event, EventBus};
use ragent_agent::session::processor::SessionProcessor;
use ragent_agent::team::{
    HookEntry, HookEvent, HookOutcome, Mailbox, MailboxMessage, MemberStatus, MessageType, Task,
    TaskStatus, TaskStore, TeamManager, TeamMember, TeamStore, deregister_notifier,
    register_notifier, run_team_hook,
};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::{stub_processor, team_manager};

// -- helpers -----------------------------------------------------------------

/// Build a `TeamManager` sharing a **strong** `Arc` to its processor (so the
/// spawn / reconcile paths can upgrade the `Weak`).await, optionally with a shortened
/// public `watchdog_timeout`, and return the `Arc<EventBus>` the manager
/// publishes on so a test can subscribe to its events.
///
/// The seeded team carries a single `Spawning` member `alice` (`tm-001`).
fn stub_manager(
    team_name: &str,
    lead_session: &str,
    watchdog_timeout: Option<Duration>,
) -> (
    Arc<TeamManager>,
    tempfile::TempDir,
    Arc<SessionProcessor>,
    Arc<EventBus>,
) {
    let bus = Arc::new(EventBus::new(256));
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().join("ws");
    std::fs::create_dir_all(workspace.join(".ragent")).expect("create .ragent");

    let mut store =
        TeamStore::create(team_name, "old-lead", &workspace, true).expect("create team");
    store
        .add_member(TeamMember::new("alice", "tm-001", "general"))
        .expect("add member");
    let team_dir = store.dir.clone();
    drop(store);

    let processor = stub_processor(bus.clone());
    let mut manager = TeamManager::new(
        team_name,
        lead_session,
        team_dir,
        Arc::clone(&processor),
        bus.clone(),
    );
    if let Some(timeout) = watchdog_timeout {
        manager.watchdog_timeout = timeout;
    }
    (Arc::new(manager), tmp, processor, bus)
}

/// A `TeamManager` whose `team_dir` points at a directory that does not exist,
/// so `TeamStore::load` fails inside background tasks.
fn manager_with_bogus_dir(
    team_name: &str,
    lead_session: &str,
) -> (Arc<TeamManager>, tempfile::TempDir) {
    let bus = Arc::new(EventBus::new(256));
    let tmp = tempfile::tempdir().expect("tempdir");
    let bogus = tmp.path().join("does-not-exist").join(team_name);
    let processor = stub_processor(bus.clone());
    let manager = Arc::new(TeamManager::new(
        team_name,
        lead_session,
        bogus,
        processor,
        bus,
    ));
    (manager, tmp)
}

fn load_member(team_dir: &std::path::Path, agent_id: &str) -> TeamMember {
    TeamStore::load(team_dir)
        .expect("load team")
        .config
        .member_by_id(agent_id)
        .cloned()
        .expect("member present")
}

/// An isolated `TaskStore` in its own team directory.
fn task_store() -> (tempfile::TempDir, TaskStore) {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(tmp.path().join(".ragent")).expect("create .ragent");
    let store = TeamStore::create("tasks-team", "lead", tmp.path(), true).expect("create team");
    let team_dir = store.dir.clone();
    drop(store);
    let store = TaskStore::open(&team_dir).expect("open task store");
    (tmp, store)
}

// -- start_watchdog ----------------------------------------------------------

/// `start_watchdog` must flag a `Working` teammate whose last progress is older
/// than `watchdog_timeout`: it marks the member `Failed` on disk and publishes
/// `Event::TeammateFailed`.
#[tokio::test]
async fn watchdog_marks_a_stale_working_member_failed() {
    // Tiny timeout so the watchdog's first couple of ticks already consider the
    // freshly-spawned handle stale.
    let (manager, _tmp, _proc, bus) =
        stub_manager("wd-team", "lead-1", Some(Duration::from_millis(40)));

    // Spawning a teammate registers a live handle (last_progress = now) and sets
    // the member to Working on disk. Only `spawn_teammate_internal` creates
    // handles.
    let wd = std::env::current_dir().expect("cwd");
    let agent_id = manager
        .spawn_teammate_internal("bob", "general", "work", None, None, &wd)
        .await
        .expect("spawn");
    assert_eq!(agent_id, "tm-002");

    let mut rx = bus.subscribe();
    Arc::clone(&manager).start_watchdog();

    // Poll for the Failed transition.
    let mut failed_on_disk = false;
    for _ in 0..200 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        if load_member(&manager.team_dir, &agent_id).status == MemberStatus::Failed {
            failed_on_disk = true;
            break;
        }
    }
    assert!(
        failed_on_disk,
        "watchdog should mark the stale Working member Failed on disk"
    );

    let member = load_member(&manager.team_dir, &agent_id);
    assert!(
        member
            .last_spawn_error
            .as_deref()
            .is_some_and(|e| e.contains("no progress")),
        "watchdog error should be recorded: {:?}",
        member.last_spawn_error
    );

    // The watchdog publishes Event::TeammateFailed with its own error string.
    let mut saw_event = false;
    while let Ok(event) = rx.try_recv() {
        if let Event::TeammateFailed {
            agent_id: id,
            error,
            ..
        } = event
            && id == agent_id
            && error.contains("no progress")
        {
            saw_event = true;
        }
    }
    assert!(saw_event, "Event::TeammateFailed (watchdog) expected");
}

/// `start_watchdog` called outside a tokio runtime is a silent no-op (returns
/// without spawning). Covers the `Handle::try_current()` error branch.
#[test]
fn start_watchdog_outside_runtime_is_a_noop() {
    let (manager, _tmp, _proc, _bus) =
        stub_manager("wd-nort", "lead-1", Some(Duration::from_millis(10)));
    // Not inside a tokio runtime context: this must return without panicking.
    Arc::clone(&manager).start_watchdog();
}

// -- reconcile_spawning_members ---------------------------------------------

/// A seeded `Spawning` member with no session id is adopted by the reconcile
/// loop: it is spawned (session id written) once the manager is live.
#[tokio::test]
async fn reconcile_spawns_a_seeded_spawning_member() {
    let (manager, _tmp, _proc, _bus) = stub_manager("reconcile-team", "lead-1", None);

    // The support-seeded "alice" (tm-001) is Spawning with no session id.
    let before = load_member(&manager.team_dir, "tm-001");
    assert_eq!(before.status, MemberStatus::Spawning);
    assert!(before.session_id.is_none());

    Arc::clone(&manager).reconcile_spawning_members();

    let mut spawned = false;
    for _ in 0..150 {
        tokio::time::sleep(Duration::from_millis(20)).await;
        if load_member(&manager.team_dir, "tm-001")
            .session_id
            .is_some()
        {
            spawned = true;
            break;
        }
    }
    assert!(
        spawned,
        "reconcile should spawn the queued Spawning member and persist a session id"
    );
}

/// With no `Spawning` members the reconcile loop takes its early `break`
/// branch and leaves the team untouched.
#[tokio::test]
async fn reconcile_with_no_spawning_members_breaks_immediately() {
    let (manager, _tmp, _bus) = team_manager("reconcile-empty", "lead-1", true);

    // Move the seeded member out of Spawning so there is nothing to reconcile.
    let mut store = TeamStore::load(&manager.team_dir).expect("load");
    store
        .config
        .member_by_id_mut("tm-001")
        .expect("member")
        .status = MemberStatus::Idle;
    store.save().expect("save");

    Arc::clone(&manager).reconcile_spawning_members();
    // Give the background task time to run at least one attempt.
    tokio::time::sleep(Duration::from_millis(400)).await;

    let member = load_member(&manager.team_dir, "tm-001");
    assert_eq!(
        member.status,
        MemberStatus::Idle,
        "no queued members -> reconcile must not touch the roster"
    );
}

/// A reconcile whose team store cannot be loaded logs and retries without
/// panicking (the `Err` branch of the load match).
#[tokio::test]
async fn reconcile_tolerates_an_unloadable_team_store() {
    let (manager, _tmp) = manager_with_bogus_dir("reconcile-bad", "lead-1");
    Arc::clone(&manager).reconcile_spawning_members();
    tokio::time::sleep(Duration::from_millis(400)).await;
    assert!(
        !std::path::Path::new(&manager.team_dir).exists(),
        "the bogus team dir must remain absent"
    );
}

// -- TaskStore exact branches ------------------------------------------------

/// The multi-agent in-progress guard: an agent already holding one task
/// `InProgress` cannot claim a different task until it completes the first.
#[tokio::test]
async fn claim_specific_rejects_a_second_in_progress_task_for_the_same_agent() {
    let (_tmp, store) = task_store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");
    store
        .add_task(Task::new("task-002", "b"))
        .await
        .expect("add");

    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");

    let err = store
        .claim_specific("task-002", "tm-001")
        .await
        .expect_err("must complete the first task");
    let text = err.to_string();
    assert!(text.contains("cannot claim 'task-002'"), "err: {text}");
    assert!(
        text.contains("is working on task 'task-001'"),
        "err: {text}"
    );
    assert!(text.contains("team_task_complete"), "err: {text}");
}

/// A pending task with an unsatisfied dependency cannot be claimed.
#[tokio::test]
async fn claim_specific_rejects_unsatisfied_dependencies() {
    let (_tmp, store) = task_store();
    // FR-039 validates `depends_on` at write time, so the dependency names a
    // real task - created *in progress* so it is present and open, which is the
    // unsatisfied state this assertion needs.
    let mut prereq = Task::new("task-000", "prereq held elsewhere");
    prereq.status = TaskStatus::InProgress;
    prereq.assigned_to = Some("tm-009".to_string());
    store.add_task(prereq).await.expect("add prereq");
    let mut task = Task::new("task-002", "dependent");
    task.depends_on = vec!["task-000".to_string()];
    store.add_task(task).await.expect("add");

    let err = store
        .claim_specific("task-002", "tm-002")
        .await
        .expect_err("unsatisfied dependency");
    let text = err.to_string();
    assert!(text.contains("its dependencies are unmet"), "err: {text}");
    assert!(text.contains("depends on [task-000]"), "err: {text}");
}

/// Completing an in-progress task owned by a different agent is rejected by the
/// wrong-owner guard (distinct from the "already completed by" guard), and the
/// refusal names the current owner (FR-001, FR-002).
#[tokio::test]
async fn complete_rejects_a_task_owned_by_another_agent() {
    let (_tmp, store) = task_store();
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");

    let err = store
        .complete("task-001", "tm-002")
        .await
        .expect_err("wrong owner");
    let msg = err.to_string();
    assert!(
        msg.contains("cannot be completed by tm-002: it is assigned to tm-001"),
        "the refusal must name the current owner and the caller: {msg}"
    );

    // FR-002: nothing was mutated - still owned and still in progress.
    let after = store.read().expect("read back");
    let task = after
        .tasks
        .iter()
        .find(|t| t.id == "task-001")
        .expect("task present");
    assert_eq!(task.status, TaskStatus::InProgress);
    assert_eq!(task.assigned_to.as_deref(), Some("tm-001"));
    assert!(task.completed_at.is_none(), "no completed_at stamp");
    assert!(task.completed_by.is_none(), "no completed_by stamp");
}

/// A `Pending` task that nonetheless carries an assignment cannot be
/// pre-assigned again.
#[tokio::test]
async fn pre_assign_rejects_a_pending_task_that_is_already_assigned() {
    let (_tmp, store) = task_store();
    let mut task = Task::new("task-001", "odd");
    // Pending but assigned - the status guard passes, the assignment guard trips.
    task.status = TaskStatus::Pending;
    task.assigned_to = Some("tm-009".to_string());
    store.add_task(task).await.expect("add");

    let err = store
        .pre_assign_task("task-001", "tm-010")
        .expect_err("already assigned");
    assert!(
        err.to_string()
            .contains("is in status pending and pre-assigned to tm-009"),
        "err: {err}"
    );
}

// -- Mailbox validation and unknown-id mark_read -----------------------------

/// `MailboxMessage::validate` rejects empty ids/from/to.
#[test]
fn mailbox_message_validation_rejects_empty_fields() {
    let mut no_id = MailboxMessage::new("tm-002", "tm-001", MessageType::Message, "x");
    no_id.message_id = String::new();
    assert!(no_id.validate().is_err(), "empty message_id");

    let no_from = MailboxMessage::new("", "tm-001", MessageType::Message, "x");
    assert!(no_from.validate().is_err(), "empty from");

    let no_to = MailboxMessage::new("tm-002", "", MessageType::Message, "x");
    assert!(no_to.validate().is_err(), "empty to");
}

/// `Mailbox::mark_read` returns `false` for a message id that does not exist.
#[test]
fn mailbox_mark_read_unknown_id_returns_false() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mbox = Mailbox::open(tmp.path(), "tm-001").expect("open");
    mbox.push(MailboxMessage::new(
        "tm-002",
        "tm-001",
        MessageType::Message,
        "hello",
    ))
    .expect("push");

    assert!(
        !mbox.mark_read("no-such-message").expect("mark_read"),
        "unknown id is unchanged"
    );
    // The real message is still unread.
    assert_eq!(mbox.peek_unread().expect("peek").len(), 1);
}

// -- run_team_hook with a configured hook (Feedback path) --------------------

/// With a quality-gate hook configured for an event, `run_team_hook` executes it
/// and returns its stdout as `Feedback` on exit code 2.
#[tokio::test]
async fn run_team_hook_with_configured_hook_returns_feedback() {
    let (manager, _tmp, _bus) = team_manager("hook-fb-team", "lead-1", true);

    // A single-token executable script (no shell metacharacters) that prints a
    // message and exits 2 (the block/feedback protocol).
    let script_dir = tempfile::tempdir().expect("script dir");
    let script = script_dir.path().join("gate.sh");
    std::fs::write(&script, "#!/bin/sh\necho needs work\nexit 2\n").expect("write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod");
    }

    let mut store = TeamStore::load(&manager.team_dir).expect("load");
    store.config.settings.hooks = vec![HookEntry {
        event: HookEvent::TaskCompleted,
        command: script.to_str().expect("utf8").to_string(),
    }];
    store.save().expect("persist hook");

    let outcome = run_team_hook(&manager.team_dir, HookEvent::TaskCompleted, Some("{}")).await;
    match outcome {
        HookOutcome::Feedback(msg) => assert!(msg.contains("needs work"), "feedback: {msg}"),
        HookOutcome::Allow => panic!("expected Feedback from an exit-2 hook"),
    }

    // An event with no matching hook still allows.
    let allow = run_team_hook(&manager.team_dir, HookEvent::TaskCreated, None).await;
    assert_eq!(allow, HookOutcome::Allow);
}

// -- Mailbox blocking wrappers and notifier free functions -------------------

#[tokio::test]
async fn mailbox_blocking_wrappers_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let mbox = Mailbox::open(tmp.path(), "tm-001").expect("open");

    mbox.push_blocking(MailboxMessage::new(
        "tm-002",
        "tm-001",
        MessageType::Message,
        "one",
    ))
    .await
    .expect("push_blocking");

    assert_eq!(mbox.peek_unread_blocking().await.expect("peek").len(), 1);
    assert_eq!(mbox.read_all_blocking().await.expect("read_all").len(), 1);

    let first_id = mbox.read_all_blocking().await.expect("read_all")[0]
        .message_id
        .clone();
    assert!(
        mbox.mark_read_blocking(first_id.clone())
            .await
            .expect("mark_read_blocking")
    );
    assert!(
        !mbox
            .mark_read_blocking(first_id)
            .await
            .expect("mark_read_blocking again")
    );

    mbox.push_blocking(MailboxMessage::new(
        "tm-002",
        "tm-001",
        MessageType::Message,
        "two",
    ))
    .await
    .expect("push_blocking");
    let drained = mbox.drain_unread_blocking().await.expect("drain");
    assert_eq!(drained.len(), 1);

    mbox.push_blocking(MailboxMessage::new(
        "tm-002",
        "tm-001",
        MessageType::Message,
        "three",
    ))
    .await
    .expect("push_blocking");
    let ids: Vec<String> = mbox
        .read_all_blocking()
        .await
        .expect("read")
        .into_iter()
        .map(|m| m.message_id)
        .collect();
    assert_eq!(mbox.mark_all_read_blocking(ids).await.expect("mark_all"), 1);
    assert_eq!(
        mbox.mark_all_read_blocking(Vec::new())
            .await
            .expect("empty"),
        0
    );
}

#[test]
fn notifier_registration_round_trip() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let notify = Arc::new(tokio::sync::Notify::new());
    // Register then deregister - both are best-effort map mutations.
    register_notifier(tmp.path(), "tm-001", Arc::clone(&notify));
    deregister_notifier(tmp.path(), "tm-001");
    // Deregistering a never-registered key is also harmless.
    deregister_notifier(tmp.path(), "tm-999");
}

// -- T-008: one completion implementation behind two entry points ---------------

/// FR-025/FR-026: `TeamManager::complete_task` and `TaskStore::complete` are the
/// same operation behind a cached and an uncached shape. A refusal from either
/// must be the identical text, because two diverging messages for one operation
/// is the defect those requirements name.
#[tokio::test]
async fn complete_task_and_store_complete_refuse_with_identical_text() {
    let (manager, _tmp, _proc, _bus) = stub_manager("parity-team", "lead-sess", None);
    let store = TaskStore::open(&manager.team_dir).expect("open store");
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");

    let cached_err = manager
        .complete_task("task-001", "tm-002")
        .await
        .expect_err("tm-002 owns nothing");
    let uncached_err = store
        .complete("task-001", "tm-002")
        .await
        .expect_err("tm-002 owns nothing");
    assert_eq!(
        cached_err.to_string(),
        uncached_err.to_string(),
        "the cached and uncached completion paths must not diverge (FR-026)"
    );
}

/// FR-001: the manager path enforces ownership too, and refusing leaves
/// `tasks.json` byte-identical (FR-002).
#[tokio::test]
async fn manager_complete_task_rejects_an_outsider_and_writes_nothing() {
    let (manager, _tmp, _proc, _bus) = stub_manager("owner-team", "lead-sess", None);
    let store = TaskStore::open(&manager.team_dir).expect("open store");
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");

    let tasks_json = manager.team_dir.join("tasks.json");
    let before = std::fs::read(&tasks_json).expect("read tasks.json");

    let err = manager
        .complete_task("task-001", "tm-002")
        .await
        .expect_err("outsider cannot complete");
    assert!(
        err.to_string()
            .contains("cannot be completed by tm-002: it is assigned to tm-001"),
        "{err}"
    );

    let after = std::fs::read(&tasks_json).expect("read tasks.json");
    assert_eq!(
        before, after,
        "a refused completion leaves tasks.json untouched"
    );
}

/// FR-001: the lead may complete on the owner's behalf through the manager path.
#[tokio::test]
async fn manager_complete_task_accepts_the_lead() {
    let (manager, _tmp, _proc, _bus) = stub_manager("lead-team", "lead-sess", None);
    let store = TaskStore::open(&manager.team_dir).expect("open store");
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");
    store
        .claim_specific("task-001", "tm-001")
        .await
        .expect("claim");

    let (done, _hook) = manager
        .complete_task("task-001", "lead")
        .await
        .expect("lead completes on the owner's behalf");
    assert_eq!(done.status, TaskStatus::Completed);
    assert_eq!(done.completed_by.as_deref(), Some("lead"));
    assert!(done.completed_at.is_some(), "completion stamps the time");
}

/// The manager path must not reintroduce the deleted auto-claim: an unowned
/// `Pending` task is refused, not silently reassigned.
#[tokio::test]
async fn manager_complete_task_does_not_auto_claim() {
    let (manager, _tmp, _proc, _bus) = stub_manager("noclaim-team", "lead-sess", None);
    let store = TaskStore::open(&manager.team_dir).expect("open store");
    store
        .add_task(Task::new("task-001", "a"))
        .await
        .expect("add");

    let err = manager
        .complete_task("task-001", "tm-001")
        .await
        .expect_err("unowned pending task");
    assert!(
        err.to_string()
            .contains("it is unassigned, and completing is not claiming"),
        "{err}"
    );

    let after = store.read().expect("read back");
    let task = after
        .tasks
        .iter()
        .find(|t| t.id == "task-001")
        .expect("task");
    assert_eq!(task.status, TaskStatus::Pending, "no implicit claim");
    assert!(task.assigned_to.is_none(), "no implicit reassignment");
}
