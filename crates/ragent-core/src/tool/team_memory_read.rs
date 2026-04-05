//! `team_memory_read` — Read a file from the agent's persistent memory directory.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{MemoryScope, TeamStore, find_team_dir, resolve_memory_dir};

/// Read a file from the calling agent's persistent memory directory.
pub struct TeamMemoryReadTool;

#[async_trait::async_trait]
impl Tool for TeamMemoryReadTool {
    fn name(&self) -> &'static str {
        "team_memory_read"
    }

    fn description(&self) -> &'static str {
        "Read a file from your persistent memory directory. \
         Defaults to MEMORY.md if no path is given. \
         Memory persists across sessions — use it to recall prior context."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team"
                },
                "path": {
                    "type": "string",
                    "description": "Relative path within the memory directory (default: MEMORY.md)"
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

        let rel_path = input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("MEMORY.md");

        let agent_id = ctx
            .team_context
            .as_ref()
            .map_or_else(|| ctx.session_id.clone(), |tc| tc.agent_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        // Look up member to get memory scope and name.
        let store = TeamStore::load(&team_dir)?;
        let member = store
            .config
            .members
            .iter()
            .find(|m| m.agent_id == agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent '{agent_id}' not found in team"))?;

        let scope = member.memory_scope;
        let teammate_name = member.name.clone();

        if scope == MemoryScope::None {
            return Ok(ToolOutput {
                content: "Memory is not enabled for this agent. \
                          Set `\"memory\": \"user\"` or `\"memory\": \"project\"` in your agent profile."
                    .to_string(),
                metadata: Some(json!({ "error": "memory_disabled" })),
            });
        }

        let mem_dir = resolve_memory_dir(scope, &teammate_name, &ctx.working_dir)
            .ok_or_else(|| anyhow::anyhow!("Could not resolve memory directory"))?;

        // Validate path doesn't escape memory dir.
        let target = mem_dir.join(rel_path);
        let canonical_dir = mem_dir.canonicalize().unwrap_or_else(|_| mem_dir.clone());
        let canonical_target = target.canonicalize().unwrap_or_else(|_| target.clone());
        if !canonical_target.starts_with(&canonical_dir) {
            return Ok(ToolOutput {
                content: format!("Path '{rel_path}' escapes the memory directory."),
                metadata: Some(json!({ "error": "path_escape" })),
            });
        }

        if !target.is_file() {
            return Ok(ToolOutput {
                content: format!("File '{rel_path}' not found in memory directory."),
                metadata: Some(json!({
                    "memory_dir": mem_dir.display().to_string(),
                    "exists": false
                })),
            });
        }

        let content = std::fs::read_to_string(&target)
            .map_err(|e| anyhow::anyhow!("Failed to read {rel_path}: {e}"))?;

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "memory_dir": mem_dir.display().to_string(),
                "path": rel_path,
                "bytes": canonical_target.metadata().map(|m| m.len()).unwrap_or(0)
            })),
        })
    }
}
