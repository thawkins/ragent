//! `team_cleanup` — Remove a team directory (requires all teammates stopped).

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{MemberStatus, TeamStatus, TeamStore, find_team_dir};

/// Tears down a team's on-disk resources.
pub struct TeamCleanupTool;

#[async_trait::async_trait]
impl Tool for TeamCleanupTool {
    fn name(&self) -> &'static str {
        "team_cleanup"
    }

    fn description(&self) -> &'static str {
        "Tear down a team and remove its on-disk resources. Lead-only. \
         Fails if any teammates are still active (not Stopped). \
         Use team_shutdown_teammate on each active member first."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team to clean up"
                },
                "force": {
                    "type": "boolean",
                    "description": "If true, remove even if teammates are still active. Default: false"
                }
            },
            "required": ["team_name"]
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
        let active: Vec<_> = store
            .config
            .members
            .iter()
            .filter(|m| m.status != MemberStatus::Stopped)
            .map(|m| m.name.clone())
            .collect();

        if !active.is_empty() && !force {
            return Err(anyhow::anyhow!(
                "Cannot clean up team '{}': {} teammate(s) are still active: {}.\n\
                 Use team_shutdown_teammate on each, or pass force: true to override.",
                team_name,
                active.len(),
                active.join(", ")
            ));
        }

        // Mark team as disbanded in config before deleting (best-effort).
        {
            let mut store = TeamStore::load(&team_dir)?;
            store.config.status = TeamStatus::Disbanded;
            store.save().ok(); // Ignore errors — we're about to delete.
        }

        std::fs::remove_dir_all(&team_dir)
            .map_err(|e| anyhow::anyhow!("Failed to remove team directory: {e}"))?;

        Ok(ToolOutput {
            content: format!("Team '{team_name}' cleaned up successfully. All resources removed."),
            metadata: Some(json!({
                "team_name": team_name,
                "status": "disbanded",
                "forced": force,
                "member_count": active.len()
            })),
        })
    }
}
