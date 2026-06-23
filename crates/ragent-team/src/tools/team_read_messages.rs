//! `team_read_messages` — Peek unread messages from the caller's mailbox.
//!
//! M4-T1: this tool uses [`Mailbox::peek_unread`] + [`Mailbox::acknowledge`]
//! so that messages are only marked read **after** the tool successfully
//! returns them to the model. If the tool fails (e.g. serialization error),
//! the messages stay unread and are redelivered on the next call —
//! at-least-once delivery semantics.
//!
//! M4-T5: the JSON metadata uses `serde_json::to_value(&m.message_type)`
//! (snake_case, matching the on-disk format) instead of `format!("{:?}", …)`
//! (PascalCase), and includes the `to` and `read` fields so P2P messages
//! carry recipient context.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{Mailbox, find_team_dir};

/// Peeks unread messages from the caller's mailbox and acknowledges them
/// only after the output has been built successfully.
pub struct TeamReadMessagesTool;

#[async_trait::async_trait]
impl Tool for TeamReadMessagesTool {
    fn name(&self) -> &'static str {
        "team_read_messages"
    }

    fn description(&self) -> &'static str {
        "Read all unread messages from your mailbox in the team. \
         Messages are marked as read only after this call returns them to you \
         (at-least-once delivery: if the call fails, messages stay unread and \
         are redelivered next time). \
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

        // M4-T1: peek without marking read so a failure here leaves the
        // messages unread and they are redelivered on the next call.
        let unread = mailbox.peek_unread()?;

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

        // M4-T5: snake_case `type` via serde (matches on-disk format), plus
        // `to` and `read` fields for recipient context.
        let messages_json: Vec<Value> = unread
            .iter()
            .map(|m| {
                json!({
                    "message_id": m.message_id,
                    "from": m.from,
                    "to": m.to,
                    "type": serde_json::to_value(&m.message_type).unwrap_or(Value::Null),
                    "read": m.read,
                    "content": m.content,
                    "sent_at": m.sent_at.to_rfc3339()
                })
            })
            .collect();

        let mut lines = vec![format!("{} new message(s):\n", unread.len())];
        for m in &unread {
            let type_str = serde_json::to_value(&m.message_type)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_else(|| format!("{:?}", m.message_type));
            lines.push(format!(
                "From: {} | To: {} | Type: {} | Read: {} | {}\n{}",
                m.from,
                m.to,
                type_str,
                m.read,
                m.sent_at.format("%Y-%m-%d %H:%M:%S UTC"),
                m.content
            ));
            lines.push(String::from("---"));
        }

        // Build the full output first; only acknowledge once we have a
        // successful `ToolOutput`. If anything above returned early via `?`,
        // the messages remain unread.
        let content = lines.join("\n");
        let metadata = json!({
            "team_name": team_name,
            "agent_id": agent_id,
            "message_count": unread.len(),
            "messages": messages_json
        });

        // M4-T1: acknowledge the messages now that we have a successful
        // output ready to return to the model.
        //
        // PERF-020: previously this called `mailbox.acknowledge(&m.message_id)`
        // once per message, producing N full read-modify-write file cycles for
        // N messages. Now we collect the message IDs and mark them all read in
        // a single lock → read → mark all → write → unlock cycle via
        // `Mailbox::mark_all_read`. A failed batch ack is non-fatal — the
        // affected messages stay unread and are re-peeked next time
        // (idempotent `mark_read`).
        let ids: Vec<String> = unread.iter().map(|m| m.message_id.clone()).collect();
        if let Err(e) = mailbox.mark_all_read(&ids) {
            tracing::warn!(
                count = ids.len(),
                error = %e,
                "team_read_messages: failed to batch-acknowledge messages; they will be redelivered"
            );
        }

        Ok(ToolOutput {
            content,
            metadata: Some(metadata),
        })
    }
}
