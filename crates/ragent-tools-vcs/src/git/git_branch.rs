//! `git_branch` — List branches.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that lists branches.
pub struct GitBranchTool;

#[async_trait::async_trait]
impl Tool for GitBranchTool {
    fn name(&self) -> &'static str {
        "git_branch"
    }

    fn description(&self) -> &'static str {
        "List branches. Shows current branch, local branches, and optionally remote branches with tracking info."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "all": {
                    "type": "boolean",
                    "description": "Include remote branches (default: true)"
                },
                "format": {
                    "type": "string",
                    "enum": ["short", "verbose"],
                    "description": "Output format: 'short' or 'verbose' with tracking info (default: short)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let all = input["all"].as_bool().unwrap_or(true);
        let format = input["format"].as_str().unwrap_or("short");
        let verbose = format == "verbose";

        let mut args = vec!["branch"];
        if verbose {
            args.push("-vv");
        }
        if all {
            args.push("-a");
        }

        let (stdout, stderr) = run_git(&args, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git branch error: {}", stderr.trim()),
                metadata: None,
            });
        }

        // Parse structured branch data
        let mut branches = Vec::new();
        let mut current_branch: Option<String> = None;

        for line in stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            let is_current = trimmed.starts_with("* ");
            let name = if is_current {
                current_branch = Some(
                    trimmed[2..]
                        .split_whitespace()
                        .next()
                        .unwrap_or("")
                        .to_string(),
                );
                trimmed[2..].to_string()
            } else {
                trimmed.to_string()
            };

            let mut tracking = None;
            if verbose {
                // Parse tracking info from verbose output: "  main                1234abcd [origin/main: ahead 1] ..."
                if let Some(open) = trimmed.find('[') {
                    if let Some(close) = trimmed.find(']') {
                        tracking = Some(trimmed[open + 1..close].to_string());
                    }
                }
            }

            branches.push(json!({
                "name": name.split_whitespace().next().unwrap_or(""),
                "current": is_current,
                "tracking": tracking,
            }));
        }

        Ok(ToolOutput {
            content: stdout,
            metadata: Some(json!({
                "branches": branches,
                "current_branch": current_branch,
                "count": branches.len(),
            })),
        })
    }
}
