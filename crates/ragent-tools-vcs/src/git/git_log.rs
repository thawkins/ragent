//! `git_log` — Show commit history.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that shows commit history.
pub struct GitLogTool;

#[async_trait::async_trait]
impl Tool for GitLogTool {
    fn name(&self) -> &'static str {
        "git_log"
    }

    fn description(&self) -> &'static str {
        "Show the commit history. Optional: limit, branch, oneline, author, since."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "limit": {
                    "type": "integer",
                    "description": "Number of commits to show (default: 20)"
                },
                "branch": {
                    "type": "string",
                    "description": "Branch or ref to log (default: current branch)"
                },
                "oneline": {
                    "type": "boolean",
                    "description": "Use one-line format (default: true)"
                },
                "author": {
                    "type": "string",
                    "description": "Filter by author name or email"
                },
                "since": {
                    "type": "string",
                    "description": "Show commits newer than this date (e.g. 2024-01-01 or 1.week)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let limit = input["limit"].as_u64().unwrap_or(20);
        let oneline = input["oneline"].as_bool().unwrap_or(true);
        let branch = input["branch"].as_str().unwrap_or("HEAD");
        let author = input["author"].as_str();
        let since = input["since"].as_str();

        let format = if oneline {
            "%h %s (%an, %ar)"
        } else {
            "%H%nAuthor: %an <%ae>%nDate:   %ad%n%n    %s%n%n%b"
        };

        // Build command as owned strings to avoid lifetime issues.
        // Use --format=... (with equals) to prevent git from misinterpreting
        // the format string as a path/revision.
        let mut args: Vec<String> = vec!["log".to_string(), format!("--format={}", format)];

        if oneline {
            args.push("--decorate".to_string());
        }

        args.push("-n".to_string());
        args.push(format!("{}", limit));

        if let Some(author) = author {
            args.push("--author".to_string());
            args.push(author.to_string());
        }

        if let Some(since) = since {
            args.push("--since".to_string());
            args.push(since.to_string());
        }

        args.push(branch.to_string());

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();

        let (stdout, stderr) = run_git(&arg_refs, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git log error: {}", stderr.trim()),
                metadata: None,
            });
        }

        // Parse structured data
        let mut commits = Vec::new();
        for line in stdout.lines() {
            if oneline {
                if let Some(hash_end) = line.find(' ') {
                    let hash = &line[..hash_end];
                    let rest = &line[hash_end + 1..];
                    // Extract subject and author from format: "%h %s (%an, %ar)"
                    if let Some(paren_start) = rest.rfind(" (") {
                        let subject = &rest[..paren_start];
                        let meta = &rest[paren_start + 2..rest.len() - 1]; // strip ()
                        let parts: Vec<&str> = meta.split(", ").collect();
                        commits.push(json!({
                            "hash": hash,
                            "subject": subject,
                            "author": parts.first().unwrap_or(&"?"),
                            "date": parts.get(1).unwrap_or(&"?"),
                        }));
                    }
                }
            }
        }

        Ok(ToolOutput {
            content: if stdout.trim().is_empty() {
                "No commits found.".to_string()
            } else {
                stdout
            },
            metadata: if oneline {
                Some(json!({ "commits": commits, "count": commits.len() }))
            } else {
                None
            },
        })
    }
}
