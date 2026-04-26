//! `git_fetch` — Fetch from remote without merging.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that fetches from a remote without merging.
pub struct GitFetchTool;

#[async_trait::async_trait]
impl Tool for GitFetchTool {
    fn name(&self) -> &'static str {
        "git_fetch"
    }

    fn description(&self) -> &'static str {
        "Fetch from a remote repository without merging. \
         Use 'prune' to remove deleted remote branches. Use 'all' to fetch all remotes."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "remote": {
                    "type": "string",
                    "description": "Remote name (default: origin)"
                },
                "branch": {
                    "type": "string",
                    "description": "Specific branch or ref to fetch"
                },
                "prune": {
                    "type": "boolean",
                    "description": "Prune deleted remote branches (git fetch --prune) (default: false)"
                },
                "all": {
                    "type": "boolean",
                    "description": "Fetch all remotes (git fetch --all) (default: false)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let remote = input["remote"].as_str().unwrap_or("origin");
        let branch = input["branch"].as_str();
        let prune = input["prune"].as_bool().unwrap_or(false);
        let all = input["all"].as_bool().unwrap_or(false);

        let mut args: Vec<String> = vec!["fetch".to_string()];

        if all {
            args.push("--all".to_string());
        }

        if prune {
            args.push("--prune".to_string());
        }

        if !all {
            args.push(remote.to_string());
            if let Some(b) = branch {
                args.push(b.to_string());
            }
        }

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, stderr) = run_git(&arg_refs, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git fetch error: {}", stderr.trim()),
                metadata: None,
            });
        }

        let content = if stdout.trim().is_empty() {
            if all {
                "Fetched all remotes.".to_string()
            } else if prune {
                format!("Fetched {} with prune.", remote)
            } else {
                format!("Fetched from {}.", remote)
            }
        } else {
            stdout
        };

        Ok(ToolOutput {
            content,
            metadata: None,
        })
    }
}
