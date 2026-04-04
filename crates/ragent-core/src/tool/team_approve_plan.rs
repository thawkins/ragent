//! `team_approve_plan` — Lead approves or rejects a teammate's submitted plan.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{
    Mailbox, MailboxMessage, MemberStatus, MessageType, PlanStatus, TeamStore, find_team_dir,
};

/// Lead approves or rejects a teammate's plan.
pub struct TeamApprovePlanTool;

#[async_trait::async_trait]
impl Tool for TeamApprovePlanTool {
    fn name(&self) -> &str {
        "team_approve_plan"
    }

    fn description(&self) -> &str {
        "Approve or reject a teammate's submitted plan. Lead-only. \
         On approval, the teammate exits plan-pending mode and begins implementation. \
         On rejection, provide feedback and the teammate will revise."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team"
                },
                "teammate": {
                    "type": "string",
                    "description": "Teammate name or agent ID whose plan is being reviewed"
                },
                "approved": {
                    "type": "boolean",
                    "description": "true to approve, false to reject"
                },
                "feedback": {
                    "type": "string",
                    "description": "Optional feedback message (required when rejecting)"
                }
            },
            "required": ["team_name", "teammate", "approved"]
        })
    }

    fn permission_category(&self) -> &str {
        "team:manage"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let team_name = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: team_name"))?;

        let teammate = input
            .get("teammate")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: teammate"))?;

        let approved = input
            .get("approved")
            .and_then(|v| v.as_bool())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: approved"))?;

        let feedback = input
            .get("feedback")
            .and_then(|v| v.as_str())
            .unwrap_or(if approved {
                "Plan approved. Proceed with implementation."
            } else {
                "Plan rejected."
            });

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let agent_id = super::team_message::resolve_agent_id(&team_dir, teammate)?;

        // Update member plan_status and status.
        {
            let mut store = TeamStore::load(&team_dir)?;
            if let Some(member) = store.config.member_by_id_mut(&agent_id) {
                if approved {
                    member.plan_status = PlanStatus::Approved;
                    member.status = MemberStatus::Working;
                } else {
                    member.plan_status = PlanStatus::Rejected;
                    // Keep PlanPending so the UI shows they need to resubmit.
                }
            }
            store.save()?;
        }

        // Send approval/rejection to the teammate's mailbox.
        let teammate_mailbox = Mailbox::open(&team_dir, &agent_id)?;
        let msg_type = if approved {
            MessageType::PlanApproved
        } else {
            MessageType::PlanRejected
        };
        teammate_mailbox.push(MailboxMessage::new(
            "lead".to_string(),
            agent_id.clone(),
            msg_type,
            feedback,
        ))?;

        let verdict = if approved { "approved" } else { "rejected" };
        Ok(ToolOutput {
            content: format!(
                "Plan for teammate '{teammate}' {verdict}.\nFeedback sent: {feedback}"
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "agent_id": agent_id,
                "approved": approved,
                "feedback": feedback
            })),
        })
    }
}
