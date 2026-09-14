//! Exa Search API-backed search backend for the `mf_search` multi-engine
//! pipeline.
//!
//! Implements **FR-001** and **FR-006** for spec `exasearch` (T-003).
//!
//! This module provides an [`ExaEngine`] that implements the
//! [`SearchEngine`] trait by calling the [Exa Search API](https://exa.ai/).
//! An API key is required; the engine is only instantiated when
//! [`ragent_config::Config::exa_api_key`] or the `EXA_API_KEY`
//! environment variable is present.
//!
//! # Request mapping
//!
//! The [`build_request_body`] helper maps the shared [`SearchOptions`] to
//! the Exa JSON body (implemented in T-004):
//!
//! - `query` — the search query verbatim, truncated to Exa's 2000-character
//!   limit.
//! - `numResults` — `opts.per_engine_results` clamped to 1–100.
//! - `includeDomains` — populated from `opts.site` when non-empty.
//! - `excludeDomains` — populated from `opts.exclude_sites` when non-empty.
//! - `startPublishedDate` — derived from `opts.freshness` as an ISO 8601
//!   date.
//! - `contents.highlights` — set to `true` to retrieve relevant excerpts.
//! - `type` — set to `"auto"` (the default search method).
//!
//! # Response parsing
//!
//! The engine parses the JSON response at `results[]` (implemented in T-005).
//! Each item is expected to contain `title`, `url`, `score`, `publishedDate`,
//! `author`, and `highlights[]`. Results are emitted as [`RawResult`]s with
//! `source` set to `"exa"` and snippets built from highlights, truncated to
//! approximately 200 characters.
//!
//! # Testability
//!
//! Request-body construction and response parsing are pure functions that take
//! plain inputs and produce plain outputs, enabling unit tests without network
//! I/O. The HTTP client is injectable via [`ExaEngine::with_client`] for
//! integration tests with a mock server.

use super::engine::{
    EngineReport, RawResult, SearchEngine, SearchOptions, api_engine_preflight, engine_http_client,
    finish_json_search, strip_disallowed_quotes,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The Exa Search API endpoint.
pub const API_URL: &str = "https://api.exa.ai/search";

/// Engine display name.
pub const ENGINE_NAME: &str = "exa";

/// Maximum number of results Exa supports per request (`numResults`).
pub const MAX_COUNT: usize = 100;

/// Minimum number of results per request (`numResults`).
pub const MIN_COUNT: usize = 1;

/// Exa accepts long, semantically rich queries; we cap defensively to 2000
/// characters.
pub const MAX_QUERY_CHARS: usize = 2_000;

/// Maximum snippet length in characters.
pub const MAX_SNIPPET_CHARS: usize = 200;

// ---------------------------------------------------------------------------
// Engine struct
// ---------------------------------------------------------------------------

/// Exa Search API-backed search backend.
///
/// Implements [`SearchEngine`] by sending an authenticated `POST` request
/// to `https://api.exa.ai/search` with the `x-api-key` header. The HTTP
/// client is injectable for testing; when `None`, the shared masterfetch
/// client from [`crate::masterfetch::http`] is used.
///
/// # Requirements
///
/// - **FR-001** — Exa backend that plugs into the `SearchEngine` trait.
/// - **FR-006** — the API key is never logged in plain text; `masked_key`
///   exposes only the first two and last two characters.
#[derive(Debug, Clone)]
pub struct ExaEngine {
    /// Exa API key (`x-api-key` header).
    api_key: String,
    /// Optional injectable HTTP client (for testing).
    client: Option<reqwest::Client>,
}

impl ExaEngine {
    /// Create a new `ExaEngine` with the given API key and the shared
    /// masterfetch HTTP client.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: None,
        }
    }

    /// Create a new `ExaEngine` with a custom HTTP client (for testing or
    /// custom timeout/redirect configuration).
    #[must_use]
    pub fn with_client(api_key: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            api_key: api_key.into(),
            client: Some(client),
        }
    }

    /// Return the Exa API key, masked for diagnostics.
    ///
    /// Only the first two and last two characters are exposed; the rest are
    /// replaced with `*` characters so the key is never fully surfaced in logs
    /// or error messages (FR-006).
    #[must_use]
    pub fn masked_key(&self) -> String {
        mask_key(&self.api_key)
    }

    /// Return the HTTP client to use for this engine.
    fn get_client(&self) -> Result<reqwest::Client, String> {
        engine_http_client(&self.client)
    }

    /// Return a reference to the stored API key (for building the `x-api-key`
    /// header). Callers must not log this value.
    pub fn api_key(&self) -> &str {
        &self.api_key
    }
}

impl Default for ExaEngine {
    fn default() -> Self {
        Self::new("")
    }
}

// ---------------------------------------------------------------------------
// API key masking (FR-006)
// ---------------------------------------------------------------------------

/// Mask a sensitive API key for display.
///
/// Delegates to the shared [`super::engine::mask_api_key`] helper so every
/// backend masks keys identically.
#[must_use]
pub fn mask_key(key: &str) -> String {
    super::engine::mask_api_key(key)
}

// ---------------------------------------------------------------------------
// SearchEngine trait impl (T-006)
// ---------------------------------------------------------------------------

#[async_trait::async_trait]
impl SearchEngine for ExaEngine {
    fn name(&self) -> &str {
        ENGINE_NAME
    }

    /// Execute an authenticated Exa Search API query.
    ///
    /// Sends a `POST` to `https://api.exa.ai/search` with the
    /// `x-api-key` header and a JSON body built by [`build_request_body`].
    /// Non-2xx responses are reported as `engine_blocked` per FR-005.
    async fn search(&self, query: &str, opts: &SearchOptions) -> EngineReport {
        let start = std::time::Instant::now();

        if let Some(report) =
            api_engine_preflight(ENGINE_NAME, query, self.api_key(), "missing Exa API key")
        {
            return report;
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
            "exa: sending search request"
        );

        let response = match client
            .post(API_URL)
            .header("Content-Type", "application/json")
            .header("x-api-key", self.api_key())
            .json(&body)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return EngineReport::error(ENGINE_NAME, format!("HTTP request failed: {e}"));
            }
        };

        finish_json_search(
            ENGINE_NAME,
            start,
            response,
            opts.max_results,
            parse_response_json,
            |status| {
                if status.as_u16() == 429 {
                    "rate-limited".to_string()
                } else if status.as_u16() == 401 || status.as_u16() == 403 {
                    format!("Exa API auth failed: HTTP {status}")
                } else {
                    format!("Exa API returned HTTP {status}")
                }
            },
        )
        .await
    }
}

// ---------------------------------------------------------------------------
// Request builder (pure, testable) — T-004
// ---------------------------------------------------------------------------

/// Build the Exa JSON request body from a query and [`SearchOptions`].
///
/// The returned [`serde_json::Value`] contains:
///
/// - `query` — trimmed to Exa's 2000-character limit.
/// - `numResults` — clamped to 1–100.
/// - `type` — `"auto"` (the default search method).
/// - `includeDomains` — from `opts.site` when non-empty.
/// - `excludeDomains` — from `opts.exclude_sites` when non-empty.
/// - `startPublishedDate` — from `opts.freshness` when not `Any`.
/// - `contents.highlights` — `true` to retrieve relevant excerpts.
#[must_use]
pub fn build_request_body(query: &str, opts: &SearchOptions) -> serde_json::Value {
    use serde_json::json;

    let truncated = truncate_query(&strip_disallowed_quotes(query));
    let num_results = opts.per_engine_results.clamp(MIN_COUNT, MAX_COUNT);

    let mut body = json!({
        "query": truncated,
        "numResults": num_results,
        "type": "auto",
        "contents": {
            "highlights": true
        }
    });

    if !opts.site.is_empty() {
        body["includeDomains"] = json!([opts.site]);
    }

    if !opts.exclude_sites.is_empty() {
        body["excludeDomains"] = serde_json::Value::Array(
            opts.exclude_sites
                .iter()
                .map(|s| serde_json::Value::String(s.clone()))
                .collect(),
        );
    }

    if let Some(start_date) = freshness_to_start_date(opts.freshness) {
        body["startPublishedDate"] = serde_json::Value::String(start_date);
    }

    body
}

/// Truncate a query to the maximum allowed character length.
#[must_use]
pub fn truncate_query(query: &str) -> String {
    super::engine::truncate_query_to(query, MAX_QUERY_CHARS)
}

/// Map a [`Freshness`] filter to an Exa `startPublishedDate` value.
///
/// Returns an ISO 8601 date string (`YYYY-MM-DD`) computed as the current
/// date minus the freshness window, or `None` when freshness is `Any`
/// (no date filter applied).
///
/// This is the Exa equivalent of a "date range" helper — Exa only supports
/// `startPublishedDate` (not an explicit end date), so the range is
/// implicitly `[startPublishedDate, now]`.
#[must_use]
pub fn freshness_to_start_date(freshness: super::engine::Freshness) -> Option<String> {
    use super::engine::Freshness;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    let now_secs = now.as_secs() as i64;

    let offset_secs: Option<i64> = match freshness {
        Freshness::Day => Some(86_400),
        Freshness::Week => Some(604_800),
        Freshness::Month => Some(2_592_000),
        Freshness::Year => Some(31_536_000),
        Freshness::Any => None,
    };

    offset_secs.map(|secs| {
        let past = now_secs - secs;
        date_string(past)
    })
}

/// Format a Unix timestamp (seconds) as an ISO 8601 date string (`YYYY-MM-DD`).
#[must_use]
pub fn date_string(secs: i64) -> String {
    // Days since epoch
    let days = secs.div_euclid(86_400);
    // Civil-from-days algorithm (Howard Hinnant)
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if m <= 2 { y + 1 } else { y };
    format!("{year:04}-{m:02}-{d:02}")
}

// ---------------------------------------------------------------------------
// Response parser (pure, testable) — T-005
// ---------------------------------------------------------------------------

/// Parse the Exa JSON response into a list of [`RawResult`]s.
///
/// Expects the response to contain a `results` array where each item has
/// `title`, `url`, `score`, `publishedDate`, `author`, and `highlights[]`.
/// Results are emitted with `source` set to `"exa"` and snippets built from
/// highlights joined by ` … `, truncated to ~200 characters. If no highlights
/// are present, the snippet is built from `publishedDate` and `author`
/// metadata.
#[must_use]
pub fn parse_response_json(value: &serde_json::Value) -> Vec<RawResult> {
    let results = match value.get("results").and_then(|r| r.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    results
        .iter()
        .map(|item| {
            let title = item
                .get("title")
                .and_then(|t| t.as_str())
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| {
                    item.get("url")
                        .and_then(|u| u.as_str())
                        .unwrap_or("(untitled)")
                })
                .to_string();

            let url = item
                .get("url")
                .and_then(|u| u.as_str())
                .unwrap_or("")
                .to_string();

            let snippet = build_snippet(item);

            let score = item
                .get("score")
                .and_then(|s| s.as_f64())
                .map(|s| s.clamp(0.0, 1.0));

            let author = item
                .get("author")
                .and_then(|a| a.as_str())
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from);

            RawResult {
                title,
                url,
                snippet,
                source: ENGINE_NAME.to_string(),
                score,
                author,
            }
        })
        .filter(|r| !r.url.is_empty())
        .collect()
}

/// Build a snippet from Exa result highlights, falling back to metadata.
fn build_snippet(item: &serde_json::Value) -> String {
    if let Some(highlights) = item.get("highlights").and_then(|h| h.as_array()) {
        let joined: Vec<String> = highlights
            .iter()
            .filter_map(|h| h.as_str().map(String::from))
            .collect();
        if !joined.is_empty() {
            return truncate_snippet(&joined.join(" … "));
        }
    }

    // Fallback: build from publishedDate and author
    let date = item
        .get("publishedDate")
        .and_then(|d| d.as_str())
        .unwrap_or("");
    let author = item.get("author").and_then(|a| a.as_str()).unwrap_or("");

    let mut parts: Vec<&str> = Vec::new();
    if !date.is_empty() {
        parts.push("Published:");
        parts.push(date);
    }
    if !author.is_empty() {
        parts.push("by");
        parts.push(author);
    }

    if parts.is_empty() {
        return String::new();
    }

    truncate_snippet(&parts.join(" "))
}

/// Truncate a snippet to [`MAX_SNIPPET_CHARS`] characters, respecting UTF-8
/// character boundaries and appending an ellipsis when truncated.
#[must_use]
pub fn truncate_snippet(snippet: &str) -> String {
    super::engine::truncate_snippet(snippet)
}
