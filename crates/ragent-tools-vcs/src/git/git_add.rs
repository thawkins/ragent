//! `git_add` — Stage files for commit.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that stages files for commit.
pub struct GitAddTool;

#[async_trait::async_trait]
impl Tool for GitAddTool {
    fn name(&self) -> &'static str {
        "git_add"
    }

    fn description(&self) -> &'static str {
        "Stage files for the next commit. \
         Provide file paths or use 'all' to stage all changes. \
         Use 'update' to stage changes only to tracked files."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Files or directories to stage"
                },
                "all": {
                    "type": "boolean",
                    "description": "Stage all changes (git add -A). Overrides paths. (default: false)"
                },
                "update": {
                    "type": "boolean",
                    "description": "Stage changes only to tracked files (git add -u). Overrides paths. (default: false)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let all = input["all"].as_bool().unwrap_or(false);
        let update = input["update"].as_bool().unwrap_or(false);

        let mut args: Vec<String> = vec!["add".to_string()];

        if all {
            args.push("-A".to_string());
        } else if update {
            args.push("-u".to_string());
        } else if let Some(paths) = input["paths"].as_array() {
            if paths.is_empty() {
                return Err(anyhow::anyhow!(
                    "No paths provided. Specify 'paths', set 'all' to true, or set 'update' to true."
                ));
            }
            for p in paths {
                if let Some(s) = p.as_str() {
                    args.push(s.to_string());
                }
            }
        } else {
            return Err(anyhow::anyhow!(
                "No paths provided. Specify 'paths', set 'all' to true, or set 'update' to true."
            ));
        }

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, stderr) = run_git(&arg_refs, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git add error: {}", stderr.trim()),
                metadata: None,
            });
        }

        let content = if stdout.trim().is_empty() {
            if all {
                "Staged all changes.".to_string()
            } else if update {
                "Staged changes to tracked files.".to_string()
            } else {
                "Staged specified files.".to_string()
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
