//! `team_assign_task` — Lead assigns a specific task to a specific teammate.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{
    Mailbox, MailboxMessage, MemberStatus, MessageType, TaskStatus, TaskStore, TeamStore,
    find_team_dir,
};

/// Assigns a pending task directly to a specific teammate (lead-only).
///
/// M4-T2: after updating `tasks.json`, the tool pushes a `MailboxMessage`
/// to the assigned teammate's mailbox so they are notified immediately
/// instead of having to poll `team_task_list` / `team_task_claim`.
pub struct TeamAssignTaskTool;

#[async_trait::async_trait]
impl Tool for TeamAssignTaskTool {
    fn name(&self) -> &'static str {
        "team_assign_task"
    }

    fn description(&self) -> &'static str {
        "Assign a specific pending task directly to a named teammate. Lead-only. \
         The task is marked InProgress and assigned to the specified agent. \
         The assigned teammate is notified via a mailbox message."
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
                    "description": "ID of the task to assign (e.g. 'task-001')"
                },
                "to": {
                    "type": "string",
                    "description": "Teammate name or agent ID to assign the task to"
                }
            },
            "required": ["team_name", "task_id", "to"]
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

        let to = input
            .get("to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: to"))?;

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        // Resolve name → agent ID.
        let agent_id = super::team_message::resolve_agent_id(&team_dir, to)?;

        // Verify the agent exists in the team config and is not dead.
        let config_store = TeamStore::load(&team_dir)?;
        if agent_id != "lead" && config_store.config.member_by_id(&agent_id).is_none() {
            return Err(anyhow::anyhow!(
                "Agent '{to}' (id: {agent_id}) is not a member of team '{team_name}'"
            ));
        }
        if let Some(member) = config_store.config.member_by_id(&agent_id) {
            if matches!(member.status, MemberStatus::Stopped | MemberStatus::Failed) {
                return Err(anyhow::anyhow!(
                    "Agent '{to}' (id: {agent_id}) is {} and cannot be assigned tasks in team '{team_name}'",
                    member.status.as_str()
                ));
            }
        }

        let task_store = TaskStore::open(&team_dir)?;
        let task = task_store.update_task(task_id, |t| {
            if t.status == TaskStatus::Pending {
                t.status = TaskStatus::InProgress;
                t.assigned_to = Some(agent_id.clone());
                t.claimed_at = Some(chrono::Utc::now());
            }
        })?;

        // M4-T2: notify the assigned teammate via their mailbox so they pick
        // up the task without having to poll. This is a best-effort delivery:
        // a failure here does not roll back the assignment (the task is
        // already InProgress on disk) — we record the notification outcome
        // in the tool output so the lead has visibility.
        let notification = match Mailbox::open(&team_dir, &agent_id) {
            Ok(mailbox) => {
                let content = format!(
                    "Task '{}' has been assigned to you by the lead.\nTitle: {}\n\
                     Call `team_task_claim` is not needed — the task is already InProgress and \
                     assigned to you. Use `team_task_complete` when done.",
                    task.id, task.title
                );
                match mailbox.push(MailboxMessage::new(
                    "lead".to_string(),
                    agent_id.clone(),
                    MessageType::Message,
                    content,
                )) {
                    Ok(()) => "delivered".to_string(),
                    Err(e) => format!("failed: {e}"),
                }
            }
            Err(e) => format!("failed to open mailbox: {e}"),
        };

        Ok(ToolOutput {
            content: format!(
                "Task '{}' assigned to '{}' in team '{}'.\nTitle: {}\n\
                 Notification: {notification}.",
                task.id, to, team_name, task.title
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "task_id": task.id,
                "assigned_to": agent_id,
                "assignee_name": to,
                "notification": notification
            })),
        })
    }
}
