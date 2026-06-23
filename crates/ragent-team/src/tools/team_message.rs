//! `team_message` — Send a direct message to one team member.
//!
//! M4-T4: validates the recipient exists and is not `Stopped` / `Failed`
//! before pushing, so messages to dead teammates are rejected up front
//! rather than sitting unread forever while the sender gets a false success.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{Mailbox, MailboxMessage, MemberStatus, MessageType, TeamStore, find_team_dir};

/// Sends a direct message to one team member by name.
pub struct TeamMessageTool;

#[async_trait::async_trait]
impl Tool for TeamMessageTool {
    fn name(&self) -> &'static str {
        "team_message"
    }

    fn description(&self) -> &'static str {
        "Send a direct message to one team member (teammate or lead) by agent ID or name. \
         The recipient must be an active member of the team; messages to Stopped or Failed \
         teammates are rejected."
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

        // M4-T4: validate the recipient is an active member before pushing.
        // Messages to `lead` are always allowed (the lead mailbox is always
        // valid). For teammates, reject `Stopped` / `Failed` so the sender
        // gets an error instead of a false success.
        if recipient_id != "lead" {
            let store = TeamStore::load(&team_dir)?;
            match store.config.member_by_id(&recipient_id) {
                None => {
                    return Err(anyhow::anyhow!(
                        "Recipient '{to}' (id: {recipient_id}) is not a member of team '{team_name}'"
                    ));
                }
                Some(m) if matches!(m.status, MemberStatus::Stopped | MemberStatus::Failed) => {
                    return Err(anyhow::anyhow!(
                        "Recipient '{to}' (id: {recipient_id}) is {} in team '{team_name}' and cannot receive messages",
                        m.status.as_str()
                    ));
                }
                _ => {}
            }
        }

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
///
/// M8-T5: if `name_or_id` starts with `"tm-"`, it is validated against the
/// actual member list before being accepted. A typo like `"tm-999"` that
/// does not correspond to a real member now returns an error instead of
/// silently writing to a mailbox nobody owns.
///
/// If `name_or_id` is `"lead"`, it is returned as-is (the lead mailbox is
/// always valid).
///
/// If `name_or_id` is neither `"lead"` nor a `"tm-"` prefix, it is treated
/// as a teammate **name** and looked up in `config.json`'s member list.
pub(crate) fn resolve_agent_id(team_dir: &std::path::Path, name_or_id: &str) -> Result<String> {
    if name_or_id == "lead" {
        return Ok(name_or_id.to_string());
    }
    // M8-T5: validate `tm-…` IDs against the actual member list.
    if name_or_id.starts_with("tm-") {
        let store = TeamStore::load(team_dir)?;
        if let Some(m) = store.config.member_by_id(name_or_id) {
            return Ok(m.agent_id.clone());
        }
        return Err(anyhow::anyhow!(
            "Agent ID '{name_or_id}' is not a member of this team (no matching member in config.json)"
        ));
    }
    // Try to find a member with this name.
    let store = TeamStore::load(team_dir)?;
    store
        .config
        .member_by_name(name_or_id)
        .map(|m| m.agent_id.clone())
        .ok_or_else(|| anyhow::anyhow!("No teammate named '{name_or_id}' in this team"))
}
