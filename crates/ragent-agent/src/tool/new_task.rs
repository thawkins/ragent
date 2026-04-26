//! The `new_task` tool — spawns a sub-agent to perform a focused task.
//!
//! Supports both synchronous (blocking) and background (non-blocking) modes.
//! Background tasks publish [`Event::SubagentComplete`] when finished.

use anyhow::Result;
use serde_json::{Value, json};

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
    fn name(&self) -> &'static str {
        "new_task"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &'static str {
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
                    "description": "If true, spawn in the background and return immediately — the agent runs concurrently. REQUIRED when spawning more than one task in the same response; background: false blocks all subsequent tool calls. Default: false (use only for a single task whose result you need immediately)"
                },
                "model": {
                    "type": "string",
                    "description": "Optional model override (e.g. 'anthropic/claude-sonnet-4-20250514' or 'openai:gpt-4o')"
                }
            },
            "required": ["agent", "task"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "agent:spawn"
    }

    /// # Errors
    ///
    /// Returns an error if required parameters `agent` or `task` are missing,
    /// if the `TaskManager` has not been initialized, or if task spawning fails.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        if let Some(team) = ctx.team_context.as_ref() {
            let guidance = if team.is_lead {
                format!(
                    "Session '{}' is the lead of active team '{}'. \
                     Do not use `new_task` for team delegation. Use team tools (`team_spawn`, \
                     `team_task_create`, `team_assign_task`, `team_message`) so teammate activity \
                     and output stay visible in the Teams UI.",
                    ctx.session_id, team.team_name
                )
            } else {
                format!(
                    "Session '{}' is teammate '{}' in active team '{}'. \
                     Do not use `new_task` from a teammate session. Use team workflow tools \
                     (`team_read_messages`, `team_task_claim`, `team_task_complete`, `team_idle`) \
                     and report progress via team messaging.",
                    ctx.session_id, team.agent_id, team.team_name
                )
            };

            return Ok(ToolOutput {
                content: guidance,
                metadata: Some(json!({
                    "blocked": true,
                    "reason": "team_context_active",
                    "team_name": team.team_name,
                    "agent_id": team.agent_id,
                    "is_lead": team.is_lead
                })),
            });
        }

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
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        let model = input
            .get("model")
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string)
            .or_else(|| {
                // Inherit the parent session's provider/model when no explicit override
                // is given. This prevents failures when the parent uses a provider
                // (e.g. copilot) that differs from the sub-agent's hardcoded default.
                ctx.active_model
                    .as_ref()
                    .map(|m| format!("{}/{}", m.provider_id, m.model_id))
            });

        let task_manager = ctx.task_manager.as_ref().ok_or_else(|| {
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
                    model.as_deref(),
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
                    model.as_deref(),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::EventBus;
    use crate::tool::TeamContext;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn base_ctx() -> ToolContext {
        ToolContext {
            session_id: "session-1".to_string(),
            working_dir: PathBuf::from("/tmp"),
            event_bus: Arc::new(EventBus::new(16)),
            storage: None,
            task_manager: None,
            active_model: None,
            team_context: None,
            team_manager: None,
            code_index: None,
        }
    }

    #[tokio::test]
    async fn test_new_task_without_team_context_tries_to_spawn() {
        let tool = NewTaskTool;
        let ctx = base_ctx();
        let err = tool
            .execute(
                json!({
                    "agent": "explore",
                    "task": "inspect the repository"
                }),
                &ctx,
            )
            .await
            .expect_err("missing task manager should be the first failure");

        assert!(
            err.to_string()
                .contains("TaskManager has not been initialised"),
            "unexpected error: {err:#}"
        );
    }

    #[tokio::test]
    async fn test_new_task_blocks_for_active_team_sessions() {
        let tool = NewTaskTool;
        let mut ctx = base_ctx();
        ctx.team_context = Some(Arc::new(TeamContext {
            team_name: "alpha".to_string(),
            agent_id: "lead".to_string(),
            is_lead: true,
        }));

        let output = tool
            .execute(
                json!({
                    "agent": "explore",
                    "task": "inspect the repository"
                }),
                &ctx,
            )
            .await
            .expect("team-context guard should return a blocked result");

        let metadata = output
            .metadata
            .expect("blocked result should include metadata");
        assert_eq!(metadata["blocked"], true);
        assert_eq!(metadata["reason"], "team_context_active");
    }
}
