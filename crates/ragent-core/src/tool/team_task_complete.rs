//! `team_task_complete` — Mark a task as completed and unblock its dependents.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{HookEvent, TaskStatus, TaskStore, find_team_dir, run_team_hook};
use crate::team::manager::HookOutcome;

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
        
        // Log current state for debugging
        if let Ok(list) = store.read() {
            let task_summary: Vec<String> = list.tasks.iter()
                .map(|t| format!("{} ({})", t.id, match t.status {
                    crate::team::TaskStatus::Pending => "pending",
                    crate::team::TaskStatus::InProgress => "in-progress",
                    crate::team::TaskStatus::Completed => "completed",
                    crate::team::TaskStatus::Cancelled => "cancelled",
                }))
                .collect();
            tracing::debug!(
                agent_id = %agent_id,
                team_name = %team_name,
                task_id = %task_id,
                tasks = ?task_summary,
                "team_task_complete: attempting to complete"
            );
        }
        
        let task = match store.complete(task_id, &agent_id) {
            Ok(t) => t,
            Err(e) => {
                // Return a tool output explaining why completion failed, rather than an error.
                // This gives the teammate a clear error in the TUI instead of generic failure.
                let err_msg = e.to_string();
                tracing::warn!(
                    agent_id = %agent_id,
                    task_id = %task_id,
                    team_name = %team_name,
                    error = %err_msg,
                    "team_task_complete failed"
                );
                return Ok(ToolOutput {
                    content: format!(
                        "Failed to mark task '{}' as completed: {}\n\
                         This usually means the task doesn't exist, is already completed, \
                         or is assigned to a different agent.",
                        task_id, err_msg
                    ),
                    metadata: Some(json!({
                        "team_name": team_name,
                        "task_id": task_id,
                        "completed": false,
                        "agent_id": agent_id,
                        "error": err_msg
                    })),
                });
            }
        };

        // Run TaskCompleted hook with task metadata on stdin.
        let hook_stdin = json!({
            "team_name": team_name,
            "task_id": task.id,
            "title": task.title,
            "description": task.description,
            "completed_by": agent_id,
            "completed_at": task.completed_at.map(|t| t.to_rfc3339())
        })
        .to_string();
        let outcome = run_team_hook(&team_dir, HookEvent::TaskCompleted, Some(&hook_stdin)).await;

        if let HookOutcome::Feedback(feedback) = outcome {
            // Hook rejected completion — revert task to InProgress.
            let _ = store.update_task(task_id, |t| {
                t.status = TaskStatus::InProgress;
                t.completed_at = None;
            });
            return Ok(ToolOutput {
                content: format!(
                    "TaskCompleted hook rejected completion of task '{task_id}'. \
                     Feedback: {feedback}\n\
                     Task reverted to in-progress. Please address the feedback and complete again."
                ),
                metadata: Some(json!({
                    "team_name": team_name,
                    "task_id": task_id,
                    "hook_rejected": true,
                    "feedback": feedback,
                    "completed": false
                })),
            });
        }

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
                "completed_at": task.completed_at.map(|t| t.to_rfc3339()),
                "completed": true
            })),
        })
    }
}
