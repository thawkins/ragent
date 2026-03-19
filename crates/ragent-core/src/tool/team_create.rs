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
        let name = input
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: name"))?;

        let project_local = input
            .get("project_local")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let store = TeamStore::create(name, &ctx.session_id, &ctx.working_dir, project_local)?;

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
                "project_local": project_local
            })),
        })
    }
}
