//! `team_shutdown_teammate` — Lead requests graceful or immediate shutdown of a teammate.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{Mailbox, MailboxMessage, MemberStatus, MessageType, TeamStore, find_team_dir};

/// Lead requests shutdown of a teammate.
///
/// By default the shutdown is **graceful**: the teammate is marked
/// `ShuttingDown` and a `ShutdownRequest` mailbox message is pushed so the
/// teammate can call `team_shutdown_ack` and terminate cleanly.
///
/// When `immediate: true` is passed, the unified `TeamManager::shutdown_teammate`
/// helper is invoked with `graceful = false`, which sets the agent-loop cancel
/// flags, deregisters the mailbox notifier, marks the member `Stopped`, and
/// pushes a `ShutdownRequest` as a fallback.
///
/// M3-T5/T6 + M5-T4: both paths route through
/// `TeamManagerInterface::shutdown_teammate` so the tool and the `TeamManager`
/// share a single shutdown implementation, and the `ShutdownRequest` carries a
/// correlation id (recorded on the member) so `team_shutdown_ack` can pair the
/// request/reply.
pub struct TeamShutdownTeammateTool;

#[async_trait::async_trait]
impl Tool for TeamShutdownTeammateTool {
    fn name(&self) -> &'static str {
        "team_shutdown_teammate"
    }

    fn description(&self) -> &'static str {
        "Request graceful shutdown of a named teammate. Lead-only. REQUIRED parameters: \
             'team_name' (string) and 'teammate' (string, name or agent ID). Sends a \
             shutdown_request to the teammate's mailbox; the teammate calls team_shutdown_ack \
             to confirm before terminating. Optional: 'reason' (string) and 'immediate' (boolean, \
             default false). Pass immediate: true to cancel the agent loop immediately and \
             mark the teammate Stopped without waiting for an ack — use only for hung or \
             unresponsive teammates. Common gotcha: graceful shutdown requires the teammate \
             to process its mailbox and ack; immediate bypasses that handshake."
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
                    "description": "Teammate name or agent ID to shut down (required)"
                },
                "reason": {
                    "type": "string",
                    "description": "Optional reason for shutdown"
                },
                "immediate": {
                    "type": "boolean",
                    "description": "If true, cancel the agent loop immediately and mark the teammate Stopped instead of waiting for team_shutdown_ack. Default: false (graceful)."
                }
            },
            "required": ["team_name", "teammate"],
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

        let reason = input
            .get("reason")
            .and_then(|v| v.as_str())
            .unwrap_or("Shutdown requested by lead.");

        let immediate = input
            .get("immediate")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let agent_id = super::team_message::resolve_agent_id(&team_dir, teammate)?;

        // M3-T5/T6: route through the unified `TeamManager::shutdown_teammate`
        // helper so the tool and the manager share a single shutdown path.
        // M5-T4: the helper stamps a correlation id on the `ShutdownRequest`
        // and records it on the member so `team_shutdown_ack` can copy it.
        if let Some(manager) = ctx.team_manager.as_ref() {
            manager.shutdown_teammate(&agent_id, !immediate).await?;
        } else {
            // Fallback: no TeamManager is wired in (e.g. the lead session has
            // not initialised one). Perform a best-effort disk-only shutdown so
            // the tool remains usable from contexts without a manager.
            tracing::warn!(
                team = %team_name,
                agent_id = %agent_id,
                "team_shutdown_teammate: no TeamManager in context; falling back to disk-only shutdown"
            );
            // M5-T4: stamp a correlation id and record it on the member.
            let correlation_id = uuid::Uuid::new_v4().to_string();
            let content = if immediate {
                "Immediate shutdown requested by lead; agent loop should terminate."
            } else {
                "Graceful shutdown requested by lead; call team_shutdown_ack to terminate."
            };
            if let Ok(mailbox) = Mailbox::open(&team_dir, &agent_id) {
                if let Err(e) = mailbox.push(MailboxMessage::new_correlated(
                    "lead".to_string(),
                    agent_id.clone(),
                    MessageType::ShutdownRequest,
                    content,
                    &correlation_id,
                )) {
                    tracing::warn!(error = %e, "disk-only shutdown: cannot push ShutdownRequest");
                }
            }
            let mut store = TeamStore::load(&team_dir)?;
            if let Some(member) = store.config.member_by_id_mut(&agent_id) {
                member.status = if immediate {
                    MemberStatus::Stopped
                } else {
                    MemberStatus::ShuttingDown
                };
                if immediate {
                    member.current_task_id = None;
                }
                member.shutdown_request_id = Some(correlation_id);
            }
            store.save()?;
        }

        let mode = if immediate { "immediate" } else { "graceful" };
        Ok(ToolOutput {
            content: format!(
                "Shutdown request ({mode}) sent to teammate '{teammate}' in team '{team_name}'.\n\
                 Reason: {reason}\n\
                 {}",
                if immediate {
                    "Agent loop cancelled; teammate marked Stopped.".to_string()
                } else {
                    "Waiting for team_shutdown_ack confirmation.".to_string()
                }
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "agent_id": agent_id,
                "status": if immediate { "stopped" } else { "shutting_down" },
                "mode": mode,
                "reason": reason
            })),
        })
    }
}
