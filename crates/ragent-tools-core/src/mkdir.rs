//! Directory creation tool.
//!
//! Provides [`MakeDirTool`], which creates a directory (and all required parent
//! directories) at the specified path.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

/// Create a directory and all missing parent directories.
pub struct MakeDirTool;

#[async_trait::async_trait]
impl Tool for MakeDirTool {
    fn name(&self) -> &'static str {
        "make_directory"
    }

    fn description(&self) -> &'static str {
        "Create a directory at the given path, including any missing parent \
         directories (equivalent to `mkdir -p`). No-op if the directory already \
         exists."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "Directory path to create" }
            },
            "required": ["path"]
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

fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}
