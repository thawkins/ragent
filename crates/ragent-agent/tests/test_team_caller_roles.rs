//! Caller role checks on the `team_*` tools (review P0-1).
//!
//! Lead-only tools must refuse a teammate, every team tool must refuse a
//! session that is not in the named team, and a session outside every team
//! must not be able to speak as the lead. Identity comes from the session's
//! resolved team context or, failing that, from the team's `config.json`.

use std::sync::Arc;

use ragent_agent::event::EventBus;
use ragent_agent::team::{TeamMember, TeamStore};
use ragent_agent::tool::team_approve_plan::TeamApprovePlanTool;
use ragent_agent::tool::team_cleanup::TeamCleanupTool;
use ragent_agent::tool::team_message::TeamMessageTool;
use ragent_agent::tool::team_read_messages::TeamReadMessagesTool;
use ragent_agent::tool::team_task_create::TeamTaskCreateTool;
use ragent_agent::tool::{TeamContext, Tool, ToolContext};
use serde_json::json;

/// Build a tool context for `session_id` rooted at `working_dir`.
fn ctx(session_id: &str, working_dir: &std::path::Path, tc: Option<TeamContext>) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        working_dir: working_dir.to_path_buf(),
        event_bus: Arc::new(EventBus::new(64)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: tc.map(Arc::new),
        team_manager: None,
        code_index: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        bg_service: None,
        cached_team_dir: Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        tool_registry: ToolContext::default_tool_registry(),
        read_timestamps: Arc::new(std::sync::RwLock::new(std::collections::HashMap::new())),
        canonical_cache: Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: Vec::new(),
    }
}

/// Create team `name` led by `lead-sess` with teammate `tm-001` (session
/// `tm-sess`), and return its directory.
fn team(workspace: &std::path::Path, name: &str) -> std::path::PathBuf {
    std::fs::create_dir_all(workspace.join(".ragent")).expect("create .ragent");
    let mut store = TeamStore::create(name, "lead-sess", workspace, true).expect("create team");
    let mut member = TeamMember::new("worker", "tm-001", "general");
    member.session_id = Some(format!("{name}-tm-sess"));
    store.add_member(member).expect("add member");
    store.dir.clone()
}

fn teammate_ctx(team_name: &str) -> TeamContext {
    TeamContext {
        team_name: team_name.to_string(),
        agent_id: "tm-001".to_string(),
        is_lead: false,
    }
}

#[tokio::test]
async fn test_team_caller_teammate_cannot_cleanup_team() {
    let tmp = tempfile::tempdir().expect("tempdir");
    let dir = team(tmp.path(), "alpha");
    let c = ctx("alpha-tm-sess", tmp.path(), Some(teammate_ctx("alpha")));

    let err = TeamCleanupTool
        .execute(json!({"team_name": "alpha", "force": true}), &c)
        .await
        .expect_err("teammate refused");
    assert!(err.to_string().contains("lead-only"), "err: {err}");
    assert!(dir.join("config.json").exists(), "team must survive");
}

#[tokio::test]
async fn test_team_caller_teammate_cannot_approve_own_plan() {
    let tmp = tempfile::tempdir().expect("tempdir");
    team(tmp.path(), "alpha");
    let c = ctx("alpha-tm-sess", tmp.path(), Some(teammate_ctx("alpha")));

    let err = TeamApprovePlanTool
        .execute(
            json!({"team_name": "alpha", "teammate": "tm-001", "approved": true}),
            &c,
        )
        .await
        .expect_err("self-approval refused");
    assert!(err.to_string().contains("lead-only"), "err: {err}");
}

#[tokio::test]
async fn test_team_caller_outsider_cannot_message_as_lead() {
    let tmp = tempfile::tempdir().expect("tempdir");
    team(tmp.path(), "alpha");
    let c = ctx("stranger", tmp.path(), None);

    let err = TeamMessageTool
        .execute(
            json!({"team_name": "alpha", "to": "tm-001", "content": "stop all work"}),
            &c,
        )
        .await
        .expect_err("outsider refused");
    assert!(err.to_string().contains("not a member"), "err: {err}");
}

#[tokio::test]
async fn test_team_caller_teammate_cannot_read_other_teams_mailbox() {
    let tmp = tempfile::tempdir().expect("tempdir");
    team(tmp.path(), "alpha");
    team(tmp.path(), "beta");
    let c = ctx("alpha-tm-sess", tmp.path(), Some(teammate_ctx("alpha")));

    let err = TeamReadMessagesTool
        .execute(json!({"team_name": "beta"}), &c)
        .await
        .expect_err("cross-team read refused");
    assert!(err.to_string().contains("not a member"), "err: {err}");
}

#[tokio::test]
async fn test_team_caller_lead_resolved_from_config_without_context() {
    let tmp = tempfile::tempdir().expect("tempdir");
    team(tmp.path(), "alpha");
    // No team context yet (e.g. the same step that created the team): the
    // lead is recognised from `lead_session_id` in config.json.
    let c = ctx("lead-sess", tmp.path(), None);

    TeamTaskCreateTool
        .execute(json!({"team_name": "alpha", "title": "first task"}), &c)
        .await
        .expect("lead may create tasks");
}
