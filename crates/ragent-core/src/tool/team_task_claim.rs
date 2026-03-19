//! `team_task_claim` — Atomically claim the next available task.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{TaskStore, find_team_dir};

/// Atomically claims the next pending task with no unresolved dependencies.
pub struct TeamTaskClaimTool;

#[async_trait::async_trait]
impl Tool for TeamTaskClaimTool {
    fn name(&self) -> &str {
        "team_task_claim"
    }

    fn description(&self) -> &str {
        "Atomically claim the next available task from the shared task list. \
         Uses file locking to prevent race conditions between teammates. \
         Returns the task details, or a message if no tasks are available."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team"
                }
            },
            "required": ["team_name"]
        })
    }

    fn permission_category(&self) -> &str {
        "team:tasks"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let team_name = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: team_name"))?;

        let agent_id = ctx
            .team_context
            .as_ref()
            .map(|tc| tc.agent_id.clone())
            .unwrap_or_else(|| ctx.session_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let store = TaskStore::open(&team_dir)?;
        let claimed = store.claim_next(&agent_id)?;

        match claimed {
            None => Ok(ToolOutput {
                content: "No tasks available to claim at this time. \
                          All tasks are either in progress, completed, or blocked by dependencies."
                    .to_string(),
                metadata: Some(json!({
                    "team_name": team_name,
                    "claimed": false
                })),
            }),
            Some(task) => Ok(ToolOutput {
                content: format!(
                    "Claimed task '{}'.\nTitle: {}\nDescription: {}\nDependencies: {}",
                    task.id,
                    task.title,
                    task.description,
                    if task.depends_on.is_empty() {
                        "none".to_string()
                    } else {
                        task.depends_on.join(", ")
                    }
                ),
                metadata: Some(json!({
                    "team_name": team_name,
                    "claimed": true,
                    "task_id": task.id,
                    "title": task.title,
                    "description": task.description,
                    "agent_id": agent_id
                })),
            }),
        }
    }
}
