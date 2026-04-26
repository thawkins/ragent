//! `git_clone` — Clone a repository.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that clones a git repository.
pub struct GitCloneTool;

#[async_trait::async_trait]
impl Tool for GitCloneTool {
    fn name(&self) -> &'static str {
        "git_clone"
    }

    fn description(&self) -> &'static str {
        "Clone a git repository into a new directory. \
         The clone is created inside the working directory. \
         Optional: specify branch, shallow clone depth, or create a bare clone."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "Repository URL to clone (required)"
                },
                "directory": {
                    "type": "string",
                    "description": "Target directory name (default: inferred from URL)"
                },
                "branch": {
                    "type": "string",
                    "description": "Branch to checkout after clone (--branch)"
                },
                "depth": {
                    "type": "integer",
                    "description": "Shallow clone depth (--depth)"
                },
                "bare": {
                    "type": "boolean",
                    "description": "Create a bare repository (--bare) (default: false)"
                }
            },
            "required": ["url"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let url = input["url"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Repository 'url' is required."))?;
        let directory = input["directory"].as_str();
        let branch = input["branch"].as_str();
        let depth = input["depth"].as_u64();
        let bare = input["bare"].as_bool().unwrap_or(false);

        let mut args: Vec<String> = vec!["clone".to_string()];

        if bare {
            args.push("--bare".to_string());
        }

        if let Some(b) = branch {
            args.push("--branch".to_string());
            args.push(b.to_string());
        }

        if let Some(d) = depth {
            args.push("--depth".to_string());
            args.push(format!("{}", d));
        }

        args.push(url.to_string());

        if let Some(dir) = directory {
            args.push(dir.to_string());
        }

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, stderr) = run_git(&arg_refs, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git clone error: {}", stderr.trim()),
                metadata: None,
            });
        }

        // Determine the clone directory for metadata
        let clone_dir = directory.map(String::from).or_else(|| {
            // Infer from URL: strip trailing .git, take last path segment
            let url_path = url.trim_end_matches(".git");
            url_path.rsplit('/').next().map(String::from)
        });

        let content = if stdout.trim().is_empty() {
            let dir_msg = clone_dir
                .as_ref()
                .map(|d| format!(" into '{}'", d))
                .unwrap_or_default();
            format!("Cloned repository{}.", dir_msg)
        } else {
            stdout
        };

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "url": url,
                "directory": clone_dir,
                "bare": bare,
            })),
        })
    }
}
