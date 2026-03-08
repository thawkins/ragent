//! File writing tool.
//!
//! Provides [`WriteTool`], which writes content to a file, creating parent
//! directories as needed. Returns a summary of bytes and lines written.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

/// Writes string content to a file, creating parent directories if they do not exist.
///
/// Returns a summary including the number of bytes and lines written.
pub struct WriteTool;

#[async_trait::async_trait]
impl Tool for WriteTool {
    fn name(&self) -> &str {
        "write"
    }

    fn description(&self) -> &str {
        "Write content to a file. Creates parent directories if needed."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to write"
                },
                "content": {
                    "type": "string",
                    "description": "Content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"].as_str().context("Missing 'path' parameter")?;
        let content = input["content"]
            .as_str()
            .context("Missing 'content' parameter")?;

        let path = resolve_path(&ctx.working_dir, path_str);

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
                "bytes": bytes,
                "lines": lines,
            })),
        })
    }
}

fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}
