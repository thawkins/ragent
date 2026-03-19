//! `team_shutdown_teammate` — Lead requests graceful shutdown of a teammate.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{Mailbox, MailboxMessage, MemberStatus, MessageType, TeamStore, find_team_dir};

/// Lead sends a shutdown_request to a teammate's mailbox.
pub struct TeamShutdownTeammateTool;

#[async_trait::async_trait]
impl Tool for TeamShutdownTeammateTool {
    fn name(&self) -> &str {
        "team_shutdown_teammate"
    }

    fn description(&self) -> &str {
        "Request graceful shutdown of a named teammate. Lead-only. \
         Sends a shutdown_request to the teammate's mailbox. \
         The teammate will call team_shutdown_ack to confirm before terminating."
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
                    "description": "Teammate name or agent ID to shut down"
                },
                "reason": {
                    "type": "string",
                    "description": "Optional reason for shutdown"
                }
            },
            "required": ["team_name", "teammate"]
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

        let reason = input
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("Shutdown requested by lead.");

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let agent_id = super::team_message::resolve_agent_id(&team_dir, teammate)?;

        // Mark member as ShuttingDown.
        {
            let mut store = TeamStore::load(&team_dir)?;
            if let Some(member) = store.config.member_by_id_mut(&agent_id) {
                member.status = MemberStatus::ShuttingDown;
            }
            store.save()?;
        }

        // Send shutdown_request to the teammate's mailbox.
        let mailbox = Mailbox::open(&team_dir, &agent_id)?;
        mailbox.push(MailboxMessage::new(
            "lead".to_string(),
            agent_id.clone(),
            MessageType::ShutdownRequest,
            reason,
        ))?;

        Ok(ToolOutput {
            content: format!(
                "Shutdown request sent to teammate '{teammate}' in team '{team_name}'.\n\
                 Reason: {reason}\n\
                 Waiting for team_shutdown_ack confirmation."
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "agent_id": agent_id,
                "status": "shutting_down"
            })),
        })
    }
}
