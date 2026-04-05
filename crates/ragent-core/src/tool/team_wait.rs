//! `team_wait` — block the lead agent until all (or specific) teammates become idle.
//!
//! Subscribes to [`Event::TeammateIdle`] on the event bus so there is no polling.
//! Returns a summary of each teammate's final status once all are idle,
//! or a timeout message if the deadline is exceeded.
//!
//! This tool is the team-aware counterpart to `wait_tasks`.  Use it after
//! spawning teammates so the lead does not race ahead and duplicate their work.

use std::collections::HashSet;
use std::time::Duration;

use anyhow::Result;
use serde_json::{Value, json};

use crate::event::Event;
use crate::team::TeamStore;
use crate::team::config::{MemberStatus, TeamMember};

use super::{Tool, ToolContext, ToolOutput};

/// Blocks until all (or specified) teammates in a team become idle.
pub struct TeamWaitTool;

#[async_trait::async_trait]
impl Tool for TeamWaitTool {
    fn name(&self) -> &'static str {
        "team_wait"
    }

    fn description(&self) -> &'static str {
        "Block until all teammates (or specific ones) in the active team finish their current work \
         and become idle. Use this after team_spawn so the lead does not race ahead and duplicate \
         the teammates' work. Returns a summary of each teammate's completion status. \
         Optionally specify `agent_ids` to wait for a subset; if omitted, waits for ALL \
         non-failed members. Specify `timeout_secs` (default 300) as the maximum wait time."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team to wait on. Uses the active team if omitted."
                },
                "agent_ids": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Specific agent IDs (e.g. 'tm-001') to wait for. Omit to wait for ALL working teammates."
                },
                "timeout_secs": {
                    "type": "number",
                    "description": "Maximum seconds to wait before returning partial results. Default: 300."
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "agent:spawn"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let timeout_secs = input
            .get("timeout_secs")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(300);

        let working_dir = ctx.working_dir.clone();

        // Resolve team name — use provided value or infer from on-disk teams.
        let team_name: Option<String> = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .map(std::string::ToString::to_string);

        // Load the team store to discover current membership.
        let store = if let Some(ref name) = team_name {
            TeamStore::load_by_name(name, &working_dir)
                .map_err(|e| anyhow::anyhow!("Cannot load team '{name}': {e}"))?
        } else {
            // Find the most recently modified team.
            let teams = TeamStore::list_teams(&working_dir);
            let (_, dir, _) = teams
                .into_iter()
                .next()
                .ok_or_else(|| anyhow::anyhow!("No teams found in this project."))?;
            TeamStore::load(&dir).map_err(|e| anyhow::anyhow!("Cannot load team store: {e}"))?
        };

        let resolved_team_name = store.config.name.clone();

        // Determine which agent IDs to wait for.
        let requested_ids: HashSet<String> = input
            .get("agent_ids")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();

        // Build the set of agent IDs we are waiting for.
        let mut waiting_for: HashSet<String> = if requested_ids.is_empty() {
            store
                .config
                .members
                .iter()
                .filter(|m| {
                    matches!(
                        m.status,
                        MemberStatus::Working
                            | MemberStatus::Spawning
                            | MemberStatus::PlanPending
                            | MemberStatus::ShuttingDown
                    )
                })
                .map(|m| m.agent_id.clone())
                .collect()
        } else {
            requested_ids
        };

        // Members that were already idle/done when we checked.
        let already_idle: Vec<String> = store
            .config
            .members
            .iter()
            .filter(|m| {
                matches!(
                    m.status,
                    MemberStatus::Idle | MemberStatus::Failed | MemberStatus::Stopped
                ) && waiting_for.contains(&m.agent_id)
            })
            .map(|m| m.agent_id.clone())
            .collect();
        for id in &already_idle {
            waiting_for.remove(id);
        }

        if waiting_for.is_empty() {
            let summary = summarise_store(&store.config.members);
            return Ok(ToolOutput {
                content: format!(
                    "All teammates in team '{resolved_team_name}' are already idle.\n\n{summary}"
                ),
                metadata: Some(json!({ "team": resolved_team_name, "timed_out": false })),
            });
        }

        // Subscribe BEFORE the wait loop to avoid the race where a teammate
        // becomes idle between the store read and the subscribe.
        let mut rx = ctx.event_bus.subscribe();
        let deadline = tokio::time::Instant::now() + Duration::from_secs(timeout_secs);

        tracing::info!(
            team = %resolved_team_name,
            waiting_for = ?waiting_for,
            timeout_secs,
            "team_wait: waiting for teammates to become idle"
        );

        loop {
            if waiting_for.is_empty() {
                break;
            }
            match tokio::time::timeout_at(deadline, rx.recv()).await {
                Ok(Ok(Event::TeammateIdle {
                    session_id,
                    team_name: ev_team,
                    agent_id,
                })) if session_id == ctx.session_id
                    && ev_team == resolved_team_name
                    && waiting_for.contains(&agent_id) =>
                {
                    tracing::info!(team = %resolved_team_name, agent_id = %agent_id, "team_wait: teammate became idle");
                    waiting_for.remove(&agent_id);
                }
                Ok(Ok(_)) => continue,
                Ok(Err(_)) => break, // channel closed
                Err(_) => break,     // timeout
            }
        }

        let timed_out = !waiting_for.is_empty();

        // Reload store for final statuses.
        let final_store =
            TeamStore::load_by_name(&resolved_team_name, &working_dir).unwrap_or(store);
        let summary = summarise_store(&final_store.config.members);

        let mut output = String::new();
        if timed_out {
            output.push_str(&format!(
                "⚠️ Timed out after {timeout_secs}s. {} teammate(s) still working: {}\n\n",
                waiting_for.len(),
                waiting_for.iter().cloned().collect::<Vec<_>>().join(", ")
            ));
        } else {
            output.push_str(&format!(
                "✅ All awaited teammates in team '{resolved_team_name}' are now idle.\n\n"
            ));
        }
        output.push_str(&summary);

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "team": resolved_team_name,
                "timed_out": timed_out,
                "still_working": waiting_for.iter().cloned().collect::<Vec<_>>(),
            })),
        })
    }
}

fn summarise_store(members: &[TeamMember]) -> String {
    let mut out = String::from("## Teammate status\n\n");
    for m in members {
        let status_icon = match m.status {
            MemberStatus::Idle => "💤",
            MemberStatus::Working => "⚙️",
            MemberStatus::Spawning => "🟡",
            MemberStatus::Blocked => "🔒",
            MemberStatus::Failed => "❌",
            MemberStatus::PlanPending => "📋",
            MemberStatus::ShuttingDown => "🔄",
            MemberStatus::Stopped => "⏹️",
        };
        let status_str = format!("{:?}", m.status).to_lowercase();
        out.push_str(&format!(
            "- {} **{}** ({}) — {}\n",
            status_icon, m.name, m.agent_id, status_str
        ));
    }
    out
}
