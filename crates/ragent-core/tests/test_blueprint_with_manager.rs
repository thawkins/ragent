//! Tests for blueprint execution with team manager.
#![allow(missing_docs)]

use std::path::PathBuf;
use std::sync::Arc;
use tempfile::TempDir;

use ragent_core::event::EventBus;
use ragent_core::team::TeamStore;
use ragent_core::tool::{ToolContext, create_default_registry};

struct DummyManager {
    /// Working directory so we can persist members to the team store.
    working_dir: PathBuf,
}

#[async_trait::async_trait]
impl ragent_core::tool::TeamManagerInterface for DummyManager {
    async fn spawn_teammate(
        &self,
        team_name: &str,
        teammate_name: &str,
        agent_type: &str,
        _prompt: &str,
        _teammate_model: Option<&ragent_core::agent::ModelRef>,
        _lead_model: Option<&ragent_core::agent::ModelRef>,
        _working_dir: &std::path::Path,
    ) -> anyhow::Result<String> {
        // Persist the member to disk like the real spawn_teammate_internal does.
        let mut store = TeamStore::load_by_name(team_name, &self.working_dir)?;
        let agent_id = store.next_agent_id();
        let mut member = ragent_core::team::TeamMember::new(teammate_name, &agent_id, agent_type);
        member.status = ragent_core::team::MemberStatus::Working;
        member.session_id = Some(format!("dummy-session-{agent_id}"));
        store.add_member(member)?;
        Ok(agent_id)
    }
}

fn tmp() -> TempDir {
    tempfile::tempdir().expect("temp dir")
}

fn make_tool_ctx(working_dir: PathBuf, session_id: &str, with_manager: bool) -> ToolContext {
    let manager = if with_manager {
        Some(Arc::new(DummyManager {
            working_dir: working_dir.clone(),
        })
            as Arc<dyn ragent_core::tool::TeamManagerInterface>)
    } else {
        None
    };
    ToolContext {
        session_id: session_id.to_string(),
        working_dir,
        event_bus: Arc::new(EventBus::new(32)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: manager,
        code_index: None,
    }
}

#[tokio::test]
async fn repro_team_create_with_team_manager() {
    let tmp = tmp();
    let project = tmp.path().join("proj-repro");
    std::fs::create_dir_all(project.join(".ragent")).unwrap();

    let bp_dir = project
        .join(".ragent")
        .join("blueprints")
        .join("teams")
        .join("bp1");
    std::fs::create_dir_all(&bp_dir).unwrap();

    // spawn-prompts.json with one entry invoking team_spawn
    let spawn_entries = serde_json::json!([
        { "tool": "team_spawn", "args": { "prompt": "Hello from spawn" }}
    ]);
    std::fs::write(
        bp_dir.join("spawn-prompts.json"),
        serde_json::to_string(&spawn_entries).unwrap(),
    )
    .unwrap();

    let registry = create_default_registry();
    let create = registry.get("team_create").expect("team_create tool");

    // First run without manager -> should record pending_manager member
    let ctx_none = make_tool_ctx(project.clone(), "lead-none", false);
    let _out = create
        .execute(
            serde_json::json!({"name":"bp-no-mgr","project_local":true,"blueprint":"bp1"}),
            &ctx_none,
        )
        .await
        .unwrap();
    let loaded_none = TeamStore::load_by_name("bp-no-mgr", &project).expect("team should load");
    let members_none: Vec<String> = loaded_none
        .config
        .members
        .iter()
        .map(|m| m.name.clone())
        .collect();
    assert!(
        members_none.iter().any(|n| n == "auto-1"),
        "auto-1 should be present when manager missing"
    );

    // Now run with manager -> should record spawned agent with id
    let ctx_mgr = make_tool_ctx(project.clone(), "lead-mgr", true);
    let _out2 = create
        .execute(
            serde_json::json!({"name":"bp-with-mgr","project_local":true,"blueprint":"bp1"}),
            &ctx_mgr,
        )
        .await
        .unwrap();
    let loaded = TeamStore::load_by_name("bp-with-mgr", &project).expect("team should load");
    let members: Vec<(String, String)> = loaded
        .config
        .members
        .iter()
        .map(|m| (m.name.clone(), m.agent_id.clone()))
        .collect();
    assert!(
        members.iter().any(|(_, id)| id.starts_with("tm-")),
        "spawned agent id should be present when manager provided"
    );

    // cleanup
    let _ = std::fs::remove_dir_all(project.join(".ragent").join("teams").join("bp-no-mgr"));
    let _ = std::fs::remove_dir_all(project.join(".ragent").join("teams").join("bp-with-mgr"));
}
