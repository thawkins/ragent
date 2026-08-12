//! Wikipedia REST API search backend for the `mf_search` multi-engine pipeline.
//!
//! Implements **FR-002**, **FR-009**, **NFR-001**, and **NFR-004** for spec
//! `wikisearch` (T-001).
//!
//! This module provides a [`WikipediaEngine`] that implements the
//! [`SearchEngine`] trait by calling the [Wikipedia REST API](https://en.wikipedia.org/api/rest_v1/)
//! page/summary endpoint. Wikipedia is a free, open encyclopedia and the REST
//! API is **unauthenticated** — no API key is required. The backend is
//! therefore keyless, like `DuckDuckGo`, Brave, and `OpenAlex`.
//!
//! # Two-step query flow
//!
//! The page/summary endpoint takes a *page title* (not a free-text query),
//! so the engine resolves the user's query to candidate page titles before
//! fetching summaries:
//!
//! 1. **Title resolution** — a `GET` to the MediaWiki Action API
//!    (`https://en.wikipedia.org/w/api.php?action=query&list=search&…`)
//!    returns the top-N matching page titles for the query.
//! 2. **Summary fetch** — for each resolved title, a `GET` to
//!    `https://en.wikipedia.org/api/rest_v1/page/summary/{title}` returns the
//!    page summary JSON (`title`, `extract`, `content_urls.desktop.page`,
//!    optional `description`, optional `thumbnail.source`).
//!
//! Summaries are fetched concurrently so the backend stays well within the
//! shared masterfetch HTTP timeout (NFR-001).
//!
//! # User-Agent requirement
//!
//! Wikipedia's robot policy rejects requests without a descriptive
//! `User-Agent` header with HTTP 403. The engine sets
//! `ragent/{version} (masterfetch)` on every outbound request (FR-007,
//! implemented in T-007).
//!
//! # Error handling
//!
//! The engine never returns `Err` for engine-level failures (FR-006,
//! FR-008). HTTP 429 (rate limited) is reported as
//! [`EngineReport::blocked`] with `"rate-limited"`; HTTP 403 (blocked /
//! missing user-agent) is reported as [`EngineReport::blocked`] with
//! `"blocked"`. Other non-2xx responses and unparseable bodies are reported
//! as [`EngineReport::error`] or [`EngineReport::blocked`] so the remaining
//! `mf_search` backends continue to return results (FR-006).
//!
//! # Testability
//!
//! Request construction and response parsing are implemented as pure
//! functions that take plain inputs and produce plain outputs, enabling
//! unit tests without network I/O (NFR-002, T-011). The HTTP client is
//! injectable via [`WikipediaEngine::with_client`] for integration tests.

use std::time::Instant;

use super::engine::{EngineReport, RawResult, SearchEngine, SearchOptions, dedup_results_by_url};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The MediaWiki Action API base URL for the `list=search` title-resolution
/// step.
pub const SEARCH_API_URL: &str = "https://en.wikipedia.org/w/api.php";

/// The Wikipedia REST API base URL for the page/summary endpoint.
pub const SUMMARY_API_URL: &str = "https://en.wikipedia.org/api/rest_v1/page/summary";

/// Engine display name.
pub const ENGINE_NAME: &str = "wikipedia";

/// Maximum number of candidate titles to request from the Action API
/// `list=search` (the `srlimit` parameter is capped at 500 by MediaWiki).
pub const MAX_SEARCH_LIMIT: usize = 500;

/// Minimum number of candidate titles to request.
pub const MIN_SEARCH_LIMIT: usize = 1;

/// Maximum snippet length in characters (FR-009).
pub const MAX_SNIPPETTE_CHARS: usize = 300;

/// Maximum query length in characters (defensive cap to avoid long URLs).
pub const MAX_QUERY_CHARS: usize = 1_000;

// ---------------------------------------------------------------------------
// Engine struct (FR-002)
// ---------------------------------------------------------------------------

/// Wikipedia REST API search backend.
///
/// Implements [`SearchEngine`] using a two-step flow: resolve the query to
/// candidate page titles via the MediaWiki Action API, then fetch a
/// page/summary for each title via the Wikipedia REST API. No API key is
/// required — the backend is keyless and always-on (FR-003).
///
/// The HTTP client is injectable for testing; when `None`, the shared
/// masterfetch client from [`crate::masterfetch::http`] is used.
///
/// # Requirements
///
/// - **FR-002** — Wikipedia backend that plugs into the `SearchEngine` trait.
/// - **FR-009** — result fields (`source`, `url`, `snippet`) populated per
///   spec.
/// - **NFR-001** — uses the shared HTTP client timeout.
/// - **NFR-004** — no `unsafe` code, no `.unwrap()` on user-facing paths.
#[derive(Debug, Clone)]
pub struct WikipediaEngine {
    /// Optional injectable HTTP client (for testing).
    client: Option<reqwest::Client>,
}

impl WikipediaEngine {
    /// Create a new `WikipediaEngine` with the shared masterfetch HTTP client.
    #[must_use]
    pub fn new() -> Self {
        Self { client: None }
    }

    /// Create a new `WikipediaEngine` with a custom HTTP client (for testing
    /// or custom timeout/redirect configuration).
    #[must_use]
    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            client: Some(client),
        }
    }

    /// Return the HTTP client to use for this engine.
    fn get_client(&self) -> Result<reqwest::Client, String> {
        if let Some(ref c) = self.client {
            return Ok(c.clone());
        }
        crate::masterfetch::http::build_default_client()
            .map_err(|e| format!("failed to build HTTP client: {e}"))
    }
}

impl Default for WikipediaEngine {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SearchEngine implementation (FR-002, FR-009)
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl SearchEngine for WikipediaEngine {
    fn name(&self) -> &str {
        ENGINE_NAME
    }

    /// Execute a Wikipedia search query.
    ///
    /// This implements the two-step flow: resolve candidate titles via the
    /// Action API, then fetch a page/summary for each title. HTTP 429 is
    /// reported as a blocked report with `"rate-limited"` (FR-008); HTTP 403
    /// is reported as blocked with `"blocked"` (FR-007). Other non-2xx
    /// responses are reported as blocked or errored reports so the remaining
    /// `mf_search` backends continue to return results (FR-006).
    async fn search(&self, query: &str, opts: &SearchOptions) -> EngineReport {
        let start = Instant::now();

        if query.trim().is_empty() {
            return EngineReport::error(ENGINE_NAME, "search query must not be empty");
        }

        let client = match self.get_client() {
            Ok(c) => c,
            Err(e) => return EngineReport::error(ENGINE_NAME, e),
        };

        // Step 1: resolve candidate titles via the Action API.
        let (search_url, search_params) = build_search_request(query, opts);

        tracing::debug!(
            query = query,
            max_results = opts.max_results,
            url = %search_url,
            "wikipedia: sending title-resolution request"
        );

        let mut req = client.get(&search_url);
        if !search_params.is_empty() {
            req = req.query(&search_params);
        }

        let response = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                return EngineReport::error(ENGINE_NAME, format!("HTTP request failed: {e}"));
            }
        };

        let status = response.status();

        if status.as_u16() == 429 {
            tracing::warn!(status = %status, "wikipedia: rate-limited");
            return EngineReport::blocked(ENGINE_NAME, "rate-limited");
        }

        if status.as_u16() == 403 {
            tracing::warn!(status = %status, "wikipedia: blocked (missing user-agent)");
            return EngineReport::blocked(ENGINE_NAME, "blocked");
        }

        if !status.is_success() {
            tracing::warn!(status = %status, "wikipedia: API returned error status");
            return EngineReport::blocked(
                ENGINE_NAME,
                format!("Wikipedia API returned HTTP {status}"),
            );
        }

        let text = match response.text().await {
            Ok(t) => t,
            Err(e) => {
                return EngineReport::error(
                    ENGINE_NAME,
                    format!("failed to read response body: {e}"),
                );
            }
        };

        let value: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(e) => {
                return EngineReport::error(
                    ENGINE_NAME,
                    format!("failed to parse response JSON: {e}"),
                );
            }
        };

        let titles = parse_search_response(&value);

        // Apply the site filter (FR-004): restrict to titles whose article
        // URL host matches the requested domain.
        let titles = apply_site_filter(titles, opts);

        if titles.is_empty() {
            let elapsed = start.elapsed().as_millis() as u64;
            let mut report = EngineReport::ok(ENGINE_NAME, Vec::new());
            report.duration_ms = elapsed;
            return report;
        }

        // Step 2: fetch a page/summary for each resolved title concurrently.
        let limit = opts.max_results.min(titles.len());
        let titles_to_fetch = &titles[..limit];

        tracing::debug!(
            count = titles_to_fetch.len(),
            "wikipedia: fetching page summaries"
        );

        let summary_futures: Vec<_> = titles_to_fetch
            .iter()
            .map(|title| fetch_summary(&client, title))
            .collect();
        let summary_results = futures::future::join_all(summary_futures).await;

        let mut results: Vec<RawResult> = summary_results.into_iter().flatten().collect();

        // Dedup by normalised URL and truncate to max_results (FR-009).
        results = dedup_results_by_url(&results);
        results.truncate(opts.max_results);

        let elapsed = start.elapsed().as_millis() as u64;
        let mut report = EngineReport::ok(ENGINE_NAME, results);
        report.duration_ms = elapsed;
        report
    }
}

// ---------------------------------------------------------------------------
// Title resolution: request builder + response parser (pure, testable)
// ---------------------------------------------------------------------------

/// Build the MediaWiki Action API request URL and query parameters for the
/// title-resolution step.
///
/// Maps the shared [`SearchOptions`] to the Action API `list=search`
/// parameters:
///
/// - `action` — `query`
/// - `list` — `search`
/// - `srsearch` — the trimmed query, capped at [`MAX_QUERY_CHARS`] chars.
/// - `srlimit` — `opts.max_results` clamped to 1–500 (FR-005).
/// - `srprop` — `snippet` (so the search step returns a snippet for
///   fallback).
/// - `format` — `json`
/// - `origin` — `*` (CORS header for cross-origin requests).
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::wikipedia::build_search_request;
/// use ragent_tools_extended::masterfetch::search::engine::SearchOptions;
///
/// let opts = SearchOptions::new(5);
/// let (url, params) = build_search_request("rust programming", &opts);
/// assert_eq!(url, "https://en.wikipedia.org/w/api.php");
/// let map: std::collections::HashMap<&str, &str> = params
///     .iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
/// assert_eq!(map["action"], "query");
/// assert_eq!(map["list"], "search");
/// assert_eq!(map["srsearch"], "rust programming");
/// assert_eq!(map["srlimit"], "5");
/// assert_eq!(map["format"], "json");
/// ```
#[must_use]
pub fn build_search_request(query: &str, opts: &SearchOptions) -> (String, Vec<(String, String)>) {
    let limit = opts.max_results.clamp(MIN_SEARCH_LIMIT, MAX_SEARCH_LIMIT);
    let search = truncate_query(query);

    let params: Vec<(String, String)> = vec![
        ("action".to_string(), "query".to_string()),
        ("list".to_string(), "search".to_string()),
        ("srsearch".to_string(), search),
        ("srlimit".to_string(), limit.to_string()),
        ("srprop".to_string(), "snippet".to_string()),
        ("format".to_string(), "json".to_string()),
        ("origin".to_string(), "*".to_string()),
    ];

    (SEARCH_API_URL.to_string(), params)
}

/// Parse the MediaWiki Action API `list=search` JSON response into a list of
/// candidate page titles.
///
/// Expects the shape `{ "query": { "search": [ { "title": "…" }, … ] } }`.
/// Entries without a `title` field are skipped.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::wikipedia::parse_search_response;
/// use serde_json::json;
///
/// let value = json!({
///     "query": {
///         "search": [
///             { "title": "Rust (programming language)" },
///             { "title": "Rust" },
///             { "ns": 0 }
///         ]
///     }
/// });
/// let titles = parse_search_response(&value);
/// assert_eq!(titles, vec!["Rust (programming language)", "Rust"]);
/// ```
#[must_use]
pub fn parse_search_response(value: &serde_json::Value) -> Vec<String> {
    let search = match value
        .get("query")
        .and_then(|q| q.get("search"))
        .and_then(|s| s.as_array())
    {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    search
        .iter()
        .filter_map(|entry| {
            entry
                .get("title")
                .and_then(|t| t.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Summary fetch: URL builder + response parser (pure, testable)
// ---------------------------------------------------------------------------

/// Build the Wikipedia REST API page/summary URL for a resolved title.
///
/// The title is URL-encoded so spaces, parentheses, and other special
/// characters are safe in the path.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::wikipedia::build_summary_url;
///
/// let url = build_summary_url("Rust (programming language)");
/// assert_eq!(url, "https://en.wikipedia.org/api/rest_v1/page/summary/Rust%20%28programming%20language%29");
///
/// let url2 = build_summary_url("Photosynthesis");
/// assert_eq!(url2, "https://en.wikipedia.org/api/rest_v1/page/summary/Photosynthesis");
/// ```
#[must_use]
pub fn build_summary_url(title: &str) -> String {
    let encoded = url_encode_path(title);
    format!("{SUMMARY_API_URL}/{encoded}")
}

/// Parse one Wikipedia page/summary JSON response into a [`RawResult`].
///
/// Expects the shape returned by `GET /page/summary/{title}`:
/// ```json
/// {
///   "title": "…",
///   "extract": "…",
///   "description": "…",
///   "content_urls": { "desktop": { "page": "https://…" } },
///   "thumbnail": { "source": "https://…" }
/// }
/// ```
///
/// Returns `None` when the response is missing both `title` and `extract`.
/// Sets `source` to `"wikipedia"` (FR-009), `url` to
/// `content_urls.desktop.page` (falling back to a constructed article URL
/// from the title), and `snippet` to the `extract` prepended with the
/// `description` when present (FR-010), truncated to ~300 chars (FR-009).
/// The thumbnail URL is appended to the snippet when present (FR-011).
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::wikipedia::parse_summary_response;
/// use serde_json::json;
///
/// let value = json!({
///     "title": "Rust (programming language)",
///     "extract": "Rust is a general-purpose programming language.",
///     "description": "General-purpose programming language",
///     "content_urls": {
///         "desktop": { "page": "https://en.wikipedia.org/wiki/Rust_(programming_language)" }
///     }
/// });
/// let result = parse_summary_response(&value).unwrap();
/// assert_eq!(result.source, "wikipedia");
/// assert_eq!(result.title, "Rust (programming language)");
/// assert_eq!(result.url, "https://en.wikipedia.org/wiki/Rust_(programming_language)");
/// assert!(result.snippet.contains("Rust is a general-purpose"));
/// assert!(result.snippet.contains("General-purpose"));
/// ```
#[must_use]
pub fn parse_summary_response(value: &serde_json::Value) -> Option<RawResult> {
    let title = value
        .get("title")
        .and_then(|t| t.as_str())
        .unwrap_or_default()
        .to_string();

    let extract = value
        .get("extract")
        .and_then(|e| e.as_str())
        .unwrap_or_default()
        .to_string();

    if title.is_empty() && extract.is_empty() {
        return None;
    }

    // URL: content_urls.desktop.page → constructed article URL (FR-009).
    let url = value
        .get("content_urls")
        .and_then(|cu| cu.get("desktop"))
        .and_then(|d| d.get("page"))
        .and_then(|p| p.as_str())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .unwrap_or_else(|| build_article_url(&title));

    // snippet: description + extract (FR-010), truncated to ~300 chars
    // (FR-009).
    let mut snippet = build_snippet(value, &extract);

    // Append thumbnail URL when present (FR-011).
    if let Some(thumb) = value
        .get("thumbnail")
        .and_then(|t| t.get("source"))
        .and_then(|s| s.as_str())
        .filter(|s| !s.is_empty())
    {
        if !snippet.is_empty() {
            snippet.push(' ');
        }
        snippet.push_str(&format!("[thumbnail: {thumb}]"));
    }

    Some(RawResult::new(title, url, snippet, ENGINE_NAME))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Construct the canonical Wikipedia article URL from a title.
fn build_article_url(title: &str) -> String {
    let encoded = url_encode_path(title);
    format!("https://en.wikipedia.org/wiki/{encoded}")
}

/// Build the snippet for a Wikipedia summary: prepend the short description
/// when present (FR-010), then append the extract, truncated to ~300 chars
/// (FR-009).
fn build_snippet(value: &serde_json::Value, extract: &str) -> String {
    let description = value
        .get("description")
        .and_then(|d| d.as_str())
        .unwrap_or_default();

    let mut parts: Vec<&str> = Vec::new();
    if !description.is_empty() {
        parts.push(description);
    }
    if !extract.is_empty() {
        parts.push(extract);
    }

    let joined = parts.join(" — ");
    truncate_snippet(&joined)
}

/// Truncate a snippet to approximately [`MAX_SNIPPETTE_CHARS`] characters,
/// respecting UTF-8 character boundaries and appending an ellipsis when
/// truncated.
fn truncate_snippet(snippet: &str) -> String {
    if snippet.chars().count() <= MAX_SNIPPETTE_CHARS {
        return snippet.to_string();
    }
    let end = snippet
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= MAX_SNIPPETTE_CHARS)
        .last()
        .unwrap_or(0);
    format!("{}…", &snippet[..end])
}

/// Truncate a search query to [`MAX_QUERY_CHARS`] characters, respecting
/// UTF-8 character boundaries.
#[must_use]
pub fn truncate_query(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.chars().count() <= MAX_QUERY_CHARS {
        return trimmed.to_string();
    }
    trimmed.chars().take(MAX_QUERY_CHARS).collect()
}

/// Apply the `site` filter to resolved titles (FR-004).
///
/// When `opts.site` is non-empty, only titles whose constructed article URL
/// host matches the requested domain are retained. When no `site` filter is
/// present, all titles are returned unchanged.
fn apply_site_filter(titles: Vec<String>, opts: &SearchOptions) -> Vec<String> {
    let site = opts.site.trim();
    if site.is_empty() {
        return titles;
    }
    let site_lower = site.to_ascii_lowercase();
    titles
        .into_iter()
        .filter(|title| {
            let url = build_article_url(title);
            url_host_matches(&url, &site_lower)
        })
        .collect()
}

/// Check whether the host of `url` matches `domain` (case-insensitive).
fn url_host_matches(url: &str, domain: &str) -> bool {
    // Simple host extraction: strip scheme, take up to first '/'.
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let host = after_scheme.split('/').next().unwrap_or(after_scheme);
    let host_lower = host.to_ascii_lowercase();
    // Match if the host equals the domain or is a subdomain of it.
    host_lower == domain || host_lower.ends_with(&format!(".{domain}"))
}

/// URL-encode a string for safe use in a URL path segment.
///
/// Encodes everything except unreserved characters (`A–Z`, `a–z`, `0–9`,
/// `-`, `_`, `.`, `~`). Spaces become `%20` (not `+`) for path segments.
fn url_encode_path(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            _ => {
                out.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    out
}

/// Fetch a page/summary for a single title.
///
/// Returns `None` on any error (network, non-2xx, parse failure) so one
/// failed summary does not discard the others.
async fn fetch_summary(client: &reqwest::Client, title: &str) -> Option<RawResult> {
    let url = build_summary_url(title);

    tracing::trace!(url = %url, "wikipedia: fetching summary");

    let response = client.get(&url).send().await.ok()?;

    let status = response.status();
    if !status.is_success() {
        tracing::debug!(status = %status, url = %url, "wikipedia: summary fetch failed");
        return None;
    }

    let text = response.text().await.ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    parse_summary_response(&value)
}
