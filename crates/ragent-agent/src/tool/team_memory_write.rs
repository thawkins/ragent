//! `team_memory_write` — Write structured memories for a team.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::memory::store::StructuredMemory;
use crate::team::{MemoryScope, TeamStore, find_team_dir};

/// Write (or append to) a structured memory in the team's SQLite store.
///
/// The optional `path` parameter is normalised into a tag (`path-<slug>`) so
/// teammates can partition notes into named buckets. Defaults to `path-memory`.
/// In `overwrite` mode the most recent memory in the same bucket is updated;
/// in `append` mode a new memory is always created.
pub struct TeamMemoryWriteTool;

#[async_trait::async_trait]
impl Tool for TeamMemoryWriteTool {
    fn name(&self) -> &'static str {
        "team_memory_write"
    }

    fn description(&self) -> &'static str {
        "Write or append a structured memory for your team. \
         Use `path` to select a memory bucket (default: MEMORY.md). \
         Mode 'append' creates a new memory; 'overwrite' updates the latest memory in the bucket."
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
                    "description": "Content to store"
                },
                "path": {
                    "type": "string",
                    "description": "Optional memory bucket/path (default: MEMORY.md)"
                },
                "mode": {
                    "type": "string",
                    "enum": ["append", "overwrite"],
                    "description": "Write mode: 'append' creates a new memory (default), 'overwrite' updates the latest in the bucket"
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

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("MEMORY.md");
        let mode = input
            .get("mode")
            .and_then(|v| v.as_str())
            .unwrap_or("append");

        let path_tag = path_tag(path);

        let agent_id = ctx
            .team_context
            .as_ref()
            .map_or_else(|| ctx.session_id.clone(), |tc| tc.agent_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        let store = TeamStore::load(&team_dir)?;
        let member = store
            .config
            .members
            .iter()
            .find(|m| m.agent_id == agent_id)
            .ok_or_else(|| anyhow::anyhow!("Agent '{agent_id}' not found in team"))?;

        if member.memory_scope == MemoryScope::None {
            return Ok(ToolOutput {
                content: "Memory is not enabled for this agent. \
                          Set `\"memory\": \"user\"` or `\"memory\": \"project\"` in your agent profile."
                    .to_string(),
                metadata: Some(json!({ "error": "memory_disabled" })),
            });
        }

        let storage = ctx
            .storage
            .as_ref()
            .context("Team memory requires storage (SQLite) but none is available")?;

        // Validate the content is non-empty after trimming.
        let trimmed = content.trim();
        if trimmed.is_empty() {
            anyhow::bail!("Memory content cannot be empty");
        }

        let tags = vec![
            format!(
                "path-{}",
                path_tag.strip_prefix("path-").unwrap_or(&path_tag)
            ),
            format!("agent-{}", slugify(&member.name)),
        ];

        let category = "workflow";
        let confidence = 0.7;

        if mode == "overwrite" {
            let memories = storage.list_memories(team_name, 50)?;
            let mut target_id: Option<i64> = None;
            for mem in &memories {
                let tags = storage.get_memory_tags(mem.id).unwrap_or_default();
                if tags.contains(&path_tag) {
                    target_id = Some(mem.id);
                    // Keep going to find the most recently updated one.
                }
            }

            if let Some(id) = target_id {
                storage.update_memory_content(id, trimmed)?;
                storage.set_memory_tags(id, &tags)?;
                return Ok(ToolOutput {
                    content: format!(
                        "Overwrote memory id {id} in team '{team_name}' bucket '{path}'."
                    ),
                    metadata: Some(json!({
                        "id": id,
                        "team": team_name,
                        "path": path,
                        "path_tag": path_tag,
                        "mode": "overwrite"
                    })),
                });
            }
        }

        // Append (or overwrite with no existing bucket): create a new memory.
        StructuredMemory::validate_category(category).map_err(|e| anyhow::anyhow!("{e}"))?;
        let id = storage.create_memory(
            trimmed,
            category,
            "team_memory_write",
            confidence,
            team_name,
            &ctx.session_id,
            &tags,
        )?;

        Ok(ToolOutput {
            content: format!(
                "Stored memory id {id} in team '{team_name}' bucket '{path}' (mode: {mode})."
            ),
            metadata: Some(json!({
                "id": id,
                "team": team_name,
                "path": path,
                "path_tag": path_tag,
                "mode": mode,
                "category": category,
                "confidence": confidence,
                "tags": tags
            })),
        })
    }
}

/// Normalise a user-supplied path into a tag-safe bucket identifier.
fn path_tag(path: &str) -> String {
    let slug: String = path
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-');
    let mut slug = slug.replace("--", "-");
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    let slug = if slug.is_empty() { "memory" } else { &slug };
    format!("path-{slug}")
}

/// Slugify an arbitrary string for use in a tag.
fn slugify(s: &str) -> String {
    let slug: String = s
        .to_ascii_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let slug = slug.trim_matches('-');
    let mut slug = slug.replace("--", "-");
    while slug.contains("--") {
        slug = slug.replace("--", "-");
    }
    slug
}

#[cfg(test)]
mod tests {
    use super::{path_tag, slugify};

    #[test]
    fn test_path_tag_normalisation() {
        assert_eq!(path_tag("MEMORY.md"), "path-memory-md");
        assert_eq!(path_tag("Notes / Decisions"), "path-notes-decisions");
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Alice Smith"), "alice-smith");
    }
}
