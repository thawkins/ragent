//! `team_memory_read` — Read structured memories for a team.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::team::{MemoryScope, TeamStore, find_team_dir};

/// Read structured memories from the team's SQLite-backed memory store.
///
/// The optional `path` parameter is normalised into a tag (`path-<slug>`) so
/// teammates can partition notes into named buckets. Defaults to `path-memory`.
pub struct TeamMemoryReadTool;

#[async_trait::async_trait]
impl Tool for TeamMemoryReadTool {
    fn name(&self) -> &'static str {
        "team_memory_read"
    }

    fn description(&self) -> &'static str {
        "Read structured memories stored for your team. REQUIRED parameter: 'team_name' \
             (string). Optional: 'path' (string) selects a labelled memory bucket, defaulting \
             to MEMORY.md. Use it to recall prior context, decisions, and notes. Common \
             gotcha: the path is relative to the team's storage directory; an absent file \
             returns an empty result rather than an error."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "team_name": {
                    "type": "string",
                    "description": "Name of the team (required)"
                },
                "path": {
                    "type": "string",
                    "description": "Optional memory bucket/path to read (default: MEMORY.md)"
                }
            },
            "required": ["team_name"],
            "additionalProperties": false
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

        let path = input
            .get("path")
            .and_then(|v| v.as_str())
            .unwrap_or("MEMORY.md");
        let path_tag = path_tag(path);

        let agent_id = ctx
            .team_context
            .as_ref()
            .map_or_else(|| ctx.session_id.clone(), |tc| tc.agent_id.clone());

        let team_dir = find_team_dir(&ctx.working_dir, team_name)
            .ok_or_else(|| anyhow::anyhow!("Team '{team_name}' not found"))?;

        // Look up member to confirm membership and check memory scope.
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

        let memories = storage.list_memories(team_name, 50)?;
        let mut matched: Vec<(i64, String)> = Vec::new();
        let mut available_paths: Vec<String> = Vec::new();

        for mem in &memories {
            let tags = storage.get_memory_tags(mem.id).unwrap_or_default();
            for tag in &tags {
                if tag.starts_with("path-") && !available_paths.contains(tag) {
                    available_paths.push(tag.clone());
                }
            }
            if tags.contains(&path_tag) {
                matched.push((
                    mem.id,
                    format!(
                        "[{}] {} (confidence: {:.2})",
                        mem.category, mem.content, mem.confidence
                    ),
                ));
            }
        }

        if matched.is_empty() {
            let available = if available_paths.is_empty() {
                String::new()
            } else {
                format!(
                    "\nAvailable memory buckets: {}",
                    available_paths
                        .iter()
                        .map(|t| t.strip_prefix("path-").unwrap_or(t))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            };
            return Ok(ToolOutput {
                content: format!(
                    "No memories found for path '{path}' in team '{team_name}'.{available}"
                ),
                metadata: Some(json!({
                    "team": team_name,
                    "path": path,
                    "path_tag": path_tag,
                    "count": 0
                })),
            });
        }

        let mut output = format!("Memory entries for team '{team_name}' (path: '{path}'):\n\n");
        for (_id, line) in &matched {
            output.push_str("- ");
            output.push_str(line);
            output.push('\n');
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "team": team_name,
                "path": path,
                "path_tag": path_tag,
                "count": matched.len()
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

#[cfg(test)]
mod tests {
    use super::path_tag;

    #[test]
    fn test_path_tag_normalisation() {
        assert_eq!(path_tag("MEMORY.md"), "path-memory-md");
        assert_eq!(path_tag("Notes / Decisions"), "path-notes-decisions");
        assert_eq!(path_tag("--weird__path!!"), "path-weird-path");
    }
}
