//! `git_remote` — Manage and inspect remotes.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that manages and inspects git remotes.
pub struct GitRemoteTool;

#[async_trait::async_trait]
impl Tool for GitRemoteTool {
    fn name(&self) -> &'static str {
        "git_remote"
    }

    fn description(&self) -> &'static str {
        "List, add, remove, or update git remotes. \
         Action: 'list' (default), 'add', 'remove', 'set-url'. \
         Caution: 'add', 'remove', and 'set-url' modify repository configuration."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "add", "remove", "set-url"],
                    "description": "Action to perform (default: list)"
                },
                "name": {
                    "type": "string",
                    "description": "Remote name (for add/remove/set-url)"
                },
                "url": {
                    "type": "string",
                    "description": "Remote URL (for add/set-url)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        // list is read-only, but add/remove/set-url write to .git/config
        "git:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let action = input["action"].as_str().unwrap_or("list");
        let name = input["name"].as_str();
        let url = input["url"].as_str();

        let (stdout, stderr) = match action {
            "list" => {
                let (out, err) = run_git(&["remote", "-v"], &ctx.working_dir)?;
                (out, err)
            }
            "add" => {
                let remote_name =
                    name.ok_or_else(|| anyhow::anyhow!("Remote name is required for 'add'"))?;
                let remote_url =
                    url.ok_or_else(|| anyhow::anyhow!("Remote URL is required for 'add'"))?;
                let (out, err) = run_git(
                    &["remote", "add", remote_name, remote_url],
                    &ctx.working_dir,
                )?;
                (out, err)
            }
            "remove" => {
                let remote_name =
                    name.ok_or_else(|| anyhow::anyhow!("Remote name is required for 'remove'"))?;
                let (out, err) = run_git(&["remote", "remove", remote_name], &ctx.working_dir)?;
                (out, err)
            }
            "set-url" => {
                let remote_name =
                    name.ok_or_else(|| anyhow::anyhow!("Remote name is required for 'set-url'"))?;
                let remote_url =
                    url.ok_or_else(|| anyhow::anyhow!("Remote URL is required for 'set-url'"))?;
                let (out, err) = run_git(
                    &["remote", "set-url", remote_name, remote_url],
                    &ctx.working_dir,
                )?;
                (out, err)
            }
            other => {
                return Err(anyhow::anyhow!(
                    "Unknown action: {}. Use 'list', 'add', 'remove', or 'set-url'.",
                    other
                ));
            }
        };

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git remote error: {}", stderr.trim()),
                metadata: None,
            });
        }

        // Parse remote list for structured data
        let mut remotes = Vec::new();
        if action == "list" {
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 3 {
                    remotes.push(json!({
                        "name": parts[0],
                        "url": parts[1],
                        "type": parts[2].trim_matches(|c| c == '(' || c == ')'),
                    }));
                }
            }
        }

        let content = if stdout.trim().is_empty() {
            match action {
                "list" => "No remotes configured.".to_string(),
                "add" => format!("Added remote '{}'.", name.unwrap_or("?")),
                "remove" => format!("Removed remote '{}'.", name.unwrap_or("?")),
                "set-url" => format!("Updated remote '{}' URL.", name.unwrap_or("?")),
                _ => "Done.".to_string(),
            }
        } else {
            stdout
        };

        Ok(ToolOutput {
            content,
            metadata: if action == "list" {
                Some(json!({ "remotes": remotes, "count": remotes.len() }))
            } else {
                None
            },
        })
    }
}
