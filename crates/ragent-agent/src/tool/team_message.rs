//! `team_message` — Send a direct message to one team member.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{Mailbox, MailboxMessage, MessageType, TeamStore, find_team_dir};

/// Sends a direct message to one team member by name.
pub struct TeamMessageTool;

#[async_trait::async_trait]
impl Tool for TeamMessageTool {
    fn name(&self) -> &'static str {
        "team_message"
    }

    fn description(&self) -> &'static str {
        "Send a direct message to one team member (teammate or lead) by agent ID or name."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team"
                },
                "to": {
                    "type": "string",
                    "description": "Recipient agent ID (e.g. 'tm-001') or 'lead'"
                },
                "content": {
                    "type": "string",
                    "description": "Message text to send"
                }
            },
            "required": ["team_name", "to", "content"]
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

        let to = input
            .get("to")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: to"))?;

        let content = input
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: content"))?;

        let from = ctx
            .team_context
            .as_ref()
            .map_or_else(|| "lead".to_string(), |tc| tc.agent_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        // Resolve recipient agent ID if a name was given.
        let recipient_id = resolve_agent_id(&team_dir, to)?;

        let mailbox = Mailbox::open(&team_dir, &recipient_id)?;
        mailbox.push(MailboxMessage::new(
            from.clone(),
            recipient_id.clone(),
            MessageType::Message,
            content,
        ))?;

        Ok(ToolOutput {
            content: format!("Message sent to '{to}' in team '{team_name}'."),
            metadata: Some(json!({
                "team_name": team_name,
                "from": from,
                "to": recipient_id,
                "message_count": 1
            })),
        })
    }
}

/// Resolve a teammate name to an agent ID by looking it up in config.json.
/// If `name_or_id` is already an agent ID (starts with "tm-" or is "lead"), return it as-is.
pub(crate) fn resolve_agent_id(team_dir: &std::path::Path, name_or_id: &str) -> Result<String> {
    if name_or_id == "lead" || name_or_id.starts_with("tm-") {
        return Ok(name_or_id.to_string());
    }
    // Try to find a member with this name.
    let store = TeamStore::load(team_dir)?;
    store
        .config
        .member_by_name(name_or_id)
        .map(|m| m.agent_id.clone())
        .ok_or_else(|| anyhow::anyhow!("No teammate named '{name_or_id}' in this team"))
}
