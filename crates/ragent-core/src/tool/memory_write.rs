//! `memory_write` / `memory_read` — Persist notes to user or project memory files.
//!
//! Agents call these tools to remember facts across sessions. Memory files are
//! automatically loaded into the system prompt at the start of each session.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Tool for persisting notes to user or project memory files.
pub struct MemoryWriteTool;

#[async_trait::async_trait]
impl Tool for MemoryWriteTool {
    fn name(&self) -> &'static str {
        "memory_write"
    }

    fn description(&self) -> &'static str {
        "Persist notes or learnings to memory files that are automatically loaded in future \
         sessions. Use scope='user' for global memory (~/.ragent/memory/MEMORY.md) or \
         scope='project' for project-specific memory (.ragent/memory/MEMORY.md in the \
         working directory)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The content to append to the memory file"
                },
                "scope": {
                    "type": "string",
                    "enum": ["user", "project"],
                    "description": "Memory scope: 'user' for global (~/.ragent/memory/) or 'project' for project-level (.ragent/memory/)"
                },
                "path": {
                    "type": "string",
                    "description": "Filename within the memory directory (default: MEMORY.md)"
                }
            },
            "required": ["content"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let content = input["content"]
            .as_str()
            .context("Missing required 'content' parameter")?;
        let scope = input["scope"].as_str().unwrap_or("project");
        let filename = input["path"].as_str().unwrap_or("MEMORY.md");

        let mem_dir = resolve_memory_dir(scope, &ctx.working_dir)?;
        std::fs::create_dir_all(&mem_dir)
            .with_context(|| format!("Failed to create memory directory: {}", mem_dir.display()))?;

        let file_path = mem_dir.join(filename);

        let now = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
        let entry = format!("\n\n<!-- {now} -->\n{content}\n");

        use std::io::Write;
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&file_path)
            .with_context(|| format!("Failed to open memory file: {}", file_path.display()))?;
        file.write_all(entry.as_bytes())
            .with_context(|| format!("Failed to write to memory file: {}", file_path.display()))?;

        let content_line_count = content.lines().count();
        Ok(ToolOutput {
            content: format!(
                "Memory written to {} (scope: {scope})\n\n{content}",
                file_path.display()
            ),
            metadata: Some(json!({
                "file": file_path.display().to_string(),
                "scope": scope,
                "byte_count": entry.len(),
                "line_count": content_line_count
            })),
        })
    }
}

/// Tool for reading back content from user or project memory files.
pub struct MemoryReadTool;

#[async_trait::async_trait]
impl Tool for MemoryReadTool {
    fn name(&self) -> &'static str {
        "memory_read"
    }

    fn description(&self) -> &'static str {
        "Read the contents of a memory file. Use to recall facts persisted by memory_write."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "scope": {
                    "type": "string",
                    "enum": ["user", "project"],
                    "description": "Memory scope: 'user' for global (~/.ragent/memory/) or 'project' for project-level (.ragent/memory/)"
                },
                "path": {
                    "type": "string",
                    "description": "Filename within the memory directory (default: MEMORY.md)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let scope = input["scope"].as_str().unwrap_or("project");
        let filename = input["path"].as_str().unwrap_or("MEMORY.md");

        let mem_dir = resolve_memory_dir(scope, &ctx.working_dir)?;
        let file_path = mem_dir.join(filename);

        if !file_path.exists() {
            return Ok(ToolOutput {
                content: format!(
                    "No memory file found at {} (scope: {scope})",
                    file_path.display()
                ),
                metadata: Some(json!({
                    "file": file_path.display().to_string(),
                    "scope": scope,
                    "line_count": 0,
                    "byte_count": 0
                })),
            });
        }

        let content = std::fs::read_to_string(&file_path)
            .with_context(|| format!("Failed to read memory file: {}", file_path.display()))?;

        let content_line_count = content.lines().count();
        let byte_count = content.len();
        Ok(ToolOutput {
            content: if content.trim().is_empty() {
                format!("Memory file {} is empty", file_path.display())
            } else {
                format!(
                    "Memory ({scope} scope, {}):\n\n{content}",
                    file_path.display()
                )
            },
            metadata: Some(json!({
                "file": file_path.display().to_string(),
                "scope": scope,
                "line_count": content_line_count,
                "byte_count": byte_count
            })),
        })
    }
}

/// Resolves the memory directory for the given scope.
pub fn resolve_memory_dir(
    scope: &str,
    working_dir: &std::path::Path,
) -> Result<std::path::PathBuf> {
    match scope {
        "user" => {
            let home = dirs::home_dir()
                .context("Cannot determine home directory for user memory scope")?;
            Ok(home.join(".ragent").join("memory"))
        }
        _ => Ok(working_dir.join(".ragent").join("memory")),
    }
}
