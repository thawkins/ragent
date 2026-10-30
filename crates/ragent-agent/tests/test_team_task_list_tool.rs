//! The `team_task_list` tool (spec `teamssetup`).
//!
//! `team_task_list` is the read-only surface a lead uses to inspect the shared
//! workload before claiming, assigning or creating work. It had **zero**
//! references in `crates/ragent-agent/tests/`, so nothing pinned its contract:
//!
//! 1. A missing `team_name` is rejected by name.
//! 2. An unknown team is rejected with the team name, not a bare I/O error.
//! 3. An empty list says so, and reports an empty `tasks` array.
//! 4. A populated list reports every task's id, status, assignee and
//!    dependencies in both the rendered text and the metadata.
//! 5. The tool is read-only: it never mutates `tasks.json`.
//!
//! Every fixture lives in its own `tempfile::tempdir()`, so nothing is written
//! into the repository.

use ragent_agent::team::{Task, TaskStatus, TaskStore, TeamStore};
use ragent_agent::tool::team_task_list::TeamTaskListTool;
use ragent_agent::tool::{TeamContext, Tool, ToolContext};
use serde_json::json;

const LEAD: &str = "lead";

/// A workspace holding a project-local team named `name`.
///
/// Returns the tempdir (keep it alive), the workspace path the tool context
/// uses, and the team directory.
fn workspace_with_team(name: &str) -> (tempfile::TempDir, std::path::PathBuf, std::path::PathBuf) {
    let tmp = tempfile::tempdir().expect("tempdir");
    let workspace = tmp.path().to_path_buf();
    std::fs::create_dir_all(workspace.join(".ragent")).expect("create .ragent");
    let store = TeamStore::create(name, LEAD, &workspace, true).expect("create team");
    (tmp, workspace, store.dir)
}

/// A minimal `ToolContext` acting as the lead of `team_name`.
fn tool_context(workspace: &std::path::Path, team_name: &str) -> ToolContext {
    ToolContext {
        session_id: LEAD.to_string(),
        working_dir: workspace.to_path_buf(),
        event_bus: std::sync::Arc::new(ragent_agent::event::EventBus::new(16)),
        storage: None,
        agent_manager: None,
        active_model: None,
        provider_registry: None,
        team_context: Some(std::sync::Arc::new(TeamContext {
            team_name: team_name.to_string(),
            agent_id: LEAD.to_string(),
            is_lead: true,
        })),
        team_manager: None,
        code_index: None,
        spec_manager: None,
        active_spec_id: None,
        config: None,
        bg_service: None,
        cached_team_dir: std::sync::Arc::new(std::sync::Mutex::new(None)),
        permission_checker: None,
        read_timestamps: std::sync::Arc::new(std::sync::RwLock::new(
            std::collections::HashMap::new(),
        )),
        canonical_cache: std::sync::Arc::new(ragent_tools_core::CanonicalPathCache::new()),
        allowed_roots: Vec::new(),
        tool_registry: ToolContext::default_tool_registry(),
    }
}

#[tokio::test]
async fn test_missing_team_name_is_rejected_by_name() {
    let (_tmp, workspace, _dir) = workspace_with_team("alpha");
    let ctx = tool_context(&workspace, "alpha");

    let err = TeamTaskListTool
        .execute(json!({}), &ctx)
        .await
        .expect_err("a call with no team_name must be refused");

    assert!(
        err.to_string().contains("team_name"),
        "the refusal must name the missing parameter, got: {err}"
    );
}

#[tokio::test]
async fn test_unknown_team_is_rejected_with_its_name() {
    let (_tmp, workspace, _dir) = workspace_with_team("alpha");
    let ctx = tool_context(&workspace, "alpha");

    let err = TeamTaskListTool
        .execute(json!({ "team_name": "no-such-team" }), &ctx)
        .await
        .expect_err("an unknown team must be refused");

    assert!(
        err.to_string().contains("no-such-team"),
        "the refusal must name the team, got: {err}"
    );
}

#[tokio::test]
async fn test_empty_list_reports_zero_tasks() {
    let (_tmp, workspace, _dir) = workspace_with_team("alpha");
    let ctx = tool_context(&workspace, "alpha");

    let out = TeamTaskListTool
        .execute(json!({ "team_name": "alpha" }), &ctx)
        .await
        .expect("listing an empty team must succeed");

    assert!(
        out.content.contains("No tasks"),
        "an empty list must say so, got: {}",
        out.content
    );
    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["team_name"], "alpha");
    assert_eq!(
        meta["tasks"],
        json!([]),
        "an empty list must report an empty tasks array"
    );
}

#[tokio::test]
async fn test_populated_list_reports_every_field_and_does_not_mutate() {
    let (_tmp, workspace, team_dir) = workspace_with_team("alpha");
    let tasks = TaskStore::open(&team_dir).expect("open task store");

    let mut first = Task::new("task-001", "design the API");
    first.status = TaskStatus::InProgress;
    first.assigned_to = Some("tm-001".to_string());
    first.description = "agree the route table".to_string();

    let mut second = Task::new("task-002", "write the tests");
    second.status = TaskStatus::Pending;
    second.depends_on = vec!["task-001".to_string()];

    tasks
        .write_through(|list| list.tasks.extend([first, second]))
        .expect("seed tasks");
    let before = std::fs::read_to_string(team_dir.join("tasks.json")).expect("read tasks.json");

    let ctx = tool_context(&workspace, "alpha");
    let out = TeamTaskListTool
        .execute(json!({ "team_name": "alpha" }), &ctx)
        .await
        .expect("listing a populated team must succeed");

    let meta = out.metadata.expect("metadata");
    assert_eq!(meta["task_count"], 2);
    let rows = meta["tasks"].as_array().expect("tasks array");
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0]["id"], "task-001");
    assert_eq!(rows[0]["status"], "in_progress");
    assert_eq!(rows[0]["assigned_to"], "tm-001");
    assert_eq!(rows[1]["id"], "task-002");
    assert_eq!(rows[1]["status"], "pending");
    assert_eq!(rows[1]["depends_on"], json!(["task-001"]));
    assert_eq!(
        rows[1]["assigned_to"],
        serde_json::Value::Null,
        "an unassigned task must report null, not a fabricated name"
    );

    // The rendered text names the ids, the assignee and the dependency edge.
    assert!(out.content.contains("task-001"), "content: {}", out.content);
    assert!(out.content.contains("tm-001"), "content: {}", out.content);
    assert!(
        out.content.contains("deps: task-001"),
        "the rendered text must show the dependency edge, got: {}",
        out.content
    );

    let after = std::fs::read_to_string(team_dir.join("tasks.json")).expect("re-read tasks.json");
    assert_eq!(
        before, after,
        "team_task_list is read-only and must not rewrite tasks.json"
    );
}
