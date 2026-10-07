//! GitHub issue tools - list, get, create, comment, and close issues.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::helpers::{detect_repo, make_client};
use crate::limits::{DEFAULT_PAGE_LIMIT, MAX_PAGE_LIMIT, NOTES_PER_PAGE};
use crate::vocab::normalize_issue_state;

use super::{Tool, ToolContext, ToolOutput};

// ---------------------------------------------------------------------------
// 1. GitHubListIssuesTool
// ---------------------------------------------------------------------------

/// Tool that lists GitHub issues in a repository.
pub struct GitHubListIssuesTool;

#[async_trait::async_trait]
impl Tool for GitHubListIssuesTool {
    fn name(&self) -> &'static str {
        "github_list_issues"
    }

    fn description(&self) -> &'static str {
        "List GitHub issues in the repository detected from the current working directory. \
         No required parameters. 'state' (enum 'open', 'closed', 'all', default 'open') filters by issue state; \
         both 'open' and GitLab's 'opened' spelling are accepted. \
         'labels' (string) is a comma-separated list of label names to filter by; \
         'limit' (integer, default 20, max 100) caps the number of issues returned. \
         Requires a configured GitHub authentication (see /github login). \
         Common gotcha: this only works when the working directory is inside a git repo with a GitHub remote; labels must already exist in the repository. \
         Cross-provider note: GitHub calls the item id 'number' where GitLab uses 'iid'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "state": {
                    "type": "string",
                    "enum": ["open", "closed", "all"],
                    "description": "Filter by issue state (GitLab's 'opened' is also accepted)"
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

        let state = normalize_issue_state(input["state"].as_str().unwrap_or("open"));
        let limit = input["limit"]
            .as_u64()
            .unwrap_or(DEFAULT_PAGE_LIMIT)
            .min(MAX_PAGE_LIMIT);

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
// 2. GitHubGetIssueTool
// ---------------------------------------------------------------------------

/// Tool that retrieves a single GitHub issue by number.
pub struct GitHubGetIssueTool;

#[async_trait::async_trait]
impl Tool for GitHubGetIssueTool {
    fn name(&self) -> &'static str {
        "github_get_issue"
    }

    fn description(&self) -> &'static str {
        "Get full details of a specific GitHub issue including title, body, state, labels, assignees, and the first 10 comments. \
         Required parameter: 'number' (integer) - the issue number. \
         Requires a configured GitHub authentication and a GitHub-backed git repo in the working directory. \
         Common gotcha: pull requests also appear in the issues endpoint but their details may be incomplete; use github_get_pr for PR-specific data."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
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
            .get(&format!(
                "/repos/{owner}/{repo}/issues/{number}/comments?per_page={NOTES_PER_PAGE}"
            ))
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
            let notes_cap = NOTES_PER_PAGE as usize;
            let shown = comments.iter().take(notes_cap);
            let total = comments.len();
            if total > 0 {
                md.push_str(&format!(
                    "\n## Comments ({total}{})\n",
                    if total > notes_cap {
                        ", showing first 10"
                    } else {
                        ""
                    }
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
// 3. GitHubCreateIssueTool
// ---------------------------------------------------------------------------

/// Tool that creates a new GitHub issue in a repository.
pub struct GitHubCreateIssueTool;

#[async_trait::async_trait]
impl Tool for GitHubCreateIssueTool {
    fn name(&self) -> &'static str {
        "github_create_issue"
    }

    fn description(&self) -> &'static str {
        "Create a new GitHub issue in the repository detected from the working directory. \
         Required parameter: 'title' (string) - the issue title. \
         Optional: 'body' (string) is the issue description in markdown; 'labels' (string) is a comma-separated list of label names to apply; \
         'assignees' (string) is a comma-separated list of GitHub usernames to assign. \
         Requires a configured GitHub authentication and a GitHub-backed git repo. \
         Common gotcha: labels and assignees must already exist in the repository; unknown labels cause the API call to fail."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
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
// 4. GitHubCommentIssueTool
// ---------------------------------------------------------------------------

/// Tool that posts a comment on an existing GitHub issue.
pub struct GitHubCommentIssueTool;

#[async_trait::async_trait]
impl Tool for GitHubCommentIssueTool {
    fn name(&self) -> &'static str {
        "github_comment_issue"
    }

    fn description(&self) -> &'static str {
        "Add a comment to an existing GitHub issue. \
         Required parameters: 'number' (integer) - the issue number, and 'body' (string) - the comment text (markdown supported). \
         Requires a configured GitHub authentication and a GitHub-backed git repo. \
         Common gotcha: 'number' is the repository-scoped issue number shown in the issue URL, not the global GitHub node ID."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
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
// 5. GitHubCloseIssueTool
// ---------------------------------------------------------------------------

/// Tool that closes a GitHub issue.
pub struct GitHubCloseIssueTool;

#[async_trait::async_trait]
impl Tool for GitHubCloseIssueTool {
    fn name(&self) -> &'static str {
        "github_close_issue"
    }

    fn description(&self) -> &'static str {
        "Close an open GitHub issue, optionally posting a closing comment first. \
         Required parameter: 'number' (integer) - the issue number. \
         Optional: 'comment' (string) - a closing note posted before the issue is closed. \
         Requires a configured GitHub authentication and a GitHub-backed git repo. \
         Common gotcha: closing an already-closed issue is a no-op but still returns success; only repository collaborators can close issues."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
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

/// Percent-encode a query-parameter value (FUNC-063).
///
/// Delegates to the shared byte-wise encoder so non-ASCII labels encode as
/// UTF-8 (e.g. `e` -> `%C3%A9`) rather than as a single wrong `%XX`.
fn urlencoded(s: &str) -> String {
    crate::percent::encode_component(s)
}
