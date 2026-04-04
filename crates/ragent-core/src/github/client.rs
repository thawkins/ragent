//! GitHub API client.

use anyhow::{bail, Context, Result};
use serde_json::Value;

/// GitHub OAuth App client ID for ragent.
///
/// Override via the `RAGENT_GITHUB_CLIENT_ID` environment variable, or set in
/// `~/.ragent/config.toml` as `github_client_id`. Requires a registered
/// GitHub OAuth App — see docs/github-oauth.md.
const GITHUB_CLIENT_ID_DEFAULT: &str = "";

/// Lightweight authenticated GitHub API client.
#[derive(Clone)]
pub struct GitHubClient {
    token: String,
    client: reqwest::Client,
}

impl GitHubClient {
    /// Create a new client, resolving the token from environment/file.
    pub fn new() -> Result<Self> {
        let token = super::auth::load_token()
            .context("No GitHub token found. Run /github login to authenticate.")?;
        Ok(Self {
            token,
            client: reqwest::Client::new(),
        })
    }

    /// Create from an explicit token.
    pub fn with_token(token: String) -> Self {
        Self {
            token,
            client: reqwest::Client::new(),
        }
    }

    /// The OAuth App client ID used for device flow login.
    /// Resolved from `RAGENT_GITHUB_CLIENT_ID` env var, falling back to the compiled default.
    pub fn client_id() -> String {
        std::env::var("RAGENT_GITHUB_CLIENT_ID")
            .unwrap_or_else(|_| GITHUB_CLIENT_ID_DEFAULT.to_string())
    }

    /// GET request to the GitHub API.
    pub async fn get(&self, path: &str) -> Result<Value> {
        let url = if path.starts_with("https://") {
            path.to_string()
        } else {
            format!("https://api.github.com{path}")
        };

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "ragent/0.1")
            .send()
            .await
            .with_context(|| format!("GitHub GET {path} failed"))?;

        self.handle_response(resp, path).await
    }

    /// POST request to the GitHub API.
    pub async fn post(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("https://api.github.com{path}");
        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "ragent/0.1")
            .json(body)
            .send()
            .await
            .with_context(|| format!("GitHub POST {path} failed"))?;

        self.handle_response(resp, path).await
    }

    /// PUT request to the GitHub API.
    pub async fn put(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("https://api.github.com{path}");
        let resp = self
            .client
            .put(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "ragent/0.1")
            .json(body)
            .send()
            .await
            .with_context(|| format!("GitHub PUT {path} failed"))?;

        self.handle_response(resp, path).await
    }

    /// PATCH request to the GitHub API.
    pub async fn patch(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("https://api.github.com{path}");
        let resp = self
            .client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "ragent/0.1")
            .json(body)
            .send()
            .await
            .with_context(|| format!("GitHub PATCH {path} failed"))?;

        self.handle_response(resp, path).await
    }

    async fn handle_response(&self, resp: reqwest::Response, path: &str) -> Result<Value> {
        let status = resp.status();

        if status.as_u16() == 403 || status.as_u16() == 429 {
            if let Some(reset) = resp.headers().get("x-ratelimit-reset") {
                let reset_str = reset.to_str().unwrap_or("unknown");
                bail!("GitHub rate limit exceeded. Resets at epoch {reset_str}. Path: {path}");
            }
        }

        if status.as_u16() == 401 {
            bail!("GitHub authentication failed. Run /github login to re-authenticate.");
        }

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("GitHub API error {status} for {path}: {body}");
        }

        let json: Value = resp
            .json()
            .await
            .with_context(|| format!("Failed to parse GitHub response for {path}"))?;
        Ok(json)
    }

    /// Get the authenticated user's profile.
    pub async fn current_user(&self) -> Result<Value> {
        self.get("/user").await
    }

    /// Detect the GitHub owner/repo from the current git repository remote.
    pub fn detect_repo(working_dir: &std::path::Path) -> Option<(String, String)> {
        let output = std::process::Command::new("git")
            .args(["remote", "get-url", "origin"])
            .current_dir(working_dir)
            .output()
            .ok()?;

        if !output.status.success() {
            return None;
        }
        let url = String::from_utf8(output.stdout).ok()?;
        let url = url.trim();

        // Parse: git@github.com:owner/repo.git  or  https://github.com/owner/repo
        let path = if url.contains("github.com:") {
            url.split("github.com:").nth(1)?
        } else if url.contains("github.com/") {
            url.split("github.com/").nth(1)?
        } else {
            return None;
        };

        let path = path.trim_end_matches(".git");
        let mut parts = path.splitn(2, '/');
        let owner = parts.next()?.to_string();
        let repo = parts.next()?.to_string();
        Some((owner, repo))
    }
}
