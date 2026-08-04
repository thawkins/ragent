//! GitHub pull request tools: list, get, create, merge, and review PRs.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};
use crate::github::client::GitHubClient;

// ── helpers ──────────────────────────────────────────────────────────────────

fn detect(ctx: &ToolContext) -> Result<(GitHubClient, String, String)> {
    let client = GitHubClient::new()
        .map_err(|_| anyhow::anyhow!("GitHub not authenticated. Run /github login."))?;
    let (owner, repo) = GitHubClient::detect_repo(&ctx.working_dir)
        .ok_or_else(|| anyhow::anyhow!("Could not detect GitHub repository from git remote."))?;
    Ok((client, owner, repo))
}

// ── GithubListPrsTool ─────────────────────────────────────────────────────────

/// Tool that lists pull requests in a GitHub repository.
pub struct GithubListPrsTool;

#[async_trait::async_trait]
impl Tool for GithubListPrsTool {
    fn name(&self) -> &'static str {
        "github_list_prs"
    }

    fn description(&self) -> &'static str {
        "List GitHub pull requests in the repository detected from the current working directory. \
         No required parameters. 'state' (enum 'open', 'closed', 'all', default 'open') filters by PR state; \
         'base' (string) filters by the target (base) branch name; 'limit' (integer, default 20, max 100) caps results. \
         Requires a configured GitHub authentication and a GitHub-backed git repo. \
         Common gotcha: 'base' must exactly match the branch name in the repository; the tool returns only the most recent PRs up to 'limit'."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "state": {
                    "type": "string",
                    "enum": ["open", "closed", "all"],
                    "description": "Filter by PR state"
                },
                "base": {
                    "type": "string",
                    "description": "Filter by base branch name"
                },
                "limit": {
                    "type": "integer",
                    "description": "Max PRs to return (default 20, max 100)"
                }
            }
        })
    }

    fn permission_category(&self) -> &'static str {
        "github:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, owner, repo) = detect(ctx)?;

        let state = input["state"].as_str().unwrap_or("open");
        let limit = input["limit"].as_u64().unwrap_or(20).min(100);

        let mut path = format!("/repos/{owner}/{repo}/pulls?state={state}&per_page={limit}");
        if let Some(base) = input["base"].as_str() {
            path.push_str(&format!("&base={base}"));
        }

        let prs = client.get(&path).await?;
        let arr = prs
            .as_array()
            .context("Unexpected response format from GitHub")?;

        if arr.is_empty() {
            return Ok(ToolOutput {
                content: format!("No pull requests found (state={state})."),
                metadata: None,
            });
        }

        let mut lines = Vec::with_capacity(arr.len());
        for pr in arr {
            let number = pr["number"].as_u64().unwrap_or(0);
            let title = pr["title"].as_str().unwrap_or("(no title)");
            let state = pr["state"].as_str().unwrap_or("?");
            let author = pr["user"]["login"].as_str().unwrap_or("?");
            let head = pr["head"]["ref"].as_str().unwrap_or("?");
            let base = pr["base"]["ref"].as_str().unwrap_or("?");
            lines.push(format!(
                "#{number} [{state}] {title} (by {author}, from {head} → {base})"
            ));
        }

        Ok(ToolOutput {
            content: lines.join("\n"),
            metadata: Some(json!({"count": arr.len()})),
        })
    }
}

// ── GithubGetPrTool ───────────────────────────────────────────────────────────

/// Tool that retrieves a single GitHub pull request by number.
pub struct GithubGetPrTool;

#[async_trait::async_trait]
impl Tool for GithubGetPrTool {
    fn name(&self) -> &'static str {
        "github_get_pr"
    }

    fn description(&self) -> &'static str {
        "Get details of a specific GitHub pull request including title, description, state, source and base branches, and review comments. \
         Required parameter: 'number' (integer) - the PR number. \
         Requires a configured GitHub authentication and a GitHub-backed git repo. \
         Common gotcha: 'number' is the repository-scoped PR number; the tool returns the first batch of reviews, not the full diff."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "number": {
                    "type": "integer",
                    "description": "PR number"
                }
            },
            "required": ["number"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "github:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, owner, repo) = detect(ctx)?;
        let number = input["number"]
            .as_u64()
            .context("Missing required 'number' parameter")?;

        let pr_path = format!("/repos/{owner}/{repo}/pulls/{number}");
        let reviews_path = format!("/repos/{owner}/{repo}/pulls/{number}/reviews");

        let (pr, reviews) = tokio::try_join!(client.get(&pr_path), client.get(&reviews_path))?;

        let title = pr["title"].as_str().unwrap_or("(no title)");
        let state = pr["state"].as_str().unwrap_or("?");
        let author = pr["user"]["login"].as_str().unwrap_or("?");
        let head = pr["head"]["ref"].as_str().unwrap_or("?");
        let base = pr["base"]["ref"].as_str().unwrap_or("?");
        let body = pr["body"].as_str().unwrap_or("*(no description)*");
        let url = pr["html_url"].as_str().unwrap_or("");

        let mut md = format!(
            "## PR #{number}: {title}\n\n\
             **State**: {state}  \n\
             **Author**: {author}  \n\
             **Branch**: {head} → {base}  \n\
             **URL**: {url}\n\n\
             ### Description\n\n{body}\n"
        );

        if let Some(rev_arr) = reviews.as_array()
            && !rev_arr.is_empty()
        {
            md.push_str("\n### Reviews\n\n");
            for rev in rev_arr {
                let reviewer = rev["user"]["login"].as_str().unwrap_or("?");
                let rev_state = rev["state"].as_str().unwrap_or("?");
                let rev_body = rev["body"].as_str().unwrap_or("");
                if rev_body.is_empty() {
                    md.push_str(&format!("- **{reviewer}**: {rev_state}\n"));
                } else {
                    md.push_str(&format!("- **{reviewer}** ({rev_state}): {rev_body}\n"));
                }
            }
        }

        Ok(ToolOutput {
            content: md,
            metadata: Some(json!({"pr_number": number, "state": state})),
        })
    }
}

// ── GithubCreatePrTool ────────────────────────────────────────────────────────

/// Tool that creates a new GitHub pull request.
pub struct GithubCreatePrTool;

#[async_trait::async_trait]
impl Tool for GithubCreatePrTool {
    fn name(&self) -> &'static str {
        "github_create_pr"
    }

    fn description(&self) -> &'static str {
        "Create a new GitHub pull request in the repository detected from the working directory. \
         Required parameter: 'title' (string) - the PR title. \
         Optional: 'base' (string, default 'main') is the target branch; 'head' (string, default current git branch) is the source branch; \
         'body' (string) is the PR description in markdown; 'draft' (boolean, default false) creates a draft PR. \
         Requires a configured GitHub authentication and a GitHub-backed git repo. \
         Common gotcha: the 'head' branch must already be pushed to the same remote; otherwise GitHub will reject the PR with a missing head reference error."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "title": {
                    "type": "string",
                    "description": "PR title"
                },
                "body": {
                    "type": "string",
                    "description": "PR description (markdown supported)"
                },
                "base": {
                    "type": "string",
                    "description": "Base branch (default: main)"
                },
                "head": {
                    "type": "string",
                    "description": "Head branch (default: current git branch)"
                },
                "draft": {
                    "type": "boolean",
                    "description": "Create as draft PR"
                }
            },
            "required": ["title"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "github:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, owner, repo) = detect(ctx)?;

        let title = input["title"]
            .as_str()
            .context("Missing required 'title' parameter")?;

        let head = if let Some(h) = input["head"].as_str() {
            h.to_string()
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

        let base = input["base"].as_str().unwrap_or("main");
        let draft = input["draft"].as_bool().unwrap_or(false);

        let mut body_json = json!({
            "title": title,
            "head": head,
            "base": base,
            "draft": draft,
        });
        if let Some(b) = input["body"].as_str() {
            body_json["body"] = Value::String(b.to_string());
        }

        let resp = client
            .post(&format!("/repos/{owner}/{repo}/pulls"), &body_json)
            .await?;

        let pr_number = resp["number"].as_u64().unwrap_or(0);
        let url = resp["html_url"].as_str().unwrap_or("");

        Ok(ToolOutput {
            content: format!("Created PR #{pr_number}: {url}"),
            metadata: Some(json!({"pr_number": pr_number, "url": url})),
        })
    }
}

// ── GithubMergePrTool ─────────────────────────────────────────────────────────

/// Tool that merges a GitHub pull request.
pub struct GithubMergePrTool;

#[async_trait::async_trait]
impl Tool for GithubMergePrTool {
    fn name(&self) -> &'static str {
        "github_merge_pr"
    }

    fn description(&self) -> &'static str {
        "Merge an open GitHub pull request using the configured merge method. \
         Required parameter: 'number' (integer) - the PR number. \
         Optional: 'method' (enum 'merge', 'squash', 'rebase', default 'merge') selects the merge strategy; \
         'message' (string) provides a custom merge commit message. \
         Requires a configured GitHub authentication and a GitHub-backed git repo. \
         Common gotcha: the PR must be mergeable and all required checks must pass; protected branches may reject merges from this tool."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "number": {
                    "type": "integer",
                    "description": "PR number"
                },
                "method": {
                    "type": "string",
                    "enum": ["merge", "squash", "rebase"],
                    "description": "Merge method (default: merge)"
                },
                "message": {
                    "type": "string",
                    "description": "Optional merge commit message"
                }
            },
            "required": ["number"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "github:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, owner, repo) = detect(ctx)?;

        let number = input["number"]
            .as_u64()
            .context("Missing required 'number' parameter")?;
        let method = input["method"].as_str().unwrap_or("merge");

        let mut body = json!({"merge_method": method});
        if let Some(msg) = input["message"].as_str() {
            body["commit_message"] = Value::String(msg.to_string());
        }

        client
            .put(
                &format!("/repos/{owner}/{repo}/pulls/{number}/merge"),
                &body,
            )
            .await?;

        Ok(ToolOutput {
            content: format!("PR #{number} merged successfully (method: {method})."),
            metadata: Some(json!({"pr_number": number, "method": method})),
        })
    }
}

// ── GithubReviewPrTool ────────────────────────────────────────────────────────

/// Tool that submits a review on a GitHub pull request.
pub struct GithubReviewPrTool;

#[async_trait::async_trait]
impl Tool for GithubReviewPrTool {
    fn name(&self) -> &'static str {
        "github_review_pr"
    }

    fn description(&self) -> &'static str {
        "Submit a review (approve, request changes, or comment) on a GitHub pull request. \
         Required parameters: 'number' (integer) - the PR number, and 'event' (enum 'APPROVE', 'REQUEST_CHANGES', 'COMMENT') - the review action. \
         Optional: 'body' (string) is the review comment text; required when 'event' is 'COMMENT' or 'REQUEST_CHANGES'. \
         Requires a configured GitHub authentication and a GitHub-backed git repo. \
         Common gotcha: 'APPROVE' cannot include a body in the GitHub API; if you need explanatory text use 'COMMENT' separately."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "additionalProperties": false,
            "properties": {
                "number": {
                    "type": "integer",
                    "description": "PR number"
                },
                "event": {
                    "type": "string",
                    "enum": ["APPROVE", "REQUEST_CHANGES", "COMMENT"],
                    "description": "Review type"
                },
                "body": {
                    "type": "string",
                    "description": "Review comment body"
                }
            },
            "required": ["number", "event"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "github:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let (client, owner, repo) = detect(ctx)?;

        let number = input["number"]
            .as_u64()
            .context("Missing required 'number' parameter")?;
        let event = input["event"]
            .as_str()
            .context("Missing required 'event' parameter")?;

        let mut body = json!({"event": event});
        if let Some(b) = input["body"].as_str() {
            body["body"] = Value::String(b.to_string());
        }

        let resp = client
            .post(
                &format!("/repos/{owner}/{repo}/pulls/{number}/reviews"),
                &body,
            )
            .await?;

        let review_id = resp["id"].as_u64().unwrap_or(0);

        Ok(ToolOutput {
            content: format!(
                "Review submitted on PR #{number} (event: {event}, review ID: {review_id})."
            ),
            metadata: Some(json!({"pr_number": number, "event": event, "review_id": review_id})),
        })
    }
}
