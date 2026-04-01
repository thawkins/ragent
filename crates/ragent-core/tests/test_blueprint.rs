//! Tests for test_blueprint.rs

//! Tests for blueprint seeding including README copy, task_seed.json handling and spawn-prompts.md

use std::path::PathBuf;
use tempfile::TempDir;
use std::sync::Arc;

use ragent_core::team::TeamStore;
use ragent_core::tool::{create_default_registry, ToolContext};
use ragent_core::event::EventBus;

fn tmp() -> TempDir { tempfile::tempdir().expect("temp dir") }

fn make_tool_ctx(working_dir: PathBuf, session_id: &str) -> ToolContext {
    ToolContext {
        session_id: session_id.to_string(),
        working_dir,
        event_bus: Arc::new(EventBus::new(32)),
        storage: None,
        task_manager: None,
        lsp_manager: None,
        active_model: None,
        team_context: None,
        team_manager: None,
    }
}

#[tokio::test]
async fn test_team_create_applies_blueprint_readme_tasks_and_spawn() {
    let tmp = tmp();
    let project = tmp.path().join("proj-blueprint");
    std::fs::create_dir_all(project.join(".ragent")).unwrap();

    let bp_dir = project.join(".ragent").join("blueprints").join("teams").join("bp1");
    std::fs::create_dir_all(&bp_dir).unwrap();

    // README.md
    std::fs::write(bp_dir.join("README.md"), "Blueprint README\n").unwrap();

    // task_seed.json: one direct task entry and one tool-invocation (team_task_create)
    let seed = serde_json::json!([
        { "title": "Seed Task", "description": "from blueprint" },
        { "tool": "team_task_create", "args": { "title": "Tool Created", "description": "via tool" } }
    ]);
    std::fs::write(bp_dir.join("task-seed.json"), serde_json::to_string(&seed).unwrap()).unwrap();

    // spawn-prompts.json with two entries invoking team_spawn
    let spawn_entries = serde_json::json!([
        { "tool": "team_spawn", "args": { "prompt": "You are a helper agent that greets." }},
        { "tool": "team_spawn", "args": { "prompt": "You are a helper agent that lists files." }}
    ]);
    std::fs::write(bp_dir.join("spawn-prompts.json"), serde_json::to_string(&spawn_entries).unwrap()).unwrap();

    let registry = create_default_registry();
    let create = registry.get("team_create").expect("team_create tool");

    let lead_ctx = make_tool_ctx(project.clone(), "lead-001");

    // Execute team_create with blueprint
    let _out = create.execute(serde_json::json!({"name":"bp-sample","project_local":true,"blueprint":"bp1"}), &lead_ctx).await.unwrap();

    // Assert README copied
    let team_dir = project.join(".ragent").join("teams").join("bp-sample");
    let readme = team_dir.join("README.md");
    assert!(readme.exists(), "README should be copied to team dir");
    let content = std::fs::read_to_string(readme).unwrap();
    assert!(content.contains("Blueprint README"));

    // Assert tasks include Seed Task and Tool Created
    let loaded = TeamStore::load_by_name("bp-sample", &project).expect("team should load");
    let task_list = loaded.task_store().expect("task store").read().expect("read tasks");
    let titles: Vec<String> = task_list.tasks.iter().map(|t| t.title.clone()).collect();
    assert!(titles.iter().any(|t| t.contains("Seed Task")), "Seed Task should exist");
    assert!(titles.iter().any(|t| t.contains("Tool Created")), "Tool-created task should exist");

    // Assert spawn created teammates (auto-1 etc.)
    let members: Vec<String> = loaded.config.members.iter().map(|m| m.name.clone()).collect();
    assert!(members.iter().any(|n| n == "auto-1"), "auto-1 should be present");
    assert!(members.iter().any(|n| n == "auto-2"), "auto-2 should be present");

    // cleanup
    let _ = std::fs::remove_dir_all(&team_dir);
}
