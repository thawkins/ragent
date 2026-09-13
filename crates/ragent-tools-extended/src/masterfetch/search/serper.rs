//! Serper API-backed search backend for the `mf_search` multi-engine
//! pipeline.
//!
//! This module provides a [`SerperEngine`] that implements the
//! [`SearchEngine`] trait by calling the [Serper Google Search API](https://serper.dev/)
//! at `https://google.serper.dev/search`. An API key is required; the engine
//! is only instantiated when [`ragent_config::Config::serper_api_key`] or the
//! `SERPER_API_KEY` environment variable is present.
//!
//! # Request mapping
//!
//! The [`build_request_body`] helper maps the shared [`SearchOptions`] to
//! the Serper JSON body:
//!
//! - `q` — the search query verbatim, with `site:` and `-site:` operators
//!   appended for the `site` / `exclude_sites` filters.
//! - `num` — `opts.per_engine_results` clamped to 1–100.
//! - `page` — `opts.page + 1` (Serper pages are 1-indexed, default 1).
//! - `tbs` — derived from `opts.freshness` (`qdr:d` / `qdr:w` / `qdr:m` /
//!   `qdr:y`); omitted when freshness is `Any`.
//!
//! # Response parsing
//!
//! The engine parses the JSON response at `organic`, where each item is
//! expected to contain `title`, `link`, and `snippet`. Results are emitted as
//! [`RawResult`]s with `source` set to `"serper"` and no `score`.
//!
//! # Testability
//!
//! Request-body construction and response parsing are pure functions that take
//! plain inputs and produce plain outputs, enabling unit tests without network
//! I/O. The HTTP client is injectable via [`SerperEngine::with_client`] for
//! integration tests with a mock server.

use std::time::Instant;

use serde_json::json;

use super::engine::{
    EngineReport, Freshness, RawResult, SearchEngine, SearchOptions, dedup_results_by_url,
    strip_disallowed_quotes,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The Serper Search API endpoint.
pub const API_URL: &str = "https://google.serper.dev/search";

/// Engine display name.
pub const ENGINE_NAME: &str = "serper";

/// Maximum number of results Serper supports per request (`num`).
pub const MAX_COUNT: usize = 100;

/// Minimum number of results per request (`num`).
pub const MIN_COUNT: usize = 1;

// ---------------------------------------------------------------------------
// Engine struct
// ---------------------------------------------------------------------------

/// Serper API-backed search backend.
///
/// Implements [`SearchEngine`] by sending an authenticated `POST` request
/// to `https://google.serper.dev/search` with the `X-API-KEY` header. The
/// HTTP client is injectable for testing; when `None`, the shared
/// masterfetch client from [`crate::masterfetch::http`] is used.
///
/// # Requirements
///
/// - Plugs into the `SearchEngine` trait alongside the other API-backed
///   backends.
/// - Failures are reported as `engine_blocked` so the other `mf_search`
///   backends can still return results.
/// - The API key is never logged in plain text; `masked_key` exposes only
///   the first two and last two characters.
#[derive(Debug, Clone)]
pub struct SerperEngine {
    /// Serper API key (`X-API-KEY` header).
    api_key: String,
    /// Optional injectable HTTP client (for testing).
    client: Option<reqwest::Client>,
}

impl SerperEngine {
    /// Create a new `SerperEngine` with the given API key and the shared
    /// masterfetch HTTP client.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: None,
        }
    }

    /// Create a new `SerperEngine` with a custom HTTP client (for testing
    /// or custom timeout/redirect configuration).
    #[must_use]
    pub fn with_client(api_key: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            api_key: api_key.into(),
            client: Some(client),
        }
    }

    /// Return the Serper API key, masked for diagnostics.
    ///
    /// Only the first two and last two characters are exposed; the rest are
    /// replaced with `*` characters so the key is never fully surfaced in logs
    /// or error messages.
    #[must_use]
    pub fn masked_key(&self) -> String {
        mask_key(&self.api_key)
    }

    /// Return the HTTP client to use for this engine.
    fn get_client(&self) -> Result<reqwest::Client, String> {
        if let Some(ref c) = self.client {
            return Ok(c.clone());
        }
        crate::masterfetch::http::build_default_client()
            .map_err(|e| format!("failed to build HTTP client: {e}"))
    }

    /// Return a reference to the stored API key (for building the `X-API-KEY`
    /// header). Callers must not log this value.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }
}

impl Default for SerperEngine {
    fn default() -> Self {
        Self::new("")
    }
}

#[async_trait::async_trait]
impl SearchEngine for SerperEngine {
    fn name(&self) -> &str {
        ENGINE_NAME
    }

    /// Execute an authenticated Serper Search API query.
    ///
    /// Sends a `POST` to `https://google.serper.dev/search` with the
    /// `X-API-KEY` header and a JSON body built by [`build_request_body`].
    /// Non-2xx responses are reported as `engine_blocked`.
    async fn search(&self, query: &str, opts: &SearchOptions) -> EngineReport {
        let start = Instant::now();

        if query.trim().is_empty() {
            return EngineReport::error(ENGINE_NAME, "search query must not be empty");
        }

        if self.api_key().is_empty() {
            return EngineReport::blocked(ENGINE_NAME, "missing Serper API key");
        }

        let client = match self.get_client() {
            Ok(c) => c,
            Err(e) => return EngineReport::error(ENGINE_NAME, e),
        };

        let body = build_request_body(query, opts);

        tracing::debug!(
            query = query,
            max_results = opts.max_results,
            url = %API_URL,
            "serper: sending search request"
        );

        let response = match client
            .post(API_URL)
            .header("Content-Type", "application/json")
            .header("X-API-KEY", self.api_key())
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return EngineReport::error(ENGINE_NAME, format!("HTTP request failed: {e}"));
            }
        };

        let status = response.status();

        if !status.is_success() {
            tracing::warn!(status = %status, "serper: API returned error status");
            let report = if status.as_u16() == 429 {
                EngineReport::blocked(ENGINE_NAME, "rate-limited")
            } else if status.as_u16() == 401 || status.as_u16() == 403 {
                EngineReport::blocked(
                    ENGINE_NAME,
                    format!("Serper API auth failed: HTTP {status}"),
                )
            } else {
                EngineReport::blocked(ENGINE_NAME, format!("Serper API returned HTTP {status}"))
            };
            return report;
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

        let mut results = parse_response_json(&value);
        results = dedup_results_by_url(&results);
        results.truncate(opts.max_results);

        let elapsed = start.elapsed().as_millis() as u64;
        let mut report = EngineReport::ok(ENGINE_NAME, results);
        report.duration_ms = elapsed;
        report
    }
}

// ---------------------------------------------------------------------------
// Request builder (pure, testable)
// ---------------------------------------------------------------------------

/// Build the Serper JSON request body from a query and [`SearchOptions`].
///
/// The returned [`serde_json::Value`] contains:
///
/// - `q` — the query with `site:` / `-site:` operators appended for the
///   site filters.
/// - `num` — clamped to 1–100.
/// - `page` — `opts.page + 1` (Serper pages are 1-indexed).
/// - `tbs` — freshness mapping when freshness is not `Any`.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::serper::build_request_body;
/// use ragent_tools_extended::masterfetch::search::engine::SearchOptions;
///
/// let opts = SearchOptions::new(25);
/// let body = build_request_body("rust async", &opts);
/// assert_eq!(body["q"], "rust async");
/// assert_eq!(body["num"], 75);
/// assert_eq!(body["page"], 1);
/// ```
#[must_use]
pub fn build_request_body(query: &str, opts: &SearchOptions) -> serde_json::Value {
    let num = opts.per_engine_results.clamp(MIN_COUNT, MAX_COUNT);
    let mut q = strip_disallowed_quotes(query).trim().to_string();

    if !opts.site.is_empty() {
        q.push_str(&format!(" site:{}", opts.site));
    }
    for domain in &opts.exclude_sites {
        q.push_str(&format!(" -site:{domain}"));
    }

    let mut body = json!({
        "q": q,
        "num": num,
        "page": opts.page.saturating_add(1),
    });

    if let Some(tbs) = freshness_to_tbs(opts.freshness) {
        body["tbs"] = serde_json::Value::String(tbs.to_string());
    }

    body
}

/// Map a [`Freshness`] value to the Serper `tbs` time-filter parameter.
///
/// Returns `None` when freshness is `Any` (no time filter applied). The
/// `tbs` values mirror Google's own time-range codes: `qdr:d` (day),
/// `qdr:w` (week), `qdr:m` (month), `qdr:y` (year).
#[must_use]
pub fn freshness_to_tbs(freshness: Freshness) -> Option<&'static str> {
    match freshness {
        Freshness::Day => Some("qdr:d"),
        Freshness::Week => Some("qdr:w"),
        Freshness::Month => Some("qdr:m"),
        Freshness::Year => Some("qdr:y"),
        Freshness::Any => None,
    }
}

// ---------------------------------------------------------------------------
// Response parsing
// ---------------------------------------------------------------------------

/// Parse a Serper JSON response into [`RawResult`]s.
///
/// Expects the shape `organic`, where each item has `title`, `link`, and
/// `snippet`. Results are returned with `source` set to `"serper"` and no
/// `score`. Snippets are truncated to approximately 200 characters.
#[must_use]
pub fn parse_response_json(value: &serde_json::Value) -> Vec<RawResult> {
    value
        .get("organic")
        .and_then(|r| r.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let title = item.get("title")?.as_str()?.to_string();
                    let url = item.get("link")?.as_str()?.to_string();
                    let snippet = item
                        .get("snippet")
                        .and_then(|s| s.as_str())
                        .unwrap_or_default()
                        .to_string();
                    Some(RawResult::new(
                        title,
                        url,
                        truncate_snippet(&snippet),
                        ENGINE_NAME,
                    ))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Truncate a snippet to approximately 200 characters, respecting UTF-8
/// character boundaries and appending an ellipsis when truncated.
fn truncate_snippet(snippet: &str) -> String {
    if snippet.chars().count() <= 200 {
        snippet.to_string()
    } else {
        let end = snippet
            .char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i <= 200)
            .last()
            .unwrap_or(0);
        format!("{}…", &snippet[..end])
    }
}

// ---------------------------------------------------------------------------
// Key masking helper
// ---------------------------------------------------------------------------

/// Mask a sensitive API key for display.
///
/// Keeps the first two and last two characters; everything in between is
/// replaced with `*`. Strings shorter than six characters are fully masked.
#[must_use]
pub fn mask_key(key: &str) -> String {
    let len = key.chars().count();
    if len <= 6 {
        return "*".repeat(len);
    }
    let first: String = key.chars().take(2).collect();
    let last: String = key
        .chars()
        .rev()
        .take(2)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{first}*{}*{last}", "*".repeat(len.saturating_sub(6)))
}
