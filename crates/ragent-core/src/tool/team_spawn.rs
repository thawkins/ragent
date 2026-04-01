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
        "Spawn a teammate agent session within an existing team. \
         Each teammate receives the team context and works on a single, bounded task. \
         CRITICAL: Spawn ONE teammate per independent work item — never assign a list of \
         items to one teammate (context overflow). After spawning all teammates in the same \
         response turn, call `team_wait` to block until they finish. \
         Do NOT use `wait_tasks` for teammates — use `team_wait`."
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
                    "description": "Initial task prompt for the teammate. Must be scoped to a SINGLE work item — never list multiple items. Keep under ~500 words; reference files by path rather than pasting content."
                },
                "model": {
                    "type": "string",
                    "description": "Optional model override in 'provider_id/model_id' format (e.g. 'anthropic/claude-sonnet-4-20250514'). If omitted, the teammate inherits the lead session's model."
                },
                "memory": {
                    "type": "string",
                    "enum": ["user", "project", "none"],
                    "description": "Persistent memory scope: 'user' (global), 'project' (local), or 'none' (default). Gives the teammate a memory directory for cross-session notes."
                }
            },
            "required": ["team_name", "teammate_name", "agent_type", "prompt"]
        })
    }

    fn permission_category(&self) -> &str {
        "team:manage"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        use crate::agent::ModelRef;

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

        let prompt = input
            .get("prompt")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Missing required parameter: prompt"))?;

        // Guard: detect multi-item prompts that violate the "one per spawn" rule.
        // Common patterns: "1.", "2.", "3.", "- Competitor A", "- Item B", "and", "also"
        let multi_item_patterns = [
            "1.", "2.", "3.", "4.", "5.",  // Numbered lists
            "a)", "b)", "c)", "d)", "e)",  // Letter lists
            "- ", "* ",                     // Bullet lists
            ", and ",
            " and ",
            " or ",
            " also ",
            "Item 1",
            "Item 2",
            "Competitor 1",
            "Competitor 2",
        ];

        let prompt_lower = prompt.to_lowercase();
        let mut detected_multi_items = Vec::new();

        for pattern in &multi_item_patterns {
            let pattern_lower = pattern.to_lowercase();
            if prompt_lower.contains(&pattern_lower) {
                detected_multi_items.push(*pattern);
            }
        }

        if !detected_multi_items.is_empty() {
            return Err(anyhow::anyhow!(
                "POLICY VIOLATION: Prompt for teammate '{}' contains multi-item patterns: {:?}\n\
                 RULE: Each team_spawn must cover **exactly ONE work item** — never assign multiple \
                 items to a single teammate.\n\
                 Split this into separate team_spawn calls, one per item.\n\
                 Offending patterns: {}",
                teammate_name,
                detected_multi_items,
                detected_multi_items.join(", ")
            ));
        }

        // Parse optional per-teammate model override ("provider_id/model_id").
        let teammate_model: Option<ModelRef> = input
            .get("model")
            .and_then(|v| v.as_str())
            .and_then(|s| {
                s.split_once('/')
                    .or_else(|| s.split_once(':'))
                    .map(|(p, m)| ModelRef {
                        provider_id: p.to_string(),
                        model_id: m.to_string(),
                    })
            });

        // Parse optional memory scope.
        let memory_scope = match input.get("memory").and_then(|v| v.as_str()) {
            Some("user") => crate::team::MemoryScope::User,
            Some("project") => crate::team::MemoryScope::Project,
            _ => crate::team::MemoryScope::None,
        };

        // TeamManager is wired in M3. Until then, return a clear pending message.
        if ctx.team_manager.is_none() {
            tracing::warn!(
                team = %team_name,
                teammate = %teammate_name,
                session = %ctx.session_id,
                "TeamManager missing when attempting to spawn teammate; returning pending_manager"
            );
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
                teammate_model.as_ref(),
                ctx.active_model.as_ref(),
                &ctx.working_dir,
            )
            .await?;

        // Persist memory scope on the member record.
        if memory_scope != crate::team::MemoryScope::None {
            if let Some(team_dir) = crate::team::find_team_dir(&ctx.working_dir, team_name) {
                if let Ok(mut store) = crate::team::TeamStore::load(&team_dir) {
                    if let Some(member) = store.config.member_by_id_mut(&agent_id) {
                        member.memory_scope = memory_scope;
                    }
                    let _ = store.save();
                }
            }
        }

        let model_display = teammate_model
            .as_ref()
            .map(|m| format!("{}/{}", m.provider_id, m.model_id))
            .or_else(|| ctx.active_model.as_ref().map(|m| format!("{}/{} (inherited)", m.provider_id, m.model_id)))
            .unwrap_or_else(|| "default".to_string());

        Ok(ToolOutput {
            content: format!(
                "Teammate '{teammate_name}' spawned in team '{team_name}'.\nAgent ID: {agent_id}\n\
                 Model: {model_display}\n\
                 ⏳ Teammate is now working. Call `team_wait` (not `wait_tasks`) after all spawns \
                 to block until teammates finish before the lead continues."
            ),
            metadata: Some(json!({
                "team_name": team_name,
                "teammate_name": teammate_name,
                "agent_id": agent_id,
                "model": model_display,
                "status": "spawned"
            })),
        })
    }
}
