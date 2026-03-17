//! The `new_task` tool — spawns a sub-agent to perform a focused task.
//!
//! Supports both synchronous (blocking) and background (non-blocking) modes.
//! Background tasks publish [`Event::SubagentComplete`] when finished.

use anyhow::Result;
use serde_json::{json, Value};

use super::{Tool, ToolContext, ToolOutput};

/// Spawns a sub-agent to perform a focused task.
///
/// Parameters:
/// - `agent` (string, required): Agent name (e.g. `"explore"`, `"build"`, `"plan"`).
/// - `task` (string, required): The prompt/instructions for the sub-agent.
/// - `background` (bool, optional): If `true`, spawns in the background and returns
///   immediately with a task ID. Defaults to `false` (synchronous).
/// - `model` (string, optional): Model override in `provider/model` or `provider:model` format.
pub struct NewTaskTool;

#[async_trait::async_trait]
impl Tool for NewTaskTool {
    fn name(&self) -> &str {
        "new_task"
    }

    fn description(&self) -> &str {
        "Spawn a sub-agent to perform a focused task. Supports synchronous (blocking) \
         and background (non-blocking) modes. Use agent names like 'explore', 'build', \
         'plan', or any custom agent."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "agent": {
                    "type": "string",
                    "description": "Name of the agent to run (e.g. 'explore', 'build', 'plan', 'general')"
                },
                "task": {
                    "type": "string",
                    "description": "The task prompt / instructions for the sub-agent"
                },
                "background": {
                    "type": "boolean",
                    "description": "If true, run in the background and return immediately with a task_id. Default: false"
                },
                "model": {
                    "type": "string",
                    "description": "Optional model override (e.g. 'anthropic/claude-sonnet-4-20250514' or 'openai:gpt-4o')"
                }
            },
            "required": ["agent", "task"]
        })
    }

    fn permission_category(&self) -> &str {
        "agent:spawn"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let agent = input
            .get("agent")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: agent"))?;

        let task = input
            .get("task")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: task"))?;

        let background = input
            .get("background")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let model = input
            .get("model")
            .and_then(|v| v.as_str());

        let task_manager = ctx
            .task_manager
            .as_ref()
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "Sub-agent spawning is not available in this context. \
                     TaskManager has not been initialised."
                )
            })?;

        if background {
            let entry = task_manager
                .spawn_background(
                    &ctx.session_id,
                    agent,
                    task,
                    model,
                    &ctx.working_dir,
                )
                .await?;

            Ok(ToolOutput {
                content: format!(
                    "Background task spawned successfully.\n\
                     Task ID: {}\n\
                     Agent: {}\n\
                     Status: running\n\
                     The task is running in the background. You will be notified \
                     when it completes via a SubagentComplete event.",
                    entry.id, entry.agent_name
                ),
                metadata: Some(json!({
                    "task_id": entry.id,
                    "agent": entry.agent_name,
                    "background": true,
                    "status": "running"
                })),
            })
        } else {
            let result = task_manager
                .spawn_sync(
                    &ctx.session_id,
                    agent,
                    task,
                    model,
                    &ctx.working_dir,
                )
                .await?;

            Ok(ToolOutput {
                content: result.response,
                metadata: Some(json!({
                    "task_id": result.entry.id,
                    "agent": result.entry.agent_name,
                    "background": false,
                    "status": "completed",
                    "child_session_id": result.entry.child_session_id
                })),
            })
        }
    }
}
