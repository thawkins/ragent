//! File reading tool.
//!
//! Provides [`ReadTool`], which reads file contents and returns them with
//! line numbers. Supports optional line-range selection for viewing subsets.

use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

/// Reads a file's contents and returns them with line numbers prefixed.
///
/// Supports optional `start_line` and `end_line` parameters (1-based, inclusive)
/// for reading a specific range of lines.
pub struct ReadTool;

#[async_trait::async_trait]
impl Tool for ReadTool {
    fn name(&self) -> &str {
        "read"
    }

    fn description(&self) -> &str {
        "Read the contents of a file. Supports optional line range selection."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to read"
                },
                "start_line": {
                    "type": "integer",
                    "description": "Starting line number (1-based, inclusive)"
                },
                "end_line": {
                    "type": "integer",
                    "description": "Ending line number (1-based, inclusive)"
                }
            },
            "required": ["path"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"].as_str().context("Missing 'path' parameter")?;

        let path = resolve_path(&ctx.working_dir, path_str);

        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        let start_line = input["start_line"].as_u64().map(|n| n as usize);
        let end_line = input["end_line"].as_u64().map(|n| n as usize);

        let output = match (start_line, end_line) {
            (Some(start), Some(end)) => {
                let lines: Vec<&str> = content.lines().collect();
                let start = start.saturating_sub(1).min(lines.len());
                let end = end.min(lines.len());
                lines[start..end]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{:>4}  {}", start + i + 1, line))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            (Some(start), None) => {
                let lines: Vec<&str> = content.lines().collect();
                let start = start.saturating_sub(1).min(lines.len());
                lines[start..]
                    .iter()
                    .enumerate()
                    .map(|(i, line)| format!("{:>4}  {}", start + i + 1, line))
                    .collect::<Vec<_>>()
                    .join("\n")
            }
            _ => content
                .lines()
                .enumerate()
                .map(|(i, line)| format!("{:>4}  {}", i + 1, line))
                .collect::<Vec<_>>()
                .join("\n"),
        };

        Ok(ToolOutput {
            content: output,
            metadata: None,
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
