//! `team_assign_task` — Lead assigns a specific task to a specific teammate.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{TaskStatus, TaskStore, TeamStore, find_team_dir};

/// Assigns a pending task directly to a specific teammate (lead-only).
pub struct TeamAssignTaskTool;

#[async_trait::async_trait]
impl Tool for TeamAssignTaskTool {
    fn name(&self) -> &'static str {
        "team_assign_task"
    }

    fn description(&self) -> &'static str {
        "Assign a specific pending task directly to a named teammate. Lead-only. \
         The task is marked InProgress and assigned to the specified agent."
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
                    "description": "ID of the task to assign (e.g. 'task-001')"
                },
                "to": {
                    "type": "string",
                    "description": "Teammate name or agent ID to assign the task to"
                }
            },
            "required": ["team_name", "task_id", "to"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "team:manage"
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

        let to = input
            .get("to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: to"))?;

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        // Resolve name → agent ID.
        let agent_id = super::team_message::resolve_agent_id(&team_dir, to)?;

        // Verify the agent exists in the team config.
        let config_store = TeamStore::load(&team_dir)?;
        if agent_id != "lead" && config_store.config.member_by_id(&agent_id).is_none() {
            return Err(anyhow::anyhow!(
                "Agent '{to}' (id: {agent_id}) is not a member of team '{team_name}'"
            ));
        }

        let task_store = TaskStore::open(&team_dir)?;
        let task = task_store.update_task(task_id, |t| {
            if t.status == TaskStatus::Pending {
                t.status = TaskStatus::InProgress;
                t.assigned_to = Some(agent_id.clone());
                t.claimed_at = Some(chrono::Utc::now());
            }
        })?;

        Ok(ToolOutput {
            content: format!(
                "Task '{}' assigned to '{}' in team '{}'.\nTitle: {}",
                task.id, to, team_name, task.title
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "task_id": task.id,
                "assigned_to": agent_id,
                "assignee_name": to
            })),
        })
    }
}
