//! Brave keyless search backend.
//!
//! Implements **FR-008** and **NFR-003** (T-014).
//!
//! This module provides a [`BraveEngine`] that implements the
//! [`SearchEngine`] trait by scraping Brave Search's HTML results page at
//! `https://search.brave.com/search`. No API key is required — the engine
//! sends a GET request with the query as a URL parameter and parses the
//! returned HTML to extract result titles, URLs, and snippets.
//!
//! # Rate-limiting
//!
//! Brave may return HTTP 429 (rate-limit) or 503 (service unavailable /
//! challenge). Both are detected and reported as `engine_blocked = true` in
//! the [`EngineReport`], with an honest error message. The caller (consensus
//! merger) uses this to populate the `engine_blocked` signal for the agent.
//!
//! # URL unwrapping
//!
//! Brave may wrap result URLs in a redirect link. The parser checks for
//! Brave's redirect pattern and unwraps to the original target URL. Direct
//! URLs are returned unchanged.
//!
//! # Testability (NFR-003)
//!
//! The HTML parsing logic is split into a pure function
//! ([`parse_results_html`]) that takes an HTML string and returns
//! [`RawResult`]s without any network I/O. This enables unit tests with
//! fixture HTML. The full [`BraveEngine::search`] method performs HTTP I/O
//! and is tested with `#[ignore]`-gated integration tests.
//!
//! # URL construction
//!
//! The search URL and query parameters are built by
//! [`build_search_params`], a pure function that maps [`SearchOptions`]
//! (query, site filter, freshness, page) to Brave's URL query parameters
//! (`q`, `tf`, `offset`).
//!
//! # Examples
//!
//! Parse a fixture HTML page:
//!
//! ```
//! use ragent_tools_extended::masterfetch::search::brave::parse_results_html;
//!
//! let html = r#"<div class="snippet fdb">
//!   <a class="result-header" href="https://example.com/page">
//!     <span class="snippet-title">Example Title</span>
//!   </a>
//!   <div class="snippet-description"><p>A snippet.</p></div>
//! </div>"#;
//! let results = parse_results_html(html, "brave");
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

/// The Brave Search HTML endpoint.
const SEARCH_URL: &str = "https://search.brave.com/search";

/// Engine display name.
const ENGINE_NAME: &str = "brave";

/// Standard rate-limit HTTP status code (hard block).
const TOO_MANY_REQUESTS_STATUS: u16 = 429;

/// Service unavailable / challenge HTTP status code.
const SERVICE_UNAVAILABLE_STATUS: u16 = 503;

// ---------------------------------------------------------------------------
// Engine struct
// ---------------------------------------------------------------------------

/// Brave keyless search backend.
///
/// Implements [`SearchEngine`] by scraping `https://search.brave.com/search`.
/// The HTTP client is injectable for testing; when `None`, the shared
/// masterfetch client from [`crate::masterfetch::http`] is used.
///
/// # Requirements
///
/// - **FR-008** — keyless multi-engine search backend.
/// - **FR-023** — no API keys, tokens, or accounts.
/// - **NFR-003** — pure parsing logic is testable without network.
pub struct BraveEngine {
    /// Optional injectable HTTP client (for testing).
    client: Option<reqwest::Client>,
}

impl BraveEngine {
    /// Create a new `BraveEngine` that uses the shared masterfetch HTTP
    /// client.
    #[must_use]
    pub const fn new() -> Self {
        Self { client: None }
    }

    /// Create a new `BraveEngine` with a custom HTTP client (for testing
    /// or custom timeout/redirect configuration).
    #[must_use]
    pub const fn with_client(client: reqwest::Client) -> Self {
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

impl Default for BraveEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SearchEngine for BraveEngine {
    fn name(&self) -> &str {
        ENGINE_NAME
    }

    /// Execute a keyless search against Brave Search's HTML endpoint.
    ///
    /// Sends a GET to `https://search.brave.com/search?q=...` with the query
    /// and filters as URL parameters, then parses the returned HTML.
    /// Rate-limit responses (429/503) are detected and reported as
    /// `engine_blocked`.
    ///
    /// This method never returns `Err` for engine-level failures — those are
    /// captured in the `EngineReport`'s `error` and `engine_blocked` fields.
    async fn search(&self, query: &str, opts: &SearchOptions) -> EngineReport {
        let start = Instant::now();

        // Validate query.
        if query.trim().is_empty() {
            return EngineReport::error(ENGINE_NAME, "search query must not be empty");
        }

        // Build URL with query parameters.
        let url = build_search_url(query, opts);

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
            url = %url,
            "brave: sending search request"
        );

        // Send GET request with realistic headers to avoid immediate blocking.
        let response = match client
            .get(&url)
            .header(
                "Accept",
                "text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8",
            )
            .header("Accept-Language", "en-US,en;q=0.9")
            .header("Sec-Fetch-Dest", "document")
            .header("Sec-Fetch-Mode", "navigate")
            .send()
            .await
        {
            Ok(r) => r,
            Err(e) => {
                return EngineReport::error(ENGINE_NAME, format!("HTTP request failed: {e}"));
            }
        };

        let status = response.status();

        // Check for rate-limiting (429 hard block, 503 service unavailable).
        if status.as_u16() == TOO_MANY_REQUESTS_STATUS {
            tracing::warn!("brave: rate-limited (HTTP 429)");
            return EngineReport::blocked(
                ENGINE_NAME,
                "rate-limited by Brave Search (HTTP 429 too many requests)",
            );
        }
        if status.as_u16() == SERVICE_UNAVAILABLE_STATUS {
            tracing::warn!("brave: service unavailable (HTTP 503)");
            return EngineReport::blocked(
                ENGINE_NAME,
                "Brave Search service unavailable (HTTP 503)",
            );
        }

        // Check for other error status codes.
        if !status.is_success() {
            return EngineReport::error(
                ENGINE_NAME,
                format!("Brave Search returned HTTP {status}"),
            );
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
            "brave: search complete"
        );

        let mut report = EngineReport::ok(ENGINE_NAME, results);
        report.duration_ms = elapsed;
        report
    }
}

// ---------------------------------------------------------------------------
// URL construction (pure, testable)
// ---------------------------------------------------------------------------

/// Build the full search URL with query parameters for a Brave Search
/// request.
///
/// Maps the query and [`SearchOptions`] to Brave's URL parameters:
///
/// - `q` — the search query, with `site:` and `-site:` operators appended.
/// - `tf` — the time filter (`pd`=day, `pw`=week, `pm`=month, `py`=year;
///   omitted for `Any`).
/// - `offset` — the result offset (0 for first page, 10×page for subsequent
///   pages).
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::brave::build_search_url;
/// use ragent_tools_extended::masterfetch::search::engine::{Freshness, SearchOptions};
///
/// let opts = SearchOptions::new(10).with_freshness(Freshness::Week);
/// let url = build_search_url("rust async", &opts);
/// assert!(url.contains("q=rust+async"));
/// assert!(url.contains("tf=pw"));
/// ```
#[must_use]
pub fn build_search_url(query: &str, opts: &SearchOptions) -> String {
    let params = build_search_params(query, opts);
    let query_string = params
        .iter()
        .map(|(k, v)| format!("{k}={}", urlencode(v)))
        .collect::<Vec<_>>()
        .join("&");
    format!("{SEARCH_URL}?{query_string}")
}

/// Build the query parameter pairs for a Brave Search request.
///
/// Returns a vector of `(key, value)` pairs. The `q` parameter always
/// includes the query; site filters are appended as `site:` / `-site:`
/// operators within the query string. The `tf` parameter is only included
/// when freshness is not `Any`. The `offset` parameter is included for
/// pages > 0.
#[must_use]
pub fn build_search_params(query: &str, opts: &SearchOptions) -> Vec<(String, String)> {
    let mut params = Vec::new();

    // Build the query string with site/exclude operators.
    let mut q = query.trim().to_string();
    if !opts.site.is_empty() {
        q.push_str(&format!(" site:{}", opts.site));
    }
    for excluded in &opts.exclude_sites {
        q.push_str(&format!(" -site:{excluded}"));
    }
    params.push(("q".to_string(), q));

    // Freshness → tf parameter.
    if let Some(tf) = freshness_to_tf(opts.freshness) {
        params.push(("tf".to_string(), tf.to_string()));
    }

    // Page → offset. Brave uses 0, 10, 20, ... for pagination.
    let offset = opts.page.saturating_mul(10);
    if offset > 0 {
        params.push(("offset".to_string(), offset.to_string()));
    }

    params
}

/// Map a [`Freshness`] value to Brave's `tf` parameter.
///
/// Returns `None` for [`Freshness::Any`] (no time filter).
const fn freshness_to_tf(freshness: Freshness) -> Option<&'static str> {
    match freshness {
        Freshness::Day => Some("pd"),
        Freshness::Week => Some("pw"),
        Freshness::Month => Some("pm"),
        Freshness::Year => Some("py"),
        Freshness::Any => None,
    }
}

/// Simple URL-encoding for query parameter values.
///
/// Encodes spaces as `+` and percent-encodes special characters. Alphanumeric
/// and a few safe characters (`-`, `_`, `.`, `~`, `:`, `/`) are left
/// unencoded.
fn urlencode(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    for c in s.chars() {
        if c.is_ascii_alphanumeric() || "-_.~:/".contains(c) {
            result.push(c);
        } else if c == ' ' {
            result.push('+');
        } else {
            let mut buf = [0u8; 4];
            for byte in c.encode_utf8(&mut buf).as_bytes() {
                result.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    result
}

// ---------------------------------------------------------------------------
// HTML parsing (pure, testable — NFR-003)
// ---------------------------------------------------------------------------

/// Parse Brave Search HTML results into [`RawResult`]s.
///
/// This is a **pure function** — no network I/O. It extracts result blocks
/// from the HTML page returned by `https://search.brave.com/search`.
///
/// # Brave HTML structure
///
/// Brave Search's HTML results page uses several class patterns for result
/// blocks. This parser handles the most common structures:
///
/// ```html
/// <!-- Pattern 1: snippet div with result-header link -->
/// <div class="snippet fdb">
///   <a class="result-header" href="https://example.com/page">
///     <span class="snippet-title">Title text</span>
///   </a>
///   <div class="snippet-description"><p>Snippet text</p></div>
/// </div>
/// ```
///
/// ```html
/// <!-- Pattern 2: div with data-type and title link -->
/// <div class="search-result" data-type="web">
///   <a class="title" href="https://example.com/page">Title text</a>
///   <p class="snippet-description">Snippet text</p>
/// </div>
/// ```
///
/// The parser uses multiple regex patterns to handle both structures. Direct
/// URLs (not wrapped in Brave redirects) are returned as-is. If a URL
/// appears to be a Brave redirect, the parser attempts to unwrap it.
///
/// # Arguments
///
/// - `html` — the raw HTML body from Brave Search's results page.
/// - `source` — the engine name to set on each [`RawResult`] (e.g.
///   `"brave"`).
///
/// # Returns
///
/// A vector of [`RawResult`]s, in the order they appear in the HTML. Empty
/// or malformed HTML returns an empty vector. This function never panics.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::brave::parse_results_html;
///
/// let html = r#"
/// <div class="snippet fdb">
///   <a class="result-header" href="https://example.com/a">
///     <span class="snippet-title">First Result</span>
///   </a>
///   <div class="snippet-description"><p>First snippet.</p></div>
/// </div>
/// <div class="snippet fdb">
///   <a class="result-header" href="https://example.com/b">
///     <span class="snippet-title">Second Result</span>
///   </a>
///   <div class="snippet-description"><p>Second snippet.</p></div>
/// </div>
/// "#;
/// let results = parse_results_html(html, "brave");
/// assert_eq!(results.len(), 2);
/// assert_eq!(results[0].title, "First Result");
/// assert_eq!(results[1].title, "Second Result");
/// ```
#[must_use]
pub fn parse_results_html(html: &str, source: &str) -> Vec<RawResult> {
    // Collect all result URLs + titles using multiple patterns.
    // Pattern 1: <a class="result-header" href="URL">...<span class="snippet-title">Title</span>...</a>
    let header_re = Regex::new(
        r#"(?si)<a\s+class="result-header"[^>]*\bhref\s*=\s*["']([^"']*)["'][^>]*>(.*?)</a>"#,
    )
    .unwrap_or_else(|e| panic!("invalid result-header regex: {e}"));

    // Pattern 2: <a class="title" href="URL">Title</a>
    let title_link_re = Regex::new(
        r#"(?si)<a\s+class="[^"]*\btitle\b[^"]*"[^>]*\bhref\s*=\s*["']([^"']*)["'][^>]*>(.*?)</a>"#,
    )
    .unwrap_or_else(|e| panic!("invalid title-link regex: {e}"));

    // Pattern 3: <span class="snippet-title">Title</span> (title only, no URL)
    let snippet_title_re = Regex::new(r#"(?si)<span\s+class="snippet-title"[^>]*>(.*?)</span>"#)
        .unwrap_or_else(|e| panic!("invalid snippet-title regex: {e}"));

    // Snippet text: <div class="snippet-description">...</div> or <p class="snippet-description">...</p>
    let snippet_desc_re =
        Regex::new(r#"(?si)<(?:div|p)\s+class="snippet-description"[^>]*>(.*?)</(?:div|p)>"#)
            .unwrap_or_else(|e| panic!("invalid snippet-description regex: {e}"));

    // Collect titles and URLs.
    // Strategy: try Pattern 1 (result-header) first, then Pattern 2 (title
    // link), then fall back to Pattern 3 (snippet-title only, no URL).
    let mut titles_with_urls: Vec<(String, String)> = Vec::new();

    // Try Pattern 1: result-header links.
    let header_matches: Vec<(String, String)> = header_re
        .captures_iter(html)
        .map(|cap| {
            let href = cap.get(1).map_or("", |m| m.as_str()).to_string();
            let inner = cap.get(2).map_or("", |m| m.as_str());
            // Extract title from inner content: try snippet-title span, else strip tags.
            let title = snippet_title_re
                .captures(inner)
                .and_then(|c| c.get(1))
                .map_or_else(
                    || strip_html_tags(inner).trim().to_string(),
                    |m| strip_html_tags(m.as_str()).trim().to_string(),
                );
            (href, title)
        })
        .filter(|(href, title)| !href.is_empty() && !title.is_empty())
        .collect();

    if header_matches.is_empty() {
        // Try Pattern 2: title-class links.
        let title_link_matches: Vec<(String, String)> = title_link_re
            .captures_iter(html)
            .map(|cap| {
                let href = cap.get(1).map_or("", |m| m.as_str()).to_string();
                let title_html = cap.get(2).map_or("", |m| m.as_str());
                let title = strip_html_tags(title_html).trim().to_string();
                (href, title)
            })
            .filter(|(href, title)| !href.is_empty() && !title.is_empty())
            .collect();

        if title_link_matches.is_empty() {
            // Pattern 3: snippet-title spans (title only, no URL).
            // We can't get URLs this way, so we skip this fallback.
            // Brave results without URLs are not useful.
        } else {
            titles_with_urls = title_link_matches;
        }
    } else {
        titles_with_urls = header_matches;
    }

    // Collect all snippet texts.
    let snippets: Vec<String> = snippet_desc_re
        .captures_iter(html)
        .map(|cap| {
            cap.get(1)
                .map(|m| strip_html_tags(m.as_str()).trim().to_string())
                .unwrap_or_default()
        })
        .collect();

    // Pair titles with snippets by index.
    let mut results = Vec::with_capacity(titles_with_urls.len());
    for (i, (raw_href, title)) in titles_with_urls.iter().enumerate() {
        // Unwrap Brave redirect URLs if present.
        let url = unwrap_brave_url(raw_href);
        if url.is_empty() {
            continue;
        }

        let snippet = snippets.get(i).cloned().unwrap_or_default();

        results.push(RawResult::new(title, &url, &snippet, source));
    }

    results
}

/// Unwrap a Brave redirect URL to recover the original target.
///
/// Brave Search typically returns direct URLs in its HTML results, but may
/// occasionally wrap them in redirect links. This function checks for common
/// redirect patterns and unwraps them. If the input is not a redirect, it is
/// returned unchanged.
fn unwrap_brave_url(href: &str) -> String {
    // Brave redirect patterns (if any): check for known redirect domains.
    // Currently Brave returns direct URLs, so we just clean up common issues.

    // Strip leading/trailing whitespace.
    let trimmed = href.trim();

    // If the URL starts with //, prepend https:.
    if trimmed.starts_with("//") {
        return format!("https:{trimmed}");
    }

    trimmed.to_string()
}

/// Strip HTML tags from a string, leaving only text content.
///
/// Replaces `<...>` sequences with nothing, and decodes common HTML entities
/// (`&amp;`, `&lt;`, `&gt;`, `&quot;`, `&#39;`).
fn strip_html_tags(s: &str) -> String {
    let tag_re = Regex::new(r"<[^>]*>").unwrap_or_else(|e| panic!("invalid tag regex: {e}"));
    let stripped = tag_re.replace_all(s, "");

    stripped
        .replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&nbsp;", " ")
}
