//! `team_task_complete` — Mark a task as completed and unblock its dependents.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::event::Event;
use crate::team::manager::HookOutcome;
use crate::team::{HookEvent, TaskStatus, TaskStore, find_team_dir, run_team_hook};

/// Marks a task as completed by the calling agent.
pub struct TeamTaskCompleteTool;

#[async_trait::async_trait]
impl Tool for TeamTaskCompleteTool {
    fn name(&self) -> &'static str {
        "team_task_complete"
    }

    fn description(&self) -> &'static str {
        "Mark a TEAM task as completed (used inside a team session). REQUIRED \
             parameters: 'team_name' (string) and 'task_id' (string, the ID of the task \
             you claimed via team_task_claim, e.g. 'task-001'). The task must be currently \
             assigned to the caller; completing it unblocks any dependent tasks. \
             \\\n\\\n\
             WARNING: DO NOT confuse with `agent_complete` (a different tool used OUTSIDE teams to \
             signal the end of the autonomous loop, which takes `summary` as its only parameter). \
             This tool takes `team_name` + `task_id` — NOT `summary`. \
             \\\n\\\n\
             Example: team_task_complete(team_name: \"audit-team\", task_id: \"task-001\")"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "REQUIRED. Name of the team. If you are NOT inside a team session, this tool will fail — use `agent_complete(summary: ...)` instead to end the autonomous loop."
                },
                "task_id": {
                    "type": "string",
                    "description": "REQUIRED. ID of the task to mark as completed (e.g. 'task-001'). This must be a task ID you claimed via `team_task_claim`."
                }
            },
            "required": ["team_name", "task_id"],
            "additionalProperties": false
        })
    }
    fn permission_category(&self) -> &'static str {
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
            .map_or_else(|| ctx.session_id.clone(), |tc| tc.agent_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let store = TaskStore::open(&team_dir)?;

        // PERF-018: gate the debug read behind `tracing::enabled!(DEBUG)` so
        // the per-completion `store.read()` (a full file read + deserialise)
        // only happens when debug logging is actually enabled, instead of
        // on every `team_task_complete` call.
        if tracing::enabled!(tracing::Level::DEBUG) {
            if let Ok(list) = store.read() {
                let task_summary: Vec<String> = list
                    .tasks
                    .iter()
                    .map(|t| {
                        format!(
                            "{} ({})",
                            t.id,
                            match t.status {
                                crate::team::TaskStatus::Pending => "pending",
                                crate::team::TaskStatus::InProgress => "in_progress",
                                crate::team::TaskStatus::Completed => "completed",
                                crate::team::TaskStatus::Cancelled => "cancelled",
                            }
                        )
                    })
                    .collect();
                tracing::debug!(
                    agent_id = %agent_id,
                    team_name = %team_name,
                    task_id = %task_id,
                    tasks = ?task_summary,
                    "team_task_complete: attempting to complete"
                );
            }
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
                        "Failed to mark task '{task_id}' as completed: {err_msg}\n\
                         This usually means the task doesn't exist, is already completed, \
                         or is assigned to a different agent."
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

        // M5-T7: publish TeamTaskCompleted so the TUI/SSE observe the
        // completion (the event variant already existed but was never
        // published).
        // PERF-018: prefer the in-memory `TeamManager` (when available on
        // the `ToolContext`) for the lead session id instead of loading
        // `TeamStore` from disk on every completion.
        let lead_sid = ctx
            .team_manager
            .as_ref()
            .and_then(|tm| tm.lead_session_id().map(str::to_string))
            .or_else(|| {
                crate::team::TeamStore::load(&team_dir)
                    .ok()
                    .map(|s| s.config.lead_session_id.clone())
            })
            .unwrap_or_else(|| ctx.session_id.clone());
        ctx.event_bus.publish(Event::TeamTaskCompleted {
            session_id: lead_sid,
            team_name: team_name.to_string(),
            agent_id: agent_id.clone(),
            task_id: task.id.clone(),
        });

        // M8-T3: clear the member's current_task_id now that the task is
        // completed.
        // PERF-018: a single `TeamStore::load` + `save` cycle clears the
        // field. (The previous version also did exactly one load here after
        // PERF-005 removed the duplicate event publish; this comment just
        // records that the count remains at one disk read+write.)
        if let Ok(mut store) = crate::team::TeamStore::load(&team_dir) {
            if let Some(m) = store.config.member_by_id_mut(&agent_id) {
                m.current_task_id = None;
            }
            let _ = store.save();
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
