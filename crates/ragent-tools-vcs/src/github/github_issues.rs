//! GitHub issue tools — list, get, create, comment, and close issues.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::github::GitHubClient;

use super::{Tool, ToolContext, ToolOutput};

/// Returns the authenticated `GitHubClient` or a human-readable error.
fn make_client() -> Result<GitHubClient> {
    GitHubClient::new().context("GitHub not authenticated. Run /github login to authenticate.")
}

/// Resolve owner/repo from the working directory or return an error.
fn detect_repo(ctx: &ToolContext) -> Result<(String, String)> {
    GitHubClient::detect_repo(&ctx.working_dir).ok_or_else(|| {
        anyhow::anyhow!(
            "Could not detect GitHub repository from git remote. \
             Ensure you're in a git repo with a GitHub remote."
        )
    })
}

// ---------------------------------------------------------------------------
// 1. GithubListIssuesTool
// ---------------------------------------------------------------------------

/// Tool that lists GitHub issues in a repository.
pub struct GithubListIssuesTool;

#[async_trait::async_trait]
impl Tool for GithubListIssuesTool {
    fn name(&self) -> &'static str {
        "github_list_issues"
    }

    fn description(&self) -> &'static str {
        "List GitHub issues for the current repository. \
         Optional: state (open/closed/all), labels (comma-separated), limit (default 20)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "state": {
                    "type": "string",
                    "enum": ["open", "closed", "all"],
                    "description": "Filter by issue state"
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
        "github:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let client = make_client()?;
        let (owner, repo) = detect_repo(ctx)?;

        let state = input["state"].as_str().unwrap_or("open");
        let limit = input["limit"].as_u64().unwrap_or(20).min(100);

        let mut path = format!("/repos/{owner}/{repo}/issues?state={state}&per_page={limit}");
        if let Some(labels) = input["labels"].as_str()
            && !labels.is_empty()
        {
            path.push_str(&format!("&labels={}", urlencoded(labels)));
        }

        let issues = client.get(&path).await?;
        let arr = issues
            .as_array()
            .context("Expected array from GitHub issues endpoint")?;

        if arr.is_empty() {
            return Ok(ToolOutput {
                content: format!("No {state} issues found in {owner}/{repo}."),
                metadata: None,
            });
        }

        let mut lines = vec![format!("Issues for {owner}/{repo} (state={state}):\n")];
        for issue in arr {
            let number = issue["number"].as_u64().unwrap_or(0);
            let title = issue["title"].as_str().unwrap_or("(no title)");
            let issue_state = issue["state"].as_str().unwrap_or("?");
            let author = issue["user"]["login"].as_str().unwrap_or("?");
            let comments = issue["comments"].as_u64().unwrap_or(0);
            lines.push(format!(
                "  #{number} [{issue_state}] {title} (by {author}, {comments} comment{})",
                if comments == 1 { "" } else { "s" }
            ));
        }

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(json!({ "count": arr.len(), "owner": owner, "repo": repo })),
        })
    }
}

// ---------------------------------------------------------------------------
// 2. GithubGetIssueTool
// ---------------------------------------------------------------------------

/// Tool that retrieves a single GitHub issue by number.
pub struct GithubGetIssueTool;

#[async_trait::async_trait]
impl Tool for GithubGetIssueTool {
    fn name(&self) -> &'static str {
        "github_get_issue"
    }

    fn description(&self) -> &'static str {
        "Get details of a specific GitHub issue including body, comments, and labels."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "number": {
                    "type": "integer",
                    "description": "Issue number"
                }
            },
            "required": ["number"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "github:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let client = make_client()?;
        let (owner, repo) = detect_repo(ctx)?;

        let number = input["number"]
            .as_u64()
            .context("Missing required parameter 'number'")?;

        let issue = client
            .get(&format!("/repos/{owner}/{repo}/issues/{number}"))
            .await?;
        let comments_val = client
            .get(&format!("/repos/{owner}/{repo}/issues/{number}/comments"))
            .await?;

        let title = issue["title"].as_str().unwrap_or("(no title)");
        let state = issue["state"].as_str().unwrap_or("?");
        let author = issue["user"]["login"].as_str().unwrap_or("?");
        let body = issue["body"].as_str().unwrap_or("(no body)");
        let html_url = issue["html_url"].as_str().unwrap_or("");

        let labels: Vec<&str> = issue["labels"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|l| l["name"].as_str()).collect())
            .unwrap_or_default();

        let assignees: Vec<&str> = issue["assignees"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|a| a["login"].as_str()).collect())
            .unwrap_or_default();

        let mut md = format!(
            "# Issue #{number}: {title}\n\n\
             **State**: {state}  \n\
             **Author**: {author}  \n\
             **URL**: {html_url}  \n"
        );

        if !labels.is_empty() {
            md.push_str(&format!("**Labels**: {}  \n", labels.join(", ")));
        }
        if !assignees.is_empty() {
            md.push_str(&format!("**Assignees**: {}  \n", assignees.join(", ")));
        }

        md.push_str(&format!("\n## Body\n\n{body}\n"));

        if let Some(comments) = comments_val.as_array() {
            let shown = comments.iter().take(10);
            let total = comments.len();
            if total > 0 {
                md.push_str(&format!(
                    "\n## Comments ({total}{})\n",
                    if total > 10 { ", showing first 10" } else { "" }
                ));
                for comment in shown {
                    let commenter = comment["user"]["login"].as_str().unwrap_or("?");
                    let created = comment["created_at"].as_str().unwrap_or("");
                    let cbody = comment["body"].as_str().unwrap_or("");
                    md.push_str(&format!("\n**{commenter}** ({created}):\n{cbody}\n"));
                }
            }
        }

        Ok(ToolOutput {
            content: md,
            metadata: Some(json!({ "number": number, "owner": owner, "repo": repo })),
        })
    }
}

// ---------------------------------------------------------------------------
// 3. GithubCreateIssueTool
// ---------------------------------------------------------------------------

/// Tool that creates a new GitHub issue in a repository.
pub struct GithubCreateIssueTool;

#[async_trait::async_trait]
impl Tool for GithubCreateIssueTool {
    fn name(&self) -> &'static str {
        "github_create_issue"
    }

    fn description(&self) -> &'static str {
        "Create a new GitHub issue in the current repository."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "title": {
                    "type": "string",
                    "description": "Issue title"
                },
                "body": {
                    "type": "string",
                    "description": "Issue body (markdown supported)"
                },
                "labels": {
                    "type": "string",
                    "description": "Comma-separated label names"
                },
                "assignees": {
                    "type": "string",
                    "description": "Comma-separated GitHub usernames to assign"
                }
            },
            "required": ["title"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "github:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let client = make_client()?;
        let (owner, repo) = detect_repo(ctx)?;

        let title = input["title"]
            .as_str()
            .context("Missing required parameter 'title'")?;

        let mut payload = json!({ "title": title });

        if let Some(body) = input["body"].as_str() {
            payload["body"] = json!(body);
        }
        if let Some(labels) = input["labels"].as_str() {
            let label_vec: Vec<&str> = labels.split(',').map(str::trim).collect();
            payload["labels"] = json!(label_vec);
        }
        if let Some(assignees) = input["assignees"].as_str() {
            let assignee_vec: Vec<&str> = assignees.split(',').map(str::trim).collect();
            payload["assignees"] = json!(assignee_vec);
        }

        let result = client
            .post(&format!("/repos/{owner}/{repo}/issues"), &payload)
            .await?;

        let number = result["number"].as_u64().unwrap_or(0);
        let html_url = result["html_url"].as_str().unwrap_or("");

        Ok(ToolOutput {
            content: format!("Created issue #{number} in {owner}/{repo}\nURL: {html_url}"),
            metadata: Some(
                json!({ "number": number, "url": html_url, "owner": owner, "repo": repo }),
            ),
        })
    }
}

// ---------------------------------------------------------------------------
// 4. GithubCommentIssueTool
// ---------------------------------------------------------------------------

/// Tool that posts a comment on an existing GitHub issue.
pub struct GithubCommentIssueTool;

#[async_trait::async_trait]
impl Tool for GithubCommentIssueTool {
    fn name(&self) -> &'static str {
        "github_comment_issue"
    }

    fn description(&self) -> &'static str {
        "Add a comment to a GitHub issue."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "number": {
                    "type": "integer",
                    "description": "Issue number"
                },
                "body": {
                    "type": "string",
                    "description": "Comment body (markdown supported)"
                }
            },
            "required": ["number", "body"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "github:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let client = make_client()?;
        let (owner, repo) = detect_repo(ctx)?;

        let number = input["number"]
            .as_u64()
            .context("Missing required parameter 'number'")?;
        let body = input["body"]
            .as_str()
            .context("Missing required parameter 'body'")?;

        let result = client
            .post(
                &format!("/repos/{owner}/{repo}/issues/{number}/comments"),
                &json!({ "body": body }),
            )
            .await?;

        let comment_id = result["id"].as_u64().unwrap_or(0);
        let html_url = result["html_url"].as_str().unwrap_or("");

        Ok(ToolOutput {
            content: format!(
                "Comment added to issue #{number} in {owner}/{repo} (comment id: {comment_id})\nURL: {html_url}"
            ),
            metadata: Some(json!({ "comment_id": comment_id, "issue": number, "url": html_url })),
        })
    }
}

// ---------------------------------------------------------------------------
// 5. GithubCloseIssueTool
// ---------------------------------------------------------------------------

/// Tool that closes a GitHub issue.
pub struct GithubCloseIssueTool;

#[async_trait::async_trait]
impl Tool for GithubCloseIssueTool {
    fn name(&self) -> &'static str {
        "github_close_issue"
    }

    fn description(&self) -> &'static str {
        "Close a GitHub issue (optionally with a comment)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "number": {
                    "type": "integer",
                    "description": "Issue number"
                },
                "comment": {
                    "type": "string",
                    "description": "Optional closing comment"
                }
            },
            "required": ["number"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "github:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let client = make_client()?;
        let (owner, repo) = detect_repo(ctx)?;

        let number = input["number"]
            .as_u64()
            .context("Missing required parameter 'number'")?;

        // Optionally post a closing comment first.
        if let Some(comment) = input["comment"].as_str()
            && !comment.is_empty()
        {
            client
                .post(
                    &format!("/repos/{owner}/{repo}/issues/{number}/comments"),
                    &json!({ "body": comment }),
                )
                .await?;
        }

        client
            .patch(
                &format!("/repos/{owner}/{repo}/issues/{number}"),
                &json!({ "state": "closed" }),
            )
            .await?;

        Ok(ToolOutput {
            content: format!("Issue #{number} in {owner}/{repo} has been closed."),
            metadata: Some(json!({ "number": number, "owner": owner, "repo": repo })),
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn urlencoded(s: &str) -> String {
    s.chars()
        .flat_map(|c| {
            if c.is_alphanumeric() || matches!(c, '-' | '_' | '.' | '~' | ',') {
                vec![c]
            } else {
                format!("%{:02X}", c as u32).chars().collect()
            }
        })
        .collect()
}
