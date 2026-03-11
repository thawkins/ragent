//! Plan agent delegation tools.
//!
//! Provides [`PlanEnterTool`] and [`PlanExitTool`] for delegating work to the
//! built-in `plan` sub-agent and returning control to the previous agent.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use crate::event::Event;

use super::{Tool, ToolContext, ToolOutput};

/// Delegates the current task to the plan agent for read-only analysis.
///
/// When executed, this tool publishes an [`Event::AgentSwitchRequested`] event
/// and returns metadata with `agent_switch: "plan"`. The session processor
/// detects this metadata and breaks the agent loop so the TUI (or other
/// consumer) can switch the active agent and forward the task.
pub struct PlanEnterTool;

#[async_trait::async_trait]
impl Tool for PlanEnterTool {
    fn name(&self) -> &str {
        "plan_enter"
    }

    fn description(&self) -> &str {
        "Delegate to the plan agent for read-only codebase analysis and \
         architecture planning. The plan agent cannot modify files."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Description of what to plan or analyse"
                },
                "context": {
                    "type": "string",
                    "description": "Additional context for the plan agent"
                }
            },
            "required": ["task"]
        })
    }

    fn permission_category(&self) -> &str {
        "plan"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let task = input["task"]
            .as_str()
            .context("Missing 'task' parameter")?;

        if task.trim().is_empty() {
            bail!("Task description must not be empty");
        }

        let context = input["context"]
            .as_str()
            .unwrap_or("")
            .to_string();

        ctx.event_bus.publish(Event::AgentSwitchRequested {
            session_id: ctx.session_id.clone(),
            to: "plan".to_string(),
            task: task.to_string(),
            context: context.clone(),
        });

        let content = if context.is_empty() {
            format!("Delegating to plan agent: {}", task)
        } else {
            format!(
                "Delegating to plan agent: {}\nContext: {}",
                task, context
            )
        };

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "agent_switch": "plan",
                "task": task,
            })),
        })
    }
}

/// Returns control from the plan agent to the previous agent.
///
/// When executed, this tool publishes an [`Event::AgentRestoreRequested`] event
/// and returns metadata with `agent_restore: true`. The session processor
/// breaks the agent loop and the TUI pops the agent stack to restore the
/// previous agent, injecting the summary into the conversation.
pub struct PlanExitTool;

#[async_trait::async_trait]
impl Tool for PlanExitTool {
    fn name(&self) -> &str {
        "plan_exit"
    }

    fn description(&self) -> &str {
        "Return control from the plan agent to the previous agent. \
         Pass back a summary of the analysis or plan."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "The plan or analysis result to return to the calling agent"
                }
            },
            "required": ["summary"]
        })
    }

    fn permission_category(&self) -> &str {
        "plan"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let summary = input["summary"]
            .as_str()
            .context("Missing 'summary' parameter")?;

        if summary.trim().is_empty() {
            bail!("Summary must not be empty");
        }

        ctx.event_bus.publish(Event::AgentRestoreRequested {
            session_id: ctx.session_id.clone(),
            summary: summary.to_string(),
        });

        Ok(ToolOutput {
            content: format!("Returning to previous agent with plan summary:\n{}", summary),
            metadata: Some(json!({
                "agent_restore": true,
                "summary_length": summary.len(),
            })),
        })
    }
}
