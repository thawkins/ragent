//! File creation tool.
//!
//! Provides [`CreateTool`], which creates a new file with the given content,
//! overwriting any existing file. Creates parent directories as needed.
//! Returns a summary of bytes and lines written.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::path_util::resolve_path;

/// Creates a new file with the given content, overwriting any existing file.
///
/// Parent directories are created automatically. Returns a summary including
/// the number of bytes and lines written.
pub struct CreateTool;

#[async_trait::async_trait]
impl Tool for CreateTool {
    fn name(&self) -> &'static str {
        "create"
    }

    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &'static str {
        "Create a new file and write the given content to it, creating parent \
         directories as needed. Required parameters: `path` (string) — the \
         file to create, and `content` (string) — the content to write. If the \
         file already exists, it is truncated and overwritten. To append to an \
         existing file without overwriting, use `append_to_file`. To update a \
         specific region, use `edit`."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "REQUIRED. Path to the file to create"
                },
                "content": {
                    "type": "string",
                    "description": "REQUIRED. Content to write to the new file"
                }
            },
            "required": ["path", "content"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    /// Creates a new file with the specified content.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `path` or `content` parameter is missing or invalid
    /// - Parent directories cannot be created due to permission issues
    /// - The file cannot be written due to permission issues or disk errors
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;
        let content = input["content"]
            .as_str()
            .context("Missing required 'content' parameter")?;

        let path = resolve_path(&ctx.working_dir, path_str);

        super::check_path_within_root_cached(&path, &ctx.working_dir, &ctx.canonical_cache)?;

        let existed = path.exists();

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directories: {}", parent.display()))?;
        }

        tokio::fs::write(&path, content)
            .await
            .with_context(|| format!("Failed to create file: {}", path.display()))?;

        let bytes = content.len();
        let lines = content.lines().count();

        let action = if existed { "Overwrote" } else { "Created" };

        Ok(ToolOutput {
            content: format!(
                "{} {} bytes ({} lines) in {}",
                action,
                bytes,
                lines,
                path.display()
            ),
            metadata: Some(json!({
                "path": path.display().to_string(),
                "byte_count": bytes,
                "line_count": lines,
                "file_count": 1,
            })),
        })
    }
}
