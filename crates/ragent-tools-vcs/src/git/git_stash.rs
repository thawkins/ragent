//! `git_stash` — Stash and unstash changes.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that stashes and unstashes changes.
pub struct GitStashTool;

#[async_trait::async_trait]
impl Tool for GitStashTool {
    fn name(&self) -> &'static str {
        "git_stash"
    }

    fn description(&self) -> &'static str {
        "Stash and unstash changes. \
         Actions: 'push' (save changes, default), 'pop' (apply and remove latest), \
         'apply' (apply without removing), 'drop' (remove a stash), \
         'list' (show all stashes), 'clear' (remove all stashes)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["push", "pop", "apply", "drop", "list", "clear"],
                    "description": "Stash action to perform (default: push)"
                },
                "message": {
                    "type": "string",
                    "description": "Message for the stash (only used with 'push')"
                },
                "index": {
                    "type": "integer",
                    "description": "Stash index (for pop/apply/drop, default: 0)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let action = input["action"].as_str().unwrap_or("push");
        let message = input["message"].as_str();
        let index = input["index"].as_u64().unwrap_or(0);

        let mut args: Vec<String> = vec!["stash".to_string()];

        match action {
            "push" => {
                args.push("push".to_string());
                if let Some(msg) = message {
                    args.push("-m".to_string());
                    args.push(msg.to_string());
                }
            }
            "pop" => {
                args.push("pop".to_string());
                if index > 0 {
                    args.push(format!("stash@{{{}}}", index));
                }
            }
            "apply" => {
                args.push("apply".to_string());
                if index > 0 {
                    args.push(format!("stash@{{{}}}", index));
                }
            }
            "drop" => {
                args.push("drop".to_string());
                if index > 0 {
                    args.push(format!("stash@{{{}}}", index));
                } else {
                    args.push("stash@{0}".to_string());
                }
            }
            "list" => {
                args.push("list".to_string());
            }
            "clear" => {
                args.push("clear".to_string());
            }
            other => {
                return Err(anyhow::anyhow!(
                    "Unknown stash action: {}. Use 'push', 'pop', 'apply', 'drop', 'list', or 'clear'.",
                    other
                ));
            }
        }

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, stderr) = run_git(&arg_refs, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git stash error: {}", stderr.trim()),
                metadata: None,
            });
        }

        // Parse stash list for structured data
        let mut stashes = Vec::new();
        if action == "list" {
            for line in stdout.lines() {
                // Format: "stash@{0}: WIP on main: abc123 message"
                if let Some(colon_pos) = line.find(": ") {
                    let id = &line[..colon_pos];
                    let rest = &line[colon_pos + 2..];
                    stashes.push(json!({
                        "id": id,
                        "description": rest,
                    }));
                }
            }
        }

        let content = if stdout.trim().is_empty() {
            match action {
                "push" => "Changes stashed.".to_string(),
                "pop" => "Applied and removed stash.".to_string(),
                "apply" => "Applied stash.".to_string(),
                "drop" => "Dropped stash.".to_string(),
                "list" => "No stashes.".to_string(),
                "clear" => "All stashes cleared.".to_string(),
                _ => "Done.".to_string(),
            }
        } else {
            stdout
        };

        Ok(ToolOutput {
            content,
            metadata: if action == "list" {
                Some(json!({ "stashes": stashes, "count": stashes.len() }))
            } else {
                None
            },
        })
    }
}
