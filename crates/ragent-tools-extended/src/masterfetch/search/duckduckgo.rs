//! DuckDuckGo keyless search backend.
//!
//! Implements **FR-008** and **NFR-003** (T-013).
//!
//! This module provides a [`DuckDuckGoEngine`] that implements the
//! [`SearchEngine`] trait by scraping DuckDuckGo's lightweight HTML results page
//! at `https://html.duckduckgo.com/html/`. No API key is required — the engine
//! sends a POST request with the query as a form field and parses the returned
//! HTML to extract result titles, URLs, and snippets.
//!
//! # Rate-limiting
//!
//! DuckDuckGo may return HTTP 202 (soft rate-limit / challenge) or 429 (hard
//! rate-limit). Both are detected and reported as `engine_blocked = true` in
//! the [`EngineReport`], with an honest error message. The caller (consensus
//! merger) uses this to populate the `engine_blocked` signal for the agent.
//!
//! # URL unwrapping
//!
//! DuckDuckGo wraps result URLs in a redirect link of the form
//! `//duckduckgo.com/l/?uddg=<encoded-url>&rut=...`. The parser unwraps these
//! to recover the original target URL via the `uddg` query parameter.
//!
//! # Testability (NFR-003)
//!
//! The HTML parsing logic is split into a pure function
//! ([`parse_results_html`]) that takes an HTML string and returns
//! [`RawResult`]s without any network I/O. This enables unit tests with fixture
//! HTML. The full [`DuckDuckGoEngine::search`] method performs HTTP I/O and is
//! tested with `#[ignore]`-gated integration tests.
//!
//! # URL construction
//!
//! The search URL and POST form body are built by [`build_form_params`], a pure
//! function that maps [`SearchOptions`] (query, site filter, freshness, page)
//! to DuckDuckGo's form parameters (`q`, `df`, `s`).
//!
//! # Examples
//!
//! Parse a fixture HTML page:
//!
//! ```
//! use ragent_tools_extended::masterfetch::search::duckduckgo::parse_results_html;
//!
//! let html = r#"<div class="result">
//!   <h2 class="result__title">
//!     <a class="result__a" href="https://example.com/page">Example Title</a>
//!   </h2>
//!   <a class="result__snippet" href="https://example.com/page">A snippet.</a>
//! </div>"#;
//! let results = parse_results_html(html, "duckduckgo");
//! assert_eq!(results.len(), 1);
//! assert_eq!(results[0].title, "Example Title");
//! assert_eq!(results[0].url, "https://example.com/page");
//! assert_eq!(results[0].snippet, "A snippet.");
//! ```

use std::time::Instant;

use regex::Regex;

use super::engine::{
    EngineReport, Freshness, RawResult, SearchEngine, SearchOptions, dedup_results_by_url,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The DuckDuckGo HTML search endpoint (lightweight, no JS required).
const SEARCH_URL: &str = "https://html.duckduckgo.com/html/";

/// Engine display name.
const ENGINE_NAME: &str = "duckduckgo";

/// DuckDuckGo's rate-limit / challenge HTTP status code (soft block).
const RATE_LIMIT_STATUS: u16 = 202;

/// Standard rate-limit HTTP status code (hard block).
const TOO_MANY_REQUESTS_STATUS: u16 = 429;

// ---------------------------------------------------------------------------
// Engine struct
// ---------------------------------------------------------------------------

/// DuckDuckGo keyless search backend.
///
/// Implements [`SearchEngine`] by scraping `https://html.duckduckgo.com/html/`.
/// The HTTP client is injectable for testing; when `None`, the shared
/// masterfetch client from [`crate::masterfetch::http`] is used.
///
/// # Requirements
///
/// - **FR-008** — keyless multi-engine search backend.
/// - **FR-023** — no API keys, tokens, or accounts.
/// - **NFR-003** — pure parsing logic is testable without network.
pub struct DuckDuckGoEngine {
    /// Optional injectable HTTP client (for testing).
    client: Option<reqwest::Client>,
}

impl DuckDuckGoEngine {
    /// Create a new `DuckDuckGoEngine` that uses the shared masterfetch HTTP
    /// client.
    #[must_use]
    pub fn new() -> Self {
        Self { client: None }
    }

    /// Create a new `DuckDuckGoEngine` with a custom HTTP client (for testing
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

impl Default for DuckDuckGoEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SearchEngine for DuckDuckGoEngine {
    fn name(&self) -> &str {
        ENGINE_NAME
    }

    /// Execute a keyless search against DuckDuckGo's HTML endpoint.
    ///
    /// Sends a POST to `https://html.duckduckgo.com/html/` with the query and
    /// filters as form fields, then parses the returned HTML. Rate-limit
    /// responses (202/429) are detected and reported as `engine_blocked`.
    ///
    /// This method never returns `Err` for engine-level failures — those are
    /// captured in the `EngineReport`'s `error` and `engine_blocked` fields.
    async fn search(&self, query: &str, opts: &SearchOptions) -> EngineReport {
        let start = Instant::now();

        // Validate query.
        if query.trim().is_empty() {
            return EngineReport::error(ENGINE_NAME, "search query must not be empty");
        }

        // Build form parameters.
        let form = build_form_params(query, opts);

        // Get HTTP client.
        let client = match self.get_client() {
            Ok(c) => c,
            Err(e) => {
                return EngineReport::error(ENGINE_NAME, e);
            }
        };

        tracing::debug!(
            query = query,
            max_results = opts.max_results,
            "duckduckgo: sending search request"
        );

        // Send POST request.
        let response = match client
            .post(SEARCH_URL)
            .header("Accept", "text/html,application/xhtml+xml")
            .header("Accept-Language", "en-US,en;q=0.9")
            .form(&form)
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return EngineReport::error(ENGINE_NAME, format!("HTTP request failed: {e}"));
            }
        };

        let status = response.status();

        // Check for rate-limiting (202 soft block, 429 hard block).
        if status.as_u16() == RATE_LIMIT_STATUS {
            tracing::warn!("duckduckgo: rate-limited (HTTP 202)");
            return EngineReport::blocked(
                ENGINE_NAME,
                "rate-limited by DuckDuckGo (HTTP 202 soft block)",
            );
        }
        if status.as_u16() == TOO_MANY_REQUESTS_STATUS {
            tracing::warn!("duckduckgo: rate-limited (HTTP 429)");
            return EngineReport::blocked(
                ENGINE_NAME,
                "rate-limited by DuckDuckGo (HTTP 429 too many requests)",
            );
        }

        // Check for other error status codes.
        if !status.is_success() {
            return EngineReport::error(ENGINE_NAME, format!("DuckDuckGo returned HTTP {status}"));
        }

        // Read response body.
        let body = match response.text().await {
            Ok(t) => t,
            Err(e) => {
                return EngineReport::error(
                    ENGINE_NAME,
                    format!("failed to read response body: {e}"),
                );
            }
        };

        // Parse results from HTML.
        let mut results = parse_results_html(&body, ENGINE_NAME);

        // Deduplicate by normalised URL.
        results = dedup_results_by_url(&results);

        // Cap to max_results.
        results.truncate(opts.max_results);

        let elapsed = start.elapsed().as_millis() as u64;
        let result_count = results.len();

        tracing::debug!(
            results = result_count,
            duration_ms = elapsed,
            "duckduckgo: search complete"
        );

        let mut report = EngineReport::ok(ENGINE_NAME, results);
        report.duration_ms = elapsed;
        report
    }
}

// ---------------------------------------------------------------------------
// Form parameter construction (pure, testable)
// ---------------------------------------------------------------------------

/// Build the POST form parameters for a DuckDuckGo HTML search.
///
/// Maps the query and [`SearchOptions`] to DuckDuckGo's form fields:
///
/// - `q` — the search query, with `site:` and `-site:` operators appended.
/// - `df` — the date filter (`d`=day, `w`=week, `m`=month, `y`=year; omitted
///   for `Any`).
/// - `s` — the result offset (0 for first page, 20×page for subsequent pages).
///
/// Returns a vector of `(key, value)` pairs suitable for `reqwest::form()`.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::duckduckgo::build_form_params;
/// use ragent_tools_extended::masterfetch::search::engine::{Freshness, SearchOptions};
///
/// let opts = SearchOptions::new(10).with_freshness(Freshness::Week);
/// let form = build_form_params("rust async", &opts);
/// let q = form.iter().find(|(k, _)| k == "q").unwrap();
/// assert!(q.1.contains("rust async"));
/// let df = form.iter().find(|(k, _)| k == "df");
/// assert_eq!(df.unwrap().1, "w");
/// ```
#[must_use]
pub fn build_form_params(query: &str, opts: &SearchOptions) -> Vec<(String, String)> {
    let mut form = Vec::new();

    // Build the query string with site/exclude operators.
    let mut q = query.trim().to_string();
    if !opts.site.is_empty() {
        q.push_str(&format!(" site:{}", opts.site));
    }
    for excluded in &opts.exclude_sites {
        q.push_str(&format!(" -site:{excluded}"));
    }
    form.push(("q".to_string(), q));

    // Freshness → df parameter.
    if let Some(df) = freshness_to_df(opts.freshness) {
        form.push(("df".to_string(), df.to_string()));
    }

    // Page → s (offset). DuckDuckGo uses 0, 20, 40, ... for pagination.
    let offset = opts.page.saturating_mul(20);
    if offset > 0 {
        form.push(("s".to_string(), offset.to_string()));
    }

    form
}

/// Map a [`Freshness`] value to DuckDuckGo's `df` parameter.
///
/// Returns `None` for [`Freshness::Any`] (no date filter).
fn freshness_to_df(freshness: Freshness) -> Option<&'static str> {
    match freshness {
        Freshness::Day => Some("d"),
        Freshness::Week => Some("w"),
        Freshness::Month => Some("m"),
        Freshness::Year => Some("y"),
        Freshness::Any => None,
    }
}

// ---------------------------------------------------------------------------
// HTML parsing (pure, testable — NFR-003)
// ---------------------------------------------------------------------------

/// Parse DuckDuckGo HTML search results into [`RawResult`]s.
///
/// This is a **pure function** — no network I/O. It extracts result blocks
/// from the HTML page returned by `https://html.duckduckgo.com/html/`.
///
/// # DuckDuckGo HTML structure
///
/// Each result is contained in a `<div class="result">` block:
///
/// ```html
/// <div class="result">
///   <h2 class="result__title">
///     <a class="result__a" href="...">Title text</a>
///   </h2>
///   <a class="result__snippet" href="...">Snippet text</a>
/// </div>
/// ```
///
/// The `href` on `result__a` may be a direct URL or a DuckDuckGo redirect link
/// of the form `//duckduckgo.com/l/?uddg=<encoded-url>&rut=...`. Redirect links
/// are unwrapped to recover the original target URL via the `uddg` parameter.
///
/// # Arguments
///
/// - `html` — the raw HTML body from DuckDuckGo's results page.
/// - `source` — the engine name to set on each [`RawResult`] (e.g.
///   `"duckduckgo"`).
///
/// # Returns
///
/// A vector of [`RawResult`]s, in the order they appear in the HTML. Empty or
/// malformed HTML returns an empty vector. This function never panics.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::duckduckgo::parse_results_html;
///
/// let html = r#"
/// <div class="result">
///   <h2 class="result__title">
///     <a class="result__a" href="https://example.com/a">First Result</a>
///   </h2>
///   <a class="result__snippet" href="https://example.com/a">First snippet.</a>
/// </div>
/// <div class="result">
///   <h2 class="result__title">
///     <a class="result__a" href="https://example.com/b">Second Result</a>
///   </h2>
///   <a class="result__snippet" href="https://example.com/b">Second snippet.</a>
/// </div>
/// "#;
/// let results = parse_results_html(html, "duckduckgo");
/// assert_eq!(results.len(), 2);
/// assert_eq!(results[0].title, "First Result");
/// assert_eq!(results[1].title, "Second Result");
/// ```
#[must_use]
pub fn parse_results_html(html: &str, source: &str) -> Vec<RawResult> {
    // Match the title link: <a class="result__a" href="URL">Title</a>
    // DuckDuckGo wraps result titles in this class.
    let title_re = Regex::new(
        r#"(?si)<a\s+class="result__a"[^>]*\bhref\s*=\s*["']([^"']*)["'][^>]*>(.*?)</a>"#,
    )
    .unwrap_or_else(|e| panic!("invalid title link regex: {e}"));

    // Match the snippet: <a class="result__snippet" ...>Snippet</a>
    let snippet_re =
        Regex::new(r#"(?si)<(?:a|span|div)\s+class="result__snippet"[^>]*>(.*?)</(?:a|span|div)>"#)
            .unwrap_or_else(|e| panic!("invalid snippet regex: {e}"));

    // Collect all title links (href, title) pairs.
    let titles: Vec<(String, String)> = title_re
        .captures_iter(html)
        .map(|cap| {
            let href = cap.get(1).map(|m| m.as_str()).unwrap_or("").to_string();
            let title_html = cap.get(2).map(|m| m.as_str()).unwrap_or("");
            let title = strip_html_tags(title_html).trim().to_string();
            (href, title)
        })
        .filter(|(_, title)| !title.is_empty())
        .collect();

    // Collect all snippet texts.
    let snippets: Vec<String> = snippet_re
        .captures_iter(html)
        .map(|cap| {
            cap.get(1)
                .map(|m| strip_html_tags(m.as_str()).trim().to_string())
                .unwrap_or_default()
        })
        .collect();

    // Pair titles with snippets by index.
    let mut results = Vec::with_capacity(titles.len());
    for (i, (raw_href, title)) in titles.iter().enumerate() {
        // Unwrap DuckDuckGo redirect URLs.
        let url = unwrap_ddg_url(raw_href);
        if url.is_empty() {
            continue;
        }

        let snippet = snippets.get(i).cloned().unwrap_or_default();

        results.push(RawResult::new(title, &url, &snippet, source));
    }

    results
}

/// Unwrap a DuckDuckGo redirect URL to recover the original target.
///
/// DuckDuckGo wraps result URLs in a redirect link:
///
/// - `//duckduckgo.com/l/?uddg=<encoded-url>&rut=...`
/// - `https://duckduckgo.com/l/?uddg=<encoded-url>&rut=...`
///
/// This function extracts the `uddg` parameter and URL-decodes it. If the
/// input is not a DuckDuckGo redirect, it is returned unchanged.
fn unwrap_ddg_url(href: &str) -> String {
    // Check if this is a DuckDuckGo redirect link.
    if !href.contains("duckduckgo.com/l/") && !href.contains("/l/?uddg=") {
        // Not a redirect — return as-is (may be a direct URL).
        return href.to_string();
    }

    // Extract the uddg parameter value.
    if let Some(url) = extract_query_param(href, "uddg") {
        return url;
    }

    // Fallback: return the original href.
    href.to_string()
}

/// Extract a URL query parameter value from a URL string.
///
/// Handles both `?uddg=value` and `&uddg=value` forms. The value is
/// percent-decoded.
fn extract_query_param(url: &str, param: &str) -> Option<String> {
    // Find the parameter in the query string.
    let needle = format!("{param}=");

    // Search for the parameter after ? or &.
    let mut search_pos = 0;
    loop {
        let pos = url[search_pos..].find(&needle)?;
        let abs_pos = search_pos + pos;

        // Check that the character before the param is ? or & (or it's at the
        // start of the query string).
        let before = url[..abs_pos].chars().last();
        if before == Some('?') || before == Some('&') || abs_pos == 0 {
            // Extract the value up to the next & or end of string.
            let value_start = abs_pos + needle.len();
            let value_end = url[value_start..]
                .find('&')
                .map(|p| value_start + p)
                .unwrap_or(url.len());
            let raw_value = &url[value_start..value_end];
            return Some(percent_decode(raw_value));
        }

        search_pos = abs_pos + needle.len();
    }
}

/// Simple percent-decoding for URL parameter values.
///
/// Decodes `%XX` hex sequences and `+` to space. Non-valid sequences are
/// passed through unchanged.
fn percent_decode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '%' {
            let hex: String = (0..2).filter_map(|_| chars.next()).collect();
            if hex.len() == 2
                && let Ok(byte) = u8::from_str_radix(&hex, 16)
            {
                result.push(byte as char);
                continue;
            }
            // Invalid hex — push as-is.
            result.push('%');
            result.push_str(&hex);
        } else if c == '+' {
            result.push(' ');
        } else {
            result.push(c);
        }
    }

    result
}

/// Strip HTML tags from a string, leaving only text content.
///
/// Replaces `<...>` sequences with nothing, and decodes common HTML entities
/// (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&#39;`).
fn strip_html_tags(s: &str) -> String {
    // Remove all <...> sequences.
    let tag_re = Regex::new(r#"<[^>]*>"#).unwrap_or_else(|e| panic!("invalid tag regex: {e}"));
    let stripped = tag_re.replace_all(s, "");

    // Decode common HTML entities.
    stripped
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}
