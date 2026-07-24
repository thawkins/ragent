//! LangSearch API-backed search backend.
//!
//! Implements **FR-001** and **FR-003** (T-003).
//!
//! This module provides a [`LangSearchEngine`] that implements the
//! [`SearchEngine`] trait by calling the LangSearch Web Search API at
//! `https://api.langsearch.com/v1/web-search`. An API key is required; the
//! engine is only instantiated when [`ragent_config::Config::langsearch_api_key`]
//! is configured.
//!
//! # Request mapping
//!
//! The [`build_request_body`] helper maps the shared [`SearchOptions`] to the
//! LangSearch JSON body:
//!
//! - `query` — the search query verbatim.
//! - `count` — `min(max_results, 10)`, clamped to the 1–10 range supported by
//!   the LangSearch API.
//! - `freshness` — mapped as `Day`→`oneDay`, `Week`→`oneWeek`,
//!   `Month`→`oneMonth`, `Year`→`oneYear`, `Any`→`noLimit`.
//! - `summary` — hard-coded to `true` so the API returns its generated summary
//!   field for each result.
//!
//! # Response parsing
//!
//! The engine parses the JSON response at `data.webPages.value`. Each item is
//! expected to contain `name`, `url`, and either `summary` or `snippet`. Results
//! are emitted as [`RawResult`]s with `source` set to `"langsearch"`.
//!
//! # Testability
//!
//! Request-body construction and response parsing are pure functions that take
//! plain inputs and produce plain outputs, enabling unit tests without network
//! I/O.

use std::time::Instant;

use serde_json::json;

use super::engine::{
    EngineReport, Freshness, RawResult, SearchEngine, SearchOptions, dedup_results_by_url,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The LangSearch Web Search API endpoint.
pub const API_URL: &str = "https://api.langsearch.com/v1/web-search";

/// Engine display name.
pub const ENGINE_NAME: &str = "langsearch";

/// Maximum number of results LangSearch supports per request.
pub const MAX_COUNT: usize = 10;

/// Minimum number of results per request.
pub const MIN_COUNT: usize = 1;

// ---------------------------------------------------------------------------
// Engine struct
// ---------------------------------------------------------------------------

/// LangSearch API-backed search backend.
///
/// Implements [`SearchEngine`] by sending an authenticated `POST` request to
/// `https://api.langsearch.com/v1/web-search`. The HTTP client is injectable
/// for testing; when `None`, the shared masterfetch client from
/// [`crate::masterfetch::http`] is used.
///
/// # Requirements
///
/// - **FR-001** — LangSearch backend that plugs into the `SearchEngine` trait.
/// - **FR-003** — translate `SearchOptions` into the LangSearch JSON body.
/// - **FR-011** — the API key is never logged.
#[derive(Debug, Clone)]
pub struct LangSearchEngine {
    /// LangSearch API key (`Authorization: Bearer {api_key}`).
    api_key: String,
    /// Optional injectable HTTP client (for testing).
    client: Option<reqwest::Client>,
}

impl LangSearchEngine {
    /// Create a new `LangSearchEngine` with the given API key and the shared
    /// masterfetch HTTP client.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: None,
        }
    }

    /// Create a new `LangSearchEngine` with a custom HTTP client (for testing
    /// or custom timeout/redirect configuration).
    #[must_use]
    pub fn with_client(api_key: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            api_key: api_key.into(),
            client: Some(client),
        }
    }

    /// Return the LangSearch API key, masked for diagnostics.
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

    /// Return a reference to the stored API key (for building the `Authorization`
    /// header). Callers must not log this value.
    fn api_key(&self) -> &str {
        &self.api_key
    }
}

impl Default for LangSearchEngine {
    fn default() -> Self {
        Self::new("")
    }
}

#[async_trait::async_trait]
impl SearchEngine for LangSearchEngine {
    fn name(&self) -> &str {
        ENGINE_NAME
    }

    /// Execute an authenticated LangSearch Web Search API query.
    ///
    /// Sends a `POST` to `https://api.langsearch.com/v1/web-search` with the
    /// `Authorization: Bearer {key}` header and a JSON body built by
    /// [`build_request_body`]. Non-2xx responses are reported as
    /// `engine_blocked = true` per FR-005.
    async fn search(&self, query: &str, opts: &SearchOptions) -> EngineReport {
        let start = Instant::now();

        if query.trim().is_empty() {
            return EngineReport::error(ENGINE_NAME, "search query must not be empty");
        }

        if self.api_key().is_empty() {
            return EngineReport::blocked(ENGINE_NAME, "missing LangSearch API key");
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
            "langsearch: sending search request"
        );

        let response = match client
            .post(API_URL)
            .header("Content-Type", "application/json")
            .header("Authorization", format!("Bearer {}", self.api_key()))
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
            tracing::warn!(status = %status, "langsearch: API returned error status");
            return EngineReport::blocked(
                ENGINE_NAME,
                format!("LangSearch API returned HTTP {status}"),
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

/// Build the LangSearch JSON request body from a query and [`SearchOptions`].
///
/// The returned [`serde_json::Value`] contains:
///
/// - `query`
/// - `count` — clamped to 1–10
/// - `freshness` — mapped from [`Freshness`]
/// - `summary` — `true`
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::langsearch::build_request_body;
/// use ragent_tools_extended::masterfetch::search::engine::{Freshness, SearchOptions};
///
/// let opts = SearchOptions::new(25).with_freshness(Freshness::Week);
/// let body = build_request_body("rust async", &opts);
/// assert_eq!(body["query"], "rust async");
/// assert_eq!(body["count"], 10);
/// assert_eq!(body["freshness"], "oneWeek");
/// assert_eq!(body["summary"], true);
/// ```
#[must_use]
pub fn build_request_body(query: &str, opts: &SearchOptions) -> serde_json::Value {
    let count = opts.max_results.clamp(MIN_COUNT, MAX_COUNT);
    json!({
        "query": query.trim(),
        "count": count,
        "freshness": freshness_to_langsearch(opts.freshness),
        "summary": true,
    })
}

/// Map a [`Freshness`] value to the LangSearch `freshness` enum value.
const fn freshness_to_langsearch(freshness: Freshness) -> &'static str {
    match freshness {
        Freshness::Day => "oneDay",
        Freshness::Week => "oneWeek",
        Freshness::Month => "oneMonth",
        Freshness::Year => "oneYear",
        Freshness::Any => "noLimit",
    }
}

// ---------------------------------------------------------------------------
// Response parsing
// ---------------------------------------------------------------------------

/// Parse a LangSearch JSON response into [`RawResult`]s.
///
/// Expects the shape `data.webPages.value`, where each item has `name`, `url`,
/// and either `summary` or `snippet`. Results are returned with `source` set to
/// `"langsearch"` and no `score`.
#[must_use]
pub fn parse_response_json(value: &serde_json::Value) -> Vec<RawResult> {
    value
        .get("data")
        .and_then(|d| d.get("webPages"))
        .and_then(|wp| wp.get("value"))
        .and_then(|v| v.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let title = item.get("name")?.as_str()?.to_string();
                    let url = item.get("url")?.as_str()?.to_string();
                    let snippet = item
                        .get("summary")
                        .and_then(|s| s.as_str())
                        .or_else(|| item.get("snippet").and_then(|s| s.as_str()))
                        .unwrap_or_default()
                        .to_string();
                    Some(RawResult::new(title, url, snippet, ENGINE_NAME))
                })
                .collect()
        })
        .unwrap_or_default()
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
