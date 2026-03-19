//! `team_broadcast` — Send a message to all active teammates.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{Mailbox, MailboxMessage, MemberStatus, MessageType, TeamStore, find_team_dir};

/// Sends a broadcast message to all active (non-stopped) teammates.
pub struct TeamBroadcastTool;

#[async_trait::async_trait]
impl Tool for TeamBroadcastTool {
    fn name(&self) -> &str {
        "team_broadcast"
    }

    fn description(&self) -> &str {
        "Send a message to all active (non-stopped) teammates in the team simultaneously."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team"
                },
                "content": {
                    "type": "string",
                    "description": "Broadcast message text"
                }
            },
            "required": ["team_name", "content"]
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

        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: content"))?;

        let from = ctx
            .team_context
            .as_ref()
            .map(|tc| tc.agent_id.clone())
            .unwrap_or_else(|| "lead".to_string());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let store = TeamStore::load(&team_dir)?;
        let active: Vec<_> = store
            .config
            .members
            .iter()
            .filter(|m| m.status != MemberStatus::Stopped)
            .map(|m| m.agent_id.clone())
            .collect();

        let mut sent = 0usize;
        for agent_id in &active {
            let mailbox = Mailbox::open(&team_dir, agent_id)?;
            mailbox.push(MailboxMessage::new(
                from.clone(),
                agent_id.clone(),
                MessageType::Broadcast,
                content,
            ))?;
            sent += 1;
        }

        Ok(ToolOutput {
            content: format!(
                "Broadcast sent to {sent} active teammate(s) in team '{team_name}'."
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "recipients": active,
                "sent_count": sent
            })),
        })
    }
}
