//! `git_checkout` — Switch branches or restore files.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that switches branches or restores working tree files.
pub struct GitCheckoutTool;

#[async_trait::async_trait]
impl Tool for GitCheckoutTool {
    fn name(&self) -> &'static str {
        "git_checkout"
    }

    fn description(&self) -> &'static str {
        "Switch branches or restore working tree files. \
         Provide 'branch' to switch branches. Set 'create_branch' to create and switch to a new branch. \
         Provide 'paths' with optional 'source' to restore specific files from a ref (default: HEAD)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "branch": {
                    "type": "string",
                    "description": "Branch name to switch to. Ignored when 'paths' is provided."
                },
                "create_branch": {
                    "type": "boolean",
                    "description": "Create and checkout a new branch (git checkout -b). Requires 'branch'. (default: false)"
                },
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Files or directories to restore from 'source'."
                },
                "source": {
                    "type": "string",
                    "description": "Ref to restore files from (default: HEAD). Only used with 'paths'."
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let paths = input["paths"].as_array();

        let mut args: Vec<String>;

        if let Some(ps) = paths {
            // Restore files: git checkout <source> -- <paths>
            if ps.is_empty() {
                return Err(anyhow::anyhow!("Paths array is empty."));
            }
            let source = input["source"].as_str().unwrap_or("HEAD");
            args = vec!["checkout".to_string(), source.to_string(), "--".to_string()];
            for p in ps {
                if let Some(s) = p.as_str() {
                    args.push(s.to_string());
                }
            }
        } else {
            // Switch branch
            let branch = input["branch"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Must provide either 'branch' or 'paths'."))?;

            let create = input["create_branch"].as_bool().unwrap_or(false);

            if create {
                args = vec!["checkout".to_string(), "-b".to_string(), branch.to_string()];
            } else {
                args = vec!["checkout".to_string(), branch.to_string()];
            }
        }

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, stderr) = run_git(&arg_refs, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git checkout error: {}", stderr.trim()),
                metadata: None,
            });
        }

        let content = if stdout.trim().is_empty() {
            if paths.is_some() {
                let source = input["source"].as_str().unwrap_or("HEAD");
                format!("Restored files from {}.", source)
            } else {
                let branch = input["branch"].as_str().unwrap_or("?");
                let create = input["create_branch"].as_bool().unwrap_or(false);
                if create {
                    format!("Switched to new branch '{}'.", branch)
                } else {
                    format!("Switched to branch '{}'.", branch)
                }
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
