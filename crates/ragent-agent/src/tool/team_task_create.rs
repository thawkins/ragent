//! `team_task_create` - Lead-only tool for adding tasks to the shared task list.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::manager::HookOutcome;
use crate::team::{Task, TaskStore, TeamStore, find_team_dir};

/// Adds a new task to the team's shared task list (lead-only).
pub struct TeamTaskCreateTool;

#[async_trait::async_trait]
impl Tool for TeamTaskCreateTool {
    fn name(&self) -> &'static str {
        "team_task_create"
    }

    fn description(&self) -> &'static str {
        "Add a new task to the team's shared task list. Lead-only. REQUIRED parameters: \
             'team_name' (string) and 'title' (string, short task title). Optional: \
             'description' (string, full work description) and 'depends_on' (array of \
             task IDs that must be completed before this task can be claimed). Teammates \
             will be able to claim it via team_task_claim. DEPENDENCY RULE (enforced): \
             every id in 'depends_on' must already name a task in the same team's list, and \
             the dependency graph must stay acyclic. A dangling id (the task does not \
             exist) or an edge that closes a cycle is refused at write time and the task is \
             NOT created, because such a task could never be claimed or completed - check \
             team_task_list for the ids that exist, create a dependency before the task \
             that needs it, and drop any id that would close a loop."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team (required)"
                },
                "title": {
                    "type": "string",
                    "description": "Short title for the task (required)"
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
            "required": ["team_name", "title"],
            "additionalProperties": false
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
                    .filter_map(|v| v.as_str().map(std::string::ToString::to_string))
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

        // FR-017: the `TaskCreated` hook fires from the task store's mutation
        // path, not from here, so a task seeded by `team_create` fires it too.
        let outcome = store.add_task(task).await?;

        if let HookOutcome::Feedback(feedback) = outcome {
            // Hook rejected creation - remove the task.
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
            content: format!("Task '{task_id}' created in team '{team_name}'.\nTitle: {title}"),
            metadata: Some(json!({
                "team_name": team_name,
                "task_id": task_id,
                "title": title,
                "depends_on_count": depends_on.len()
            })),
        })
    }
}
