//! File copy tool.
//!
//! Provides [`CopyFileTool`], which copies a file to a new location using
//! `tokio::fs::copy`, preserving file contents.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

/// Copy a file to a new location.
pub struct CopyFileTool;

#[async_trait::async_trait]
impl Tool for CopyFileTool {
    fn name(&self) -> &'static str {
        "copy_file"
    }

    fn description(&self) -> &'static str {
        "Copy a file to a new location. Creates the destination's parent \
         directories if they do not exist. The source file is not modified."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "source":      { "type": "string", "description": "Path to the source file" },
                "destination": { "type": "string", "description": "Destination path for the copy" }
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

        if let Some(parent) = dst.parent() {
            tokio::fs::create_dir_all(parent).await.with_context(|| {
                format!("Failed to create parent directory: {}", parent.display())
            })?;
        }

        let bytes = tokio::fs::copy(&src, &dst).await.with_context(|| {
            format!("Failed to copy '{}' to '{}'", src.display(), dst.display())
        })?;

        Ok(ToolOutput {
            content: format!(
                "Copied '{}' → '{}' ({bytes} bytes)",
                src.display(),
                dst.display()
            ),
            metadata: Some(json!({
                "source": src.display().to_string(),
                "destination": dst.display().to_string(),
                "bytes": bytes,
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
