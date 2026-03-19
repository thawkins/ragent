//! `team_idle` — Teammate reports idle state; fires TeammateIdle hook.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{MemberStatus, TeamStore, find_team_dir};

/// Teammate notifies lead it has no more work (idle state).
pub struct TeamIdleTool;

#[async_trait::async_trait]
impl Tool for TeamIdleTool {
    fn name(&self) -> &str {
        "team_idle"
    }

    fn description(&self) -> &str {
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

    fn permission_category(&self) -> &str {
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
            .map(|tc| tc.agent_id.clone())
            .unwrap_or_else(|| ctx.session_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        // Mark the member as idle.
        {
            let mut store = TeamStore::load(&team_dir)?;
            if let Some(member) = store.config.member_by_id_mut(&agent_id) {
                member.status = MemberStatus::Idle;
                member.current_task_id = None;
            }
            store.save()?;
        }

        // Hook execution is implemented in M3 (TeamManager).
        // For now, log the idle event and return.
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
