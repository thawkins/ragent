//! The `cancel_task` tool — cancels a running background sub-agent task.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Cancels a running background sub-agent task by its task ID.
///
/// Parameters:
/// - `task_id` (string, required): The ID of the task to cancel.
pub struct CancelTaskTool;

#[async_trait::async_trait]
impl Tool for CancelTaskTool {
    fn name(&self) -> &'static str {
        "cancel_task"
    }

    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &'static str {
        "Cancel a running background sub-agent task that was spawned with background: true. \
             REQUIRED parameter: 'task_id' (string, the unique task identifier returned by \
             new_task when background mode was used). The task must belong to the current \
             session. Common gotcha: do not use this tool for team tasks — inside a team, \
             use team tools such as team_shutdown_teammate instead."
    }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task_id": {
                    "type": "string",
                    "description": "The unique identifier of the task to cancel"
                }
            },
            "required": ["task_id"],
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
        "agent:spawn"
    }

    /// Cancels a running background sub-agent task.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `task_id` parameter is missing or invalid
    /// - `TaskManager` is not available in the context
    /// - The task does not belong to the current session
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let task_id = input
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: task_id"))?;

        let task_manager = ctx.task_manager.as_ref().ok_or_else(|| {
            anyhow::anyhow!(
                "Sub-agent management is not available in this context. \
                     TaskManager has not been initialised."
            )
        })?;

        // Verify the task belongs to this session
        let entry = task_manager.get_task(task_id).await;
        match entry {
            Some(ref e) if e.parent_session_id != ctx.session_id => {
                anyhow::bail!(
                    "Task '{}' does not belong to session '{}'",
                    task_id,
                    ctx.session_id
                );
            }
            None => {
                return Ok(ToolOutput {
                    content: format!(
                        "Task '{task_id}' not found. It may have already completed or was never created."
                    ),
                    metadata: Some(json!({ "task_id": task_id, "cancelled": false })),
                });
            }
            _ => {}
        }

        match task_manager.cancel_task(task_id).await {
            Ok(()) => Ok(ToolOutput {
                content: format!("Task '{task_id}' has been cancelled."),
                metadata: Some(json!({
                    "task_id": task_id,
                    "status": "cancelled"
                })),
            }),
            Err(e) => Ok(ToolOutput {
                content: format!("Could not cancel task '{task_id}': {e}"),
                metadata: Some(json!({
                    "task_id": task_id,
                    "status": "cancel_failed",
                    "error": e.to_string()
                })),
            }),
        }
    }
}
