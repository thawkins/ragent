//! GitLab issue tools — list, get, create, comment, and close issues.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::gitlab::GitLabClient;

use super::{Tool, ToolContext, ToolOutput};

/// Returns the authenticated `GitLabClient` or a human-readable error.
fn make_client(ctx: &ToolContext) -> Result<GitLabClient> {
    let storage = ctx
        .storage
        .as_ref()
        .context("Storage not available for GitLab client")?;
    GitLabClient::new(storage)
        .context("GitLab not configured. Run /gitlab setup to configure.")
}

/// Resolve the URL-encoded project path from the working directory.
fn detect_project(ctx: &ToolContext) -> Result<String> {
    GitLabClient::detect_project(&ctx.working_dir).ok_or_else(|| {
        anyhow::anyhow!(
            "Could not detect GitLab project from git remote. \
             Ensure you're in a git repo with a GitLab remote."
        )
    })
}

// ---------------------------------------------------------------------------
// 1. GitlabListIssuesTool
// ---------------------------------------------------------------------------

/// Tool that lists GitLab issues in a project.
pub struct GitlabListIssuesTool;

#[async_trait::async_trait]
impl Tool for GitlabListIssuesTool {
    fn name(&self) -> &'static str {
        "gitlab_list_issues"
    }

    fn description(&self) -> &'static str {
        "List GitLab issues for the current project. \
         Optional: state (opened/closed/all), labels (comma-separated), limit (default 20)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "state": {
                    "type": "string",
                    "enum": ["opened", "closed", "all"],
                    "description": "Filter by issue state (default: opened)"
                },
                "labels": {
                    "type": "string",
                    "description": "Comma-separated label names to filter by"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max issues to return (default 20, max 100)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let client = make_client(ctx)?;
        let project = detect_project(ctx)?;

        let state = input["state"].as_str().unwrap_or("opened");
        let limit = input["limit"].as_u64().unwrap_or(20).min(100);

        let mut path = format!(
            "/projects/{project}/issues?state={state}&per_page={limit}"
        );
        if let Some(labels) = input["labels"].as_str() {
            if !labels.is_empty() {
                path.push_str(&format!("&labels={labels}"));
            }
        }

        let issues = client.get(&path).await?;
        let arr = issues
            .as_array()
            .context("Expected array from GitLab issues endpoint")?;

        if arr.is_empty() {
            return Ok(ToolOutput {
                content: format!("No {state} issues found."),
                metadata: None,
            });
        }

        let mut lines = vec![format!("Issues (state={state}):\n")];
        for issue in arr {
            let iid = issue["iid"].as_u64().unwrap_or(0);
            let title = issue["title"].as_str().unwrap_or("(no title)");
            let issue_state = issue["state"].as_str().unwrap_or("?");
            let author = issue["author"]["username"].as_str().unwrap_or("?");
            let notes = issue["user_notes_count"].as_u64().unwrap_or(0);
            lines.push(format!(
                "  #{iid} [{issue_state}] {title} (by {author}, {notes} note{})",
                if notes == 1 { "" } else { "s" }
            ));
        }

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(json!({ "count": arr.len(), "project": project })),
        })
    }
}

// ---------------------------------------------------------------------------
// 2. GitlabGetIssueTool
// ---------------------------------------------------------------------------

/// Tool that retrieves a single GitLab issue by IID.
pub struct GitlabGetIssueTool;

#[async_trait::async_trait]
impl Tool for GitlabGetIssueTool {
    fn name(&self) -> &'static str {
        "gitlab_get_issue"
    }

    fn description(&self) -> &'static str {
        "Get details of a specific GitLab issue including description, notes, and labels."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "iid": {
                    "type": "integer",
                    "description": "Issue IID (project-scoped number)"
                }
            },
            "required": ["iid"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let client = make_client(ctx)?;
        let project = detect_project(ctx)?;

        let iid = input["iid"]
            .as_u64()
            .context("Missing required parameter 'iid'")?;

        let issue = client
            .get(&format!("/projects/{project}/issues/{iid}"))
            .await?;
        let notes_val = client
            .get(&format!(
                "/projects/{project}/issues/{iid}/notes?per_page=10"
            ))
            .await?;

        let title = issue["title"].as_str().unwrap_or("(no title)");
        let state = issue["state"].as_str().unwrap_or("?");
        let author = issue["author"]["username"].as_str().unwrap_or("?");
        let body = issue["description"].as_str().unwrap_or("(no description)");
        let web_url = issue["web_url"].as_str().unwrap_or("");

        let labels: Vec<&str> = issue["labels"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|l| l.as_str()).collect())
            .unwrap_or_default();

        let assignees: Vec<&str> = issue["assignees"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .filter_map(|a| a["username"].as_str())
                    .collect()
            })
            .unwrap_or_default();

        let mut md = format!(
            "# Issue #{iid}: {title}\n\n\
             **State**: {state}  \n\
             **Author**: {author}  \n\
             **URL**: {web_url}  \n"
        );

        if !labels.is_empty() {
            md.push_str(&format!("**Labels**: {}  \n", labels.join(", ")));
        }
        if !assignees.is_empty() {
            md.push_str(&format!("**Assignees**: {}  \n", assignees.join(", ")));
        }

        md.push_str(&format!("\n## Description\n\n{body}\n"));

        if let Some(notes) = notes_val.as_array() {
            let user_notes: Vec<&Value> = notes
                .iter()
                .filter(|n| !n["system"].as_bool().unwrap_or(true))
                .collect();
            let total = user_notes.len();
            if total > 0 {
                md.push_str(&format!("\n## Notes ({total})\n"));
                for note in user_notes.iter().take(10) {
                    let commenter = note["author"]["username"].as_str().unwrap_or("?");
                    let created = note["created_at"].as_str().unwrap_or("");
                    let nbody = note["body"].as_str().unwrap_or("");
                    md.push_str(&format!("\n**{commenter}** ({created}):\n{nbody}\n"));
                }
            }
        }

        Ok(ToolOutput {
            content: md,
            metadata: Some(json!({ "iid": iid, "project": project })),
        })
    }
}

// ---------------------------------------------------------------------------
// 3. GitlabCreateIssueTool
// ---------------------------------------------------------------------------

/// Tool that creates a new GitLab issue in a project.
pub struct GitlabCreateIssueTool;

#[async_trait::async_trait]
impl Tool for GitlabCreateIssueTool {
    fn name(&self) -> &'static str {
        "gitlab_create_issue"
    }

    fn description(&self) -> &'static str {
        "Create a new GitLab issue in the current project."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Issue title"
                },
                "description": {
                    "type": "string",
                    "description": "Issue description (markdown supported)"
                },
                "labels": {
                    "type": "string",
                    "description": "Comma-separated label names"
                },
                "assignee_ids": {
                    "type": "string",
                    "description": "Comma-separated user IDs to assign"
                }
            },
            "required": ["title"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let client = make_client(ctx)?;
        let project = detect_project(ctx)?;

        let title = input["title"]
            .as_str()
            .context("Missing required parameter 'title'")?;

        let mut payload = json!({ "title": title });

        if let Some(desc) = input["description"].as_str() {
            payload["description"] = json!(desc);
        }
        if let Some(labels) = input["labels"].as_str() {
            payload["labels"] = json!(labels);
        }
        if let Some(assignee_ids) = input["assignee_ids"].as_str() {
            let ids: Vec<u64> = assignee_ids
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .collect();
            if !ids.is_empty() {
                payload["assignee_ids"] = json!(ids);
            }
        }

        let result = client
            .post(&format!("/projects/{project}/issues"), &payload)
            .await?;

        let iid = result["iid"].as_u64().unwrap_or(0);
        let web_url = result["web_url"].as_str().unwrap_or("");

        Ok(ToolOutput {
            content: format!("Created issue #{iid}\nURL: {web_url}"),
            metadata: Some(json!({ "iid": iid, "url": web_url, "project": project })),
        })
    }
}

// ---------------------------------------------------------------------------
// 4. GitlabCommentIssueTool
// ---------------------------------------------------------------------------

/// Tool that posts a note (comment) on an existing GitLab issue.
pub struct GitlabCommentIssueTool;

#[async_trait::async_trait]
impl Tool for GitlabCommentIssueTool {
    fn name(&self) -> &'static str {
        "gitlab_comment_issue"
    }

    fn description(&self) -> &'static str {
        "Add a note (comment) to a GitLab issue."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "iid": {
                    "type": "integer",
                    "description": "Issue IID"
                },
                "body": {
                    "type": "string",
                    "description": "Note body (markdown supported)"
                }
            },
            "required": ["iid", "body"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let client = make_client(ctx)?;
        let project = detect_project(ctx)?;

        let iid = input["iid"]
            .as_u64()
            .context("Missing required parameter 'iid'")?;
        let body = input["body"]
            .as_str()
            .context("Missing required parameter 'body'")?;

        let result = client
            .post(
                &format!("/projects/{project}/issues/{iid}/notes"),
                &json!({ "body": body }),
            )
            .await?;

        let note_id = result["id"].as_u64().unwrap_or(0);

        Ok(ToolOutput {
            content: format!("Note added to issue #{iid} (note id: {note_id})"),
            metadata: Some(json!({ "note_id": note_id, "iid": iid, "project": project })),
        })
    }
}

// ---------------------------------------------------------------------------
// 5. GitlabCloseIssueTool
// ---------------------------------------------------------------------------

/// Tool that closes a GitLab issue.
pub struct GitlabCloseIssueTool;

#[async_trait::async_trait]
impl Tool for GitlabCloseIssueTool {
    fn name(&self) -> &'static str {
        "gitlab_close_issue"
    }

    fn description(&self) -> &'static str {
        "Close a GitLab issue (optionally with a note)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "iid": {
                    "type": "integer",
                    "description": "Issue IID"
                },
                "comment": {
                    "type": "string",
                    "description": "Optional closing note"
                }
            },
            "required": ["iid"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "gitlab:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let client = make_client(ctx)?;
        let project = detect_project(ctx)?;

        let iid = input["iid"]
            .as_u64()
            .context("Missing required parameter 'iid'")?;

        if let Some(comment) = input["comment"].as_str() {
            if !comment.is_empty() {
                client
                    .post(
                        &format!("/projects/{project}/issues/{iid}/notes"),
                        &json!({ "body": comment }),
                    )
                    .await?;
            }
        }

        client
            .put(
                &format!("/projects/{project}/issues/{iid}"),
                &json!({ "state_event": "close" }),
            )
            .await?;

        Ok(ToolOutput {
            content: format!("Issue #{iid} has been closed."),
            metadata: Some(json!({ "iid": iid, "project": project })),
        })
    }
}
