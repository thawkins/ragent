//! `team_cleanup` - Remove a team directory (requires all teammates stopped).

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::event::Event;
use crate::team::{TeamStatus, TeamStore, find_team_dir};

/// Tears down a team's on-disk resources.
pub struct TeamCleanupTool;

#[async_trait::async_trait]
impl Tool for TeamCleanupTool {
    fn name(&self) -> &'static str {
        "team_cleanup"
    }

    fn description(&self) -> &'static str {
        "Tear down a team and remove its on-disk resources. Lead-only. Fails if any \
             teammates are still live or shutting down (spawning, working, or \
             shutting_down) unless force is true; idle, plan_pending, blocked, \
             suspended, stopped and failed members do not block. Use \
             team_shutdown_teammate on each live member first. REQUIRED parameter: \
             'team_name' (string). Optional: 'force' (boolean, default false). The \
             refusal names every blocking member with its status, and `force: true` is \
             the only way past it. A blocked member is parked, not live, so it never \
             blocks a teardown - resume it with a `Blocked`-aware resume or shut it down \
             if you want it to stop. The guard is shared with `/team cleanup` and \
             `/team delete`, so all three surfaces refuse the same rosters with the same \
             words. Common \
             gotcha: cleanup is permanent and deletes the team's directory; make sure \
             all results have been extracted before calling it."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team to clean up (required)"
                },
                "force": {
                    "type": "boolean",
                    "description": "If true, remove even if teammates are still live or shutting down. Default: false"
                }
            },
            "required": ["team_name"],
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

        let force = input
            .get("force")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        // Load config to check for active teammates.
        let store = TeamStore::load(&team_dir)?;
        // teamssetup FR-022: the teardown guard is shared with `/team cleanup`
        // and `/team delete` through `cleanup_refusal`, so the three surfaces
        // cannot disagree. The pre-fix rule here was `status != Stopped`, which
        // refused a teardown while a member was merely `Idle` or `Suspended` -
        // parked states with nothing in flight.
        let refusal = crate::team::cleanup_refusal(
            team_name,
            store
                .config
                .members
                .iter()
                .map(|m| (m.name.as_str(), m.status)),
        );

        if let Some(refusal) = refusal.filter(|_| !force) {
            return Err(anyhow::anyhow!(refusal));
        }
        let member_count = store.config.members.len();

        // Mark team as disbanded in config before deleting (best-effort).
        let lead_sid = {
            let mut store = TeamStore::load(&team_dir)?;
            let sid = store.config.lead_session_id.clone();
            store.config.status = TeamStatus::Disbanded;
            store.save().ok(); // Ignore errors - we're about to delete.
            sid
        };

        std::fs::remove_dir_all(&team_dir)
            .map_err(|e| anyhow::anyhow!("Failed to remove team directory: {e}"))?;

        // M5-T7: publish TeamCleanedUp so the TUI/SSE observe the teardown
        // (the event variant already existed but was never published).
        ctx.event_bus.publish(Event::TeamCleanedUp {
            session_id: lead_sid,
            team_name: team_name.to_string(),
        });
        Ok(ToolOutput {
            content: format!("Team '{team_name}' cleaned up successfully. All resources removed."),
            metadata: Some(json!({
                "team_name": team_name,
                "status": "disbanded",
                "forced": force,
                "member_count": member_count
            })),
        })
    }
}
