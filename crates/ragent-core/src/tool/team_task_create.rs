//! `team_task_create` — Lead-only tool for adding tasks to the shared task list.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{HookEvent, Task, TaskStore, TeamStore, find_team_dir, run_team_hook};
use crate::team::manager::HookOutcome;

/// Adds a new task to the team's shared task list (lead-only).
pub struct TeamTaskCreateTool;

#[async_trait::async_trait]
impl Tool for TeamTaskCreateTool {
    fn name(&self) -> &str {
        "team_task_create"
    }

    fn description(&self) -> &str {
        "Add a new task to the team's shared task list. Lead-only. \
         Teammates will be able to claim it via team_task_claim."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team"
                },
                "title": {
                    "type": "string",
                    "description": "Short title for the task"
                },
                "description": {
                    "type": "string",
                    "description": "Full description of the work to be done"
                },
                "depends_on": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Task IDs that must be completed before this task can be claimed"
                }
            },
            "required": ["team_name", "title"]
        })
    }

    fn permission_category(&self) -> &str {
        "team:manage"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let team_name = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: team_name"))?;

        let title = input
            .get("title")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: title"))?;

        let description = input
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let depends_on: Vec<String> = input
            .get("depends_on")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(|s| s.to_string()))
                    .collect()
            })
            .unwrap_or_default();

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let store = TeamStore::load(&team_dir)?;
        let task_id = store.next_task_id()?;

        let mut task = Task::new(&task_id, title);
        task.description = description.clone();
        task.depends_on = depends_on.clone();

        store.add_task(task)?;

        // Run TaskCreated hook with task metadata on stdin.
        let hook_stdin = json!({
            "team_name": team_name,
            "task_id": task_id,
            "title": title,
            "description": description,
            "depends_on": depends_on
        })
        .to_string();
        let outcome = run_team_hook(&team_dir, HookEvent::TaskCreated, Some(&hook_stdin)).await;

        if let HookOutcome::Feedback(feedback) = outcome {
            // Hook rejected creation — remove the task.
            let task_store = TaskStore::open(&team_dir)?;
            let _ = task_store.remove_task(&task_id);
            return Ok(ToolOutput {
                content: format!(
                    "TaskCreated hook rejected task '{task_id}'. \
                     Feedback: {feedback}"
                ),
                metadata: Some(json!({
                    "team_name": team_name,
                    "task_id": task_id,
                    "hook_rejected": true,
                    "feedback": feedback
                })),
            });
        }

        Ok(ToolOutput {
            content: format!(
                "Task '{task_id}' created in team '{team_name}'.\nTitle: {title}"
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "task_id": task_id,
                "title": title,
                "depends_on": depends_on
            })),
        })
    }
}
