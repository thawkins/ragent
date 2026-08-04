//! File removal tool.
//!
//! Provides [`RmTool`], which deletes a single specified file.
//! Wildcards and glob patterns are rejected. Returns success or failure status.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::path_util::resolve_path;

/// Deletes a single file. Rejects wildcards and glob patterns.
pub struct RmTool;

#[async_trait::async_trait]
impl Tool for RmTool {
    fn name(&self) -> &'static str {
        "rm"
    }

    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &'static str {
        "Delete a single file. Required parameter: `path` (string). Wildcards and \
         glob patterns (`*`, `?`, `[...]`) are not allowed — specify one exact \
         file. Fails if the file does not exist or if the path is a directory. \
         To delete directories, use `bash` with an appropriate command instead."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "REQUIRED. Path to the file to delete. Must be a single file, no wildcards or glob patterns."
                }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    /// Deletes a single file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `path` parameter is missing or invalid
    /// - The path contains wildcards or glob patterns (`*`, `?`, `[`)
    /// - The file does not exist
    /// - The path is a directory, not a file
    /// - The file cannot be deleted due to permission issues
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;

        if path_str.contains('*') || path_str.contains('?') || path_str.contains('[') {
            bail!(
                "Wildcards and glob patterns are not allowed in file paths. Specify a single file to delete: {path_str}"
            );
        }

        let path = resolve_path(&ctx.working_dir, path_str);

        super::check_path_within_root(&path, &ctx.working_dir)?;

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
