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
    fn name(&self) -> &'static str {
        "team_approve_plan"
    }

    fn description(&self) -> &'static str {
        "Approve or reject a teammate's submitted plan. Lead-only. On approval, \
             the teammate exits plan-pending mode and begins implementation; on \
             rejection, provide feedback and the teammate will revise. REQUIRED \
             parameters: 'team_name' (string), 'teammate' (string or agent ID), and \
             'approved' (boolean). Optional: 'feedback' (string, required when rejecting). \
             Common gotcha: feedback is mandatory when approved is false; omitting it \
             returns an error."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team (required)"
                },
                "teammate": {
                    "type": "string",
                    "description": "Teammate name or agent ID whose plan is being reviewed (required)"
                },
                "approved": {
                    "type": "boolean",
                    "description": "true to approve, false to reject (required)"
                },
                "feedback": {
                    "type": "string",
                    "description": "Optional feedback message (required when rejecting)"
                }
            },
            "required": ["team_name", "teammate", "approved"],
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

        let teammate = input
            .get("teammate")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: teammate"))?;

        let approved = input
            .get("approved")
            .and_then(serde_json::Value::as_bool)
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

        let teammate_mailbox = Mailbox::open(&team_dir, &agent_id)?;
        let mut msg = MailboxMessage::new(
            "lead".to_string(),
            agent_id.clone(),
            if approved {
                MessageType::PlanApproved
            } else {
                MessageType::PlanRejected
            },
            feedback,
        );
        // M5-T4: copy the correlation id from the member's plan_request_id so
        // the teammate can tell which plan was approved/rejected.
        {
            let store = TeamStore::load(&team_dir)?;
            if let Some(m) = store.config.member_by_id(&agent_id) {
                msg.correlation_id = m.plan_request_id.clone();
            }
        }
        teammate_mailbox.push(msg)?;

        // M5-T4: clear the member's plan_request_id now that the reply is sent.
        {
            let mut store = TeamStore::load(&team_dir)?;
            if let Some(m) = store.config.member_by_id_mut(&agent_id) {
                m.plan_request_id = None;
            }
            store.save()?;
        }

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
