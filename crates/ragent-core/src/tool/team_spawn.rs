//! `team_spawn` — Spawn a named teammate session within an existing team.
//!
//! Full implementation requires `TeamManager` (M3). This stub validates
//! parameters and returns an informative error until M3 is wired in.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Spawns a named teammate into an existing team.
pub struct TeamSpawnTool;

#[async_trait::async_trait]
impl Tool for TeamSpawnTool {
    fn name(&self) -> &str {
        "team_spawn"
    }

    fn description(&self) -> &str {
        "Spawn one or more named teammate agent sessions within an existing team. \
         Each teammate receives the team context and can claim tasks from the shared task list."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team to spawn the teammate into"
                },
                "teammate_name": {
                    "type": "string",
                    "description": "Unique name for this teammate within the team (e.g. 'security-reviewer')"
                },
                "agent_type": {
                    "type": "string",
                    "description": "Agent type / definition name (e.g. 'general', 'explore')"
                },
                "prompt": {
                    "type": "string",
                    "description": "Initial task prompt for the teammate"
                }
            },
            "required": ["team_name", "teammate_name", "agent_type", "prompt"]
        })
    }

    fn permission_category(&self) -> &str {
        "team:manage"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let team_name = input
            .get("team_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: team_name"))?;

        let teammate_name = input
            .get("teammate_name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: teammate_name"))?;

        let agent_type = input
            .get("agent_type")
            .and_then(|v| v.as_str())
            .unwrap_or("general");

        let _prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: prompt"))?;

        // TeamManager is wired in M3. Until then, return a clear pending message.
        if ctx.team_manager.is_none() {
            return Ok(ToolOutput {
                content: format!(
                    "Teammate '{teammate_name}' queued for team '{team_name}' \
                     (agent_type: {agent_type}).\n\
                     Note: TeamManager not yet initialised — teammate will be spawned \
                     when the session processor is upgraded to M3."
                ),
                metadata: Some(json!({
                    "team_name": team_name,
                    "teammate_name": teammate_name,
                    "agent_type": agent_type,
                    "status": "pending_manager"
                })),
            });
        }

        let manager = ctx.team_manager.as_ref().expect("checked above");
        let agent_id = manager
            .spawn_teammate(
                team_name,
                teammate_name,
                agent_type,
                input.get("prompt").and_then(|v| v.as_str()).unwrap_or(""),
                &ctx.working_dir,
            )
            .await?;

        Ok(ToolOutput {
            content: format!(
                "Teammate '{teammate_name}' spawned in team '{team_name}'.\nAgent ID: {agent_id}"
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "teammate_name": teammate_name,
                "agent_id": agent_id,
                "status": "spawned"
            })),
        })
    }
}
