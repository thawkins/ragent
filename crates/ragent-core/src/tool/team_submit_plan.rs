//! `team_submit_plan` — Teammate submits a plan to the lead for approval.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{Mailbox, MailboxMessage, MemberStatus, MessageType, PlanStatus, TeamStore, find_team_dir};

/// Teammate submits a plan; sets member plan_status to Pending.
pub struct TeamSubmitPlanTool;

#[async_trait::async_trait]
impl Tool for TeamSubmitPlanTool {
    fn name(&self) -> &str {
        "team_submit_plan"
    }

    fn description(&self) -> &str {
        "Submit a plan to the team lead for approval before starting implementation. \
         After calling this tool, wait for plan_approved or plan_rejected messages \
         via team_read_messages before proceeding."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team"
                },
                "plan": {
                    "type": "string",
                    "description": "Full text of the plan describing the intended approach"
                }
            },
            "required": ["team_name", "plan"]
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

        let plan = input
            .get("plan")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: plan"))?;

        let agent_id = ctx
            .team_context
            .as_ref()
            .map(|tc| tc.agent_id.clone())
            .unwrap_or_else(|| ctx.session_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        // Update member's plan_status to Pending and status to PlanPending.
        {
            let mut store = TeamStore::load(&team_dir)?;
            if let Some(member) = store.config.member_by_id_mut(&agent_id) {
                member.plan_status = PlanStatus::Pending;
                member.status = MemberStatus::PlanPending;
            }
            store.save()?;
        }

        // Send plan_request to lead mailbox.
        let lead_mailbox = Mailbox::open(&team_dir, "lead")?;
        lead_mailbox.push(MailboxMessage::new(
            agent_id.clone(),
            "lead".to_string(),
            MessageType::PlanRequest,
            plan,
        ))?;

        Ok(ToolOutput {
            content: format!(
                "Plan submitted to lead for approval.\n\
                 Use team_read_messages to check for plan_approved or plan_rejected response.\n\
                 Do not start implementation until you receive approval."
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "agent_id": agent_id,
                "plan_status": "pending"
            })),
        })
    }
}
