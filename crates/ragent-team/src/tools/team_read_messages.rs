//! `team_read_messages` — Drain unread messages from the caller's mailbox.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{Mailbox, find_team_dir};

/// Reads and marks as read all unread messages in the caller's mailbox.
pub struct TeamReadMessagesTool;

#[async_trait::async_trait]
impl Tool for TeamReadMessagesTool {
    fn name(&self) -> &'static str {
        "team_read_messages"
    }

    fn description(&self) -> &'static str {
        "Read all unread messages from your mailbox in the team. \
         Messages are marked as read after this call. \
         Call this at the start of each turn to check for new instructions."
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
            .map_or_else(|| "lead".to_string(), |tc| tc.agent_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let mailbox = Mailbox::open(&team_dir, &agent_id)?;
        let unread = mailbox.drain_unread()?;

        if unread.is_empty() {
            return Ok(ToolOutput {
                content: "No new messages.".to_string(),
                metadata: Some(json!({
                    "team_name": team_name,
                    "agent_id": agent_id,
                    "message_count": 0,
                    "messages": []
                })),
            });
        }

        let mut lines = vec![format!("{} new message(s):\n", unread.len())];
        for msg in &unread {
            lines.push(format!(
                "From: {} | Type: {:?} | {}\n{}",
                msg.from,
                msg.message_type,
                msg.sent_at.format("%Y-%m-%d %H:%M:%S UTC"),
                msg.content
            ));
            lines.push(String::from("---"));
        }

        let messages_json: Vec<Value> = unread
            .iter()
            .map(|m| {
                json!({
                    "message_id": m.message_id,
                    "from": m.from,
                    "type": format!("{:?}", m.message_type),
                    "content": m.content,
                    "sent_at": m.sent_at.to_rfc3339()
                })
            })
            .collect();

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(json!({
                "team_name": team_name,
                "agent_id": agent_id,
                "message_count": unread.len(),
                "messages": messages_json
            })),
        })
    }
}
