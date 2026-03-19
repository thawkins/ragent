//! Unit + integration tests for Teams data layer and tools.
//!
//! Tests cover:
//! - `TaskStore::claim_next()` basic claim
//! - Dependency blocking (dependent task not claimable until predecessor done)
//! - `TaskStore::complete()` unblocks a dependent
//! - Duplicate agent-ID guard
//! - `Mailbox::push()` / `drain_unread()` round-trip
//! - `Mailbox::mark_read()` targeting a specific message
//! - `TeamStore::create()` / `load()` / `list_teams()`
//! - `TeamStore::next_task_id()` / `next_agent_id()` sequence
//! - `TeamConfig` serde round-trip
//! - Plan approval transitions via `team_submit_plan` / `team_approve_plan`
//! - Lifecycle flow via team tools
//! - Hook exit-2 feedback behaviour

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

use ragent_core::event::EventBus;
use ragent_core::team::{
    HookOutcome, Mailbox, MailboxMessage, MemberStatus, MessageType, PlanStatus, Task, TaskStatus,
    TaskStore, TeamConfig, TeamMember, TeamStore, run_hook,
};
use ragent_core::tool::{TeamContext, ToolContext, create_default_registry};
use tempfile::TempDir;

// ── Helpers ───────────────────────────────────────────────────────────────────

fn tmp() -> TempDir {
    tempfile::tempdir().expect("temp dir")
}

fn make_team_dir(tmp: &TempDir, name: &str) -> std::path::PathBuf {
    let dir = tmp.path().join(name);
    std::fs::create_dir_all(dir.join("mailbox")).expect("dirs");
    dir
}

fn make_task(id: &str) -> Task {
    Task::new(id, format!("Task {id}"))
}

fn make_task_with_dep(id: &str, dep: &str) -> Task {
    let mut t = Task::new(id, format!("Task {id}"));
    t.depends_on.push(dep.to_owned());
    t
}

fn make_tool_ctx(
    working_dir: PathBuf,
    session_id: &str,
    team_context: Option<Arc<TeamContext>>,
) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        working_dir,
        event_bus: Arc::new(EventBus::new(32)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context,
        team_manager: None,
    }
}

// ── TaskStore tests ───────────────────────────────────────────────────────────

#[test]
fn test_task_claim_basic() {
    let tmp = tmp();
    let dir = make_team_dir(&tmp, "claim-basic");
    let store = TaskStore::open(&dir).unwrap();

    store.add_task(make_task("task-001")).unwrap();

    let claimed = store.claim_next("tm-001").unwrap();
    assert!(claimed.is_some());
    let t = claimed.unwrap();
    assert_eq!(t.id, "task-001");
    assert_eq!(t.status, TaskStatus::InProgress);
    assert_eq!(t.assigned_to.as_deref(), Some("tm-001"));
    assert!(t.claimed_at.is_some());

    // No more tasks.
    let second = store.claim_next("tm-002").unwrap();
    assert!(second.is_none());
}

#[test]
fn test_task_dependency_blocks_claim() {
    let tmp = tmp();
    let dir = make_team_dir(&tmp, "dep-block");
    let store = TaskStore::open(&dir).unwrap();

    store.add_task(make_task("task-001")).unwrap();
    store
        .add_task(make_task_with_dep("task-002", "task-001"))
        .unwrap();

    // Claim task-001.
    let first = store.claim_next("tm-001").unwrap().unwrap();
    assert_eq!(first.id, "task-001");

    // task-002 should be blocked (task-001 not yet complete).
    let blocked = store.claim_next("tm-002").unwrap();
    assert!(blocked.is_none(), "dependent task should not be claimable");
}

#[test]
fn test_task_complete_unblocks_dependent() {
    let tmp = tmp();
    let dir = make_team_dir(&tmp, "unblock");
    let store = TaskStore::open(&dir).unwrap();

    store.add_task(make_task("task-001")).unwrap();
    store
        .add_task(make_task_with_dep("task-002", "task-001"))
        .unwrap();

    // Claim and complete task-001.
    store.claim_next("tm-001").unwrap().unwrap();
    store.complete("task-001", "tm-001").unwrap();

    // task-002 should now be claimable.
    let unblocked = store.claim_next("tm-002").unwrap();
    assert!(unblocked.is_some());
    assert_eq!(unblocked.unwrap().id, "task-002");
}

#[test]
fn test_task_complete_wrong_agent_fails() {
    let tmp = tmp();
    let dir = make_team_dir(&tmp, "wrong-agent");
    let store = TaskStore::open(&dir).unwrap();

    store.add_task(make_task("task-001")).unwrap();
    store.claim_next("tm-001").unwrap().unwrap();

    // tm-002 should not be able to complete a task owned by tm-001.
    let result = store.complete("task-001", "tm-002");
    assert!(result.is_err(), "wrong agent should not complete task");
}

#[test]
fn test_task_duplicate_id_rejected() {
    let tmp = tmp();
    let dir = make_team_dir(&tmp, "dup-id");
    let store = TaskStore::open(&dir).unwrap();

    store.add_task(make_task("task-001")).unwrap();
    let dup = store.add_task(make_task("task-001"));
    assert!(dup.is_err(), "duplicate task ID should be rejected");
}

#[test]
fn test_task_claim_concurrent() {
    // Spawn multiple threads that all try to claim the same single task.
    // Exactly one should succeed.
    let tmp = tmp();
    let dir = Arc::new(make_team_dir(&tmp, "concurrent"));
    let store = TaskStore::open(&dir).unwrap();
    store.add_task(make_task("task-001")).unwrap();

    let dir_clone = Arc::clone(&dir);
    let handles: Vec<_> = (0..8)
        .map(move |i| {
            let d = Arc::clone(&dir_clone);
            thread::spawn(move || {
                let s = TaskStore::open(&d).unwrap();
                s.claim_next(&format!("tm-{i:03}")).unwrap()
            })
        })
        .collect();

    let results: Vec<_> = handles
        .into_iter()
        .map(|h| h.join().expect("thread"))
        .collect();

    let claims: Vec<_> = results.into_iter().flatten().collect();
    assert_eq!(claims.len(), 1, "exactly one thread should claim the task");
}

#[test]
fn test_task_update() {
    let tmp = tmp();
    let dir = make_team_dir(&tmp, "update");
    let store = TaskStore::open(&dir).unwrap();

    let mut task = make_task("task-001");
    task.description = "original".to_owned();
    store.add_task(task).unwrap();

    store
        .update_task("task-001", |t| {
            t.description = "updated".to_owned();
        })
        .unwrap();

    let list = store.read().unwrap();
    assert_eq!(list.tasks[0].description, "updated");
}

// ── Mailbox tests ─────────────────────────────────────────────────────────────

#[test]
fn test_mailbox_push_drain() {
    let tmp = tmp();
    let dir = make_team_dir(&tmp, "mailbox-drain");

    let mailbox = Mailbox::open(&dir, "tm-001").unwrap();

    let msg = MailboxMessage::new("lead", "tm-001", MessageType::Message, "hello teammate");
    mailbox.push(msg).unwrap();

    let unread = mailbox.drain_unread().unwrap();
    assert_eq!(unread.len(), 1);
    assert_eq!(unread[0].content, "hello teammate");
    assert_eq!(unread[0].from, "lead");

    // Second drain should return empty (already marked read).
    let second = mailbox.drain_unread().unwrap();
    assert!(second.is_empty());
}

#[test]
fn test_mailbox_mark_read() {
    let tmp = tmp();
    let dir = make_team_dir(&tmp, "mailbox-mark");

    let mailbox = Mailbox::open(&dir, "tm-001").unwrap();
    let msg = MailboxMessage::new("lead", "tm-001", MessageType::Broadcast, "broadcast");
    let id = msg.message_id.clone();
    mailbox.push(msg).unwrap();

    let found = mailbox.mark_read(&id).unwrap();
    assert!(found);

    // drain_unread should now return empty.
    let unread = mailbox.drain_unread().unwrap();
    assert!(unread.is_empty());
}

#[test]
fn test_mailbox_multiple_messages() {
    let tmp = tmp();
    let dir = make_team_dir(&tmp, "mailbox-multi");
    let mailbox = Mailbox::open(&dir, "lead").unwrap();

    for i in 0..5u32 {
        mailbox
            .push(MailboxMessage::new(
                "tm-001",
                "lead",
                MessageType::Message,
                format!("msg {i}"),
            ))
            .unwrap();
    }

    let unread = mailbox.drain_unread().unwrap();
    assert_eq!(unread.len(), 5);

    // All marked read.
    let all = mailbox.read_all().unwrap();
    assert!(all.iter().all(|m| m.read));
}

// ── TeamStore tests ───────────────────────────────────────────────────────────

#[test]
fn test_team_store_create_load() {
    let tmp = tmp();
    // Set up a fake project with .ragent/ directory.
    let project = tmp.path().join("project");
    std::fs::create_dir_all(project.join(".ragent")).unwrap();

    let store =
        TeamStore::create("alpha", "sess-abc", &project, true).unwrap();
    assert_eq!(store.config.name, "alpha");
    assert_eq!(store.config.lead_session_id, "sess-abc");
    assert!(store.dir.exists());

    let loaded = TeamStore::load(&store.dir).unwrap();
    assert_eq!(loaded.config.name, "alpha");
    assert_eq!(loaded.config.lead_session_id, "sess-abc");
}

#[test]
fn test_team_store_duplicate_create_fails() {
    let tmp = tmp();
    let project = tmp.path().join("proj2");
    std::fs::create_dir_all(project.join(".ragent")).unwrap();

    TeamStore::create("beta", "sess-1", &project, true).unwrap();
    let dup = TeamStore::create("beta", "sess-2", &project, true);
    assert!(dup.is_err(), "duplicate team name should fail");
}

#[test]
fn test_team_store_list_teams() {
    let tmp = tmp();
    let project = tmp.path().join("proj3");
    std::fs::create_dir_all(project.join(".ragent")).unwrap();

    TeamStore::create("gamma", "sess-1", &project, true).unwrap();
    TeamStore::create("delta", "sess-2", &project, true).unwrap();

    let teams = TeamStore::list_teams(&project);
    let names: HashSet<String> = teams.into_iter().map(|(n, _, _)| n).collect();
    assert!(names.contains("gamma"));
    assert!(names.contains("delta"));
}

#[test]
fn test_team_store_next_ids() {
    let tmp = tmp();
    let project = tmp.path().join("proj4");
    std::fs::create_dir_all(project.join(".ragent")).unwrap();

    let mut store = TeamStore::create("epsilon", "sess-1", &project, true).unwrap();

    // Initial IDs.
    assert_eq!(store.next_agent_id(), "tm-001");
    assert_eq!(store.next_task_id().unwrap(), "task-001");

    // Add a member and a task, then check sequence increments.
    let member = TeamMember::new("reviewer", "tm-001", "general");
    store.add_member(member).unwrap();
    assert_eq!(store.next_agent_id(), "tm-002");

    store
        .add_task(Task::new("task-001", "First task"))
        .unwrap();
    assert_eq!(store.next_task_id().unwrap(), "task-002");
}

#[test]
fn test_team_config_member_lookup() {
    let mut config = TeamConfig::new("zeta", "sess-001");
    let m = TeamMember::new("frontend", "tm-001", "general");
    config.members.push(m);

    assert!(config.member_by_id("tm-001").is_some());
    assert!(config.member_by_id("tm-999").is_none());
    assert!(config.member_by_name("frontend").is_some());
    assert!(config.member_by_name("backend").is_none());

    let active: Vec<_> = config.active_members().collect();
    assert_eq!(active.len(), 1);

    config.members[0].status = MemberStatus::Stopped;
    let active_after: Vec<_> = config.active_members().collect();
    assert!(active_after.is_empty());
}

#[test]
fn test_team_config_serde_roundtrip() {
    let mut config = TeamConfig::new("serde-team", "sess-rt");
    let mut member = TeamMember::new("planner", "tm-001", "general");
    member.status = MemberStatus::PlanPending;
    member.plan_status = PlanStatus::Pending;
    member.current_task_id = Some("task-101".to_string());
    config.members.push(member);

    let json = serde_json::to_string(&config).expect("serialize TeamConfig");
    let parsed: TeamConfig = serde_json::from_str(&json).expect("deserialize TeamConfig");

    assert_eq!(parsed.name, "serde-team");
    assert_eq!(parsed.lead_session_id, "sess-rt");
    assert_eq!(parsed.members.len(), 1);
    assert_eq!(parsed.members[0].name, "planner");
    assert_eq!(parsed.members[0].status, MemberStatus::PlanPending);
    assert_eq!(parsed.members[0].plan_status, PlanStatus::Pending);
    assert_eq!(parsed.members[0].current_task_id.as_deref(), Some("task-101"));
}

// ── Plan approval state transitions ───────────────────────────────────────────

#[tokio::test]
async fn test_plan_submit_and_approve_transitions() {
    let tmp = tmp();
    let project = tmp.path().join("proj-plan-approve");
    std::fs::create_dir_all(project.join(".ragent")).unwrap();
    let mut store = TeamStore::create("alpha-plan", "lead-session", &project, true).unwrap();
    store
        .add_member(TeamMember::new("planner", "tm-001", "general"))
        .unwrap();

    let registry = create_default_registry();
    let submit = registry.get("team_submit_plan").unwrap();
    let approve = registry.get("team_approve_plan").unwrap();

    let teammate_ctx = make_tool_ctx(
        project.clone(),
        "tm-sess",
        Some(Arc::new(TeamContext {
            team_name: "alpha-plan".to_string(),
            agent_id: "tm-001".to_string(),
            is_lead: false,
        })),
    );
    submit
        .execute(
            serde_json::json!({"team_name":"alpha-plan","plan":"Plan: add API + tests"}),
            &teammate_ctx,
        )
        .await
        .unwrap();

    let after_submit = TeamStore::load_by_name("alpha-plan", &project).unwrap();
    let member = after_submit.config.member_by_id("tm-001").unwrap();
    assert_eq!(member.plan_status, PlanStatus::Pending);
    assert_eq!(member.status, MemberStatus::PlanPending);

    let lead_ctx = make_tool_ctx(project.clone(), "lead-session", None);
    approve
        .execute(
            serde_json::json!({"team_name":"alpha-plan","teammate":"tm-001","approved":true}),
            &lead_ctx,
        )
        .await
        .unwrap();

    let after_approve = TeamStore::load_by_name("alpha-plan", &project).unwrap();
    let member = after_approve.config.member_by_id("tm-001").unwrap();
    assert_eq!(member.plan_status, PlanStatus::Approved);
    assert_eq!(member.status, MemberStatus::Working);
}

#[tokio::test]
async fn test_plan_submit_and_reject_transitions() {
    let tmp = tmp();
    let project = tmp.path().join("proj-plan-reject");
    std::fs::create_dir_all(project.join(".ragent")).unwrap();
    let mut store = TeamStore::create("beta-plan", "lead-session", &project, true).unwrap();
    store
        .add_member(TeamMember::new("planner", "tm-001", "general"))
        .unwrap();

    let registry = create_default_registry();
    let submit = registry.get("team_submit_plan").unwrap();
    let approve = registry.get("team_approve_plan").unwrap();

    let teammate_ctx = make_tool_ctx(
        project.clone(),
        "tm-sess",
        Some(Arc::new(TeamContext {
            team_name: "beta-plan".to_string(),
            agent_id: "tm-001".to_string(),
            is_lead: false,
        })),
    );
    submit
        .execute(
            serde_json::json!({"team_name":"beta-plan","plan":"Proposed plan"}),
            &teammate_ctx,
        )
        .await
        .unwrap();

    let lead_ctx = make_tool_ctx(project.clone(), "lead-session", None);
    approve
        .execute(
            serde_json::json!({
                "team_name":"beta-plan",
                "teammate":"tm-001",
                "approved":false,
                "feedback":"Needs stronger rollback strategy"
            }),
            &lead_ctx,
        )
        .await
        .unwrap();

    let after_reject = TeamStore::load_by_name("beta-plan", &project).unwrap();
    let member = after_reject.config.member_by_id("tm-001").unwrap();
    assert_eq!(member.plan_status, PlanStatus::Rejected);
    assert_eq!(
        member.status,
        MemberStatus::PlanPending,
        "reject keeps teammate in plan-pending mode"
    );
}

// ── Integration: create → spawn → claim → complete → cleanup ─────────────────

#[tokio::test]
async fn test_team_lifecycle_with_tools() {
    let tmp = tmp();
    let project = tmp.path().join("proj-lifecycle");
    std::fs::create_dir_all(project.join(".ragent")).unwrap();

    let registry = create_default_registry();
    let create = registry.get("team_create").unwrap();
    let spawn = registry.get("team_spawn").unwrap();
    let task_create = registry.get("team_task_create").unwrap();
    let task_claim = registry.get("team_task_claim").unwrap();
    let task_complete = registry.get("team_task_complete").unwrap();
    let cleanup = registry.get("team_cleanup").unwrap();

    let lead_ctx = make_tool_ctx(project.clone(), "lead-001", None);

    create
        .execute(
            serde_json::json!({"name":"lifecycle","project_local":true}),
            &lead_ctx,
        )
        .await
        .unwrap();

    let spawn_out = spawn
        .execute(
            serde_json::json!({
                "team_name":"lifecycle",
                "teammate_name":"worker-a",
                "agent_type":"general",
                "prompt":"Implement assigned task"
            }),
            &lead_ctx,
        )
        .await
        .unwrap();
    assert_eq!(
        spawn_out
            .metadata
            .as_ref()
            .and_then(|m| m.get("status"))
            .and_then(|v| v.as_str()),
        Some("pending_manager")
    );

    task_create
        .execute(
            serde_json::json!({
                "team_name":"lifecycle",
                "title":"Implement feature slice",
                "description":"Complete the first implementation slice"
            }),
            &lead_ctx,
        )
        .await
        .unwrap();

    let teammate_ctx = make_tool_ctx(
        project.clone(),
        "tm-session-1",
        Some(Arc::new(TeamContext {
            team_name: "lifecycle".to_string(),
            agent_id: "tm-001".to_string(),
            is_lead: false,
        })),
    );
    let claim_out = task_claim
        .execute(serde_json::json!({"team_name":"lifecycle"}), &teammate_ctx)
        .await
        .unwrap();
    assert!(
        claim_out.content.contains("Claimed task"),
        "expected task claim, got: {}",
        claim_out.content
    );

    task_complete
        .execute(
            serde_json::json!({"team_name":"lifecycle","task_id":"task-001"}),
            &teammate_ctx,
        )
        .await
        .unwrap();

    cleanup
        .execute(serde_json::json!({"team_name":"lifecycle","force":true}), &lead_ctx)
        .await
        .unwrap();

    let team_dir = project.join(".ragent").join("teams").join("lifecycle");
    assert!(
        !team_dir.exists(),
        "cleanup should remove lifecycle team directory"
    );
}

// ── Integration: hook exit-2 feedback semantics ───────────────────────────────

#[tokio::test]
async fn test_hook_exit_2_feedback_blocks_idle() {
    let tmp = tmp();
    let script = tmp.path().join("idle-hook.sh");
    std::fs::write(
        &script,
        "#!/usr/bin/env bash\necho 'idle blocked: missing tests'\nexit 2\n",
    )
    .unwrap();

    let outcome = run_hook("bash", &[script.to_string_lossy().to_string()]).await;
    match outcome {
        HookOutcome::Feedback(msg) => {
            assert!(msg.contains("idle blocked"), "unexpected feedback: {msg}");
        }
        HookOutcome::Allow => panic!("expected feedback/block outcome for exit code 2"),
    }
}
