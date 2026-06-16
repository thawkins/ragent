//! `task_complete` — Signal that the current autonomous task is complete.
//!
//! Used by agents in autopilot mode to indicate that a task has finished
//! and to provide a human-readable summary. The TUI displays the summary
//! and exits autopilot mode.
//!
//! ## ⚠️ DO NOT CONFUSE WITH `team_task_complete`
//!
//! These two tools have similar names but VERY different purposes and
//! parameter signatures:
//!
//! | Tool | Purpose | Required parameters |
//! |------|---------|---------------------|
//! | `task_complete`     | Signal the **current autonomous task** is done — ends the session loop. | `summary` (string) |
//! | `team_task_complete` | Mark a **team task** as completed (used inside a team session). | `team_name` (string), `task_id` (string) |
//!
//! `team_task_complete` takes `task_id` and `team_name` — NOT `summary`.
//! `task_complete` takes `summary` — NOT `task_id` or `team_name`.
//!
//! Common mistakes to avoid:
//! - Calling `task_complete` with `task_id=...` (use `team_task_complete` if you have a team task).
//! - Calling `task_complete` with a `result` or `output` key (the only field is `summary`).
//! - Calling `task_complete` to "submit" a result mid-task — it **ends the loop**; only call it when the
//!   requested work is genuinely complete (e.g. files written, tests passing).

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::event::Event;

use super::{Tool, ToolContext, ToolOutput};

/// Signals that the agent's current autonomous task is complete.
///
/// Call this tool **only** when the requested task has been fully accomplished
/// and you are ready to return control to the user. It will display the
/// `summary` to the user and stop the autonomous loop.
///
/// ## Required parameter
/// - `summary` (string) — a concise summary of what was accomplished.
///
/// This tool takes ONLY a `summary`. It does NOT take `task_id`, `team_name`,
/// `result`, or any other key. If you are inside a team session and have a
/// team task to mark complete, use `team_task_complete` instead.
pub struct TaskCompleteTool;

#[async_trait::async_trait]
impl Tool for TaskCompleteTool {
    fn name(&self) -> &'static str {
        "task_complete"
    }

    fn description(&self) -> &'static str {
        "TERMINAL SIGNAL — call ONLY when the current autonomous task is fully done. \
         This ends the agent loop and returns control to the user. \
         Takes exactly ONE required parameter: `summary` (string). \
         \n\n\
         ⚠️ DO NOT confuse with `team_task_complete` (a different tool used inside teams, \
         which takes `team_name` + `task_id`, NOT `summary`). \
         \n\n\
         Common mistakes to avoid:\n\
         - Do NOT pass `task_id`, `team_name`, `result`, or `output` — the only valid key is `summary`.\n\
         - Do NOT call this to 'submit' a result mid-task — calling it ENDS the loop.\n\
         - Do NOT call this before all requested files/outputs have been produced.\n\
         \n\n\
         Example: task_complete(summary: \"Implemented feature X, wrote 3 tests, updated docs\")"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "summary": {
                    "type": "string",
                    "description": "REQUIRED. A concise summary of what was accomplished. \
                                   This is the ONLY parameter this tool accepts — do not pass \
                                   `task_id`, `team_name`, `result`, or any other key. \
                                   If you are marking a team task complete, use `team_task_complete` instead."
                }
            },
            "required": ["summary"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "none"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let summary = input["summary"]
            .as_str()
            .context("Missing required 'summary' parameter for task_complete. \
                      The only valid parameter is `summary` (string). \
                      If you intended to mark a team task complete, use `team_task_complete` with `team_name` and `task_id`.")?;

        ctx.event_bus.publish(Event::TaskCompleted {
            session_id: ctx.session_id.clone(),
            summary: summary.to_string(),
        });

        Ok(ToolOutput {
            content: format!("✅ Task complete.\n\n{summary}"),
            metadata: Some(json!({
                "task_complete": true,
                "summary": summary
            })),
        })
    }
}
