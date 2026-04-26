//! `git_reset` — Unstage files or reset to a commit.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that unstages files or resets the repository.
pub struct GitResetTool;

#[async_trait::async_trait]
impl Tool for GitResetTool {
    fn name(&self) -> &'static str {
        "git_reset"
    }

    fn description(&self) -> &'static str {
        "Unstage files or reset the repository to a specific commit. \
         CAUTION: mode 'hard' discards all local changes. \
         Modes: 'soft' (keep changes staged), 'mixed' (keep changes unstaged, default), \
         'hard' (discard changes), 'keep' (discard changes but abort if overridden). \
         Provide 'paths' to unstage specific files without resetting commits."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "mode": {
                    "type": "string",
                    "enum": ["soft", "mixed", "hard", "keep"],
                    "description": "Reset mode (default: mixed). Ignored when 'paths' is provided."
                },
                "target": {
                    "type": "string",
                    "description": "Commit ref to reset to (default: HEAD). Ignored when 'paths' is provided."
                },
                "paths": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Specific files to unstage. When provided, mode and target are ignored."
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
            // Unstage specific files: git reset HEAD -- <paths>
            if ps.is_empty() {
                return Err(anyhow::anyhow!("Paths array is empty."));
            }
            args = vec!["reset".to_string(), "HEAD".to_string(), "--".to_string()];
            for p in ps {
                if let Some(s) = p.as_str() {
                    args.push(s.to_string());
                }
            }
        } else {
            let mode = input["mode"].as_str().unwrap_or("mixed");
            let target = input["target"].as_str().unwrap_or("HEAD");

            args = vec!["reset".to_string()];
            match mode {
                "soft" => args.push("--soft".to_string()),
                "mixed" => {} // default
                "hard" => args.push("--hard".to_string()),
                "keep" => args.push("--keep".to_string()),
                other => {
                    return Err(anyhow::anyhow!(
                        "Unknown reset mode: {}. Use 'soft', 'mixed', 'hard', or 'keep'.",
                        other
                    ));
                }
            }
            args.push(target.to_string());
        }

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, stderr) = run_git(&arg_refs, &ctx.working_dir)?;

        if !stderr.is_empty() && stdout.trim().is_empty() {
            return Ok(ToolOutput {
                content: format!("git reset error: {}", stderr.trim()),
                metadata: None,
            });
        }

        let content = if stdout.trim().is_empty() {
            if paths.is_some() {
                "Unstaged specified files.".to_string()
            } else {
                let mode = input["mode"].as_str().unwrap_or("mixed");
                let target = input["target"].as_str().unwrap_or("HEAD");
                format!("Reset to {} ({} mode).", target, mode)
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
