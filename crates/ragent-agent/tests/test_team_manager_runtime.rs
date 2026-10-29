//! Tests-only coverage for the `TeamManager` runtime surface that is reachable
//! without a live model: task-cache coordination, plan-approval gating, leader
//! adoption, and quality-gate hook execution.
//!
//! A real `TeamManager` is built over a stub `SessionProcessor` (see
//! `manager_support`); every operation touches only on-disk team state.

use std::path::Path;

use ragent_agent::team::config::{HookEvent, PlanStatus};
use ragent_agent::team::manager::{
    HookOutcome, build_team_prompt_addition, run_hook, run_team_hook, teammate_retry_backoff,
    validate_hook_command,
};
use ragent_agent::team::{Mailbox, MemberStatus, MessageType, Task, TaskStore, TeamStore};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::team_manager;

// -- task list cache ---------------------------------------------------------

#[test]
fn task_list_starts_empty_and_mirrors_disk_writes() {
    let (manager, _tmp, _bus) = team_manager("cache-team", "lead-1", true);

    let initial = manager.task_list().expect("task_list");
    assert!(initial.tasks.is_empty());

    // A write-through mutation is visible on the next read.
    manager
        .apply_to_task_list(|list| {
            list.tasks.push(Task::new("task-001", "do work"));
        })
        .expect("apply");
    let after = manager.task_list().expect("task_list");
    assert_eq!(after.tasks.len(), 1);
    assert_eq!(after.tasks[0].id, "task-001");
}

#[tokio::test]
async fn claim_and_complete_task_through_manager() {
    let (manager, _tmp, _bus) = team_manager("claim-team", "lead-1", true);
    manager
        .apply_to_task_list(|list| list.tasks.push(Task::new("task-001", "do work")))
        .expect("seed task");

    let (claimed, already_had) = manager.claim_next_task("tm-001").await.expect("claim");
    let claimed = claimed.expect("a task should be claimed");
    assert_eq!(claimed.id, "task-001");
    assert!(!already_had);

    // The same agent claiming again gets the in-progress task back.
    let (again, already_had) = manager
        .claim_next_task("tm-001")
        .await
        .expect("claim again");
    assert!(already_had);
    assert_eq!(again.expect("in-progress task").id, "task-001");

    let (done, _hook) = manager
        .complete_task("task-001", "tm-001")
        .await
        .expect("complete");
    assert_eq!(done.status, ragent_agent::team::TaskStatus::Completed);

    // Idempotent re-completion by the same agent returns the task.
    let (done_again, _hook) = manager
        .complete_task("task-001", "tm-001")
        .await
        .expect("re-complete");
    assert_eq!(done_again.id, "task-001");
}

#[tokio::test]
async fn complete_missing_task_errors() {
    let (manager, _tmp, _bus) = team_manager("missing-team", "lead-1", true);
    let err = manager
        .complete_task("task-999", "tm-001")
        .await
        .expect_err("must error");
    assert!(err.to_string().contains("task-999"), "err: {err}");
}

#[test]
fn invalidate_task_cache_forces_reload() {
    let (manager, _tmp, _bus) = team_manager("inval-team", "lead-1", true);
    manager
        .apply_to_task_list(|list| list.tasks.push(Task::new("task-001", "t1")))
        .expect("seed");
    manager.invalidate_task_cache();
    let reloaded = manager.task_list().expect("reload");
    assert_eq!(reloaded.tasks.len(), 1);
}

// -- plan gating -------------------------------------------------------------

#[test]
fn plan_pending_reflects_submitted_plan_and_approval() {
    let (manager, _tmp, _bus) = team_manager("plan-team", "lead-1", true);
    assert!(!manager.is_plan_pending("tm-001"), "no plan yet");

    // Mark the member's plan Pending on disk.
    let mut store = TeamStore::load(&manager.team_dir).expect("load");
    store
        .config
        .member_by_id_mut("tm-001")
        .expect("member")
        .plan_status = PlanStatus::Pending;
    store.save().expect("save");
    manager.invalidate_plan_pending_cache("tm-001");

    assert!(manager.is_plan_pending("tm-001"), "pending plan detected");

    manager.approve_plan("tm-001", true).expect("approve");
    assert!(
        !manager.is_plan_pending("tm-001"),
        "approved clears pending"
    );

    let store = TeamStore::load(&manager.team_dir).expect("reload");
    let member = store.config.member_by_id("tm-001").expect("member");
    assert_eq!(member.plan_status, PlanStatus::Approved);
    assert_eq!(member.status, MemberStatus::Working);
}

#[test]
fn reject_plan_sets_rejected_status() {
    let (manager, _tmp, _bus) = team_manager("reject-team", "lead-1", true);
    manager.approve_plan("tm-001", false).expect("reject");
    let store = TeamStore::load(&manager.team_dir).expect("reload");
    assert_eq!(
        store
            .config
            .member_by_id("tm-001")
            .expect("member")
            .plan_status,
        PlanStatus::Rejected
    );
}

// -- leader adoption ---------------------------------------------------------

#[tokio::test]
async fn adopting_a_team_from_a_new_lead_reassigns_in_progress_tasks() {
    // The support helper creates the team with lead "old-lead" then constructs
    // the manager with "lead-1", which triggers adopt_orphaned_tasks.
    let (manager, _tmp, _bus) = team_manager("adopt-team", "lead-1", true);

    // Seed an in-progress task assigned to the old lead, then adopt again.
    let task_store = TaskStore::open(&manager.team_dir).expect("task store");
    let mut task = Task::new("task-001", "old work");
    task.status = ragent_agent::team::TaskStatus::InProgress;
    task.assigned_to = Some("lead".to_string());
    task_store.add_task(task).await.expect("add task");

    ragent_agent::team::manager::TeamManager::adopt_orphaned_tasks(&manager.team_dir, "lead")
        .expect("adopt");

    let list = task_store.read().expect("read");
    let adopted = list
        .tasks
        .iter()
        .find(|t| t.id == "task-001")
        .expect("task");
    assert_eq!(adopted.status, ragent_agent::team::TaskStatus::Pending);
    assert!(adopted.assigned_to.is_none());
}

// -- helper functions --------------------------------------------------------

#[test]
fn retry_backoff_grows_then_caps() {
    assert_eq!(teammate_retry_backoff(0), std::time::Duration::ZERO);
    let first = teammate_retry_backoff(1);
    let later = teammate_retry_backoff(10);
    assert!(first < later, "backoff must grow: {first:?} vs {later:?}");
    assert!(
        teammate_retry_backoff(40) <= std::time::Duration::from_secs(31),
        "backoff must stay capped"
    );
}

#[test]
fn team_prompt_addition_lists_roster() {
    let roster = vec![
        ("bob".to_string(), "tm-002".to_string()),
        ("carol".to_string(), "tm-003".to_string()),
    ];
    let prompt = build_team_prompt_addition("t", "alice", "tm-001", &roster, true);
    assert!(prompt.contains("team \"t\""));
    assert!(prompt.contains("\"alice\""));
    assert!(prompt.contains("bob (tm-002)"));
    assert!(prompt.contains("carol (tm-003)"));

    let empty = build_team_prompt_addition("t", "solo", "tm-001", &[], true);
    assert!(empty.contains("none yet"));
}

/// FR-027: the claim rule the prompt states is the rule `auto_claim_tasks`
/// enforces, so the two settings cannot ship the same instruction.
#[test]
fn team_prompt_addition_states_the_claim_rule_the_setting_enforces() {
    let auto_on = build_team_prompt_addition("t", "alice", "tm-001", &[], true);
    assert!(
        auto_on.contains("call `team_task_complete`"),
        "auto-claim on must tell the teammate to complete its task: {auto_on}"
    );
    assert!(
        auto_on.contains("Call `team_task_claim` to pick"),
        "auto-claim on must tell the teammate it may claim the next task: {auto_on}"
    );
    assert!(
        !auto_on.contains("Do not call `team_task_claim`"),
        "auto-claim on must not forbid claiming: {auto_on}"
    );

    let auto_off = build_team_prompt_addition("t", "alice", "tm-001", &[], false);
    assert!(
        auto_off.contains("`auto_claim_tasks` turned off"),
        "auto-claim off must say the setting is off: {auto_off}"
    );
    // The prompt is a literal with `\n\` line continuations, so each phrase is
    // broken across source lines: compare against the collapsed text.
    let auto_off_flat = auto_off.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(
        auto_off_flat.contains("Do not call `team_task_claim` without an explicit instruction"),
        "auto-claim off must forbid unilateral claiming (FR-027): {auto_off}"
    );
    assert!(
        auto_off.contains("call `team_task_complete`: it parks you idle"),
        "auto-claim off must say completing the task parks the teammate idle (FR-014): {auto_off}"
    );
    assert_ne!(
        auto_on, auto_off,
        "the two settings must produce different claim instructions"
    );

    // Everything outside the claim paragraph is shared, so the setting cannot
    // silently drop the read-messages, shutdown or plan-approval rules.
    for clause in [
        "Call `team_read_messages`",
        "`team_shutdown_ack`",
        "`team_submit_plan`",
        "### Peer collaboration",
    ] {
        assert!(auto_on.contains(clause), "auto-on lost {clause:?}");
        assert!(auto_off.contains(clause), "auto-off lost {clause:?}");
    }
}

#[test]
fn validate_hook_command_rejects_dangerous_commands() {
    assert!(validate_hook_command("cargo").is_ok());
    assert!(validate_hook_command("").is_err());
    assert!(validate_hook_command("rm -rf; echo hi").is_err());
    assert!(validate_hook_command("echo $(whoami)").is_err());
    assert!(validate_hook_command("cat | grep x").is_err());
    assert!(validate_hook_command(&"a".repeat(5000)).is_err());
    assert!(validate_hook_command("echo bad\nline").is_err());
}

// -- hook execution ----------------------------------------------------------

#[tokio::test]
async fn run_hook_interprets_exit_codes() {
    let (manager, _tmp, _bus) = team_manager("hook-exit-codes", "lead-1", true);
    let dir = manager.team_dir.clone();
    let event = HookEvent::TaskCompleted;

    // Exit 0 -> allow.
    let allow = run_hook(&dir, event, "true", &[], None).await;
    assert_eq!(allow, HookOutcome::Allow);

    // A disallowed command is refused before spawn -> allow.
    let refused = run_hook(&dir, event, "echo; rm", &[], None).await;
    assert_eq!(refused, HookOutcome::Allow);

    // A missing binary -> allow (best-effort).
    let missing = run_hook(&dir, event, "this-is-not-a-binary-anywhere", &[], None).await;
    assert_eq!(missing, HookOutcome::Allow);
}

#[tokio::test]
async fn run_hook_feedback_path_returns_stdout() {
    // `sh -c` is unavailable (metacharacters banned), so drive exit-code 2
    // through the standard `false` binary's sibling: `sh` with a script file.
    // Instead use a tiny shell script written to a temp dir and executed by path.
    let (manager, _tmp, _bus) = team_manager("hook-feedback", "lead-1", true);
    let ledger_dir = manager.team_dir.clone();
    let script_dir = tempfile::tempdir().expect("tempdir");
    let script = script_dir.path().join("gate.sh");
    std::fs::write(&script, "#!/bin/sh\necho needs work\nexit 2\n").expect("write script");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).expect("meta").permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).expect("chmod");
    }

    let outcome = run_hook(
        &ledger_dir,
        HookEvent::TaskCompleted,
        script.to_str().expect("utf8"),
        &[],
        None,
    )
    .await;
    match outcome {
        HookOutcome::Feedback(msg) => assert!(msg.contains("needs work"), "feedback: {msg}"),
        HookOutcome::Allow => panic!("expected feedback from an exit-2 hook"),
    }
}

#[tokio::test]
async fn run_team_hook_returns_allow_when_no_hook_configured() {
    let (manager, _tmp, _bus) = team_manager("hook-team", "lead-1", true);
    let outcome = run_team_hook(&manager.team_dir, HookEvent::TaskCompleted, None).await;
    assert_eq!(outcome, HookOutcome::Allow);
}

// -- record progress ---------------------------------------------------------

#[test]
fn record_progress_for_unknown_agent_is_a_noop() {
    let (manager, _tmp, _bus) = team_manager("progress-team", "lead-1", true);
    // No live handles exist for "tm-999"; this must not panic.
    manager.record_progress("tm-999");
    assert!(!manager.is_plan_pending("tm-999"));
}

#[test]
fn lead_session_id_is_exposed() {
    let (manager, _tmp, _bus) = team_manager("lead-team", "lead-42", true);
    assert_eq!(manager.lead_session_id, "lead-42");
}

#[test]
fn team_dir_exists_on_disk() {
    let (manager, _tmp, _bus) = team_manager("disk-team", "lead-1", true);
    assert!(Path::new(&manager.team_dir).join("config.json").exists());
}

// -- suspend / resume / shutdown --------------------------------------------

#[tokio::test]
async fn suspend_then_resume_round_trips_member_status() {
    let (manager, _tmp, bus) = team_manager("suspend-team", "lead-1", true);
    let mut rx = bus.subscribe();

    manager.suspend_teammate("tm-001").await.expect("suspend");
    let suspended = TeamStore::load(&manager.team_dir).expect("load");
    assert_eq!(
        suspended
            .config
            .member_by_id("tm-001")
            .expect("member")
            .status,
        MemberStatus::Suspended
    );

    // Suspending again is harmless (idempotent disk write).
    manager
        .suspend_teammate("tm-001")
        .await
        .expect("suspend again");

    manager.resume_teammate("tm-001").await.expect("resume");
    let resumed = TeamStore::load(&manager.team_dir).expect("load");
    assert_eq!(
        resumed
            .config
            .member_by_id("tm-001")
            .expect("member")
            .status,
        MemberStatus::Idle
    );

    let mut saw_suspended = false;
    let mut saw_resumed = false;
    while let Ok(event) = rx.try_recv() {
        match event {
            ragent_agent::event::Event::TeammateSuspended { .. } => saw_suspended = true,
            ragent_agent::event::Event::TeammateResumed { .. } => saw_resumed = true,
            _ => {}
        }
    }
    assert!(saw_suspended, "TeammateSuspended event expected");
    assert!(saw_resumed, "TeammateResumed event expected");
}

#[tokio::test]
async fn resume_non_suspended_member_errors() {
    let (manager, _tmp, _bus) = team_manager("resume-team", "lead-1", true);
    // The fixture member is `Working`: neither `Suspended` nor `Blocked`, so
    // resume has nothing to restore (FR-010 accepts only the two parked states).
    let err = manager
        .resume_teammate("tm-001")
        .await
        .expect_err("a working member must error");
    let msg = err.to_string();
    assert!(msg.contains("working"), "err names the status: {msg}");
    assert!(
        msg.contains("suspended or blocked"),
        "err names the statuses resume accepts: {msg}"
    );
}

// `TeamManager::shutdown_teammate` builds its `ShutdownRequest` as a
// lead-authored message, so it must carry the authenticated sender session id
// (SEC-ragent-team-003): without it the mailbox push rejects the request before
// the on-disk status update, and the graceful path can never shut a teammate
// down. With the sender id attached the call succeeds, pushes the request into
// the teammate's mailbox, and marks the member `ShuttingDown` on disk.
#[tokio::test]
async fn shutdown_teammate_stamps_lead_sender_session_and_marks_shutting_down() {
    let (manager, _tmp, _bus) = team_manager("shutdown-team", "lead-1", true);

    manager
        .shutdown_teammate("tm-001", true)
        .await
        .expect("graceful shutdown succeeds once the sender session id is attached");

    let mbox = Mailbox::open(&manager.team_dir, "tm-001").expect("open teammate mailbox");
    let messages = mbox.read_all().expect("read mailbox");
    let request = messages
        .iter()
        .find(|m| m.message_type == MessageType::ShutdownRequest)
        .expect("the shutdown request was pushed");
    assert_eq!(
        request.sender_session_id.as_deref(),
        Some("lead-1"),
        "the lead-authored request carries the authenticated sender session id"
    );

    let store = TeamStore::load(&manager.team_dir).expect("load");
    assert_eq!(
        store.config.member_by_id("tm-001").expect("member").status,
        MemberStatus::ShuttingDown,
        "the on-disk status moves to ShuttingDown (graceful path)"
    );
}

#[tokio::test]
async fn shutdown_all_tolerates_the_shutdown_error() {
    // `shutdown_all` logs (not propagates) per-teammate shutdown errors, so it
    // returns Ok even while the underlying `shutdown_teammate` push is rejected.
    let (manager, _tmp, _bus) = team_manager("all-team", "lead-1", true);
    manager
        .shutdown_all()
        .await
        .expect("shutdown_all is best-effort");
}

#[tokio::test]
async fn add_task_and_next_task_id_increment() {
    let (manager, _tmp, _bus) = team_manager("nextid-team", "lead-1", true);
    let store = TeamStore::load(&manager.team_dir).expect("load");

    assert_eq!(store.next_task_id().expect("first id"), "task-001");
    store
        .add_task(Task::new("task-001", "first"))
        .await
        .expect("add task");
    assert_eq!(store.next_task_id().expect("second id"), "task-002");
}
