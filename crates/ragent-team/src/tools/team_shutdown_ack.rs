//! `team_shutdown_ack` — Teammate acknowledges a shutdown request.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{Mailbox, MailboxMessage, MemberStatus, MessageType, TeamStore, find_team_dir};

/// Teammate acknowledges shutdown; marks itself as Stopped.
pub struct TeamShutdownAckTool;

#[async_trait::async_trait]
impl Tool for TeamShutdownAckTool {
    fn name(&self) -> &'static str {
        "team_shutdown_ack"
    }

    fn description(&self) -> &'static str {
        "Acknowledge a shutdown request from the team lead. \
         Call this after receiving a shutdown_request via team_read_messages. \
         The teammate session will terminate after this call."
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

    fn permission_category(&self) -> &'static str {
        "team:communicate"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let team_name = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: team_name"))?;

        let agent_id = ctx
            .team_context
            .as_ref()
            .map_or_else(|| ctx.session_id.clone(), |tc| tc.agent_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        // Mark member as Stopped.
        {
            let mut store = TeamStore::load(&team_dir)?;
            if let Some(member) = store.config.member_by_id_mut(&agent_id) {
                member.status = MemberStatus::Stopped;
                member.current_task_id = None;
            }
            store.save()?;
        }

        // Send ack to lead mailbox.
        // M5-T4: copy the correlation id from the member's shutdown_request_id
        // so the lead can pair request/ack.
        let correlation_id = {
            let store = TeamStore::load(&team_dir)?;
            store
                .config
                .member_by_id(&agent_id)
                .and_then(|m| m.shutdown_request_id.clone())
        };
        let lead_mailbox = Mailbox::open(&team_dir, "lead")?;
        let mut msg = MailboxMessage::new(
            agent_id.clone(),
            "lead".to_string(),
            MessageType::ShutdownAck,
            format!("Teammate '{agent_id}' acknowledges shutdown and is terminating."),
        );
        msg.correlation_id = correlation_id;
        lead_mailbox.push(msg)?;

        // M5-T4: clear the member's shutdown_request_id now that the ack is sent.
        {
            let mut store = TeamStore::load(&team_dir)?;
            if let Some(m) = store.config.member_by_id_mut(&agent_id) {
                m.shutdown_request_id = None;
            }
            store.save()?;
        }

        Ok(ToolOutput {
            content: format!(
                "Shutdown acknowledged. Teammate '{agent_id}' will now terminate.\n\
                 Goodbye from team '{team_name}'."
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "agent_id": agent_id,
                "status": "stopped"
            })),
        })
    }
}
