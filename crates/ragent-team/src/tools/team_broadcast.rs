//! `team_broadcast` — Send a message to all active teammates.
//!
//! M4-T3: per-recipient results are collected and reported. A failure to
//! deliver to one teammate no longer aborts delivery to the rest; the tool
//! returns a JSON summary of succeeded and failed recipients.
//!
//! PERF-021: delivery to the active teammates is now **concurrent**. Each
//! recipient's mailbox is a separate file guarded by its own advisory
//! `flock`, so there is no contention between recipients. Previously the
//! tool pushed to each teammate sequentially (T sequential lock acquisitions
//! + T full file rewrites). It now drives the per-recipient pushes through
//! `futures::future::join_all`, reducing the wall-clock time from O(T)
//! sequential to O(1) parallel (bounded by the blocking-pool size).

use anyhow::Result;
use futures::future::join_all;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{Mailbox, MailboxMessage, MemberStatus, MessageType, TeamStore, find_team_dir};

/// Sends a broadcast message to all active (non-stopped) teammates.
pub struct TeamBroadcastTool;

#[async_trait::async_trait]
impl Tool for TeamBroadcastTool {
    fn name(&self) -> &'static str {
        "team_broadcast"
    }

    fn description(&self) -> &'static str {
        "Send a message to all active (non-stopped) teammates in the team simultaneously. \
         Returns a per-recipient summary so partial failures are visible."
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

    fn permission_category(&self) -> &'static str {
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
            .map_or_else(|| "lead".to_string(), |tc| tc.agent_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let store = TeamStore::load(&team_dir)?;
        let active: Vec<_> = store
            .config
            .members
            .iter()
            .filter(|m| m.status != MemberStatus::Stopped)
            .map(|m| (m.agent_id.clone(), m.name.clone()))
            .collect();

        // M4-T3: collect per-recipient results so partial failures are
        // reported. A failure on one recipient does not abort the loop.
        //
        // PERF-021: drive the per-recipient pushes concurrently through
        // `join_all`. Each recipient's mailbox is a separate file guarded by
        // its own `flock`, so there is no contention between recipients and
        // the pushes can run in parallel on the tokio blocking pool. We
        // construct one `MailboxMessage` per recipient up-front (the only
        // field that differs is `to`) and move each into its own async task.
        let succeeded: Vec<String> = Vec::with_capacity(active.len());
        let failed: Vec<Value> = Vec::with_capacity(active.len());
        let push_futs = active.iter().map(|(agent_id, name)| {
            let team_dir = team_dir.clone();
            let from = from.clone();
            let agent_id = agent_id.clone();
            let name = name.clone();
            let content = content.to_string();
            async move {
                let outcome = Mailbox::open(&team_dir, &agent_id).and_then(|mailbox| {
                    mailbox.push(MailboxMessage::new(
                        from,
                        agent_id.clone(),
                        MessageType::Broadcast,
                        content,
                    ))
                });
                (agent_id, name, outcome)
            }
        });
        let results = join_all(push_futs).await;

        let mut succeeded = succeeded;
        let mut failed = failed;
        for (agent_id, name, outcome) in results {
            match outcome {
                Ok(()) => succeeded.push(agent_id),
                Err(e) => failed.push(json!({
                    "agent_id": agent_id,
                    "name": name,
                    "error": format!("{e}")
                })),
            }
        }

        let sent = succeeded.len();
        let failed_count = failed.len();
        let content_str = if failed_count == 0 {
            format!("Broadcast sent to {sent} active teammate(s) in team '{team_name}'.")
        } else {
            format!(
                "Broadcast sent to {sent} active teammate(s) in team '{team_name}'. \
                 {failed_count} delivery failure(s): see metadata for details."
            )
        };

        Ok(ToolOutput {
            content: content_str,
            metadata: Some(json!({
                "team_name": team_name,
                "from": from,
                "recipients": active.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
                "succeeded": succeeded,
                "failed": failed,
                "message_count": sent,
                "failed_count": failed_count
            })),
        })
    }
}
