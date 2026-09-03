//! GitLab API client.
//!
//! Mirrors the [`GitHubClient`](crate::github::GitHubClient) pattern but
//! targets a configurable GitLab instance URL and authenticates with a
//! Personal Access Token via the `PRIVATE-TOKEN` header.

use anyhow::{Context, Result, bail};
use serde_json::Value;

use crate::github::RepoMetadata;

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

    /// Fetch a GitLab project's metadata via `GET /projects/:id` (FR-008,
    /// FR-009).
    ///
    /// `project_path` is the `namespace/project` identifier (which may contain
    /// nested groups, e.g. `group/subgroup/project`). It is URL-encoded into
    /// the `:id` path segment before the API call. The GitLab project JSON
    /// response is mapped onto a [`RepoMetadata`] struct:
    ///
    /// - `description` → `description`
    /// - `topics` (array of strings) → `topics`
    /// - `star_count` → `stargazers_count`
    /// - `default_branch` → `default_branch`
    /// - primary language → `language`, resolved from the separate
    ///   `GET /projects/:id/languages` endpoint (the language with the highest
    ///   share). If the languages call fails or is empty, `language` defaults
    ///   to an empty string.
    ///
    /// # Errors
    ///
    /// Returns an error if the `GET /projects/:id` call fails (network, auth,
    /// 404, etc.). A failure of the secondary `GET /projects/:id/languages`
    /// call is **not** propagated — `language` simply defaults to empty.
    pub async fn fetch_project_metadata(&self, project_path: &str) -> Result<RepoMetadata> {
        let encoded = urlencoded_path(project_path);
        let path = format!("/projects/{encoded}");
        let value = self.get(&path).await?;

        let language = self
            .fetch_primary_language(&encoded)
            .await
            .unwrap_or_default();
        Ok(gitlab_project_to_metadata(&value, &language))
    }

    /// Resolve the primary programming language for a project from the
    /// `GET /projects/:id/languages` endpoint.
    ///
    /// The endpoint returns a JSON object mapping language names to their
    /// share (a float between 0 and 100). The language with the highest share
    /// is returned. Returns an empty string if the response is empty or the
    /// call fails.
    async fn fetch_primary_language(&self, encoded_project: &str) -> Result<String> {
        let path = format!("/projects/{encoded_project}/languages");
        let value = self.get(&path).await?;
        Ok(top_language(&value))
    }

    /// Fetch the root-level repository file tree via
    /// `GET /projects/:id/repository/tree` (FR-008, FR-010).
    ///
    /// Returns the list of root-level file and directory names, in the order
    /// returned by the GitLab API. Each entry in the response array is a JSON
    /// object with at minimum a `name` string (and a `type` field such as
    /// `"blob"` or `"tree"`). An empty repository (or one with no visible root
    /// entries) yields an empty vector.
    ///
    /// `project_path` is the `namespace/project` identifier (which may contain
    /// nested groups, e.g. `group/subgroup/project`). It is URL-encoded into
    /// the `:id` path segment before the API call.
    ///
    /// # Errors
    ///
    /// Returns an error if the GitLab API call fails (network, auth, 404,
    /// etc.).
    pub async fn fetch_repository_tree(&self, project_path: &str) -> Result<Vec<String>> {
        let encoded = urlencoded_path(project_path);
        let path = format!("/projects/{encoded}/repository/tree");
        let value = self.get(&path).await?;
        Ok(parse_gitlab_tree(&value))
    }

    /// Fetch the repository file tree recursively up to the specified depth
    /// (FR-027, FR-028).
    ///
    /// At depth 1, only the root-level entries are returned (directory names
    /// get a trailing `/`). At depth N > 1, directories are recursively
    /// expanded by calling `GET /projects/:id/repository/tree?path=<dir>` for
    /// each subdirectory, stopping when the requested depth is reached or no
    /// more directories exist.
    ///
    /// Directory entries are formatted with a trailing `/` to distinguish them
    /// from files (FR-028). The returned paths use `/` separators so the LLM
    /// can infer the full directory structure (e.g. `src/`, `src/main.rs`,
    /// `src/models/`, `src/models/user.rs`).
    ///
    /// `project_path` is the `namespace/project` identifier (which may contain
    /// nested groups). It is URL-encoded into the `:id` path segment.
    ///
    /// # Errors
    ///
    /// Returns an error if any API call fails. Individual directory fetch
    /// failures are tolerated — the directory entry is still listed (with a
    /// trailing slash) but its children are omitted.
    pub async fn fetch_repository_tree_recursive(
        &self,
        project_path: &str,
        depth: u32,
    ) -> Result<Vec<String>> {
        let encoded = urlencoded_path(project_path);
        let mut entries = Vec::new();
        Box::pin(self.fetch_gitlab_tree_inner(&encoded, "", depth, &mut entries)).await?;
        Ok(entries)
    }

    /// Internal recursive helper for [`fetch_repository_tree_recursive`].
    ///
    /// `encoded` is the URL-encoded project `:id`. `prefix` is the directory
    /// path within the repo (empty for root). `remaining` is the number of
    /// levels left to fetch (1 = this level only, no recursion).
    async fn fetch_gitlab_tree_inner(
        &self,
        encoded: &str,
        prefix: &str,
        remaining: u32,
        entries: &mut Vec<String>,
    ) -> Result<()> {
        let path = if prefix.is_empty() {
            format!("/projects/{encoded}/repository/tree")
        } else {
            format!("/projects/{encoded}/repository/tree?path={prefix}")
        };
        let value = self.get(&path).await?;
        let items = parse_gitlab_tree_entries(&value);

        for item in &items {
            let full_path = if prefix.is_empty() {
                item.name.clone()
            } else {
                format!("{prefix}/{}", item.name)
            };
            if item.is_dir {
                entries.push(format!("{full_path}/"));
                if remaining > 1 {
                    let _ = Box::pin(self.fetch_gitlab_tree_inner(
                        encoded,
                        &full_path,
                        remaining - 1,
                        entries,
                    ))
                    .await;
                }
            } else {
                entries.push(full_path);
            }
        }

        Ok(())
    }

    /// Fetch the README content for a GitLab project (FR-008, FR-011).
    ///
    /// Calls `GET /projects/:id/readme` to obtain the README metadata. If the
    /// project has no README (HTTP 404), returns `Ok(None)` so the caller can
    /// proceed with an empty README string rather than failing. On success,
    /// the raw README text is fetched via the `readme_url` field from the
    /// metadata response (a direct URL to the raw blob). If `readme_url` is
    /// absent, the method falls back to the
    /// `GET /projects/:id/repository/files/:file_path/raw` endpoint using
    /// the `file_path` field.
    ///
    /// `project_path` is the `namespace/project` identifier (which may contain
    /// nested groups). It is URL-encoded into the `:id` path segment before
    /// the API call.
    ///
    /// # Errors
    ///
    /// Returns an error for non-404 API failures (network, auth, rate limit,
    /// 500, etc.). A 404 is **not** an error — it yields `Ok(None)`.
    pub async fn fetch_readme(&self, project_path: &str) -> Result<Option<String>> {
        let encoded = urlencoded_path(project_path);
        let path = format!("/projects/{encoded}/readme");
        let url = format!("{}/api/v4{path}", self.base_url);

        let resp = self
            .client
            .get(&url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "ragent/0.1")
            .send()
            .await
            .with_context(|| format!("GitLab GET {path} failed"))?;

        let status = resp.status();
        if status.as_u16() == 404 {
            // No README present — proceed with empty README, per FR-011.
            return Ok(None);
        }
        if status.as_u16() == 401 {
            bail!(
                "GitLab authentication failed. Run /gitlab setup to update your Personal Access Token."
            );
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("GitLab API error {status} for {path}: {body}");
        }

        let value: Value = resp
            .json()
            .await
            .with_context(|| format!("Failed to parse README metadata JSON for {path}"))?;

        // Prefer the readme_url field (a direct raw-blob URL).
        if let Some(readme_url) = extract_readme_url(&value) {
            return self.fetch_raw_text(&readme_url).await.map(Some);
        }

        // Fall back to the repository/files/:file_path/raw endpoint using the
        // file_path field from the README metadata.
        if let Some(file_path) = value.get("file_path").and_then(Value::as_str) {
            let encoded_file = file_path.replace('/', "%2F");
            let raw_path = format!("/projects/{encoded}/repository/files/{encoded_file}/raw");
            let raw_value = self.get(&raw_path).await?;
            return raw_value
                .as_str()
                .map(|s| Some(s.to_string()))
                .ok_or_else(|| anyhow::anyhow!("Expected raw text from {raw_path}"));
        }

        bail!("README metadata response missing 'readme_url' and 'file_path' for {path}");
    }

    /// Fetch raw text content from an absolute URL, authenticating with the
    /// configured GitLab token.
    async fn fetch_raw_text(&self, url: &str) -> Result<String> {
        let resp = self
            .client
            .get(url)
            .header("PRIVATE-TOKEN", &self.token)
            .header("User-Agent", "ragent/0.1")
            .send()
            .await
            .with_context(|| format!("GitLab GET (raw) {url} failed"))?;

        let status = resp.status();
        if status.as_u16() == 401 {
            bail!(
                "GitLab authentication failed. Run /gitlab setup to update your Personal Access Token."
            );
        }
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            bail!("GitLab API error {status} for {url}: {body}");
        }

        let text = resp
            .text()
            .await
            .with_context(|| format!("Failed to read GitLab raw text for {url}"))?;
        Ok(text)
    }
}

/// Map a GitLab `GET /projects/:id` JSON response onto [`RepoMetadata`]
/// (FR-009).
///
/// GitLab uses `star_count` (GitHub uses `stargazers_count`), exposes `topics`
/// as an array of strings, and reports the default branch via `default_branch`.
/// The `language` field is supplied separately (resolved from the
/// `/languages` endpoint) because GitLab's project JSON does not carry it.
pub(crate) fn gitlab_project_to_metadata(value: &Value, language: &str) -> RepoMetadata {
    let description = value
        .get("description")
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
    let stargazers_count = value.get("star_count").and_then(Value::as_u64).unwrap_or(0);
    let default_branch = value
        .get("default_branch")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    RepoMetadata {
        description,
        language: language.to_string(),
        topics,
        stargazers_count,
        default_branch,
    }
}

/// Select the language with the highest share from a GitLab
/// `GET /projects/:id/languages` response.
///
/// The response is a JSON object mapping language names to a float share, e.g.
/// `{"Rust": 80.5, "Shell": 19.5}`. The entry with the largest share wins. If
/// the response is not an object, is empty, or no share parses as a number,
/// an empty string is returned.
#[must_use]
pub(crate) fn top_language(value: &Value) -> String {
    let obj = match value.as_object() {
        Some(obj) => obj,
        None => return String::new(),
    };
    obj.iter()
        .filter_map(|(lang, share)| share.as_f64().map(|s| (lang.as_str(), s)))
        .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
        .map(|(lang, _)| lang.to_string())
        .unwrap_or_default()
}

/// Parse a GitLab `GET /projects/:id/repository/tree` JSON response into a
/// list of root-level file and directory names (FR-010).
///
/// The GitLab tree endpoint returns a JSON array of objects, each with a
/// `name` string and a `type` field (e.g. `"blob"` for files, `"tree"` for
/// directories). This helper extracts the `name` of every entry, preserving
/// the API's ordering. A non-array response (e.g. an empty-repo error body or
/// a single object) yields an empty vector.
#[must_use]
pub(crate) fn parse_gitlab_tree(value: &Value) -> Vec<String> {
    let arr = match value.as_array() {
        Some(arr) => arr,
        None => return Vec::new(),
    };
    arr.iter()
        .filter_map(|entry| entry.get("name").and_then(Value::as_str).map(String::from))
        .collect()
}

/// Extract the `readme_url` from a GitLab `GET /projects/:id/readme` JSON
/// response (FR-011).
///
/// The README metadata object includes a `readme_url` string pointing at the
/// raw README blob. Returns `None` if the field is absent, null, or not a
/// string.
#[must_use]
pub(crate) fn extract_readme_url(value: &Value) -> Option<String> {
    value
        .get("readme_url")
        .and_then(Value::as_str)
        .map(String::from)
}

/// A parsed entry from a GitLab repository/tree API response.
struct GitLabTreeEntry {
    name: String,
    is_dir: bool,
}

/// Parse a GitLab `GET /projects/:id/repository/tree` JSON response into
/// typed entries with name and type information (FR-027, FR-028).
///
/// Each entry has a `name` and a `type` field (`"blob"` for files, `"tree"`
/// for directories). A non-array response yields an empty vector.
fn parse_gitlab_tree_entries(value: &Value) -> Vec<GitLabTreeEntry> {
    let arr = match value.as_array() {
        Some(arr) => arr,
        None => return Vec::new(),
    };
    arr.iter()
        .filter_map(|entry| {
            let name = entry.get("name").and_then(Value::as_str)?;
            let is_dir = entry
                .get("type")
                .and_then(Value::as_str)
                .is_some_and(|t| t == "tree");
            Some(GitLabTreeEntry {
                name: name.to_string(),
                is_dir,
            })
        })
        .collect()
}

/// URL-encode a GitLab project path (e.g. `group/project` → `group%2Fproject`).
fn urlencoded_path(path: &str) -> String {
    path.replace('/', "%2F")
}

#[cfg(test)]
mod tests {
    use super::{extract_readme_url, gitlab_project_to_metadata, parse_gitlab_tree, top_language};
    use serde_json::json;

    #[test]
    fn test_gitlab_project_all_fields_present() {
        let value = json!({
            "description": "A GitLab project",
            "topics": ["rust", "cli"],
            "star_count": 17,
            "default_branch": "main",
            "name": "my-project",
            "path_with_namespace": "group/my-project",
        });
        let md = gitlab_project_to_metadata(&value, "Rust");
        assert_eq!(md.description, "A GitLab project");
        assert_eq!(md.language, "Rust");
        assert_eq!(md.topics, vec!["rust".to_string(), "cli".to_string()]);
        assert_eq!(md.stargazers_count, 17);
        assert_eq!(md.default_branch, "main");
    }

    #[test]
    fn test_gitlab_project_null_description() {
        let value = json!({
            "description": null,
            "star_count": 0,
            "default_branch": "master",
        });
        let md = gitlab_project_to_metadata(&value, "");
        assert_eq!(md.description, "");
        assert_eq!(md.language, "");
        assert_eq!(md.topics, Vec::<String>::new());
        assert_eq!(md.stargazers_count, 0);
    }

    #[test]
    fn test_gitlab_project_missing_fields_default() {
        let value = json!({});
        let md = gitlab_project_to_metadata(&value, "");
        assert_eq!(md.description, "");
        assert_eq!(md.language, "");
        assert_eq!(md.topics, Vec::<String>::new());
        assert_eq!(md.stargazers_count, 0);
        assert_eq!(md.default_branch, "");
    }

    #[test]
    fn test_gitlab_project_empty_topics_array() {
        let value = json!({
            "description": "desc",
            "topics": [],
            "star_count": 5,
            "default_branch": "develop",
        });
        let md = gitlab_project_to_metadata(&value, "Go");
        assert_eq!(md.topics, Vec::<String>::new());
        assert_eq!(md.language, "Go");
        assert_eq!(md.default_branch, "develop");
    }

    #[test]
    fn test_gitlab_project_topics_with_non_string_entries_filtered() {
        let value = json!({
            "topics": ["valid", 123, true, "also-valid"],
            "star_count": 3,
        });
        let md = gitlab_project_to_metadata(&value, "");
        assert_eq!(
            md.topics,
            vec!["valid".to_string(), "also-valid".to_string()]
        );
    }

    #[test]
    fn test_gitlab_project_star_count_maps_to_stargazers_count() {
        let value = json!({
            "star_count": 999,
        });
        let md = gitlab_project_to_metadata(&value, "");
        assert_eq!(md.stargazers_count, 999);
    }

    #[test]
    fn test_gitlab_project_star_count_as_float_ignored() {
        let value = json!({
            "star_count": 42.5,
        });
        let md = gitlab_project_to_metadata(&value, "");
        assert_eq!(md.stargazers_count, 0);
    }

    #[test]
    fn test_gitlab_project_nested_namespace_metadata() {
        // FR-022: nested namespaces like group/subgroup/project are URL-encoded
        // as a single :id. The metadata mapping does not depend on the namespace
        // shape; this verifies the mapping works for nested projects.
        let value = json!({
            "description": "nested",
            "path_with_namespace": "group/subgroup/project",
            "star_count": 1,
            "default_branch": "main",
            "topics": ["nested"],
        });
        let md = gitlab_project_to_metadata(&value, "Python");
        assert_eq!(md.default_branch, "main");
        assert_eq!(md.topics, vec!["nested".to_string()]);
        assert_eq!(md.language, "Python");
    }

    #[test]
    fn test_top_language_picks_highest_share() {
        let value = json!({
            "Rust": 80.5,
            "Shell": 19.5,
        });
        assert_eq!(top_language(&value), "Rust");
    }

    #[test]
    fn test_top_language_single_language() {
        let value = json!({
            "Go": 100.0,
        });
        assert_eq!(top_language(&value), "Go");
    }

    #[test]
    fn test_top_language_empty_object() {
        let value = json!({});
        assert_eq!(top_language(&value), "");
    }

    #[test]
    fn test_top_language_non_object_response() {
        let value = json!(["Rust", "Go"]);
        assert_eq!(top_language(&value), "");
    }

    #[test]
    fn test_top_language_shares_not_numbers() {
        let value = json!({
            "Rust": "high",
            "Go": "low",
        });
        assert_eq!(top_language(&value), "");
    }

    #[test]
    fn test_top_language_tie_returns_one_of_the_tied() {
        // When shares are equal, the max_by result is implementation-defined
        // but must return one of the tied languages (not empty).
        let value = json!({
            "Rust": 50.0,
            "Go": 50.0,
        });
        let lang = top_language(&value);
        assert!(lang == "Rust" || lang == "Go");
    }

    // --- parse_gitlab_tree -------------------------------------------------

    #[test]
    fn test_parse_gitlab_tree_mixed_blobs_and_trees() {
        let value = json!([
            {"id": "abc", "name": "README.md", "type": "blob"},
            {"id": "def", "name": "src", "type": "tree"},
            {"id": "ghi", "name": "Cargo.toml", "type": "blob"}
        ]);
        let tree = parse_gitlab_tree(&value);
        assert_eq!(
            tree,
            vec![
                "README.md".to_string(),
                "src".to_string(),
                "Cargo.toml".to_string()
            ]
        );
    }

    #[test]
    fn test_parse_gitlab_tree_preserves_api_ordering() {
        let value = json!([
            {"name": "zlib", "type": "tree"},
            {"name": "Apple", "type": "blob"},
            {"name": "apple", "type": "blob"}
        ]);
        let tree = parse_gitlab_tree(&value);
        assert_eq!(
            tree,
            vec!["zlib".to_string(), "Apple".to_string(), "apple".to_string()]
        );
    }

    #[test]
    fn test_parse_gitlab_tree_empty_array() {
        let value = json!([]);
        assert_eq!(parse_gitlab_tree(&value), Vec::<String>::new());
    }

    #[test]
    fn test_parse_gitlab_tree_non_array_yields_empty() {
        let value = json!({"message": "404 Not Found"});
        assert_eq!(parse_gitlab_tree(&value), Vec::<String>::new());
    }

    #[test]
    fn test_parse_gitlab_tree_null_value() {
        let value = serde_json::Value::Null;
        assert_eq!(parse_gitlab_tree(&value), Vec::<String>::new());
    }

    #[test]
    fn test_parse_gitlab_tree_entries_without_name_skipped() {
        let value = json!([
            {"name": "keep.rs", "type": "blob"},
            {"id": "abc", "type": "tree"},
            {"name": "also-keep.txt", "type": "blob"}
        ]);
        let tree = parse_gitlab_tree(&value);
        assert_eq!(
            tree,
            vec!["keep.rs".to_string(), "also-keep.txt".to_string()]
        );
    }

    #[test]
    fn test_parse_gitlab_tree_non_string_name_skipped() {
        let value = json!([
            {"name": 123, "type": "blob"},
            {"name": "valid.rs", "type": "blob"}
        ]);
        let tree = parse_gitlab_tree(&value);
        assert_eq!(tree, vec!["valid.rs".to_string()]);
    }

    #[test]
    fn test_parse_gitlab_tree_extra_fields_ignored() {
        let value = json!([
            {"id": "a1", "name": "src", "type": "tree", "path": "src", "mode": "040000"},
            {"id": "b2", "name": "main.rs", "type": "blob", "path": "main.rs", "mode": "100644"}
        ]);
        let tree = parse_gitlab_tree(&value);
        assert_eq!(tree, vec!["src".to_string(), "main.rs".to_string()]);
    }

    // --- extract_readme_url ------------------------------------------------

    #[test]
    fn test_extract_readme_url_present() {
        let value = json!({
            "readme_url": "https://gitlab.com/group/project/-/raw/main/README.md"
        });
        assert_eq!(
            extract_readme_url(&value),
            Some("https://gitlab.com/group/project/-/raw/main/README.md".to_string())
        );
    }

    #[test]
    fn test_extract_readme_url_missing() {
        let value = json!({"file_path": "README.md"});
        assert_eq!(extract_readme_url(&value), None);
    }

    #[test]
    fn test_extract_readme_url_null() {
        let value = json!({"readme_url": null});
        assert_eq!(extract_readme_url(&value), None);
    }

    #[test]
    fn test_extract_readme_url_non_string() {
        let value = json!({"readme_url": 123});
        assert_eq!(extract_readme_url(&value), None);
    }

    #[test]
    fn test_extract_readme_url_empty_object() {
        let value = json!({});
        assert_eq!(extract_readme_url(&value), None);
    }

    #[test]
    fn test_extract_readme_url_empty_string() {
        let value = json!({"readme_url": ""});
        assert_eq!(extract_readme_url(&value), Some(String::new()));
    }
}
