//! `git_status` — Show working tree status.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that shows the working tree status.
pub struct GitStatusTool;

#[async_trait::async_trait]
impl Tool for GitStatusTool {
    fn name(&self) -> &'static str {
        "git_status"
    }

    fn description(&self) -> &'static str {
        "Show the working tree status: modified, staged, untracked, and conflicted files."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "short": {
                    "type": "boolean",
                    "description": "Use short format (default: false)"
                },
                "branch": {
                    "type": "boolean",
                    "description": "Include branch name in output (default: true)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let short = input["short"].as_bool().unwrap_or(false);
        let branch = input["branch"].as_bool().unwrap_or(true);

        let mut args = vec!["status"];
        if short {
            args.push("--short");
            if branch {
                args.push("--branch");
            }
        } else {
            if branch {
                args.push("--branch");
            }
            args.push("--porcelain=v2");
        }

        let (stdout, stderr) = run_git(&args, &ctx.working_dir)?;

        if stdout.trim().is_empty() && !stderr.is_empty() {
            return Ok(ToolOutput {
                content: format!("git status error: {}", stderr.trim()),
                metadata: None,
            });
        }

        // Parse structured status data
        let mut modified = Vec::new();
        let mut staged = Vec::new();
        let mut untracked = Vec::new();
        let mut conflicted = Vec::new();

        for line in stdout.lines() {
            if line.starts_with("1 ") && line.len() >= 4 {
                let xy = &line[2..4];
                let path = line[4..].trim_start();
                match xy.chars().next() {
                    Some('M') | Some('A') | Some('D') | Some('R') | Some('C') => {
                        staged.push(path.to_string())
                    }
                    _ => {}
                }
                match xy.chars().nth(1) {
                    Some('M') | Some('D') => modified.push(path.to_string()),
                    Some('U') | Some('A') if xy == "UU" || xy == "AA" || xy == "DD" => {
                        conflicted.push(path.to_string());
                    }
                    _ => {}
                }
            } else if line.starts_with("? ") {
                untracked.push(line[2..].to_string());
            } else if line.starts_with("2 ") && line.len() >= 4 {
                // renamed entry
                let xy = &line[2..4];
                let rest = &line[4..];
                if let Some(path) = rest.split('\t').next() {
                    staged.push(path.to_string());
                    if xy.chars().nth(1) == Some('M') {
                        modified.push(path.to_string());
                    }
                }
            }
        }

        let branch_name = if short {
            stdout
                .lines()
                .find(|l| l.starts_with("## "))
                .map(|l| l[3..].to_string())
        } else {
            stdout
                .lines()
                .find(|l| l.starts_with("# branch.head "))
                .map(|l| l[14..].trim().to_string())
        };

        let content = if stdout.trim().is_empty() {
            format!(
                "Working tree clean{}.",
                branch_name
                    .as_ref()
                    .map(|b| format!(" on branch {}", b))
                    .unwrap_or_default()
            )
        } else {
            stdout
        };

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "branch": branch_name,
                "staged": staged,
                "modified": modified,
                "untracked": untracked,
                "conflicted": conflicted,
                "staged_count": staged.len(),
                "modified_count": modified.len(),
                "untracked_count": untracked.len(),
                "conflicted_count": conflicted.len(),
            })),
        })
    }
}
