//! `team_create` — Create a new named team and write its config to disk.

use anyhow::Result;
use chrono::Utc;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::TeamStore;
use crate::team::manager::HookOutcome;
use crate::team::{HookEvent, TaskStore, run_team_hook};
use crate::tool::metadata::MetadataBuilder;

/// Creates a new team directory and initial config.
pub struct TeamCreateTool;

#[async_trait::async_trait]
impl Tool for TeamCreateTool {
    fn name(&self) -> &'static str {
        "team_create"
    }

    fn description(&self) -> &'static str {
        "Create a new named agent team. ALWAYS pass `context` with the user's specific request \
         (e.g. which directory/files to review, what task to perform, where to write output). \
         If a blueprint is provided, all teammates defined in the blueprint's spawn-prompts.json \
         are spawned automatically — do NOT spawn them again. \
         After team_create, call `team_wait` to block until all teammates finish their initial work."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "blueprint": {
                    "type": "string",
                    "description": "Blueprint name to seed the team from [PROJECT]/.ragent/blueprints/teams/<name> or ~/.ragent/blueprints/teams/<name>"
                },
                "context": {
                    "type": "string",
                    "description": "REQUIRED: The specific work context from the user's request — which files/directories to target, what to produce, where to write output. This is prepended to every teammate's spawn prompt so they know exactly what to work on."
                },
                "name": {
                    "type": "string",
                    "description": "Optional team name (lowercase, hyphens OK). If omitted, name will be generated from blueprint and timestamp"
                },
                "project_local": {
                    "type": "boolean",
                    "description": "If true, store team in [PROJECT]/.ragent/teams/; otherwise in ~/.ragent/teams/. Default: true"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "team:manage"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // Blueprint is mandatory now. Validate presence.
        let bp = input
            .get("blueprint")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .ok_or_else(|| anyhow::anyhow!("'blueprint' parameter is required"))?;

        // Determine team name: use provided name or generate from blueprint + timestamp
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map_or_else(
                || format!("{}-{}", bp, Utc::now().format("%Y%m%d-%H-%M-%S")),
                ToString::to_string,
            );

        let project_local = input
            .get("project_local")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true);

        // Work context from the user's request — prepended to every teammate's
        // spawn prompt so they know which files/directories to target.
        let work_context = input
            .get("context")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string);

        let mut store =
            match TeamStore::create(&name, &ctx.session_id, &ctx.working_dir, project_local) {
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
        if let Some(bp) = input
            .get("blueprint")
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
        {
            // Locate blueprint directory: project-local .ragent/blueprints/teams/<bp> or ~/.ragent/blueprints/teams/<bp>
            let mut blueprint_dir: Option<std::path::PathBuf> = None;
            // Walk up to find project .ragent (check the current dir, then parent, etc.)
            let mut cur_opt = Some(ctx.working_dir.as_path());
            while let Some(cur) = cur_opt {
                let candidate = cur
                    .join(".ragent")
                    .join("blueprints")
                    .join("teams")
                    .join(bp);
                if candidate.is_dir() {
                    blueprint_dir = Some(candidate);
                    break;
                }
                cur_opt = cur.parent();
            }
            // Fallback to global
            if blueprint_dir.is_none()
                && let Some(home) = dirs::home_dir()
            {
                let candidate = home
                    .join(".ragent")
                    .join("blueprints")
                    .join("teams")
                    .join(bp);
                if candidate.is_dir() {
                    blueprint_dir = Some(candidate);
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

                // If task-seed.json exists, parse and execute each seed
                let seed = bdir.join("task-seed.json");
                if seed.exists()
                    && let Ok(raw) = std::fs::read_to_string(&seed)
                    && let Ok(arr) = serde_json::from_str::<serde_json::Value>(&raw)
                    && let Some(items) = arr.as_array()
                {
                    let registry = crate::tool::create_default_registry();
                    for item in items {
                        if let Some(obj) = item.as_object() {
                            if let Some(tool_name) = obj.get("tool").and_then(|v| v.as_str()) {
                                let args = obj
                                    .get("args")
                                    .or_else(|| obj.get("input"))
                                    .cloned()
                                    .unwrap_or(serde_json::json!({}));
                                // Always override team_name to the actual team name
                                // (seed files may contain placeholder/template names)
                                let mut args_obj = if args.is_object() {
                                    args
                                } else {
                                    serde_json::json!({})
                                };
                                args_obj.as_object_mut().map(|m| {
                                    m.insert(
                                        "team_name".to_string(),
                                        serde_json::Value::String(name.clone()),
                                    )
                                });
                                if let Some(tool) = registry.get(tool_name) {
                                    let args_debug = format!("{args_obj:?}");
                                    tracing::info!(tool = %tool_name, team = %name, session = %ctx.session_id, team_manager_present = %ctx.team_manager.is_some(), "Invoking seed tool");
                                    match tool.execute(args_obj, ctx).await {
                                        Ok(_out) => {
                                            tracing::info!(tool = %tool_name, "Seed tool executed successfully");
                                        }
                                        Err(e) => {
                                            tracing::error!(tool = %tool_name, error = %e, args = %args_debug, "Seed tool execution failed");
                                        }
                                    }
                                }
                            } else {
                                // If no tool specified but task fields present, create a task directly
                                if obj.get("title").is_some() {
                                    let title = obj
                                        .get("title")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("untitled")
                                        .to_string();
                                    let id = store.next_task_id().unwrap_or_else(|_| {
                                        format!("task-{}", chrono::Utc::now().timestamp_millis())
                                    });
                                    let mut task = crate::team::task::Task::new(&id, &title);
                                    if let Some(desc) =
                                        obj.get("description").and_then(|v| v.as_str())
                                    {
                                        task.description = desc.to_string();
                                    }
                                    let _ = store.add_task(task.clone());

                                    // Run TaskCreated hook; remove task if rejected.
                                    let hook_stdin = serde_json::json!({
                                        "team_name": name,
                                        "task_id": id,
                                        "title": title,
                                        "description": task.description,
                                    })
                                    .to_string();
                                    let outcome = run_team_hook(
                                        &store.dir,
                                        HookEvent::TaskCreated,
                                        Some(&hook_stdin),
                                    )
                                    .await;
                                    if let HookOutcome::Feedback(feedback) = outcome {
                                        if let Ok(ts) = TaskStore::open(&store.dir) {
                                            let _ = ts.remove_task(&id);
                                        }
                                        tracing::warn!(task_id = %id, feedback = %feedback, "TaskCreated hook rejected seeded task");
                                    }
                                }
                            }
                        }
                    }
                }

                // If spawn-prompts.json exists, parse entries and execute listed tools with args
                let prompts_json = bdir.join("spawn-prompts.json");
                if prompts_json.exists()
                    && let Ok(raw) = std::fs::read_to_string(&prompts_json)
                    && let Ok(val) = serde_json::from_str::<serde_json::Value>(&raw)
                    && let Some(items) = val.as_array()
                {
                    let registry = crate::tool::create_default_registry();
                    for (i, item) in items.iter().enumerate() {
                        if let Some(obj) = item.as_object() {
                            // Support either {"tool": "name", "args": {...}} or
                            // flattened formats using keys like tool_name/team_name/teammate_name
                            let tool_name_opt = obj
                                .get("tool")
                                .and_then(|v| v.as_str())
                                .or_else(|| obj.get("tool_name").and_then(|v| v.as_str()));

                            if let Some(tool_name) = tool_name_opt {
                                // Extract args if present, else build from flattened fields
                                let mut args =
                                    obj.get("args").cloned().unwrap_or(serde_json::json!({}));
                                if !args.is_object() {
                                    args = serde_json::json!({});
                                }

                                // If spawn-prompts.json uses flattened keys, copy them into args
                                // common flattened keys: team_name, teammate_name, prompt, agent_type, model, memory
                                for key in &[
                                    "team_name",
                                    "teammate_name",
                                    "prompt",
                                    "agent_type",
                                    "model",
                                    "memory",
                                ] {
                                    if args.get(*key).is_none()
                                        && let Some(v) = obj.get(*key)
                                    {
                                        args.as_object_mut()
                                            .map(|m| m.insert((*key).to_string(), v.clone()));
                                    }
                                }

                                // Allow "profile" as an alias for "agent_type" so
                                // blueprints can reference declarative agent profiles.
                                if args.get("agent_type").is_none()
                                    && let Some(v) = obj.get("profile")
                                {
                                    args.as_object_mut()
                                        .map(|m| m.insert("agent_type".to_string(), v.clone()));
                                }

                                // Ensure team_name is present
                                if args.get("team_name").is_none() {
                                    args.as_object_mut().map(|m| {
                                        m.insert(
                                            "team_name".to_string(),
                                            serde_json::Value::String(name.clone()),
                                        )
                                    });
                                }
                                // Ensure teammate_name is present, otherwise auto-generate
                                if args.get("teammate_name").is_none() {
                                    let teammate_name = format!("auto-{}", i + 1);
                                    args.as_object_mut().map(|m| {
                                        m.insert(
                                            "teammate_name".to_string(),
                                            serde_json::Value::String(teammate_name),
                                        )
                                    });
                                }

                                // Prepend work context to the spawn prompt so
                                // teammates know which code to target.
                                if let Some(ref ctx_text) = work_context {
                                    let original_prompt = args
                                        .get("prompt")
                                        .and_then(|v| v.as_str())
                                        .unwrap_or("")
                                        .to_string();
                                    let combined = format!(
                                        "## Work Context\n{ctx_text}\n\n## Your Role\n{original_prompt}"
                                    );
                                    args.as_object_mut().map(|m| {
                                        m.insert(
                                            "prompt".to_string(),
                                            serde_json::Value::String(combined),
                                        )
                                    });
                                }

                                if let Some(tool) = registry.get(tool_name) {
                                    tracing::info!(tool = %tool_name, team = %name, session = %ctx.session_id, team_manager_present = %ctx.team_manager.is_some(), "Invoking spawn tool from blueprint (spawn-prompts.json)");
                                    match tool.execute(args.clone(), ctx).await {
                                        Ok(out) => {
                                            tracing::info!(tool = %tool_name, "Spawn tool executed");
                                            if let Some(meta) = out.metadata
                                                && let Some(status) =
                                                    meta.get("status").and_then(|v| v.as_str())
                                            {
                                                if status == "pending_manager" {
                                                    tracing::warn!(tool = %tool_name, team = %name, "Spawn returned pending_manager; recording spawning member in team config");
                                                    // Record a Spawning member so the team config reflects the queued teammate.
                                                    // Reload the store fresh before mutating so we don't clobber state
                                                    // written by previous tool calls.
                                                    let teammate_name = args
                                                        .get("teammate_name")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("auto")
                                                        .to_string();
                                                    let agent_type_str = args
                                                        .get("agent_type")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("general")
                                                        .to_string();
                                                    let prompt_str = args
                                                        .get("prompt")
                                                        .and_then(|v| v.as_str())
                                                        .unwrap_or("")
                                                        .to_string();
                                                    let model_override = args
                                                        .get("model")
                                                        .and_then(|v| v.as_str())
                                                        .and_then(|s| {
                                                            s.split_once('/')
                                                                .or_else(|| s.split_once(':'))
                                                                .map(|(p, m)| {
                                                                    crate::agent::ModelRef {
                                                                        provider_id: p.to_string(),
                                                                        model_id: m.to_string(),
                                                                    }
                                                                })
                                                        });
                                                    let memory_scope = match args
                                                        .get("memory")
                                                        .and_then(|v| v.as_str())
                                                    {
                                                        Some("user") => {
                                                            crate::team::MemoryScope::User
                                                        }
                                                        Some("project") => {
                                                            crate::team::MemoryScope::Project
                                                        }
                                                        _ => crate::team::MemoryScope::None,
                                                    };
                                                    if let Ok(mut fresh_store) =
                                                        TeamStore::load_by_name(
                                                            &name,
                                                            &ctx.working_dir,
                                                        )
                                                    {
                                                        // Only add if not already present (idempotency).
                                                        if fresh_store
                                                            .config
                                                            .member_by_name(&teammate_name)
                                                            .is_none()
                                                        {
                                                            let agent_id =
                                                                fresh_store.next_agent_id();
                                                            let mut member = crate::team::config::TeamMember::new(&teammate_name, &agent_id, &agent_type_str);
                                                            if !prompt_str.is_empty() {
                                                                member.spawn_prompt =
                                                                    Some(prompt_str);
                                                            }
                                                            member.model_override = model_override;
                                                            member.memory_scope = memory_scope;
                                                            let _ = fresh_store.add_member(member);
                                                        }
                                                    }
                                                    // Keep local store in sync for next_agent_id() calculations.
                                                    store = TeamStore::load_by_name(
                                                        &name,
                                                        &ctx.working_dir,
                                                    )
                                                    .unwrap_or(store);
                                                } else if status == "spawned" {
                                                    // spawn_teammate_internal already persisted the member with the
                                                    // correct session_id and Working status.  Adding a new TeamMember
                                                    // record here (with default Spawning status and no session_id)
                                                    // would overwrite that correct state and leave the member stuck in
                                                    // "spawning" forever.  Just refresh the local store reference so
                                                    // next_agent_id() stays accurate.
                                                    store = TeamStore::load_by_name(
                                                        &name,
                                                        &ctx.working_dir,
                                                    )
                                                    .unwrap_or(store);
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            tracing::error!(tool = %tool_name, error = %e, "Spawn tool execution failed");
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // Reload store to get the final member list after blueprint seeding.
        let final_store = TeamStore::load_by_name(&name, &ctx.working_dir).unwrap_or(store);
        let member_list: Vec<String> = final_store
            .config
            .members
            .iter()
            .map(|m| {
                format!(
                    "  - {} ({}, {})",
                    m.name,
                    m.agent_id,
                    format!("{:?}", m.status).to_lowercase()
                )
            })
            .collect();
        let member_summary = if member_list.is_empty() {
            "No teammates spawned yet. Use team_spawn to add teammates.".to_string()
        } else {
            format!(
                "Blueprint spawned {} teammate(s) — do NOT spawn these again:\n{}",
                member_list.len(),
                member_list.join("\n")
            )
        };

        Ok(ToolOutput {
            content: format!(
                "Team '{}' created successfully.\nDirectory: {}\nLead session: {}\n\n{}\n\n\
                           Next: call `team_wait` to block until all teammates finish their initial work.",
                name,
                final_store.dir.display(),
                ctx.session_id,
                member_summary
            ),
            metadata: MetadataBuilder::new()
                .custom("team_name", &name)
                .custom("team_dir", final_store.dir.to_string_lossy())
                .custom("lead_session_id", &ctx.session_id)
                .custom("project_local", project_local)
                .custom("members_spawned", final_store.config.members.len())
                .custom(
                    "auto_named",
                    input
                        .get("name")
                        .and_then(|v| v.as_str())
                        .map(str::trim)
                        .is_none_or(str::is_empty),
                )
                .build(),
        })
    }
}
