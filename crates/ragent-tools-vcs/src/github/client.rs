//! GitHub API client.

use anyhow::{Context, Result, bail};
use serde_json::Value;

/// GitHub OAuth App client ID for ragent.
///
/// Override via the `RAGENT_GITHUB_CLIENT_ID` environment variable, or set in
/// `~/.ragent/config.toml` as `github_client_id`. Requires a registered
/// GitHub OAuth App — see docs/github-oauth.md.
///
/// This value is shared with the GitHub Copilot provider, which already
/// performs GitHub OAuth device flow. Using the same OAuth application keeps
/// the login experience consistent across VCS tools and Copilot.
const GITHUB_CLIENT_ID_DEFAULT: &str = "Iv1.b507a08c87ecfe98";

/// Typed repository metadata extracted from `GET /repos/{owner}/{repo}`.
///
/// All string fields default to an empty string when the API response omits
/// them, so callers can rely on non-`None` values. `topics` defaults to an
/// empty vector. `stargazers_count` defaults to `0`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepoMetadata {
    /// Repository description (may be empty if the repo has none).
    pub description: String,
    /// Primary programming language reported by GitHub (may be empty).
    pub language: String,
    /// Repository topics (labels).
    pub topics: Vec<String>,
    /// Number of stars (stargazers count).
    pub stargazers_count: u64,
    /// Default branch name (e.g. `main`, `master`).
    pub default_branch: String,
}

impl RepoMetadata {
    /// Parse a `GET /repos/{owner}/{repo}` JSON response into [`RepoMetadata`].
    ///
    /// Missing optional fields default to empty strings / `0` / empty vector.
    #[must_use]
    pub fn from_response(value: &Value) -> Self {
        let description = value
            .get("description")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let language = value
            .get("language")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        let topics = value
            .get("topics")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let stargazers_count = value
            .get("stargazers_count")
            .and_then(Value::as_u64)
            .unwrap_or(0);
        let default_branch = value
            .get("default_branch")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string();
        Self {
            description,
            language,
            topics,
            stargazers_count,
            default_branch,
        }
    }
}

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
    #[must_use]
    pub fn with_token(token: String) -> Self {
        Self {
            token,
            client: reqwest::Client::new(),
        }
    }

    /// The OAuth App client ID used for device flow login.
    /// Resolved from `RAGENT_GITHUB_CLIENT_ID` env var, falling back to the compiled default.
    #[must_use]
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

        if (status.as_u16() == 403 || status.as_u16() == 429)
            && let Some(reset) = resp.headers().get("x-ratelimit-reset")
        {
            let reset_str = reset.to_str().unwrap_or("unknown");
            bail!("GitHub rate limit exceeded. Resets at epoch {reset_str}. Path: {path}");
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

    /// GET request returning raw bytes (follows redirects). Used for
    /// endpoints that serve binary blobs such as the Actions logs zip.
    pub async fn get_bytes(&self, path: &str) -> Result<Vec<u8>> {
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
            .with_context(|| format!("GitHub GET (bytes) {path} failed"))?;

        let status = resp.status();
        if status.as_u16() == 401 {
            bail!("GitHub authentication failed. Run /github login to re-authenticate.");
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("GitHub API error {status} for {path}: {body}");
        }
        let bytes = resp
            .bytes()
            .await
            .with_context(|| format!("Failed to read GitHub bytes for {path}"))?;
        Ok(bytes.to_vec())
    }

    /// Get the authenticated user's profile.
    pub async fn current_user(&self) -> Result<Value> {
        self.get("/user").await
    }

    /// Detect the GitHub owner/repo from the current git repository remote.
    #[must_use]
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

    /// Parse a user-supplied repository identifier into an `(owner, repo)` pair.
    ///
    /// Accepts three forms:
    /// - Full HTTPS URL: `https://github.com/owner/repo`
    /// - SSH URL: `git@github.com:owner/repo.git`
    /// - Shorthand: `owner/repo`
    ///
    /// Trailing slashes, trailing `.git`, and optional query-string fragments
    /// are stripped. Returns `None` if the identifier does not resolve to
    /// exactly two non-empty path segments.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_tools_vcs::github::GitHubClient;
    /// assert_eq!(
    ///     GitHubClient::parse_repo_url("https://github.com/octocat/Hello-World"),
    ///     Some(("octocat".to_string(), "Hello-World".to_string()))
    /// );
    /// assert_eq!(
    ///     GitHubClient::parse_repo_url("octocat/Hello-World"),
    ///     Some(("octocat".to_string(), "Hello-World".to_string()))
    /// );
    /// assert_eq!(GitHubClient::parse_repo_url("justoneword"), None);
    /// ```
    #[must_use]
    pub fn parse_repo_url(input: &str) -> Option<(String, String)> {
        let input = input.trim();
        if input.is_empty() {
            return None;
        }

        // Strip query string and fragments.
        let input = input.split(['?', '#']).next().unwrap_or(input);

        // Determine the owner/repo path portion based on the URL scheme.
        let path = if let Some(rest) = input
            .strip_prefix("https://github.com/")
            .or_else(|| input.strip_prefix("http://github.com/"))
        {
            rest
        } else if let Some(rest) = input.strip_prefix("git@github.com:") {
            rest
        } else if input.contains("github.com/") {
            // Generic github.com URL (e.g. with www or other subdomains).
            input.split("github.com/").nth(1).unwrap_or(input)
        } else {
            // Bare owner/repo shorthand.
            input
        };

        // Strip trailing slashes, then trailing .git, then trailing slashes
        // again so inputs like "owner/repo.git/" are normalised correctly.
        let path = path.trim_end_matches('/');
        let path = path.trim_end_matches(".git");
        let path = path.trim_end_matches('/');

        let mut parts = path.splitn(2, '/');
        let owner = parts.next()?.trim();
        let repo = parts.next()?.trim();

        if owner.is_empty() || repo.is_empty() {
            return None;
        }

        // Reject identifiers with more than two segments (e.g. owner/repo/extra).
        if repo.contains('/') {
            return None;
        }

        Some((owner.to_string(), repo.to_string()))
    }

    /// Validate a user-supplied repository identifier, returning either the
    /// parsed `(owner, repo)` pair or a human-readable usage message.
    ///
    /// This wraps [`parse_repo_url`] for the user-facing rejection path
    /// (FR-016): when the input does not resolve to exactly two non-empty
    /// path segments (empty string, single word, three-segment path, etc.),
    /// a usage message is returned instead of a silent `None`, and no API
    /// calls should be made.
    ///
    /// # Errors
    ///
    /// Returns `Err(message)` when the identifier is invalid. The `message`
    /// is a human-readable usage string suitable for display in the TUI.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_tools_vcs::github::GitHubClient;
    /// assert_eq!(
    ///     GitHubClient::validate_repo_input("octocat/Hello-World").unwrap(),
    ///     ("octocat".to_string(), "Hello-World".to_string())
    /// );
    /// assert!(GitHubClient::validate_repo_input("").is_err());
    /// assert!(GitHubClient::validate_repo_input("justoneword").is_err());
    /// assert!(GitHubClient::validate_repo_input("owner/repo/extra").is_err());
    /// ```
    pub fn validate_repo_input(input: &str) -> std::result::Result<(String, String), String> {
        Self::parse_repo_url(input).ok_or_else(|| {
            format!(
                "Invalid repository identifier: '{input}'.\n\
                 Usage: /reverse <owner/repo | https://github.com/owner/repo | \
                 git@github.com:owner/repo.git>\n\
                 The identifier must resolve to exactly two non-empty path segments \
                 (owner and repo)."
            )
        })
    }

    /// Fetch repository metadata via `GET /repos/{owner}/{repo}`.
    ///
    /// Returns a typed [`RepoMetadata`] struct extracting at minimum the
    /// description, primary language, topics, star count, and default branch.
    /// Missing optional fields default to empty strings / `0` / empty vector.
    ///
    /// # Errors
    ///
    /// Returns an error if the GitHub API call fails (network, auth, or
    /// non-success status code).
    pub async fn fetch_repo_metadata(&self, owner: &str, repo: &str) -> Result<RepoMetadata> {
        let path = format!("/repos/{owner}/{repo}");
        let value = self.get(&path).await?;
        Ok(RepoMetadata::from_response(&value))
    }

    /// Fetch the root-level file tree via `GET /repos/{owner}/{repo}/contents`.
    ///
    /// Returns the list of file and directory names at the repository root, in
    /// the order returned by the API. An empty repository (or one with no
    /// visible root entries) yields an empty vector.
    ///
    /// # Errors
    ///
    /// Returns an error if the GitHub API call fails (network, auth, or
    /// non-success status code).
    pub async fn fetch_root_tree(&self, owner: &str, repo: &str) -> Result<Vec<String>> {
        let path = format!("/repos/{owner}/{repo}/contents");
        let value = self.get(&path).await?;
        Ok(parse_root_tree(&value))
    }

    /// Fetch the README content for a repository (FR-007).
    ///
    /// Calls `GET /repos/{owner}/{repo}/readme` to obtain the README metadata
    /// (which includes a `download_url`), then fetches the raw README text via
    /// [`get_bytes`]. If the repository has no README (HTTP 404), returns
    /// `Ok(None)` so the caller can proceed with an empty README string rather
    /// than failing. Any other API error (auth, rate limit, network) is
    /// propagated.
    ///
    /// # Errors
    ///
    /// Returns an error for non-404 API failures (network, auth, rate limit,
    /// 500, etc.). A 404 is **not** an error — it yields `Ok(None)`.
    pub async fn fetch_readme(&self, owner: &str, repo: &str) -> Result<Option<String>> {
        let path = format!("/repos/{owner}/{repo}/readme");
        let url = format!("https://api.github.com{path}");

        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3+json")
            .header("User-Agent", "ragent/0.1")
            .send()
            .await
            .with_context(|| format!("GitHub GET {path} failed"))?;

        let status = resp.status();
        if status.as_u16() == 404 {
            // No README present — proceed with empty README, per FR-007.
            return Ok(None);
        }
        if status.as_u16() == 401 {
            bail!("GitHub authentication failed. Run /github login to re-authenticate.");
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("GitHub API error {status} for {path}: {body}");
        }

        let value: Value = resp
            .json()
            .await
            .with_context(|| format!("Failed to parse README metadata JSON for {path}"))?;

        let download_url = extract_download_url(&value).ok_or_else(|| {
            anyhow::anyhow!("README metadata response missing 'download_url' field for {path}")
        })?;

        let bytes = self.get_bytes(&download_url).await?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        Ok(Some(text))
    }
}

/// Parse a `GET /repos/{owner}/{repo}/contents` JSON response into a list of
/// root file and directory names.
///
/// The GitHub contents endpoint returns a JSON array of objects, each with a
/// `name` string and a `type` field (e.g. `"file"` or `"dir"`). This helper
/// extracts the `name` of every entry, preserving the API's ordering. A
/// non-array response (e.g. an empty-repo 404 body or a single-file object)
/// yields an empty vector.
#[must_use]
pub fn parse_root_tree(value: &Value) -> Vec<String> {
    let arr = match value.as_array() {
        Some(arr) => arr,
        None => return Vec::new(),
    };
    arr.iter()
        .filter_map(|entry| entry.get("name").and_then(Value::as_str).map(String::from))
        .collect()
}

/// Extract the `download_url` from a `GET /repos/{owner}/{repo}/readme` JSON
/// response.
///
/// The README metadata object includes a `download_url` string pointing at the
/// raw README content. Returns `None` if the field is absent or not a string.
#[must_use]
pub fn extract_download_url(value: &Value) -> Option<String> {
    value
        .get("download_url")
        .and_then(Value::as_str)
        .map(String::from)
}

/// Extract the `X-RateLimit-Reset` header value (a Unix timestamp) from a
/// response's headers, if present and parseable as `u64`.
#[must_use]
pub fn extract_rate_limit_reset(headers: &reqwest::header::HeaderMap) -> Option<u64> {
    headers
        .get("x-ratelimit-reset")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.trim().parse::<u64>().ok())
}

/// Format a Unix timestamp as a human-readable UTC time string.
#[must_use]
pub fn format_reset_time(reset: u64) -> String {
    // Convert the Unix timestamp to a UTC calendar representation without
    // pulling in chrono. We use a simple civil-from-days algorithm to avoid
    // adding a new dependency (NFR-001).
    let total_secs = reset as i64;
    let days = total_secs.div_euclid(86_400);
    let secs_of_day = total_secs.rem_euclid(86_400);

    // Civil date from days since 1970-01-01 (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = y + if m <= 2 { 1 } else { 0 };

    let hour = secs_of_day / 3600;
    let minute = (secs_of_day % 3600) / 60;
    let second = secs_of_day % 60;

    format!("{year:04}-{m:02}-{d:02} {hour:02}:{minute:02}:{second:02} UTC")
}

/// Classify a GitHub API error into a human-readable message that includes the
/// HTTP status code and the repository identifier (FR-014, FR-015, FR-017).
///
/// # Arguments
///
/// - `status` — the HTTP status code.
/// - `repo_id` — the repository identifier (e.g. `"owner/repo"`).
/// - `body` — the raw response body (for diagnostic context).
/// - `reset_time` — optional `X-RateLimit-Reset` Unix timestamp (for 403/429).
///
/// # Returns
///
/// A human-readable error string. For 404, the message states the repository
/// was not found or is private (FR-017). For 403/429, the message includes the
/// rate-limit reset time if available (FR-015). For all other non-success
/// statuses, a generic message with the status code and repo identifier is
/// returned (FR-014).
#[must_use]
pub fn classify_api_error(
    status: u16,
    repo_id: &str,
    body: &str,
    reset_time: Option<u64>,
) -> String {
    match status {
        404 => format!(
            "Repository '{repo_id}' was not found, or it is private and your \
             token lacks access. No further API calls will be made."
        ),
        403 | 429 => {
            let reset_msg = reset_time
                .map(|r| format!("\nRate limit resets at: {}", format_reset_time(r)))
                .unwrap_or_default();
            format!(
                "GitHub API rate limit hit (HTTP {status}) for repository \
                 '{repo_id}'.{reset_msg}\nThe request was not retried."
            )
        }
        401 => format!(
            "GitHub authentication failed (HTTP 401). Run /github login to \
             re-authenticate. Repository: '{repo_id}'."
        ),
        _ => {
            let body_snippet = if body.is_empty() {
                String::new()
            } else {
                let trimmed = body.trim();
                if trimmed.len() > 200 {
                    format!("\nResponse: {}...", &trimmed[..200])
                } else {
                    format!("\nResponse: {trimmed}")
                }
            };
            format!(
                "GitHub API error (HTTP {status}) for repository \
                 '{repo_id}'.{body_snippet}"
            )
        }
    }
}

/// Maximum number of README characters included in the reverse-engineering
/// context block (NFR-003).
pub const README_MAX_CHARS: usize = 8000;

/// Assemble the reverse-engineering context block from repo metadata, root
/// file tree, README content, and an optional technology-stack constraint
/// (FR-008, NFR-003).
///
/// The returned string is a single context block suitable for passing to the
/// LLM as part of the prompt that instructs the model to generate a synthetic
/// creation prompt for the repository.
///
/// # README truncation
///
/// The README content is truncated to [`README_MAX_CHARS`] (8000) characters
/// before inclusion, with a truncation notice appended when truncation occurs.
/// If `readme` is `None` (no README found) or empty, a placeholder line is
/// emitted instead.
///
/// # Arguments
///
/// - `metadata` — typed repo metadata (description, language, topics, stars,
///   default branch).
/// - `tree` — root-level file and directory names.
/// - `readme` — optional raw README text (`None` means no README was found).
/// - `tech` — optional technology-stack constraint to include in the context.
#[must_use]
pub fn build_reverse_prompt(
    metadata: &RepoMetadata,
    tree: &[String],
    readme: Option<&str>,
    tech: Option<&str>,
) -> String {
    let mut sections: Vec<String> = Vec::new();

    // --- Repo metadata ---
    let mut meta_lines = Vec::new();
    meta_lines.push(format!("Description: {}", metadata.description));
    meta_lines.push(format!("Language: {}", metadata.language));
    meta_lines.push(format!("Stars: {}", metadata.stargazers_count));
    meta_lines.push(format!("Default branch: {}", metadata.default_branch));
    if metadata.topics.is_empty() {
        meta_lines.push("Topics: (none)".to_string());
    } else {
        meta_lines.push(format!("Topics: {}", metadata.topics.join(", ")));
    }
    sections.push(format!("## Repository Metadata\n{}", meta_lines.join("\n")));

    // --- Optional tech-stack constraint ---
    if let Some(stack) = tech {
        sections.push(format!("## Technology Stack Constraint\n{stack}"));
    }

    // --- Root file tree ---
    let tree_block = if tree.is_empty() {
        "(empty repository)".to_string()
    } else {
        tree.join("\n")
    };
    sections.push(format!("## Root File Tree\n{tree_block}"));

    // --- README (truncated to 8000 chars) ---
    let readme_block = match readme {
        Some(text) if !text.is_empty() => {
            if text.chars().count() > README_MAX_CHARS {
                let truncated: String = text.chars().take(README_MAX_CHARS).collect();
                format!(
                    "{truncated}\n\n[... README truncated at {README_MAX_CHARS} characters ...]"
                )
            } else {
                text.to_string()
            }
        }
        _ => "(no README found)".to_string(),
    };
    sections.push(format!("## README\n{readme_block}"));

    sections.join("\n\n")
}
