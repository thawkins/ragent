//! `git_show` — Show commit details.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that shows details of a commit or tag.
pub struct GitShowTool;

#[async_trait::async_trait]
impl Tool for GitShowTool {
    fn name(&self) -> &'static str {
        "git_show"
    }

    fn description(&self) -> &'static str {
        "Show details of a commit, tag, or other git object. \
         Shows author, date, message, and file statistics."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "ref": {
                    "type": "string",
                    "description": "Commit hash, tag, or ref to show (default: HEAD)"
                },
                "stat": {
                    "type": "boolean",
                    "description": "Include file change statistics (default: true)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let git_ref = input["ref"].as_str().unwrap_or("HEAD");
        let stat = input["stat"].as_bool().unwrap_or(true);

        let mut args = vec!["show", "--format=fuller"];
        if stat {
            args.push("--stat");
        }
        args.push(git_ref);

        let (stdout, stderr) = run_git(&args, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git show error: {}", stderr.trim()),
                metadata: None,
            });
        }

        // Extract structured metadata from the output
        let mut author = None;
        let mut date = None;
        let mut subject = None;
        let mut commit_hash = None;

        for line in stdout.lines() {
            if line.starts_with("commit ") && commit_hash.is_none() {
                commit_hash = Some(line[7..].to_string());
            }
            if line.starts_with("Author:     ") && author.is_none() {
                author = Some(line[12..].to_string());
            }
            if line.starts_with("CommitDate: ") && date.is_none() {
                date = Some(line[12..].to_string());
            }
            if line.starts_with("    ") && subject.is_none() {
                subject = Some(line.trim().to_string());
            }
        }

        Ok(ToolOutput {
            content: stdout,
            metadata: Some(json!({
                "commit": commit_hash,
                "author": author,
                "date": date,
                "subject": subject,
            })),
        })
    }
}
