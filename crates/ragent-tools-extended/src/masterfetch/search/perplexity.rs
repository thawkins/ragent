//! Perplexity Sonar API-backed search backend for the `mf_search` multi-engine
//! pipeline.
//!
//! This module provides a [`PerplexityEngine`] that implements the
//! [`SearchEngine`] trait by calling the [Perplexity Sonar API](https://docs.perplexity.ai/).
//! An API key is required; the engine is only instantiated when
//! [`ragent_config::Config::perplexity_api_key`] or the `PERPLEXITY_API_KEY`
//! environment variable is present.
//!
//! # Request mapping
//!
//! The [`build_request_body`] helper maps the shared [`SearchOptions`] to
//! the Perplexity Sonar JSON body (OpenAI chat-completions compatible):
//!
//! - `model` — `"sonar"`, the base Sonar model for web-grounded answers.
//! - `messages` — a single user message containing the search query verbatim,
//!   truncated to the 4000-character limit.
//! - `max_tokens` — `min(opts.max_results * 200, 4000)` as a rough budget; the
//!   Sonar API does not accept a `max_results` field directly.
//! - `search_recency_filter` — mapped from [`Freshness`] as `Day`→`day`,
//!   `Week`→`week`, `Month`→`month`, `Year`→`hour` (Perplexity has no "year"
//!   option so the broadest available filter is used), `Any`→omitted.
//!
//! # Response parsing
//!
//! The engine parses the JSON response at `search_results`, where each item is
//! expected to contain `title`, `url`, and `snippet` (or `text`). Results are
//! emitted as [`RawResult`]s with `source` set to `"perplexity"` and snippets
//! truncated to approximately 200 characters. If `search_results` is absent
//! the engine falls back to the `citations` array (URLs only, empty
//! titles/snippets).
//!
//! # Testability
//!
//! Request-body construction and response parsing are pure functions that take
//! plain inputs and produce plain outputs, enabling unit tests without network
//! I/O. The HTTP client is injectable via [`PerplexityEngine::with_client`] for
//! integration tests with a mock server.

use std::time::Instant;

use serde_json::json;

use super::engine::{
    EngineReport, Freshness, RawResult, SearchEngine, SearchOptions, dedup_results_by_url,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The Perplexity Sonar API endpoint (OpenAI chat-completions compatible).
pub const API_URL: &str = "https://api.perplexity.ai/chat/completions";

/// Engine display name.
pub const ENGINE_NAME: &str = "perplexity";

/// The default Sonar model used for web-grounded search.
pub const DEFAULT_MODEL: &str = "sonar";

/// Perplexity rejects very long queries; we truncate to this many characters.
pub const MAX_QUERY_CHARS: usize = 4000;

/// Rough token budget per requested result.
const TOKENS_PER_RESULT: usize = 200;

/// Maximum token budget for a single request.
const MAX_TOKEN_BUDGET: usize = 4000;

// ---------------------------------------------------------------------------
// Engine struct
// ---------------------------------------------------------------------------

/// Perplexity Sonar API-backed search backend.
///
/// Implements [`SearchEngine`] by sending an authenticated `POST` request
/// to `https://api.perplexity.ai/chat/completions`. The HTTP client is
/// injectable for testing; when `None`, the shared masterfetch client from
/// [`crate::masterfetch::http`] is used.
///
/// # Requirements
///
/// - The API key is never logged in plain text.
/// - Failures are reported as `engine_blocked` so the other `mf_search`
///   backends can still return results.
#[derive(Debug, Clone)]
pub struct PerplexityEngine {
    /// Perplexity API key (`Authorization: Bearer {api_key}`).
    api_key: String,
    /// Optional injectable HTTP client (for testing).
    client: Option<reqwest::Client>,
    /// Sonar model name (defaults to [`DEFAULT_MODEL`]).
    model: String,
}

impl PerplexityEngine {
    /// Create a new `PerplexityEngine` with the given API key and the shared
    /// masterfetch HTTP client.
    #[must_use]
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            client: None,
            model: DEFAULT_MODEL.to_string(),
        }
    }

    /// Create a new `PerplexityEngine` with a custom HTTP client (for testing
    /// or custom timeout/redirect configuration).
    #[must_use]
    pub fn with_client(api_key: impl Into<String>, client: reqwest::Client) -> Self {
        Self {
            api_key: api_key.into(),
            client: Some(client),
            model: DEFAULT_MODEL.to_string(),
        }
    }

    /// Set the Sonar model to use (e.g. `"sonar"`, `"sonar-pro"`).
    #[must_use]
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model = model.into();
        self
    }

    /// Return the Perplexity API key, masked for diagnostics.
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
    pub fn api_key(&self) -> &str {
        &self.api_key
    }
}

impl Default for PerplexityEngine {
    fn default() -> Self {
        Self::new("")
    }
}

#[async_trait::async_trait]
impl SearchEngine for PerplexityEngine {
    fn name(&self) -> &str {
        ENGINE_NAME
    }

    /// Execute an authenticated Perplexity Sonar API query.
    ///
    /// Sends a `POST` to `https://api.perplexity.ai/chat/completions` with the
    /// `Authorization: Bearer {key}` header and a JSON body built by
    /// [`build_request_body`]. Non-2xx responses are reported as
    /// `engine_blocked = true`.
    async fn search(&self, query: &str, opts: &SearchOptions) -> EngineReport {
        let start = Instant::now();

        if query.trim().is_empty() {
            return EngineReport::error(ENGINE_NAME, "search query must not be empty");
        }

        if self.api_key().is_empty() {
            return EngineReport::blocked(ENGINE_NAME, "missing Perplexity API key");
        }

        let client = match self.get_client() {
            Ok(c) => c,
            Err(e) => return EngineReport::error(ENGINE_NAME, e),
        };

        let body = build_request_body(query, opts, &self.model);

        tracing::debug!(
            query = query,
            max_results = opts.max_results,
            url = %API_URL,
            model = %self.model,
            "perplexity: sending search request"
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
            tracing::warn!(status = %status, "perplexity: API returned error status");
            return EngineReport::blocked(
                ENGINE_NAME,
                format!("Perplexity API returned HTTP {status}"),
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

/// Build the Perplexity Sonar JSON request body from a query and
/// [`SearchOptions`].
///
/// The returned [`serde_json::Value`] contains:
///
/// - `model` — the Sonar model name.
/// - `messages` — a single user message with the truncated query.
/// - `max_tokens` — a rough token budget based on `max_results`.
/// - `search_recency_filter` — mapped from [`Freshness`] (omitted for `Any`).
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::perplexity::build_request_body;
/// use ragent_tools_extended::masterfetch::search::engine::SearchOptions;
///
/// let opts = SearchOptions::new(10);
/// let body = build_request_body("rust async", &opts, "sonar");
/// assert_eq!(body["model"], "sonar");
/// assert_eq!(body["messages"][0]["role"], "user");
/// assert_eq!(body["messages"][0]["content"], "rust async");
/// ```
#[must_use]
pub fn build_request_body(query: &str, opts: &SearchOptions, model: &str) -> serde_json::Value {
    let max_tokens = (opts.max_results * TOKENS_PER_RESULT).min(MAX_TOKEN_BUDGET);
    let mut body = json!({
        "model": model,
        "messages": [
            { "role": "user", "content": truncate_query(query) }
        ],
        "max_tokens": max_tokens,
    });
    if let Some(recency) = freshness_to_recency(opts.freshness) {
        body["search_recency_filter"] = json!(recency);
    }
    body
}

/// Truncate a search query to Perplexity's maximum accepted length, respecting
/// UTF-8 character boundaries.
#[must_use]
pub fn truncate_query(query: &str) -> String {
    if query.chars().count() <= MAX_QUERY_CHARS {
        query.to_string()
    } else {
        query.chars().take(MAX_QUERY_CHARS).collect()
    }
}

/// Map a [`Freshness`] value to the Perplexity `search_recency_filter` string.
///
/// Returns `None` for [`Freshness::Any`] (no filter). Perplexity does not have
/// a "year" option, so `Year` maps to `"hour"` — the broadest available filter
/// that still constrains recency.
const fn freshness_to_recency(freshness: Freshness) -> Option<&'static str> {
    match freshness {
        Freshness::Day => Some("day"),
        Freshness::Week => Some("week"),
        Freshness::Month => Some("month"),
        Freshness::Year => Some("hour"),
        Freshness::Any => None,
    }
}

// ---------------------------------------------------------------------------
// Response parsing
// ---------------------------------------------------------------------------

/// Parse a Perplexity Sonar JSON response into [`RawResult`]s.
///
/// Expects the `search_results` array, where each item has `title`, `url`, and
/// `snippet` (or `text`). If `search_results` is absent, falls back to the
/// `citations` array (URLs only). Results are returned with `source` set to
/// `"perplexity"` and no `score`. Snippets are truncated to approximately 200
/// characters.
#[must_use]
pub fn parse_response_json(value: &serde_json::Value) -> Vec<RawResult> {
    // Prefer the structured `search_results` array.
    if let Some(items) = value.get("search_results").and_then(|r| r.as_array()) {
        return items
            .iter()
            .filter_map(|item| {
                let title = item
                    .get("title")
                    .and_then(|s| s.as_str())
                    .unwrap_or_default()
                    .to_string();
                let url = item.get("url")?.as_str()?.to_string();
                let snippet = item
                    .get("snippet")
                    .and_then(|s| s.as_str())
                    .or_else(|| item.get("text").and_then(|s| s.as_str()))
                    .unwrap_or_default()
                    .to_string();
                Some(RawResult::new(
                    title,
                    url,
                    truncate_snippet(&snippet),
                    ENGINE_NAME,
                ))
            })
            .collect();
    }

    // Fall back to `citations` (array of URL strings) when `search_results`
    // is absent.
    if let Some(citations) = value.get("citations").and_then(|c| c.as_array()) {
        return citations
            .iter()
            .filter_map(|url| {
                let url = url.as_str()?.to_string();
                Some(RawResult::new(
                    String::new(),
                    url,
                    String::new(),
                    ENGINE_NAME,
                ))
            })
            .collect();
    }

    Vec::new()
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
