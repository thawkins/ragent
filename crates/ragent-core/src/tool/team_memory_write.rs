//! `team_memory_write` — Write a file to the agent's persistent memory directory.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{MemoryScope, TeamStore, find_team_dir, resolve_memory_dir};

/// Write (or append to) a file in the calling agent's persistent memory directory.
pub struct TeamMemoryWriteTool;

#[async_trait::async_trait]
impl Tool for TeamMemoryWriteTool {
    fn name(&self) -> &str {
        "team_memory_write"
    }

    fn description(&self) -> &str {
        "Write or append to a file in your persistent memory directory. \
         Defaults to MEMORY.md if no path is given. \
         Use mode 'append' to add to an existing file, or 'overwrite' to replace it."
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
                    "description": "Content to write"
                },
                "path": {
                    "type": "string",
                    "description": "Relative path within the memory directory (default: MEMORY.md)"
                },
                "mode": {
                    "type": "string",
                    "enum": ["append", "overwrite"],
                    "description": "Write mode: 'append' adds to the end (default), 'overwrite' replaces the file"
                }
            },
            "required": ["team_name", "content"]
        })
    }

    fn permission_category(&self) -> &str {
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

        let rel_path = input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("MEMORY.md");

        let write_mode = input
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("append");

        let agent_id = ctx
            .team_context
            .as_ref()
            .map(|tc| tc.agent_id.clone())
            .unwrap_or_else(|| ctx.session_id.clone());

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

        // Create directory if it doesn't exist.
        if !mem_dir.exists() {
            std::fs::create_dir_all(&mem_dir)
                .map_err(|e| anyhow::anyhow!("Failed to create memory directory: {}", e))?;
        }

        let target = mem_dir.join(rel_path);

        // Validate path doesn't escape memory dir.
        // Use the now-existing mem_dir for canonical comparison.
        let canonical_dir = mem_dir.canonicalize().unwrap_or_else(|_| mem_dir.clone());
        // For new files, check the parent directory.
        let check_path = if target.exists() {
            target.canonicalize().unwrap_or_else(|_| target.clone())
        } else {
            let parent = target.parent().unwrap_or(&mem_dir);
            if !parent.exists() {
                std::fs::create_dir_all(parent)
                    .map_err(|e| anyhow::anyhow!("Failed to create subdirectory: {}", e))?;
            }
            parent
                .canonicalize()
                .unwrap_or_else(|_| parent.to_path_buf())
                .join(target.file_name().unwrap_or_default())
        };
        if !check_path.starts_with(&canonical_dir) {
            return Ok(ToolOutput {
                content: format!("Path '{}' escapes the memory directory.", rel_path),
                metadata: Some(json!({ "error": "path_escape" })),
            });
        }

        // Write or append.
        use std::io::Write;
        let bytes_written = match write_mode {
            "overwrite" => {
                std::fs::write(&target, content)
                    .map_err(|e| anyhow::anyhow!("Failed to write {}: {}", rel_path, e))?;
                content.len()
            }
            _ => {
                // append
                let mut file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&target)
                    .map_err(|e| anyhow::anyhow!("Failed to open {}: {}", rel_path, e))?;
                file.write_all(content.as_bytes())
                    .map_err(|e| anyhow::anyhow!("Failed to append to {}: {}", rel_path, e))?;
                content.len()
            }
        };

        Ok(ToolOutput {
            content: format!(
                "Wrote {} bytes to '{}' in memory directory (mode: {}).",
                bytes_written, rel_path, write_mode
            ),
            metadata: Some(json!({
                "memory_dir": mem_dir.display().to_string(),
                "path": rel_path,
                "mode": write_mode,
                "bytes_written": bytes_written
            })),
        })
    }
}
