//! `git_pull` — Fetch and integrate remote changes.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that fetches and integrates remote changes.
pub struct GitPullTool;

#[async_trait::async_trait]
impl Tool for GitPullTool {
    fn name(&self) -> &'static str {
        "git_pull"
    }

    fn description(&self) -> &'static str {
        "Fetch and integrate changes from a remote repository. \
         Use 'rebase' to rebase instead of merging."
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
                    "description": "Branch to pull (default: current tracking branch)"
                },
                "rebase": {
                    "type": "boolean",
                    "description": "Rebase instead of merge (git pull --rebase) (default: false)"
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
        let rebase = input["rebase"].as_bool().unwrap_or(false);

        let mut args: Vec<String> = vec!["pull".to_string()];

        if rebase {
            args.push("--rebase".to_string());
        }

        args.push(remote.to_string());

        if let Some(b) = branch {
            args.push(b.to_string());
        }

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, stderr) = run_git(&arg_refs, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git pull error: {}", stderr.trim()),
                metadata: None,
            });
        }

        let content = if stdout.trim().is_empty() {
            "Pulled changes from remote.".to_string()
        } else {
            stdout
        };

        Ok(ToolOutput {
            content,
            metadata: None,
        })
    }
}
