//! File append tool.
//!
//! Provides [`AppendFileTool`], which appends text to an existing file without
//! reading or rewriting the entire file.  Creates the file (and parent
//! directories) if they do not exist.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use tokio::fs::OpenOptions;
use tokio::io::AsyncWriteExt as _;

use super::{Tool, ToolContext, ToolOutput};

/// Append text to a file.
pub struct AppendFileTool;

#[async_trait::async_trait]
impl Tool for AppendFileTool {
    fn name(&self) -> &'static str {
        "append_to_file"
    }

    fn description(&self) -> &'static str {
        "Append text to the end of a file. Creates the file and any missing \
         parent directories if they do not exist. More efficient than a full \
         rewrite when only adding content to the end."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path":    { "type": "string", "description": "Path to the file to append to" },
                "content": { "type": "string", "description": "Text to append" }
            },
            "required": ["path", "content"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

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

        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .await
            .with_context(|| format!("Failed to open file for appending: {}", path.display()))?;

        file.write_all(content.as_bytes())
            .await
            .with_context(|| format!("Failed to append to file: {}", path.display()))?;

        let bytes = content.len();
        let lines = content.lines().count();

        Ok(ToolOutput {
            content: format!(
                "Appended {} bytes ({} lines) to {}",
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
