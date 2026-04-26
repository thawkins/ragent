//! `git_push` — Push branches and tags to remote.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that pushes branches and tags to a remote.
pub struct GitPushTool;

#[async_trait::async_trait]
impl Tool for GitPushTool {
    fn name(&self) -> &'static str {
        "git_push"
    }

    fn description(&self) -> &'static str {
        "Push branches and tags to a remote repository. \
         CAUTION: force push can rewrite history. Use 'force' with care."
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
                    "description": "Branch to push (default: current branch)"
                },
                "force": {
                    "type": "boolean",
                    "description": "Force push with lease (git push --force-with-lease) (default: false)"
                },
                "tags": {
                    "type": "boolean",
                    "description": "Push all tags (git push --tags) (default: false)"
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
        let force = input["force"].as_bool().unwrap_or(false);
        let tags = input["tags"].as_bool().unwrap_or(false);

        let mut args: Vec<String> = vec!["push".to_string()];

        if force {
            args.push("--force-with-lease".to_string());
        }

        if tags {
            args.push("--tags".to_string());
        }

        args.push(remote.to_string());

        if let Some(b) = branch {
            args.push(b.to_string());
        }

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, stderr) = run_git(&arg_refs, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git push error: {}", stderr.trim()),
                metadata: None,
            });
        }

        let content = if stdout.trim().is_empty() {
            if tags {
                format!("Pushed tags to {}.", remote)
            } else {
                format!(
                    "Pushed {} to {}.",
                    branch.unwrap_or("current branch"),
                    remote
                )
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
