//! `task_complete` — Signal that the current autonomous task is complete.
//!
//! Used by agents in autopilot mode to indicate that a task has finished
//! and to provide a human-readable summary. The TUI displays the summary
//! and exits autopilot mode.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::event::Event;

use super::{Tool, ToolContext, ToolOutput};

/// Signals that the agent's current autonomous task is complete.
///
/// Call this tool when the requested task has been fully accomplished.
/// It will display the summary to the user and stop the autonomous loop.
pub struct TaskCompleteTool;

#[async_trait::async_trait]
impl Tool for TaskCompleteTool {
    fn name(&self) -> &str {
        "task_complete"
    }

    fn description(&self) -> &str {
        "Signal that the current task is fully complete. Call this when you have \
         finished all requested work. Provide a concise summary of what was \
         accomplished. This stops the autonomous loop and returns control to the user."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "A concise summary of what was accomplished"
                }
            },
            "required": ["summary"]
        })
    }

    fn permission_category(&self) -> &str {
        "task:complete"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let summary = input["summary"]
            .as_str()
            .context("Missing required 'summary' parameter")?;

        ctx.event_bus.publish(Event::TaskCompleted {
            session_id: ctx.session_id.clone(),
            summary: summary.to_string(),
        });

        Ok(ToolOutput {
            content: format!("✅ Task complete.\n\n{}", summary),
            metadata: Some(json!({
                "task_complete": true,
                "summary": summary
            })),
        })
    }
}
