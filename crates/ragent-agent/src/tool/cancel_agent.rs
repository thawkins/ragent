//! The `cancel_agent` tool - cancels a running background sub-agent task.

use anyhow::Result;
use serde_json::{Value, json};

use crate::task::TaskStatus;

use super::new_agent::{REASON_MANAGER_UNAVAILABLE, REASON_TASK_NOT_FOUND, REASON_WRONG_SESSION};
use super::{Tool, ToolContext, ToolOutput};

/// Machine-readable reason for a cancel request the registry rejected
/// (FR-010). The registry's own message is kept in `metadata.error`.
pub const REASON_CANCEL_FAILED: &str = "cancel_failed";

/// The canonical status strings `cancel_agent` may report.
///
/// FR-013 forbids claiming a terminal state the registry has not reached, so a
/// successful request reports the entry's *current* status - one of these - and
/// never the word `cancelled`. `unknown` is the one documented refinement: it
/// marks a status the tool could not read back (FR-007), never a real
/// [`TaskStatus`].
const STATUS_UNKNOWN: &str = "unknown";

/// Cancels a running background sub-agent task by its task ID.
///
/// Parameters:
/// - `task_id` (string, required): The ID of the task to cancel.
pub struct CancelAgentTool;

#[async_trait::async_trait]
impl Tool for CancelAgentTool {
    fn name(&self) -> &'static str {
        "cancel_agent"
    }

    /// Returns the generated description of the tool (FR-016).
    ///
    /// It names every property in [`Self::parameters_schema`];
    /// `test_tool_descriptions` asserts that each one appears here.
    fn description(&self) -> &'static str {
        "Cancel a sub-agent task that was spawned with `background: true`. \
             REQUIRED parameter: `task_id` (string) - the unique task identifier returned by \
             new_agent when background mode was used. The task must belong to the calling \
             session; a task from another session is refused.\n\n\
             A successful call means the cancel was REQUESTED, not that the task is finished: \
             the result reports `cancel_requested: true` plus the task's CURRENT status (read \
             back from the registry, never assumed), and the terminal `cancelled` status \
             arrives later - poll `list_agents` or await the task with `wait_agents`. \
             Cancellation is the supported way to free a slot of the background limit: \
             suspending a task frees no headroom.\n\n\
             A parameter this schema does not declare is refused, not ignored. \
             Common gotcha: do not use this tool for team tasks - inside a team, use team \
             tools such as team_shutdown_teammate instead."
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
    /// Cancelling is a *control* action over a task that already exists, not a
    /// spawn (FR-005): it creates nothing and is gated like `wait_agents`, so a
    /// single `agent:control` rule covers both.
    fn permission_category(&self) -> &'static str {
        "agent:control"
    }

    /// Cancels a running background sub-agent task.
    ///
    /// A successful request reports `cancel_requested: true` and the task's
    /// *current* status (FR-013), never a terminal state the registry has not
    /// reached. A request that cannot be delivered reports the status the task
    /// is actually in (FR-040).
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The call carries a property the schema does not declare (FR-004, FR-038)
    /// - The `task_id` parameter is missing or invalid
    /// - `AgentManager` is not available in the context (`reason_code: manager_unavailable`)
    /// - The task does not belong to the current session (`reason_code: wrong_session`)
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // FR-004 / FR-038: an undeclared property is refused, not ignored.
        super::param_schema::validate_tool_input(&self.parameters_schema(), &input)?;

        let task_id = input
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: task_id"))?;

        let agent_manager = ctx.agent_manager.as_ref().ok_or_else(|| {
            super::param_schema::reason_error(
                REASON_MANAGER_UNAVAILABLE,
                "Sub-agent management is not available in this context. \
                 AgentManager has not been initialised.",
            )
        })?;

        // Verify the task belongs to this session
        let entry = agent_manager.get_task(task_id).await;
        match entry {
            Some(ref e) if e.parent_session_id != ctx.session_id => {
                return Err(super::param_schema::reason_error(
                    REASON_WRONG_SESSION,
                    format!(
                        "Task '{}' does not belong to session '{}'. \
                         Only the session that spawned a task can cancel it - \
                         list your own tasks with `list_agents`.",
                        task_id, ctx.session_id
                    ),
                ));
            }
            None => {
                return Ok(ToolOutput {
                    content: format!(
                        "Task '{task_id}' not found. It may have already completed or was never \
                         created. List the tasks this session can see with `list_agents`."
                    ),
                    // FR-010: the not-found case is a result, not an error, so
                    // the reason rides in the metadata the caller reads. The
                    // `cancel_requested` flag uses the same vocabulary as the
                    // accepted path (FR-013), so one read answers "did this call
                    // request a cancel?" on every branch.
                    metadata: Some(super::param_schema::with_reason(
                        Some(json!({
                            "task_id": task_id,
                            "cancel_requested": false,
                            "status": STATUS_UNKNOWN
                        })),
                        REASON_TASK_NOT_FOUND,
                    )),
                });
            }
            _ => {}
        }

        // FR-013: a cancel request is not a cancellation. The registry keeps
        // running the task until its loop reaches a safe point, so the status
        // reported here is whatever the entry holds *after* the request - read
        // back rather than assumed - and `cancel_requested` carries the fact
        // that a request was accepted so the two are never conflated.
        match agent_manager.cancel_agent(task_id).await {
            Ok(()) => {
                let status = agent_manager
                    .get_task(task_id)
                    .await
                    .map(|e| e.status)
                    .unwrap_or(TaskStatus::Running);
                let status_str = status.as_str();
                Ok(ToolOutput {
                    content: format!(
                        "Cancel requested for task '{task_id}'. \
                         Current status: {status_str}. The task stops at its next \
                         safe point - poll `list_agents` or await it with \
                         `wait_agents` for the terminal status."
                    ),
                    metadata: Some(json!({
                        "task_id": task_id,
                        "cancel_requested": true,
                        "status": status_str
                    })),
                })
            }
            Err(e) => {
                // FR-040: a request that could not be delivered is a failure
                // that names the status the task is actually in. The registry's
                // cancel flag is already gone for a task whose loop finished, so
                // re-reading the entry turns "already completed" into a status
                // the caller can act on instead of a bare error string.
                let status = agent_manager
                    .get_task(task_id)
                    .await
                    .map(|entry| entry.status);
                let (reason, status_str, detail) = match status {
                    Some(TaskStatus::Running | TaskStatus::Suspended | TaskStatus::Terminating) => {
                        // Still live in the registry: the flag vanished for a
                        // reason the caller cannot infer from the status, so
                        // keep it a plain delivery failure.
                        (REASON_CANCEL_FAILED, STATUS_UNKNOWN, e.to_string())
                    }
                    Some(status) => (
                        REASON_CANCEL_FAILED,
                        status.as_str(),
                        format!(
                            "Task '{task_id}' is already {} - there is nothing to cancel.",
                            status.as_str()
                        ),
                    ),
                    None => (
                        REASON_CANCEL_FAILED,
                        STATUS_UNKNOWN,
                        format!("Task '{task_id}' is no longer tracked: {e}"),
                    ),
                };
                Ok(ToolOutput {
                    content: format!("Could not cancel task '{task_id}': {detail}"),
                    // FR-010: the caller can branch on `reason` instead of matching
                    // the English text of `error`.
                    metadata: Some(super::param_schema::with_reason(
                        Some(json!({
                            "task_id": task_id,
                            "cancel_requested": false,
                            "status": status_str,
                            "error": e.to_string()
                        })),
                        reason,
                    )),
                })
            }
        }
    }
}
