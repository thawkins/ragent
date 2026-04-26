//! `git_diff` — Show changes between working tree, index, and commits.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that shows changes (diff) in the working tree or index.
pub struct GitDiffTool;

#[async_trait::async_trait]
impl Tool for GitDiffTool {
    fn name(&self) -> &'static str {
        "git_diff"
    }

    fn description(&self) -> &'static str {
        "Show changes (diff) between working tree, staged index, or commits. \
         Target: 'working' (default), 'staged', or a commit ref. \
         Optional: path filter, stat summary."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "target": {
                    "type": "string",
                    "description": "What to diff: 'working' (default), 'staged', or a commit ref"
                },
                "path": {
                    "type": "string",
                    "description": "Limit diff to a specific file or directory"
                },
                "stat": {
                    "type": "boolean",
                    "description": "Show stat summary instead of full diff (default: false)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let target = input["target"].as_str().unwrap_or("working");
        let path = input["path"].as_str();
        let stat = input["stat"].as_bool().unwrap_or(false);

        let mut args: Vec<&str> = vec!["diff"];

        if stat {
            args.push("--stat");
        }

        match target {
            "staged" | "cached" => args.push("--cached"),
            "working" => {} // default
            ref_str => args.push(ref_str),
        }

        // Add -- before path to disambiguate from refs
        if let Some(p) = path {
            args.push("--");
            args.push(p);
        }

        let (stdout, stderr) = run_git(&args, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git diff error: {}", stderr.trim()),
                metadata: None,
            });
        }

        Ok(ToolOutput {
            content: if stdout.trim().is_empty() {
                "No differences found.".to_string()
            } else {
                stdout
            },
            metadata: None,
        })
    }
}
