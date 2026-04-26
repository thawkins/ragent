# VCS Tool Implementation Pattern

Complete reference for implementing tools in `ragent-tools-vcs`.

---

## 1. Tool Trait Definition

**Location:** `crates/ragent-tools-vcs/src/lib.rs` (lines 66-73)

```rust
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &str;
    fn description(&self) -> &str;
    fn parameters_schema(&self) -> Value;
    fn permission_category(&self) -> &str;
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput>;
}
```

---

## 2. ToolContext Definition (VCS-specific)

**Location:** `crates/ragent-tools-vcs/src/lib.rs` (lines 57-63)

```rust
#[derive(Clone)]
pub struct ToolContext {
    pub session_id: String,
    pub working_dir: PathBuf,
    pub storage: Option<Arc<dyn storage::StorageBackend>>,
}
```

> **Note:** This differs from `ragent-tools-core`, which has `event_bus: Arc<EventBus>` instead of `storage`.

---

## 3. ToolOutput Definition

**Location:** `crates/ragent-tools-vcs/src/lib.rs` (lines 42-55)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolOutput {
    pub content: String,
    pub metadata: Option<Value>,
}

impl Default for ToolOutput {
    fn default() -> Self {
        Self {
            content: String::new(),
            metadata: None,
        }
    }
}
```

---

## 4. Required Cargo.toml Dependencies

**File:** `crates/ragent-tools-vcs/Cargo.toml`

```toml
[dependencies]
ragent-config = { path = "../ragent-config" }
ragent-types = { path = "../ragent-types" }

tokio = { workspace = true }
async-trait = { workspace = true }
serde = { workspace = true }
serde_json = { workspace = true }
anyhow = { workspace = true }
thiserror = { workspace = true }
reqwest = { workspace = true }
dirs = { workspace = true }
```

---

## 5. Exact Imports at the Top of a VCS Tool File

**From:** `crates/ragent-tools-vcs/src/github/github_issues.rs` (lines 1-8)

```rust
//! One-line doc comment describing what this tool does.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use crate::github::GitHubClient;
use super::{Tool, ToolContext, ToolOutput};
```

---

## 6. Complete Implementation Template

Based on `github_issues.rs` — this is the exact pattern to follow:

```rust
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
```

---

## 7. Module Wiring (`mod.rs`)

**File:** `crates/ragent-tools-vcs/src/github/mod.rs`

```rust
//! GitHub API client and GitHub-backed tools for ragent.

pub mod auth;
pub mod client;
pub mod github_issues;
pub mod github_prs;

pub use auth::{delete_token, device_flow_login, load_token, save_token};
pub use client::GitHubClient;
pub use github_issues::{
    GithubCloseIssueTool, GithubCommentIssueTool, GithubCreateIssueTool,
    GithubGetIssueTool, GithubListIssuesTool,
};
pub use github_prs::{
    GithubCreatePrTool, GithubGetPrTool, GithubListPrsTool,
    GithubMergePrTool, GithubReviewPrTool,
};

pub use crate::{Tool, ToolContext, ToolOutput};
```

---

## 8. Key Rules Summary

| Aspect | Rule |
|--------|------|
| **Struct** | Unit struct: `pub struct ToolName;` |
| **Trait impl** | Must have `#[async_trait::async_trait]` |
| **Return type** | `Result<ToolOutput>` — always use `anyhow::Result` |
| **Parameter extraction** | Use `input["field"].as_str()`, `.as_u64()`, `.as_bool()`, then `.context("...")` |
| **Error handling** | Use `.context("...")?` from `anyhow::Context` for descriptive errors |
| **Schema** | Build with `json!({...})` macro, always include `"type": "object"` |
| **Permission category** | Use colon-separated names: `"github:read"`, `"github:write"`, `"gitlab:read"` |
| **Output** | Always return `ToolOutput { content, metadata }`. `metadata` can be `None` |
| **Naming** | Tool name must be `snake_case` and globally unique |
| **Client** | Create a helper `make_client()` that returns a descriptive auth error |

---

## 9. Error Handling Pattern

Always use `anyhow::Context` to add human-readable descriptions:

```rust
let client = GitHubClient::new()
    .context("GitHub not authenticated. Run /github login to authenticate.")?;

let number = input["number"]
    .as_u64()
    .context("Missing required parameter 'number'")?;
```

---

## 10. Output Formatting Pattern

Return `ToolOutput` with both human-readable `content` and structured `metadata`:

```rust
Ok(ToolOutput {
    content: format!("Created issue #{number} in {owner}/{repo}\nURL: {html_url}"),
    metadata: Some(
        json!({ "number": number, "url": html_url, "owner": owner, "repo": repo }),
    ),
})
```

The `metadata` field is optional but recommended for machine-readable follow-up.
