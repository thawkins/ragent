//! File removal tool.
//!
//! Provides [`RmTool`], which deletes a single specified file.
//! Wildcards and glob patterns are rejected. Returns success or failure status.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

/// Deletes a single file. Rejects wildcards and glob patterns.
pub struct RmTool;

#[async_trait::async_trait]
impl Tool for RmTool {
    fn name(&self) -> &str {
        "rm"
    }

    fn description(&self) -> &str {
        "Delete a single file. Wildcards are not allowed. Fails if the file does not exist."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to delete. Must be a single file, no wildcards or glob patterns."
                }
            },
            "required": ["path"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"].as_str().context("Missing 'path' parameter")?;

        if path_str.contains('*') || path_str.contains('?') || path_str.contains('[') {
            bail!("Wildcards are not allowed: {}", path_str);
        }

        let path = resolve_path(&ctx.working_dir, path_str);

        if !path.exists() {
            bail!("File not found: {}", path.display());
        }

        if path.is_dir() {
            bail!("Path is a directory, not a file: {}", path.display());
        }

        tokio::fs::remove_file(&path)
            .await
            .with_context(|| format!("Failed to delete file: {}", path.display()))?;

        Ok(ToolOutput {
            content: format!("Deleted {}", path.display()),
            metadata: Some(json!({
                "path": path.display().to_string(),
                "deleted": true,
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
