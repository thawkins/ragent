//! GitLab merge request tools: list, get, create, merge, and approve MRs.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::gitlab::client::GitLabClient;

// ── helpers ──────────────────────────────────────────────────────────────────

/// Create an authenticated client and detect the project path.
fn detect(ctx: &ToolContext) -> Result<(GitLabClient, String)> {
    let storage = ctx
        .storage
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("Storage not available for GitLab client"))?;
    let client = GitLabClient::new(storage)
        .map_err(|_| anyhow::anyhow!("GitLab not configured. Run /gitlab setup."))?;
    let project = GitLabClient::detect_project(&ctx.working_dir)
        .ok_or_else(|| anyhow::anyhow!("Could not detect GitLab project from git remote."))?;
    Ok((client, project))
}

// ── GitlabListMrsTool ─────────────────────────────────────────────────────────

/// Tool that lists merge requests in a GitLab project.
pub struct GitlabListMrsTool;

#[async_trait::async_trait]
impl Tool for GitlabListMrsTool {
    fn name(&self) -> &'static str {
        "gitlab_list_mrs"
    }

    fn description(&self) -> &'static str {
        "List GitLab merge requests for the current project. \
         Optional: state (opened/closed/merged/all), target_branch, limit (default 20)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "state": {
                    "type": "string",
                    "enum": ["opened", "closed", "merged", "all"],
                    "description": "Filter by MR state (default: opened)"
                },
                "target_branch": {
                    "type": "string",
                    "description": "Filter by target branch name"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max MRs to return (default 20, max 100)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, project) = detect(ctx)?;

        let state = input["state"].as_str().unwrap_or("opened");
        let limit = input["limit"].as_u64().unwrap_or(20).min(100);

        let mut path = format!("/projects/{project}/merge_requests?state={state}&per_page={limit}");
        if let Some(target) = input["target_branch"].as_str() {
            path.push_str(&format!("&target_branch={target}"));
        }

        let mrs = client.get(&path).await?;
        let arr = mrs
            .as_array()
            .context("Unexpected response format from GitLab")?;

        if arr.is_empty() {
            return Ok(ToolOutput {
                content: format!("No merge requests found (state={state})."),
                metadata: None,
            });
        }

        let mut lines = Vec::with_capacity(arr.len());
        for mr in arr {
            let iid = mr["iid"].as_u64().unwrap_or(0);
            let title = mr["title"].as_str().unwrap_or("(no title)");
            let mr_state = mr["state"].as_str().unwrap_or("?");
            let author = mr["author"]["username"].as_str().unwrap_or("?");
            let source = mr["source_branch"].as_str().unwrap_or("?");
            let target = mr["target_branch"].as_str().unwrap_or("?");
            lines.push(format!(
                "!{iid} [{mr_state}] {title} (by {author}, from {source} → {target})"
            ));
        }

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(json!({"count": arr.len()})),
        })
    }
}

// ── GitlabGetMrTool ───────────────────────────────────────────────────────────

/// Tool that retrieves a single GitLab merge request by IID.
pub struct GitlabGetMrTool;

#[async_trait::async_trait]
impl Tool for GitlabGetMrTool {
    fn name(&self) -> &'static str {
        "gitlab_get_mr"
    }

    fn description(&self) -> &'static str {
        "Get details of a specific GitLab merge request including description, \
         approvals, and review notes."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "iid": {
                    "type": "integer",
                    "description": "Merge request IID"
                }
            },
            "required": ["iid"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, project) = detect(ctx)?;
        let iid = input["iid"]
            .as_u64()
            .context("Missing required 'iid' parameter")?;

        let mr_path = format!("/projects/{project}/merge_requests/{iid}");
        let notes_path = format!("/projects/{project}/merge_requests/{iid}/notes?per_page=10");

        let (mr, notes_val) = tokio::try_join!(client.get(&mr_path), client.get(&notes_path))?;

        let title = mr["title"].as_str().unwrap_or("(no title)");
        let state = mr["state"].as_str().unwrap_or("?");
        let author = mr["author"]["username"].as_str().unwrap_or("?");
        let source = mr["source_branch"].as_str().unwrap_or("?");
        let target = mr["target_branch"].as_str().unwrap_or("?");
        let body = mr["description"].as_str().unwrap_or("*(no description)*");
        let url = mr["web_url"].as_str().unwrap_or("");

        let mut md = format!(
            "## MR !{iid}: {title}\n\n\
             **State**: {state}  \n\
             **Author**: {author}  \n\
             **Branch**: {source} → {target}  \n\
             **URL**: {url}\n\n\
             ### Description\n\n{body}\n"
        );

        if let Some(notes) = notes_val.as_array() {
            let user_notes: Vec<&Value> = notes
                .iter()
                .filter(|n| !n["system"].as_bool().unwrap_or(true))
                .collect();
            if !user_notes.is_empty() {
                md.push_str(&format!("\n### Notes ({})\n\n", user_notes.len()));
                for note in user_notes.iter().take(10) {
                    let reviewer = note["author"]["username"].as_str().unwrap_or("?");
                    let nbody = note["body"].as_str().unwrap_or("");
                    if nbody.is_empty() {
                        md.push_str(&format!("- **{reviewer}**: (empty)\n"));
                    } else {
                        md.push_str(&format!("- **{reviewer}**: {nbody}\n"));
                    }
                }
            }
        }

        Ok(ToolOutput {
            content: md,
            metadata: Some(json!({"iid": iid, "state": state})),
        })
    }
}

// ── GitlabCreateMrTool ────────────────────────────────────────────────────────

/// Tool that creates a new GitLab merge request.
pub struct GitlabCreateMrTool;

#[async_trait::async_trait]
impl Tool for GitlabCreateMrTool {
    fn name(&self) -> &'static str {
        "gitlab_create_mr"
    }

    fn description(&self) -> &'static str {
        "Create a new GitLab merge request from the current branch."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "MR title"
                },
                "description": {
                    "type": "string",
                    "description": "MR description (markdown supported)"
                },
                "target_branch": {
                    "type": "string",
                    "description": "Target branch (default: main)"
                },
                "source_branch": {
                    "type": "string",
                    "description": "Source branch (default: current git branch)"
                },
                "draft": {
                    "type": "boolean",
                    "description": "Create as draft MR"
                }
            },
            "required": ["title"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, project) = detect(ctx)?;

        let title = input["title"]
            .as_str()
            .context("Missing required 'title' parameter")?;

        let source_branch = if let Some(s) = input["source_branch"].as_str() {
            s.to_string()
        } else {
            let out = std::process::Command::new("git")
                .args(["rev-parse", "--abbrev-ref", "HEAD"])
                .current_dir(&ctx.working_dir)
                .output()
                .context("Failed to run git rev-parse")?;
            String::from_utf8(out.stdout)
                .context("Non-UTF8 branch name")?
                .trim()
                .to_string()
        };

        let target_branch = input["target_branch"].as_str().unwrap_or("main");
        let draft = input["draft"].as_bool().unwrap_or(false);

        let display_title = if draft {
            format!("Draft: {title}")
        } else {
            title.to_string()
        };

        let mut body_json = json!({
            "title": display_title,
            "source_branch": source_branch,
            "target_branch": target_branch,
        });
        if let Some(desc) = input["description"].as_str() {
            body_json["description"] = Value::String(desc.to_string());
        }

        let resp = client
            .post(&format!("/projects/{project}/merge_requests"), &body_json)
            .await?;

        let mr_iid = resp["iid"].as_u64().unwrap_or(0);
        let url = resp["web_url"].as_str().unwrap_or("");

        Ok(ToolOutput {
            content: format!("Created MR !{mr_iid}: {url}"),
            metadata: Some(json!({"iid": mr_iid, "url": url})),
        })
    }
}

// ── GitlabMergeMrTool ─────────────────────────────────────────────────────────

/// Tool that merges a GitLab merge request.
pub struct GitlabMergeMrTool;

#[async_trait::async_trait]
impl Tool for GitlabMergeMrTool {
    fn name(&self) -> &'static str {
        "gitlab_merge_mr"
    }

    fn description(&self) -> &'static str {
        "Merge a GitLab merge request."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "iid": {
                    "type": "integer",
                    "description": "Merge request IID"
                },
                "squash": {
                    "type": "boolean",
                    "description": "Squash commits on merge (default: false)"
                },
                "message": {
                    "type": "string",
                    "description": "Optional merge commit message"
                },
                "delete_source_branch": {
                    "type": "boolean",
                    "description": "Delete source branch after merge (default: false)"
                }
            },
            "required": ["iid"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, project) = detect(ctx)?;

        let iid = input["iid"]
            .as_u64()
            .context("Missing required 'iid' parameter")?;
        let squash = input["squash"].as_bool().unwrap_or(false);
        let delete_branch = input["delete_source_branch"].as_bool().unwrap_or(false);

        let mut body = json!({
            "squash": squash,
            "should_remove_source_branch": delete_branch,
        });
        if let Some(msg) = input["message"].as_str() {
            body["merge_commit_message"] = Value::String(msg.to_string());
        }

        client
            .put(
                &format!("/projects/{project}/merge_requests/{iid}/merge"),
                &body,
            )
            .await?;

        Ok(ToolOutput {
            content: format!(
                "MR !{iid} merged successfully{}.",
                if squash { " (squashed)" } else { "" }
            ),
            metadata: Some(json!({"iid": iid, "squash": squash})),
        })
    }
}

// ── GitlabApproveMrTool ──────────────────────────────────────────────────────

/// Tool that approves a GitLab merge request.
pub struct GitlabApproveMrTool;

#[async_trait::async_trait]
impl Tool for GitlabApproveMrTool {
    fn name(&self) -> &'static str {
        "gitlab_approve_mr"
    }

    fn description(&self) -> &'static str {
        "Approve a GitLab merge request."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "iid": {
                    "type": "integer",
                    "description": "Merge request IID"
                }
            },
            "required": ["iid"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, project) = detect(ctx)?;

        let iid = input["iid"]
            .as_u64()
            .context("Missing required 'iid' parameter")?;

        client
            .post(
                &format!("/projects/{project}/merge_requests/{iid}/approve"),
                &json!({}),
            )
            .await?;

        Ok(ToolOutput {
            content: format!("MR !{iid} approved."),
            metadata: Some(json!({"iid": iid})),
        })
    }
}
