//! File writing tool.
//!
//! Provides [`WriteTool`], which writes content to a file, creating parent
//! directories as needed. Returns a summary of bytes and lines written.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::path_util::resolve_path;

/// Writes string content to a file, creating parent directories if they do not exist.
///
/// Returns a summary including the number of bytes and lines written.
pub struct WriteTool;

#[async_trait::async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &'static str {
        "write"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &'static str {
        "Write content to a file, creating parent directories if needed. \
         Required parameters: `path` (string) — the destination file, and \
         `content` (string) — the content to write. If the file already exists, \
         it is overwritten in full. To append without overwriting, use \
         `append_to_file`; to create only when the file does not exist, use \
         `create`. The path must stay within the agent's working-directory root."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "REQUIRED. Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "REQUIRED. Content to write to the file"
                }
            },
            "required": ["path", "content"],
            "additionalProperties": false
        })
    }

    /// # Errors
    ///
    /// Returns an error if the category string cannot be converted or returned.
    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    /// # Errors
    ///
    /// Returns an error if the `path` or `content` parameters are missing,
    /// if parent directory creation fails, or if writing to the file fails.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;
        let content = input["content"]
            .as_str()
            .context("Missing required 'content' parameter")?;

        let path = resolve_path(&ctx.working_dir, path_str);

        super::check_path_within_root(&path, &ctx.working_dir)?;

        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .with_context(|| format!("Failed to create directories: {}", parent.display()))?;
        }

        tokio::fs::write(&path, content)
            .await
            .with_context(|| format!("Failed to write file: {}", path.display()))?;

        let bytes = content.len();
        let lines = content.lines().count();

        Ok(ToolOutput {
            content: format!(
                "Wrote {} bytes ({} lines) to {}",
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
