//! `team_task_list` — Read all tasks and their status (read-only).

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{TaskStatus, TaskStore, find_team_dir};

/// Returns the full task list for a team.
pub struct TeamTaskListTool;

#[async_trait::async_trait]
impl Tool for TeamTaskListTool {
    fn name(&self) -> &str {
        "team_task_list"
    }

    fn description(&self) -> &str {
        "List all tasks in the team's shared task list, including their status, \
         assignment, and dependencies."
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
        "team:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let team_name = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: team_name"))?;

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let store = TaskStore::open(&team_dir)?;
        let list = store.read()?;

        if list.tasks.is_empty() {
            return Ok(ToolOutput {
                content: format!("No tasks in team '{team_name}' yet."),
                metadata: Some(json!({ "team_name": team_name, "tasks": [] })),
            });
        }

        let mut lines = vec![format!("Tasks for team '{team_name}':\n")];
        for task in &list.tasks {
            let status_icon = match task.status {
                TaskStatus::Pending => "⬜",
                TaskStatus::InProgress => "🔄",
                TaskStatus::Completed => "✅",
                TaskStatus::Cancelled => "❌",
            };
            let assignee = task
                .assigned_to
                .as_deref()
                .unwrap_or("unassigned");
            let deps = if task.depends_on.is_empty() {
                String::new()
            } else {
                format!(" [deps: {}]", task.depends_on.join(", "))
            };
            lines.push(format!(
                "{status_icon} [{}] {} — {}{deps}",
                task.id, task.title, assignee
            ));
            if !task.description.is_empty() {
                lines.push(format!("   {}", task.description));
            }
        }

        let tasks_json: Vec<Value> = list
            .tasks
            .iter()
            .map(|t| {
                json!({
                    "id": t.id,
                    "title": t.title,
                    "status": format!("{:?}", t.status).to_lowercase(),
                    "assigned_to": t.assigned_to,
                    "depends_on": t.depends_on
                })
            })
            .collect();

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(json!({
                "team_name": team_name,
                "tasks": tasks_json,
                "total": list.tasks.len()
            })),
        })
    }
}
