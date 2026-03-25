//! `team_create` — Create a new named team and write its config to disk.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::TeamStore;

/// Creates a new team directory and initial config.
pub struct TeamCreateTool;

#[async_trait::async_trait]
impl Tool for TeamCreateTool {
    fn name(&self) -> &str {
        "team_create"
    }

    fn description(&self) -> &str {
        "Create a new named agent team. Returns the team name and directory path. \
         Teams coordinate multiple agent sessions via a shared task list and mailboxes."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Unique name for the team (lowercase, hyphens OK)"
                },
                "project_local": {
                    "type": "boolean",
                    "description": "If true, store team in [PROJECT]/.ragent/teams/; otherwise in ~/.ragent/teams/. Default: true"
                },
                "blueprint": {
                    "type": "string",
                    "description": "Optional blueprint name to seed the team from [PROJECT]/.ragent/blueprints/teams/<name> or ~/.ragent/blueprints/teams/<name>"
                }
            },
            "required": ["name"]
        })
    }

    fn permission_category(&self) -> &str {
        "team:manage"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let generated_name = format!(
            "team-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis()
        );
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .unwrap_or(generated_name);

        let project_local = input
            .get("project_local")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let mut store = match TeamStore::create(&name, &ctx.session_id, &ctx.working_dir, project_local) {
            Ok(store) => store,
            Err(e) if e.to_string().contains("already exists") => {
                // Team already exists; try to load it and continue so that blueprint
                // seeding can run against an existing team when requested.
                match TeamStore::load_by_name(&name, &ctx.working_dir) {
                    Ok(existing_store) => existing_store,
                    Err(load_err)
                        if load_err.to_string().contains("read")
                            && load_err.to_string().contains("config.json") =>
                    {
                        TeamStore::initialize_existing_without_config(
                            &name,
                            &ctx.session_id,
                            &ctx.working_dir,
                        )?
                    }
                    Err(load_err) => return Err(load_err),
                }
            }
            Err(e) => return Err(e),
        };

        // If the caller provided a blueprint, apply it now.
        if let Some(bp) = input.get("blueprint").and_then(|v| v.as_str()).map(str::trim).filter(|s| !s.is_empty()) {
            // Locate blueprint directory: project-local .ragent/blueprints/teams/<bp> or ~/.ragent/blueprints/teams/<bp>
            let mut blueprint_dir: Option<std::path::PathBuf> = None;
            // Walk up to find project .ragent (check the current dir, then parent, etc.)
            let mut cur_opt = Some(ctx.working_dir.as_path());
            while let Some(cur) = cur_opt {
                let candidate = cur.join(".ragent").join("blueprints").join("teams").join(bp);
                if candidate.is_dir() {
                    blueprint_dir = Some(candidate);
                    break;
                }
                cur_opt = cur.parent();
            }
            // Fallback to global
            if blueprint_dir.is_none() {
                if let Some(home) = dirs::home_dir() {
                    let candidate = home.join(".ragent").join("blueprints").join("teams").join(bp);
                    if candidate.is_dir() {
                        blueprint_dir = Some(candidate);
                    }
                }
            }

            if let Some(bdir) = blueprint_dir {
                // If README.md exists in blueprint, copy it to team dir
                let readme = bdir.join("README.md");
                if readme.exists() {
                    let _ = std::fs::create_dir_all(&store.dir);
                    let dest = store.dir.join("README.md");
                    let _ = std::fs::copy(&readme, &dest);
                }

                // If task_seed.json exists, parse and execute each seed
                let seed = bdir.join("task_seed.json");
                if seed.exists() {
                    if let Ok(raw) = std::fs::read_to_string(&seed) {
                        if let Ok(arr) = serde_json::from_str::<serde_json::Value>(&raw) {
                            if let Some(items) = arr.as_array() {
                                let registry = crate::tool::create_default_registry();
                                for item in items {
                                    if let Some(obj) = item.as_object() {
                                        if let Some(tool_name) = obj.get("tool").and_then(|v| v.as_str()) {
                                            let args = obj.get("args").cloned().unwrap_or(serde_json::json!({}));
                                            // Ensure team_name is present for team tools
                                            let mut args_obj = if args.is_object() { args } else { serde_json::json!({}) };
                                            if args_obj.get("team_name").is_none() {
                                                args_obj.as_object_mut().map(|m| m.insert("team_name".to_string(), serde_json::Value::String(name.clone())));
                                            }
                                            if let Some(tool) = registry.get(tool_name) {
                                                let _ = tool.execute(args_obj, ctx).await;
                                            }
                                        } else {
                                            // If no tool specified but task fields present, create a task directly
                                            if obj.get("title").is_some() {
                                                let title = obj.get("title").and_then(|v| v.as_str()).unwrap_or("untitled").to_string();
                                                let id = store.next_task_id().unwrap_or_else(|_| format!("task-{}", chrono::Utc::now().timestamp_millis()));
                                                let mut task = crate::team::task::Task::new(&id, &title);
                                                if let Some(desc) = obj.get("description").and_then(|v| v.as_str()) {
                                                    task.description = desc.to_string();
                                                }
                                                let _ = store.add_task(task);
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }

                // If spawn-prompts.md exists, run prompts with team_spawn
                let prompts = bdir.join("spawn-prompts.md");
                if prompts.exists() {
                    if let Ok(raw) = std::fs::read_to_string(&prompts) {
                        let chunks: Vec<&str> = raw.split("\n\n").map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
                        let registry = crate::tool::create_default_registry();
                        if let Some(spawn_tool) = registry.get("team_spawn") {
                            for (i, chunk) in chunks.iter().enumerate() {
                                let teammate_name = format!("auto-{}", i + 1);
                                let input = serde_json::json!({
                                    "team_name": name,
                                    "teammate_name": teammate_name,
                                    "agent_type": "general",
                                    "prompt": chunk
                                });
                                // Execute spawn; if TeamManager not present, team_spawn returns a pending status.
                                if let Ok(out) = spawn_tool.execute(input.clone(), ctx).await {
                                    if let Some(meta) = out.metadata {
                                        if let Some(status) = meta.get("status").and_then(|v| v.as_str()) {
                                            if status == "pending_manager" {
                                                // Record a Spawning member so the team config reflects the queued teammate.
                                                let agent_id = store.next_agent_id();
                                                let member = crate::team::config::TeamMember::new(&teammate_name, &agent_id, "general");
                                                let _ = store.add_member(member);
                                            } else if status == "spawned" {
                                                if let Some(agent_id_val) = meta.get("agent_id").and_then(|v| v.as_str()) {
                                                    let member = crate::team::config::TeamMember::new(&teammate_name, agent_id_val, "general");
                                                    let _ = store.add_member(member);
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        Ok(ToolOutput {
            content: format!(
                "Team '{}' created successfully.\nDirectory: {}\nLead session: {}",
                name,
                store.dir.display(),
                ctx.session_id
            ),
            metadata: Some(json!({
                "team_name": name,
                "team_dir": store.dir.to_string_lossy(),
                "lead_session_id": ctx.session_id,
                "project_local": project_local,
                "auto_named": input.get("name").and_then(|v| v.as_str()).map(str::trim).map(|s| s.is_empty()).unwrap_or(true)
            })),
        })
    }
}
