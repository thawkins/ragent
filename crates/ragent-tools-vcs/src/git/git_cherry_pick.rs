//! `git_cherry_pick` — Apply changes from specific commits.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that applies changes from specific commits.
pub struct GitCherryPickTool;

#[async_trait::async_trait]
impl Tool for GitCherryPickTool {
    fn name(&self) -> &'static str {
        "git_cherry_pick"
    }

    fn description(&self) -> &'static str {
        "Apply the changes introduced by specific commits onto the current branch. \
         Provide commit hashes. Set 'no_commit' to apply changes without creating a commit."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "commits": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Commit hashes to cherry-pick (required)"
                },
                "no_commit": {
                    "type": "boolean",
                    "description": "Apply changes without committing (git cherry-pick -n) (default: false)"
                }
            },
            "required": ["commits"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let commits = input["commits"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("'commits' array is required."))?;

        if commits.is_empty() {
            return Err(anyhow::anyhow!("'commits' array is empty."));
        }

        let no_commit = input["no_commit"].as_bool().unwrap_or(false);

        let mut args: Vec<String> = vec!["cherry-pick".to_string()];

        if no_commit {
            args.push("-n".to_string());
        }

        for c in commits {
            if let Some(hash) = c.as_str() {
                args.push(hash.to_string());
            }
        }

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, stderr) = run_git(&arg_refs, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git cherry-pick error: {}", stderr.trim()),
                metadata: None,
            });
        }

        let content = if stdout.trim().is_empty() {
            let count = commits.len();
            if no_commit {
                format!(
                    "Applied changes from {} commit(s) without committing.",
                    count
                )
            } else {
                format!("Cherry-picked {} commit(s).", count)
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
