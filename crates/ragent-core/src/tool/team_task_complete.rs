//! `team_task_complete` — Mark a task as completed and unblock its dependents.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{TaskStore, find_team_dir};

/// Marks a task as completed by the calling agent.
pub struct TeamTaskCompleteTool;

#[async_trait::async_trait]
impl Tool for TeamTaskCompleteTool {
    fn name(&self) -> &str {
        "team_task_complete"
    }

    fn description(&self) -> &str {
        "Mark a task as completed. The task must be currently assigned to the caller. \
         Completing a task automatically unblocks any tasks that depend on it."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team"
                },
                "task_id": {
                    "type": "string",
                    "description": "ID of the task to mark as completed (e.g. 'task-001')"
                }
            },
            "required": ["team_name", "task_id"]
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

        let task_id = input
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: task_id"))?;

        let agent_id = ctx
            .team_context
            .as_ref()
            .map(|tc| tc.agent_id.clone())
            .unwrap_or_else(|| ctx.session_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let store = TaskStore::open(&team_dir)?;
        let task = store.complete(task_id, &agent_id)?;

        Ok(ToolOutput {
            content: format!(
                "Task '{}' marked as completed by '{}'.\nTitle: {}",
                task.id, agent_id, task.title
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "task_id": task.id,
                "title": task.title,
                "completed_by": agent_id,
                "completed_at": task.completed_at.map(|t| t.to_rfc3339())
            })),
        })
    }
}
