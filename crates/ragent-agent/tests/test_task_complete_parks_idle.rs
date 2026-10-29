//! FR-014: completing a task parks the completing member as idle.
//!
//! A swarm teammate that completed its only subtask used to stay `Working`
//! with `current_task_id = None` for ever: `team_task_complete` cleared the task
//! field directly on the roster and never moved the member, and no other route
//! could move it (`team_idle` refuses only while a task is held; the spawn
//! loop's own idle move had already happened). The swarm roster then showed a
//! completed task beside a member that never became idle.
//!
//! These tests drive the real tool against a real manager over a temp team and
//! assert the status the roster ends up with, for both `auto_claim_tasks`
//! settings and for the lead completing on a teammate's behalf.

use std::sync::Arc;

use ragent_agent::team::{
    MemberStatus, Task, TaskStatus, TaskStore, TeamManager, TeamMember, TeamStore,
};
use ragent_agent::tool::team_task_complete::TeamTaskCompleteTool;
use ragent_agent::tool::{TeamContext, Tool, ToolContext};

#[path = "manager_support/mod.rs"]
mod manager_support;
use manager_support::team_manager;

/// A tool context bound to `manager` and the workspace root its team was
/// created under, with the calling member's agent id set. The tool resolves the
/// team directory from `working_dir`, so it must be the workspace, not the team
/// directory nested under it (`<workspace>/.ragent/teams/<name>`).
fn ctx_for(manager: &Arc<TeamManager>, agent_id: &str) -> ToolContext {
    let team_dir = manager.team_dir.clone();
    let working_dir = team_dir
        .parent()
        .and_then(|p| p.parent())
        .and_then(|p| p.parent())
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| team_dir.clone());
    ToolContext {
        session_id: manager.lead_session_id.clone(),
        working_dir,
        event_bus: Arc::new(ragent_agent::event::EventBus::new(64)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: Some(Arc::new(TeamContext {
            team_name: manager.team_name.clone(),
            agent_id: agent_id.to_string(),
            is_lead: agent_id == "lead",
        })),
        team_manager: Some(manager.clone()),
        code_index: None,
        bg_service: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        allowed_roots: vec![manager.team_dir.clone()],
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        tool_registry: ToolContext::default_tool_registry(),
    }
}

/// Put one in-progress, owned task on disk - the shape a teammate is in when it
/// calls `team_task_complete`.
fn assign_and_claim(team_dir: &std::path::Path, agent_id: &str) {
    let store = TaskStore::open(team_dir).expect("open task store");
    let mut list = store.read().expect("read tasks");
    let mut task = Task::new("t-1", "count the functions");
    task.status = TaskStatus::InProgress;
    task.assigned_to = Some(agent_id.to_string());
    list.tasks.push(task);
    let json = serde_json::to_string_pretty(&list).expect("serialize task list");
    std::fs::write(store.path(), json).expect("write tasks.json");
}

fn roster_status(team_dir: &std::path::Path, agent_id: &str) -> (MemberStatus, Option<String>) {
    let store = TeamStore::load(team_dir).expect("load team");
    let member = store
        .config
        .member_by_id(agent_id)
        .expect("member in roster")
        .clone();
    (member.status, member.current_task_id)
}

/// With `auto_claim_tasks` on but nothing left to claim, completing the task
/// parks the member idle - the swarm case.
#[tokio::test]
async fn completing_the_last_task_parks_the_member_idle() {
    let (manager, _tmp, _bus) = team_manager("idle-team", "lead-1", true);
    let team_dir = manager.team_dir.clone();
    assign_and_claim(&team_dir, "tm-001");

    let ctx = ctx_for(&manager, "tm-001");
    let out = TeamTaskCompleteTool
        .execute(
            serde_json::json!({"team_name": "idle-team", "task_id": "t-1"}),
            &ctx,
        )
        .await
        .expect("complete");
    assert!(
        out.metadata
            .as_ref()
            .and_then(|m| m.get("completed"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        "completion must report success: {}",
        out.content
    );

    let (status, held) = roster_status(&team_dir, "tm-001");
    assert_eq!(
        status,
        MemberStatus::Idle,
        "a teammate that finished its only task must end idle, not {status:?}"
    );
    assert_eq!(held, None, "an idle member holds no task");
}

/// The move is a real transition: it reaches the roster through the single
/// status writer, which is the only thing that records a history row.
#[tokio::test]
async fn the_idle_move_is_recorded_in_the_status_history() {
    let (manager, _tmp, _bus) = team_manager("history-team", "lead-1", true);
    let team_dir = manager.team_dir.clone();
    assign_and_claim(&team_dir, "tm-001");

    let ctx = ctx_for(&manager, "tm-001");
    TeamTaskCompleteTool
        .execute(
            serde_json::json!({"team_name": "history-team", "task_id": "t-1"}),
            &ctx,
        )
        .await
        .expect("complete");

    let history = std::fs::read_to_string(team_dir.join("status-history.jsonl"))
        .expect("status history written");
    assert!(
        history.contains("idle"),
        "the idle move must be recorded as a transition: {history}"
    );
    assert!(
        history.contains("task_completed"),
        "the recorded cause must be the completion: {history}"
    );
}

/// `auto_claim_tasks` off: the member parks idle and nothing is claimed for it.
#[tokio::test]
async fn auto_claim_off_leaves_the_member_idle() {
    let (manager, _tmp, _bus) = team_manager("manual-team", "lead-1", true);
    let team_dir = manager.team_dir.clone();
    {
        let mut store = TeamStore::load(&team_dir).expect("load team");
        store.config.settings.auto_claim_tasks = false;
        store.save().expect("save team");
    }
    assign_and_claim(&team_dir, "tm-001");

    let ctx = ctx_for(&manager, "tm-001");
    TeamTaskCompleteTool
        .execute(
            serde_json::json!({"team_name": "manual-team", "task_id": "t-1"}),
            &ctx,
        )
        .await
        .expect("complete");

    let (status, held) = roster_status(&team_dir, "tm-001");
    assert_eq!(status, MemberStatus::Idle);
    assert_eq!(held, None);
}

/// A lead completing on a teammate's behalf does not move the lead - the lead
/// is not a roster member - and the completion still succeeds.
#[tokio::test]
async fn lead_completion_leaves_the_lead_alone() {
    let (manager, _tmp, _bus) = team_manager("lead-team", "lead-1", true);
    let team_dir = manager.team_dir.clone();
    assign_and_claim(&team_dir, "tm-001");

    let ctx = ctx_for(&manager, "lead");
    let out = TeamTaskCompleteTool
        .execute(
            serde_json::json!({"team_name": "lead-team", "task_id": "t-1"}),
            &ctx,
        )
        .await
        .expect("complete");
    assert!(
        out.metadata
            .as_ref()
            .and_then(|m| m.get("completed"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
        "a lead completion must succeed: {}",
        out.content
    );

    let store = TeamStore::load(&team_dir).expect("load team");
    assert!(
        store.config.member_by_id("lead").is_none(),
        "a lead completion must not create a roster member for the lead"
    );
    let names: Vec<&str> = store
        .config
        .members
        .iter()
        .map(|m: &TeamMember| m.name.as_str())
        .collect();
    assert_eq!(names, vec!["alice"]);
}
