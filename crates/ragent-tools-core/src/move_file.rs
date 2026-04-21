//! File move / rename tool.
//!
//! Provides [`MoveFileTool`], which moves or renames a file or directory using
//! the operating system's atomic rename syscall (`std::fs::rename`).

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

/// Move or rename a file or directory.
pub struct MoveFileTool;

#[async_trait::async_trait]
impl Tool for MoveFileTool {
    fn name(&self) -> &'static str {
        "move_file"
    }

    fn description(&self) -> &'static str {
        "Move or rename a file or directory. Uses an atomic OS rename so the \
         operation is instant on the same filesystem. Fails if source does not \
         exist or destination's parent directory does not exist."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source":      { "type": "string", "description": "Path to the file or directory to move" },
                "destination": { "type": "string", "description": "Destination path (including new name)" }
            },
            "required": ["source", "destination"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let src_str = input["source"]
            .as_str()
            .context("Missing required 'source' parameter")?;
        let dst_str = input["destination"]
            .as_str()
            .context("Missing required 'destination' parameter")?;

        let src = resolve_path(&ctx.working_dir, src_str);
        let dst = resolve_path(&ctx.working_dir, dst_str);

        super::check_path_within_root(&src, &ctx.working_dir)?;
        super::check_path_within_root(&dst, &ctx.working_dir)?;

        // Create destination parent directory if needed
        if let Some(parent) = dst.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!("Failed to create parent directory: {}", parent.display())
            })?;
        }

        tokio::fs::rename(&src, &dst).await.with_context(|| {
            format!("Failed to move '{}' to '{}'", src.display(), dst.display())
        })?;

        Ok(ToolOutput {
            content: format!("Moved '{}' → '{}'", src.display(), dst.display()),
            metadata: Some(json!({
                "source": src.display().to_string(),
                "destination": dst.display().to_string(),
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
