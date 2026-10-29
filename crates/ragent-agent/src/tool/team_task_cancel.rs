//! `team_task_cancel` - Lead cancels a `Pending` or `InProgress` task.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{TaskStatus, TaskStore, find_team_dir};

/// Cancels a task so it can never be claimed or completed (lead-only).
///
/// This is the tool half of FR-005: before it existed `TaskStatus::Cancelled`
/// had **no producer at all**, so a lead could only abandon work by editing
/// `tasks.json` by hand (F-05). The tool is a thin surface over the runtime's
/// single cancellation implementation - `TeamManager::cancel_task` when the
/// session has a manager, `TaskStore::cancel` otherwise - so the status move,
/// the legal-transition check, the mailbox notice to the owning teammate and
/// the refusal wording are the same ones every other caller gets (FR-025,
/// FR-026).
pub struct TeamTaskCancelTool;

#[async_trait::async_trait]
impl Tool for TeamTaskCancelTool {
    fn name(&self) -> &'static str {
        "team_task_cancel"
    }

    fn description(&self) -> &'static str {
        "Cancel a task in the team's shared task list. LEAD-ONLY: a teammate calling this \
             is refused - only the lead may withdraw work (a teammate that cannot finish a \
             task should report it and call team_idle instead). REQUIRED parameters: \
             'team_name' (string) and 'task_id' (string, e.g. 'task-001'). Optional: \
             'reason' (string) explaining why, which is carried to the owning teammate. \
             Only a 'pending' or 'in_progress' task can be cancelled: 'completed' and \
             'cancelled' are terminal, and the refusal names the status the task actually \
             holds. A cancelled task can never be claimed or completed again, and an \
             'in_progress' task's owner is notified through its mailbox so it stops working \
             on withdrawn work. Common gotcha: this does not delete the task - it stays in \
             the list with status 'cancelled', which is how the team records that the work \
             was dropped rather than done."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team (required)"
                },
                "task_id": {
                    "type": "string",
                    "description": "ID of the task to cancel, e.g. 'task-001' (required)"
                },
                "reason": {
                    "type": "string",
                    "description": "Optional short reason for the cancellation, carried to the owning teammate's mailbox"
                }
            },
            "required": ["team_name", "task_id"],
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

        let task_id = input
            .get("task_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: task_id"))?;

        let reason = input.get("reason").and_then(|v| v.as_str());

        // FR-005: the actor is `lead` or the calling teammate's agent id. The
        // runtime enforces the lead-only rule (and refuses before the task list
        // is even loaded); naming the caller here means the refusal can say who
        // tried, instead of the tool silently acting as the lead.
        let agent_id = ctx
            .team_context
            .as_ref()
            .map_or_else(|| ctx.session_id.clone(), |tc| tc.agent_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        // One cancellation implementation (FR-025). A session with a manager
        // reaches it through the trait, so the lead-only rule and the FR-005
        // mailbox notice are the manager's own; a session without one falls back
        // to the store's `cancel` - the same in-list implementation behind it -
        // which cannot send the notice because a store holds no mailbox session
        // identity.
        let (from_status, owner, notified, title) = match ctx.team_manager.as_ref() {
            Some(manager) => {
                let (from, owner, notified) =
                    manager.cancel_task(task_id, &agent_id, reason).await?;
                let title = TaskStore::open(&team_dir)?
                    .read()?
                    .tasks
                    .iter()
                    .find(|t| t.id == task_id)
                    .map(|t| t.title.clone())
                    .unwrap_or_default();
                (from, owner, notified, title)
            }
            None => {
                tracing::debug!(
                    team = %team_name,
                    "team_task_cancel: no TeamManager in context; cancelling through the task store"
                );
                let cancelled = TaskStore::open(&team_dir)?.cancel(task_id, &agent_id)?;
                (
                    cancelled.from.as_str().to_string(),
                    cancelled.owner.clone(),
                    cancelled.notified.clone(),
                    cancelled.task.title.clone(),
                )
            }
        };

        // The lead reads a different sentence depending on whether anybody was
        // working on the task: an `InProgress` task had an owner told to stop, a
        // `Pending` one was unclaimed work the lead simply withdrew.
        let outcome = match (&notified, &owner) {
            (Some(told), _) => format!("Teammate '{told}' has been told to stop working on it."),
            (None, Some(who)) => format!("It was not in progress, so {who} was not notified."),
            (None, None) => "It was unclaimed, so nobody was notified.".to_string(),
        };

        Ok(ToolOutput {
            content: format!(
                "Task '{task_id}' cancelled in team '{team_name}'.\nTitle: {title}\n\
                 Status: {from_status} -> {}\n{outcome}\n\
                 A cancelled task cannot be claimed or completed again.",
                TaskStatus::Cancelled.as_str(),
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "task_id": task_id,
                "title": title,
                "cancelled": true,
                "from_status": from_status,
                "status": TaskStatus::Cancelled.as_str(),
                "owner": owner,
                "notified": notified,
                "actor": agent_id,
            })),
        })
    }
}
