//! Tavily API-backed search backend for the `mf_search` multi-engine
//! pipeline.
//!
//! Implements **FR-001**, **FR-003**, **FR-004**, **FR-005**, and
//! **FR-012** for spec `tavmove` (T-001).
//!
//! This module provides a [`TavilyEngine`] that implements the
//! [`SearchEngine`] trait by calling the [Tavily Search API](https://tavily.com/).
//! An API key is required; the engine is only instantiated when
//! [`ragent_config::Config::tavily_api_key`] or the `TAVILY_API_KEY`
//! environment variable is present.
//!
//! # Request mapping
//!
//! The [`build_request_body`] helper maps the shared [`SearchOptions`] to
//! the Tavily JSON body:
//!
//! - `query` — the search query verbatim, truncated to Tavily's 400-character
//!   limit.
//! - `max_results` — `min(opts.max_results, 20)`, clamped to Tavily's
//!   documented maximum of 20.
//! - `include_answer` — `false`, matching the legacy `websearch` tool
//!   behaviour and avoiding extra answer text in the response.
//! - `search_depth` — `"basic"`, matching the legacy `websearch` tool
//!   behaviour.
//!
//! # Response parsing
//!
//! The engine parses the JSON response at `results`, where each item is
//! expected to contain `title`, `url`, and `content`. Results are emitted as
//! [`RawResult`]s with `source` set to `"tavily"` and snippets truncated to
//! approximately 200 characters.
//!
//! # Testability
//!
//! Request-body construction and response parsing are pure functions that take
//! plain inputs and produce plain outputs, enabling unit tests without network
//! I/O. The HTTP client is injectable via [`TavilyEngine::with_client`] for
//! integration tests with a mock server.

use std::time::Instant;

use serde_json::json;

use super::engine::{EngineReport, RawResult, SearchEngine, SearchOptions, dedup_results_by_url};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The Tavily Search API endpoint.
pub const API_URL: &str = "https://api.tavily.com/search";

/// Engine display name.
pub const ENGINE_NAME: &str = "tavily";

/// Maximum number of results Tavily supports per request.
pub const MAX_COUNT: usize = 20;

/// Minimum number of results per request.
pub const MIN_COUNT: usize = 1;

/// Tavily rejects queries longer than 400 characters.
pub const MAX_QUERY_CHARS: usize = 400;

// ---------------------------------------------------------------------------
// Engine struct
// ---------------------------------------------------------------------------

/// Tavily API-backed search backend.
///
/// Implements [`SearchEngine`] by sending an authenticated `POST` request
/// to `https://api.tavily.com/search`. The HTTP client is injectable for
/// testing; when `None`, the shared masterfetch client from
/// [`crate::masterfetch::http`] is used.
///
/// # Requirements
///
/// - **FR-001** — Tavily backend that plugs into the `SearchEngine` trait.
/// - **FR-003** — translate `SearchOptions` into the Tavily JSON body.
/// - **FR-005** — failures are reported as `engine_blocked` so the other
///   `mf_search` backends can still return results.
/// - **FR-012** — the API key is never logged in plain text.
#[derive(Debug, Clone)]
pub struct TavilyEngine {
    /// Tavily API key (`Authorization: Bearer {api_key}`).
    api_key: String,
    /// Optional injectable HTTP client (for testing).
    client: Option<reqwest::Client>,
}

impl TavilyEngine {
    /// Create a new `TavilyEngine` with the given API key and the shared
    /// masterfetch HTTP client.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: None,
        }
    }

    /// Create a new `TavilyEngine` with a custom HTTP client (for testing or
    /// custom timeout/redirect configuration).
    #[must_use]
    pub fn with_client(api_key: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            api_key: api_key.into(),
            client: Some(client),
        }
    }

    /// Return the Tavily API key, masked for diagnostics.
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

impl Default for TavilyEngine {
    fn default() -> Self {
        Self::new("")
    }
}

#[async_trait::async_trait]
impl SearchEngine for TavilyEngine {
    fn name(&self) -> &str {
        ENGINE_NAME
    }

    /// Execute an authenticated Tavily Search API query.
    ///
    /// Sends a `POST` to `https://api.tavily.com/search` with the
    /// `Authorization: Bearer {key}` header and a JSON body built by
    /// [`build_request_body`]. Non-2xx responses are reported as
    /// `engine_blocked = true` per FR-005.
    async fn search(&self, query: &str, opts: &SearchOptions) -> EngineReport {
        let start = Instant::now();

        if query.trim().is_empty() {
            return EngineReport::error(ENGINE_NAME, "search query must not be empty");
        }

        if self.api_key().is_empty() {
            return EngineReport::blocked(ENGINE_NAME, "missing Tavily API key");
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
            "tavily: sending search request"
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
            tracing::warn!(status = %status, "tavily: API returned error status");
            return EngineReport::blocked(
                ENGINE_NAME,
                format!("Tavily API returned HTTP {status}"),
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

/// Build the Tavily JSON request body from a query and [`SearchOptions`].
///
/// The returned [`serde_json::Value`] contains:
///
/// - `query` — trimmed to Tavily's 400-character limit.
/// - `max_results` — clamped to 1–20.
/// - `include_answer` — `false`.
/// - `search_depth` — `"basic"`.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::tavily::build_request_body;
/// use ragent_tools_extended::masterfetch::search::engine::SearchOptions;
///
/// let opts = SearchOptions::new(25);
/// let body = build_request_body("rust async", &opts);
/// assert_eq!(body["query"], "rust async");
/// assert_eq!(body["max_results"], 20);
/// assert_eq!(body["include_answer"], false);
/// assert_eq!(body["search_depth"], "basic");
/// ```
#[must_use]
pub fn build_request_body(query: &str, opts: &SearchOptions) -> serde_json::Value {
    let max_results = opts.max_results.clamp(MIN_COUNT, MAX_COUNT);
    json!({
        "query": truncate_query(query),
        "max_results": max_results,
        "include_answer": false,
        "search_depth": "basic",
    })
}

/// Truncate a search query to Tavily's maximum accepted length, respecting
/// UTF-8 character boundaries.
#[must_use]
pub fn truncate_query(query: &str) -> String {
    if query.chars().count() <= MAX_QUERY_CHARS {
        query.to_string()
    } else {
        query.chars().take(MAX_QUERY_CHARS).collect()
    }
}

// ---------------------------------------------------------------------------
// Response parsing
// ---------------------------------------------------------------------------

/// Parse a Tavily JSON response into [`RawResult`]s.
///
/// Expects the shape `results`, where each item has `title`, `url`, and
/// `content`. Results are returned with `source` set to `"tavily"` and no
/// `score`. Snippets are truncated to approximately 200 characters.
#[must_use]
pub fn parse_response_json(value: &serde_json::Value) -> Vec<RawResult> {
    value
        .get("results")
        .and_then(|r| r.as_array())
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let title = item.get("title")?.as_str()?.to_string();
                    let url = item.get("url")?.as_str()?.to_string();
                    let content = item
                        .get("content")
                        .and_then(|s| s.as_str())
                        .unwrap_or_default()
                        .to_string();
                    Some(RawResult::new(
                        title,
                        url,
                        truncate_snippet(&content),
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

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_engine_has_empty_key() {
        let engine = TavilyEngine::default();
        assert!(engine.api_key().is_empty());
    }

    #[test]
    fn test_engine_name_is_tavily() {
        let engine = TavilyEngine::new("test");
        assert_eq!(engine.name(), "tavily");
    }
}
