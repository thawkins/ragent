//! Directory creation tool.
//!
//! Provides [`MakeDirTool`], which creates a directory (and all required parent
//! directories) at the specified path.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::path_util::resolve_path;

/// Create a directory and all missing parent directories.
pub struct MakeDirTool;

#[async_trait::async_trait]
impl Tool for MakeDirTool {
    fn name(&self) -> &'static str {
        "make_directory"
    }

    fn description(&self) -> &'static str {
        "Create a directory at the given path, including any missing parent \
         directories (equivalent to `mkdir -p`). Required parameter: `path` \
         (string). It is a no-op if the directory already exists. The path must \
         stay within the agent's working-directory root. To create multiple \
         nested directories in one call, include them all in `path`."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "REQUIRED. Directory path to create" }
            },
            "required": ["path"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;

        let path = resolve_path(&ctx.working_dir, path_str);
        super::check_path_within_root(&path, &ctx.working_dir)?;

        tokio::fs::create_dir_all(&path)
            .await
            .with_context(|| format!("Failed to create directory: {}", path.display()))?;

        Ok(ToolOutput {
            content: format!("Created directory: {}", path.display()),
            metadata: Some(json!({ "path": path.display().to_string() })),
        })
    }
}
