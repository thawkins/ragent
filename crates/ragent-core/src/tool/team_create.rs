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

        let store = match TeamStore::create(&name, &ctx.session_id, &ctx.working_dir, project_local) {
            Ok(store) => store,
            Err(e) if e.to_string().contains("already exists") => {
                match TeamStore::load_by_name(&name, &ctx.working_dir) {
                    Ok(existing_store) => {
                        // Team already exists: return a non-error ToolOutput with guidance
                        // and structured metadata so callers (UI/agents) can handle it.
                        return Ok(ToolOutput {
                            content: format!(
                                "Team '{}' already exists. Use team_open or /team open to reuse it.\nDirectory: {}",
                                name,
                                existing_store.dir.display()
                            ),
                            metadata: Some(json!({
                                "team_name": name,
                                "team_exists": true,
                                "team_dir": existing_store.dir.to_string_lossy(),
                                "project_local": project_local
                            })),
                        });
                    }
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
