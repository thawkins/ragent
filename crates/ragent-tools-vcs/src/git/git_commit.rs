//! `git_commit` — Create a commit.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that creates a git commit.
pub struct GitCommitTool;

#[async_trait::async_trait]
impl Tool for GitCommitTool {
    fn name(&self) -> &'static str {
        "git_commit"
    }

    fn description(&self) -> &'static str {
        "Create a new git commit from staged changes. Required parameter: 'message' (string) - the commit message. \
         Optional: 'all' (boolean, default false) to stage all modified tracked files before committing, \
         'amend' (boolean, default false) to amend the previous commit rather than creating a new one, \
         and 'no_verify' (boolean, default false) to bypass pre-commit hooks. \
         Common gotcha: if nothing is staged and 'all' is false the commit will fail; use git_add first or set 'all' to true."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "message": {
                    "type": "string",
                    "description": "Commit message (required)"
                },
                "all": {
                    "type": "boolean",
                    "description": "Stage all modified tracked files before committing (git commit -a) (default: false)"
                },
                "amend": {
                    "type": "boolean",
                    "description": "Amend the previous commit (git commit --amend) (default: false)"
                },
                "no_verify": {
                    "type": "boolean",
                    "description": "Bypass pre-commit hooks (git commit --no-verify) (default: false)"
                }
            },
            "required": ["message"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let message = input["message"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Commit 'message' is required."))?;

        let all = input["all"].as_bool().unwrap_or(false);
        let amend = input["amend"].as_bool().unwrap_or(false);
        let no_verify = input["no_verify"].as_bool().unwrap_or(false);

        let mut args: Vec<String> =
            vec!["commit".to_string(), "-m".to_string(), message.to_string()];

        if all {
            args.push("-a".to_string());
        }
        if amend {
            args.push("--amend".to_string());
        }
        if no_verify {
            args.push("--no-verify".to_string());
        }

        let arg_refs: Vec<&str> = args.iter().map(std::string::String::as_str).collect();
        let (stdout, stderr) = run_git(&arg_refs, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git commit error: {}", stderr.trim()),
                metadata: None,
            });
        }

        // Extract commit hash from output like "[main abc1234] message"
        let commit_hash = stdout.lines().find(|l| l.starts_with('[')).and_then(|l| {
            l.split(')')
                .next()
                .and_then(|s| s.split_whitespace().nth(1))
                .map(std::string::ToString::to_string)
        });

        let content = if stdout.trim().is_empty() {
            if amend {
                "Amended previous commit.".to_string()
            } else {
                "Committed changes.".to_string()
            }
        } else {
            stdout
        };

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "commit_hash": commit_hash,
                "amended": amend,
            })),
        })
    }
}
