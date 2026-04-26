//! `git_merge` — Merge branches.

use anyhow::Result;
use serde_json::{Value, json};

use crate::git::run_git;
use crate::{Tool, ToolContext, ToolOutput};

/// Tool that merges branches.
pub struct GitMergeTool;

#[async_trait::async_trait]
impl Tool for GitMergeTool {
    fn name(&self) -> &'static str {
        "git_merge"
    }

    fn description(&self) -> &'static str {
        "Merge a branch into the current branch. \
         If there are conflicts, the tool reports the conflicted files and \
         suggests running git_status next. \
         Options: 'no_ff' to always create a merge commit, 'ff_only' to abort \
         if not fast-forward, 'squash' to squash all changes into a single commit."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "branch": {
                    "type": "string",
                    "description": "Branch to merge into the current branch (required)"
                },
                "message": {
                    "type": "string",
                    "description": "Custom merge commit message"
                },
                "no_ff": {
                    "type": "boolean",
                    "description": "Create a merge commit even if fast-forward is possible (default: false)"
                },
                "ff_only": {
                    "type": "boolean",
                    "description": "Abort if not a fast-forward merge (default: false)"
                },
                "squash": {
                    "type": "boolean",
                    "description": "Squash all changes into a single commit (default: false)"
                }
            },
            "required": ["branch"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "git:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let branch = input["branch"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("'branch' is required for merge."))?;
        let message = input["message"].as_str();
        let no_ff = input["no_ff"].as_bool().unwrap_or(false);
        let ff_only = input["ff_only"].as_bool().unwrap_or(false);
        let squash = input["squash"].as_bool().unwrap_or(false);

        let mut args: Vec<String> = vec!["merge".to_string()];

        if squash {
            args.push("--squash".to_string());
        }
        if no_ff {
            args.push("--no-ff".to_string());
        }
        if ff_only {
            args.push("--ff-only".to_string());
        }
        if let Some(msg) = message {
            args.push("-m".to_string());
            args.push(msg.to_string());
        }

        args.push(branch.to_string());

        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let (stdout, stderr) = run_git(&arg_refs, &ctx.working_dir)?;

        let combined = format!("{}\n{}", stdout, stderr).trim().to_string();

        // Detect conflicts from stderr
        let mut conflicted: Vec<String> = Vec::new();
        let has_conflicts =
            stderr.contains("CONFLICT") || stderr.contains("Automatic merge failed");

        if has_conflicts {
            for line in stderr.lines() {
                if line.contains("CONFLICT") {
                    if let Some(start) = line.rfind("in ") {
                        let file = line[start + 3..].trim();
                        conflicted.push(file.to_string());
                    }
                }
            }
        }

        let content = if stdout.trim().is_empty() && stderr.trim().is_empty() {
            format!("Merged '{}' into current branch.", branch)
        } else {
            combined
        };

        let content = if has_conflicts {
            let conflict_list = if conflicted.is_empty() {
                String::new()
            } else {
                format!(
                    "\n\nConflicted files:\n{}",
                    conflicted
                        .iter()
                        .map(|f| format!("  - {}", f))
                        .collect::<Vec<_>>()
                        .join("\n")
                )
            };
            format!(
                "{}\n\nMerge conflicts detected. Resolve the conflicts, then run git_status to see remaining issues, and git_commit to complete the merge.{}",
                content.trim(),
                conflict_list
            )
        } else {
            content
        };

        Ok(ToolOutput {
            content,
            metadata: Some(json!({
                "branch": branch,
                "conflicts": has_conflicts,
                "conflicted_files": conflicted,
            })),
        })
    }
}
