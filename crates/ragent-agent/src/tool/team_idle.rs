//! `team_idle` — Teammate reports idle state; fires `TeammateIdle` hook.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::manager::HookOutcome;
use crate::team::{HookEvent, MemberStatus, TaskStore, TeamStore, find_team_dir, run_team_hook};

/// Teammate notifies lead it has no more work (idle state).
pub struct TeamIdleTool;

#[async_trait::async_trait]
impl Tool for TeamIdleTool {
    fn name(&self) -> &'static str {
        "team_idle"
    }

    fn description(&self) -> &'static str {
        "Notify the team lead that you have no more tasks to work on (idle state). \
         This triggers the TeammateIdle hook (if configured). \
         Use team_task_claim first to verify no tasks remain before calling this."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team"
                },
                "summary": {
                    "type": "string",
                    "description": "Optional summary of work completed before going idle"
                }
            },
            "required": ["team_name"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "team:communicate"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let team_name = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: team_name"))?;

        let summary = input
            .get("summary")
            .and_then(|v| v.as_str())
            .unwrap_or("No summary provided.");

        let agent_id = ctx
            .team_context
            .as_ref()
            .map_or_else(|| ctx.session_id.clone(), |tc| tc.agent_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        // Guard: block idle if this agent still has InProgress tasks that aren't completed.
        // This prevents teammates from going idle mid-task, which leaves tasks stuck.
        {
            let task_store = TaskStore::open(&team_dir)?;
            let list = task_store.read()?;
            if let Some(active) = list.in_progress_for(&agent_id) {
                return Ok(ToolOutput {
                    content: format!(
                        "⚠ You cannot go idle while task '{}' is still in progress.\n\
                         Title: {}\n\
                         Call `team_task_complete` (task_id: '{}') first, then call \
                         `team_task_claim` to pick up more work or `team_idle` once done.",
                        active.id, active.title, active.id
                    ),
                    metadata: Some(json!({
                        "team_name": team_name,
                        "agent_id": agent_id,
                        "blocked_by_task": active.id,
                        "idle_blocked": true,
                    })),
                });
            }
        }

        // Run TeammateIdle hook before committing idle state.
        let hook_stdin = json!({
            "team_name": team_name,
            "agent_id": agent_id,
            "summary": summary
        })
        .to_string();
        let outcome = run_team_hook(&team_dir, HookEvent::TeammateIdle, Some(&hook_stdin)).await;

        if let HookOutcome::Feedback(feedback) = outcome {
            // Hook rejected idle — keep teammate working.
            {
                let mut store = TeamStore::load(&team_dir)?;
                if let Some(member) = store.config.member_by_id_mut(&agent_id) {
                    member.status = MemberStatus::Working;
                }
                store.save()?;
            }
            return Ok(ToolOutput {
                content: format!(
                    "TeammateIdle hook rejected idle for '{agent_id}'. \
                     Feedback: {feedback}\n\
                     Please address the feedback and try again."
                ),
                metadata: Some(json!({
                    "team_name": team_name,
                    "agent_id": agent_id,
                    "hook_rejected": true,
                    "feedback": feedback
                })),
            });
        }

        // Mark the member as idle.
        {
            let mut store = TeamStore::load(&team_dir)?;
            if let Some(member) = store.config.member_by_id_mut(&agent_id) {
                member.status = MemberStatus::Idle;
                member.current_task_id = None;
            }
            store.save()?;
        }

        Ok(ToolOutput {
            content: format!(
                "Teammate '{agent_id}' is now idle in team '{team_name}'.\n\
                           Summary: {summary}\n\
                           Waiting for new tasks or shutdown instructions."
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "agent_id": agent_id,
                "status": "idle",
                "summary": summary
            })),
        })
    }
}
