//! `git_tag` — List, show, create, and delete tags.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that manages git tags.
pub struct GitTagTool;

#[async_trait::async_trait]
impl Tool for GitTagTool {
    fn name(&self) -> &'static str {
        "git_tag"
    }

    fn description(&self) -> &'static str {
        "List, show, create, or delete git tags. \
         Action: 'list' (default), 'show', 'create', 'delete'. \
         Caution: 'create' and 'delete' modify the repository."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "show", "create", "delete"],
                    "description": "Action to perform (default: list)"
                },
                "name": {
                    "type": "string",
                    "description": "Tag name (for show/create/delete)"
                },
                "message": {
                    "type": "string",
                    "description": "Annotated tag message (for create)"
                },
                "ref": {
                    "type": "string",
                    "description": "Target commit or ref (for create, default: HEAD)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        // list/show are read-only, but create/delete modify the repo
        "git:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let action = input["action"].as_str().unwrap_or("list");
        let name = input["name"].as_str();
        let message = input["message"].as_str();
        let git_ref = input["ref"].as_str().unwrap_or("HEAD");

        let (stdout, stderr) = match action {
            "list" => {
                let (out, err) = run_git(&["tag", "-l"], &ctx.working_dir)?;
                (out, err)
            }
            "show" => {
                let tag_name =
                    name.ok_or_else(|| anyhow::anyhow!("Tag name is required for 'show'"))?;
                let (out, err) = run_git(&["show", tag_name], &ctx.working_dir)?;
                (out, err)
            }
            "create" => {
                let tag_name =
                    name.ok_or_else(|| anyhow::anyhow!("Tag name is required for 'create'"))?;
                let mut args = vec!["tag"];
                if let Some(msg) = message {
                    args.push("-a");
                    args.push(tag_name);
                    args.push("-m");
                    args.push(msg);
                } else {
                    args.push(tag_name);
                }
                args.push(git_ref);
                let (out, err) = run_git(&args, &ctx.working_dir)?;
                (out, err)
            }
            "delete" => {
                let tag_name =
                    name.ok_or_else(|| anyhow::anyhow!("Tag name is required for 'delete'"))?;
                let (out, err) = run_git(&["tag", "-d", tag_name], &ctx.working_dir)?;
                (out, err)
            }
            other => {
                return Err(anyhow::anyhow!(
                    "Unknown action: {}. Use 'list', 'show', 'create', or 'delete'.",
                    other
                ));
            }
        };

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git tag error: {}", stderr.trim()),
                metadata: None,
            });
        }

        // Parse tag list for structured data
        let mut tags = Vec::new();
        if action == "list" {
            for line in stdout.lines() {
                let trimmed = line.trim();
                if !trimmed.is_empty() {
                    tags.push(json!({ "name": trimmed }));
                }
            }
        }

        let content = if stdout.trim().is_empty() {
            match action {
                "list" => "No tags found.".to_string(),
                "create" => format!("Created tag '{}'.", name.unwrap_or("?")),
                "delete" => format!("Deleted tag '{}'.", name.unwrap_or("?")),
                "show" => "Tag details shown.".to_string(),
                _ => "Done.".to_string(),
            }
        } else {
            stdout
        };

        Ok(ToolOutput {
            content,
            metadata: if action == "list" {
                Some(json!({ "tags": tags, "count": tags.len() }))
            } else {
                None
            },
        })
    }
}
