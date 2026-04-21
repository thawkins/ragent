//! GitLab API client.
//!
//! Mirrors the [`GitHubClient`](crate::github::GitHubClient) pattern but
//! targets a configurable GitLab instance URL and authenticates with a
//! Personal Access Token via the `PRIVATE-TOKEN` header.

use anyhow::{Context, Result, bail};
use serde_json::Value;

/// Lightweight authenticated GitLab API client.
#[derive(Clone)]
pub struct GitLabClient {
    token: String,
    base_url: String,
    client: reqwest::Client,
}

impl GitLabClient {
    /// Create a new client from stored configuration and token.
    ///
    /// Resolves credentials using the layered priority: env vars → ragent.json → database.
    pub fn new(storage: &crate::storage::Storage) -> Result<Self> {
        let config = super::auth::load_config(storage).context(
            "GitLab not configured. Run /gitlab setup to configure instance URL and credentials.",
        )?;
        let token = super::auth::load_token(storage).context(
            "No GitLab token found. Run /gitlab setup to configure your Personal Access Token.",
        )?;
        Ok(Self {
            token,
            base_url: config.instance_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        })
    }

    /// Create from explicit values.
    #[must_use]
    pub fn with_credentials(base_url: String, token: String) -> Self {
        Self {
            token,
            base_url: base_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
        }
    }

    /// GET request to the GitLab API.
    pub async fn get(&self, path: &str) -> Result<Value> {
        let url = if path.starts_with("https://") || path.starts_with("http://") {
            path.to_string()
        } else {
            format!("{}/api/v4{path}", self.base_url)
        };

        let resp = self
            .client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "ragent/0.1")
            .send()
            .await
            .with_context(|| format!("GitLab GET {path} failed"))?;

        self.handle_response(resp, path).await
    }

    /// POST request to the GitLab API.
    pub async fn post(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("{}/api/v4{path}", self.base_url);
        let resp = self
            .client
            .post(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "ragent/0.1")
            .json(body)
            .send()
            .await
            .with_context(|| format!("GitLab POST {path} failed"))?;

        self.handle_response(resp, path).await
    }

    /// PUT request to the GitLab API.
    pub async fn put(&self, path: &str, body: &Value) -> Result<Value> {
        let url = format!("{}/api/v4{path}", self.base_url);
        let resp = self
            .client
            .put(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "ragent/0.1")
            .json(body)
            .send()
            .await
            .with_context(|| format!("GitLab PUT {path} failed"))?;

        self.handle_response(resp, path).await
    }

    /// Handle a GitLab API response, checking for errors and parsing JSON.
    async fn handle_response(&self, resp: reqwest::Response, path: &str) -> Result<Value> {
        let status = resp.status();

        if status.as_u16() == 429 {
            bail!("GitLab rate limit exceeded. Path: {path}");
        }

        if status.as_u16() == 401 {
            bail!(
                "GitLab authentication failed. Run /gitlab setup to update your Personal Access Token."
            );
        }

        if status.as_u16() == 403 {
            bail!("GitLab permission denied for {path}. Check your token scopes.");
        }

        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("GitLab API error {status} for {path}: {body}");
        }

        // Some endpoints (e.g. DELETE) return 204 with no body.
        let body_text = resp.text().await.unwrap_or_default();
        if body_text.is_empty() {
            return Ok(Value::Null);
        }

        serde_json::from_str(&body_text)
            .with_context(|| format!("Failed to parse GitLab response for {path}"))
    }

    /// Detect the GitLab project path from the current git repository remote.
    ///
    /// Returns a URL-encoded project path (e.g. `namespace%2Fproject`) suitable
    /// for use in `/projects/:id/` API endpoints.
    #[must_use]
    pub fn detect_project(working_dir: &std::path::Path) -> Option<String> {
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

        // Try to extract the path after a gitlab-ish host.
        // Supports: git@host:namespace/project.git, https://host/namespace/project.git
        let path = if let Some(idx) = url.find(':') {
            // SSH format: git@gitlab.example.com:group/project.git
            if url[..idx].contains('@') {
                Some(&url[idx + 1..])
            } else {
                None
            }
        } else {
            None
        }
        .or_else(|| {
            // HTTPS format: https://gitlab.example.com/group/project.git
            url.split("//")
                .nth(1)
                .and_then(|rest| rest.find('/').map(|i| &rest[i + 1..]))
        })?;

        let path = path.trim_end_matches(".git");
        if path.is_empty() || !path.contains('/') {
            return None;
        }

        Some(urlencoded_path(path))
    }

    /// Return the configured instance URL.
    #[must_use]
    pub fn instance_url(&self) -> &str {
        &self.base_url
    }
}

/// URL-encode a GitLab project path (e.g. `group/project` → `group%2Fproject`).
fn urlencoded_path(path: &str) -> String {
    path.replace('/', "%2F")
}
