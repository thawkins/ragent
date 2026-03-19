//! `team_status` — Returns a formatted team status report.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{MemberStatus, TaskStatus, TaskStore, TeamStore, find_team_dir};

/// Returns human-readable team status.
pub struct TeamStatusTool;

#[async_trait::async_trait]
impl Tool for TeamStatusTool {
    fn name(&self) -> &str {
        "team_status"
    }

    fn description(&self) -> &str {
        "Get the current status of a team: member list, their states, and task progress summary."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team"
                }
            },
            "required": ["team_name"]
        })
    }

    fn permission_category(&self) -> &str {
        "team:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let team_name = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: team_name"))?;

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let store = TeamStore::load(&team_dir)?;
        let task_store = TaskStore::open(&team_dir)?;
        let task_list = task_store.read()?;

        let total_tasks = task_list.tasks.len();
        let done_tasks = task_list
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Completed)
            .count();
        let in_progress_tasks = task_list
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::InProgress)
            .count();
        let pending_tasks = task_list
            .tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Pending)
            .count();

        let mut lines = vec![
            format!("Team: {} [{:?}]", store.config.name, store.config.status),
            format!("Lead session: {}", store.config.lead_session_id),
            format!(
                "Tasks: {done_tasks}/{total_tasks} done | {in_progress_tasks} in progress | {pending_tasks} pending"
            ),
            String::new(),
            format!("Members ({}):", store.config.members.len()),
        ];

        for m in &store.config.members {
            let status_icon = match m.status {
                MemberStatus::Working => "🔄",
                MemberStatus::Idle => "⏸",
                MemberStatus::PlanPending => "📋",
                MemberStatus::ShuttingDown => "🛑",
                MemberStatus::Stopped => "⬛",
                MemberStatus::Spawning => "🚀",
            };
            let task_info = m
                .current_task_id
                .as_deref()
                .map(|id| format!(" (task: {id})"))
                .unwrap_or_default();
            lines.push(format!(
                "  {status_icon} {} [{}] {:?}{task_info}",
                m.name, m.agent_id, m.status
            ));
        }

        let members_json: Vec<Value> = store
            .config
            .members
            .iter()
            .map(|m| {
                json!({
                    "name": m.name,
                    "agent_id": m.agent_id,
                    "status": format!("{:?}", m.status).to_lowercase(),
                    "current_task_id": m.current_task_id
                })
            })
            .collect();

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(json!({
                "team_name": team_name,
                "team_status": format!("{:?}", store.config.status).to_lowercase(),
                "members": members_json,
                "tasks": {
                    "total": total_tasks,
                    "done": done_tasks,
                    "in_progress": in_progress_tasks,
                    "pending": pending_tasks
                }
            })),
        })
    }
}
