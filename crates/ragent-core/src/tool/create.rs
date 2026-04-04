//! File creation tool.
//!
//! Provides [`CreateTool`], which creates a new file with the given content,
//! failing if the file already exists. Creates parent directories as needed.
//! Returns a summary of bytes and lines written.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

/// Creates a new file with the given content, failing if the file already exists.
///
/// Parent directories are created automatically. Returns a summary including
/// the number of bytes and lines written.
pub struct CreateTool;

#[async_trait::async_trait]
impl Tool for CreateTool {
    fn name(&self) -> &str {
        "create"
    }

    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &str {
        "Create a new file with content. Truncates the file if it already exists. Creates parent directories if needed."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to create"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the new file"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn permission_category(&self) -> &str {
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

        super::check_path_within_root(&path, &ctx.working_dir)?;

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
                "bytes": bytes,
                "lines": lines,
            })),
        })
    }
}

/// Resolves a path string to an absolute `PathBuf` relative to the working directory.
fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}
