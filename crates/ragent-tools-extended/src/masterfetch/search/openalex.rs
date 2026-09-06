//! OpenAlex API-backed search backend for the `mf_search` multi-engine
//! pipeline.
//!
//! Implements **FR-002**, **FR-010**, and **FR-012** for spec `openalex`
//! (T-001).
//!
//! This module provides an [`OpenAlexEngine`] that implements the
//! [`SearchEngine`] trait by calling the [OpenAlex REST API](https://api.openalex.org).
//! OpenAlex is a fully-open catalog of scholarly works (≈240M records) and
//! requires **no API key** — it is a keyless backend like `DuckDuckGo` and
//! Brave. An optional `mailto` query parameter (resolved from the
//! `OPENALEX_EMAIL` environment variable or an `openalex_email` config field
//! — wired in later tasks) participates in the OpenAlex polite pool and raises
//! the daily request limit (FR-007).
//!
//! # Request mapping
//!
//! The [`build_request`] helper maps the shared [`SearchOptions`] to OpenAlex
//! query parameters for the `GET /works` endpoint:
//!
//! - `search` — the query verbatim.
//! - `per_page` — `opts.max_results` clamped to OpenAlex's 1–200 range
//!   (FR-012).
//! - `page` — `opts.page + 1` (OpenAlex pages are 1-indexed); when
//!   `opts.page` exceeds the documented 10,000-result basic-pagination
//!   limit, `cursor=*` is used instead (FR-008, handled in T-002).
//! - `filter` — built from the `site` and `freshness` options:
//!   - `site` → `primary_location.source.host_organization:<domain>` (FR-004).
//!   - `freshness` (non-`Any`) → `from_publication_date` / `to_publication_date`
//!     date range (FR-005).
//! - `mailto` — appended when a polite-pool email is configured (FR-007).
//!
//! # Response parsing
//!
//! The [`parse_response`] helper parses the OpenAlex JSON response at
//! `results[]`. Each work is emitted as a [`RawResult`] with:
//!
//! - `source` set to `"openalex"` (FR-010).
//! - `url` taken from `primary_location.landing_page_url`, falling back to the
//!   OpenAlex `id` URI, then to the DOI URL (FR-010).
//! - `snippet` reconstructed from the work's `abstract_inverted_index` (stripped
//!   of HTML), truncated to ~200 characters (FR-010).
//! - `score` set to the `relevance_score` normalised to the 0.0–1.0 range used
//!   by the consensus ranker (FR-011).
//!
//! Scholarly metadata (DOI, publication year, citation count, open-access URL,
//! source display name) is surfaced via the snippet text (FR-001).
//!
//! # Error handling
//!
//! The engine never returns `Err` for engine-level failures (FR-006, FR-009).
//! HTTP 429 (rate limited) is reported as [`EngineReport::blocked`] with the
//! message `"rate-limited"` (FR-009). Other non-2xx responses and unparseable
//! bodies are reported as [`EngineReport::error`] or [`EngineReport::blocked`]
//! so the remaining `mf_search` backends continue to return results (FR-006).
//!
//! # Testability
//!
//! Request construction ([`build_request`]) and response parsing
//! ([`parse_response`]) are pure functions that take plain inputs and produce
//! plain outputs, enabling unit tests without network I/O (NFR-002). The HTTP
//! client is injectable via [`OpenAlexEngine::with_client`] for integration
//! tests with a mock server.

use std::time::Instant;

use super::engine::{
    EngineReport, Freshness, RawResult, SearchEngine, SearchOptions, dedup_results_by_url,
};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// The OpenAlex REST API base URL for the `/works` endpoint.
pub const API_URL: &str = "https://api.openalex.org/works";

/// Engine display name.
pub const ENGINE_NAME: &str = "openalex";

/// Maximum number of results OpenAlex supports per request (`per_page`).
pub const MAX_PER_PAGE: usize = 200;

/// Minimum number of results per request (`per_page`).
pub const MIN_PER_PAGE: usize = 1;

/// Basic pagination result limit: OpenAlex supports up to 10,000 results via
/// the `page` parameter; beyond that, `cursor=*` deep paging is required
/// (FR-008).
pub const BASIC_PAGINATION_LIMIT: usize = 10_000;

/// Maximum snippet length in characters.
pub const MAX_SNIPPETTE_CHARS: usize = 200;

/// Maximum query length in characters (OpenAlex has no documented hard limit,
/// but we cap defensively to avoid excessively long URLs).
pub const MAX_QUERY_CHARS: usize = 1_000;

// ---------------------------------------------------------------------------
// Engine struct (FR-002)
// ---------------------------------------------------------------------------

/// OpenAlex API-backed search backend.
///
/// Implements [`SearchEngine`] by sending an unauthenticated `GET` request to
/// `https://api.openalex.org/works`. No API key is required — the backend is
/// keyless and always-on (FR-003). An optional `mailto` email participates in
/// the OpenAlex polite pool (FR-007).
///
/// The HTTP client is injectable for testing; when `None`, the shared
/// masterfetch client from [`crate::masterfetch::http`] is used.
///
/// # Requirements
///
/// - **FR-002** — OpenAlex backend that plugs into the `SearchEngine` trait.
/// - **FR-010** — result fields (`source`, `url`, `snippet`) populated per spec.
/// - **FR-012** — `per_page` clamped to 1–200; results truncated to
///   `opts.max_results`.
/// - **NFR-001** — uses the shared HTTP client timeout.
/// - **NFR-005** — no `unsafe` code, no `.unwrap()` on user-facing paths.
#[derive(Debug, Clone)]
pub struct OpenAlexEngine {
    /// Optional polite-pool email (appended as `mailto=`). May be empty.
    mailto: String,
    /// Optional injectable HTTP client (for testing).
    client: Option<reqwest::Client>,
}

impl OpenAlexEngine {
    /// Create a new `OpenAlexEngine` with no polite-pool email and the shared
    /// masterfetch HTTP client.
    #[must_use]
    pub fn new() -> Self {
        Self {
            mailto: String::new(),
            client: None,
        }
    }

    /// Create a new `OpenAlexEngine` with a polite-pool email.
    ///
    /// When `mailto` is non-empty, it is appended as the `mailto` query
    /// parameter on every request to participate in the OpenAlex polite pool
    /// (FR-007).
    #[must_use]
    pub fn with_mailto(mailto: impl Into<String>) -> Self {
        Self {
            mailto: mailto.into(),
            client: None,
        }
    }

    /// Create a new `OpenAlexEngine` with a custom HTTP client (for testing or
    /// custom timeout/redirect configuration).
    #[must_use]
    pub fn with_client(client: reqwest::Client) -> Self {
        Self {
            mailto: String::new(),
            client: Some(client),
        }
    }

    /// Return the configured polite-pool email, masked for diagnostics
    /// (NFR-003).
    ///
    /// Only the first two and last two characters are exposed; the rest are
    /// replaced with `*` characters so the email is never fully surfaced in
    /// logs or error messages.
    #[must_use]
    pub fn masked_mailto(&self) -> String {
        mask_email(&self.mailto)
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

impl Default for OpenAlexEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl SearchEngine for OpenAlexEngine {
    fn name(&self) -> &str {
        ENGINE_NAME
    }

    /// Execute an OpenAlex `/works` search query.
    ///
    /// Sends a `GET` to `https://api.openalex.org/works` with query parameters
    /// built by [`build_request`]. HTTP 429 is reported as a blocked report
    /// with the message `"rate-limited"` (FR-009). Other non-2xx responses are
    /// reported as blocked or errored reports so the remaining `mf_search`
    /// backends continue to return results (FR-006).
    async fn search(&self, query: &str, opts: &SearchOptions) -> EngineReport {
        let start = Instant::now();

        if query.trim().is_empty() {
            return EngineReport::error(ENGINE_NAME, "search query must not be empty");
        }

        let client = match self.get_client() {
            Ok(c) => c,
            Err(e) => return EngineReport::error(ENGINE_NAME, e),
        };

        let (url, params) = build_request(query, opts, &self.mailto);

        tracing::debug!(
            query = query,
            max_results = opts.max_results,
            url = %url,
            mailto_masked = %self.masked_mailto(),
            "openalex: sending search request"
        );

        let mut req = client.get(&url);
        if !params.is_empty() {
            req = req.query(&params);
        }

        let response = match req.send().await {
            Ok(r) => r,
            Err(e) => {
                return EngineReport::error(ENGINE_NAME, format!("HTTP request failed: {e}"));
            }
        };

        let status = response.status();

        if status.as_u16() == 429 {
            // OpenAlex's budget-based rate limiter returns a JSON body
            // explaining the reason (e.g. "Insufficient budget ... Resets at
            // midnight UTC"). Surface that message so the per-engine error
            // report is actionable instead of a bare "rate-limited".
            let detail = response
                .text()
                .await
                .ok()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
                .and_then(|value| {
                    value
                        .get("message")
                        .and_then(|m| m.as_str())
                        .map(str::to_string)
                });
            tracing::warn!(status = %status, "openalex: rate-limited");
            return match detail {
                Some(msg) => EngineReport::blocked(ENGINE_NAME, format!("rate-limited: {msg}")),
                None => EngineReport::blocked(ENGINE_NAME, "rate-limited"),
            };
        }

        if !status.is_success() {
            tracing::warn!(status = %status, "openalex: API returned error status");
            return EngineReport::blocked(
                ENGINE_NAME,
                format!("OpenAlex API returned HTTP {status}"),
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

        let mut results = parse_response(&value);
        results = dedup_results_by_url(&results);
        results.truncate(opts.max_results);

        let elapsed = start.elapsed().as_millis() as u64;
        let mut report = EngineReport::ok(ENGINE_NAME, results);
        report.duration_ms = elapsed;
        report
    }
}

// ---------------------------------------------------------------------------
// Request builder (pure, testable) — FR-004, FR-005, FR-008, FR-012
// ---------------------------------------------------------------------------

/// Build the OpenAlex request URL and query parameters from a query,
/// [`SearchOptions`], and an optional polite-pool email.
///
/// Returns a tuple of the base URL and a vector of `(key, value)` query
/// parameter pairs. The caller is responsible for URL-encoding the values
/// (e.g. via `reqwest::RequestBuilder::query`).
///
/// # Parameter mapping
///
/// - `search` — the trimmed query, capped at [`MAX_QUERY_CHARS`] characters.
/// - `per_page` — `opts.max_results` clamped to 1–200 (FR-012).
/// - `page` — `opts.page + 1` (1-indexed) for basic pagination; when
///   `opts.page * per_page` exceeds [`BASIC_PAGINATION_LIMIT`], `cursor=*` is
///   used instead and `page` is omitted (FR-008).
/// - `filter` — `site` and `freshness` filters combined with a comma
///   (FR-004, FR-005).
/// - `mailto` — appended when `mailto` is non-empty (FR-007).
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::openalex::build_request;
/// use ragent_tools_extended::masterfetch::search::engine::SearchOptions;
///
/// let opts = SearchOptions::new(5);
/// let (url, params) = build_request("machine learning", &opts, "");
/// assert_eq!(url, "https://api.openalex.org/works");
/// let map: std::collections::HashMap<&str, &str> = params
///     .iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
/// assert_eq!(map["search"], "machine learning");
/// assert_eq!(map["per_page"], "5");
/// assert_eq!(map["page"], "1");
/// assert!(map.get("filter").is_none());
/// assert!(map.get("mailto").is_none());
/// ```
#[must_use]
pub fn build_request(
    query: &str,
    opts: &SearchOptions,
    mailto: &str,
) -> (String, Vec<(String, String)>) {
    let per_page = opts.max_results.clamp(MIN_PER_PAGE, MAX_PER_PAGE);
    let mut params: Vec<(String, String)> = Vec::new();

    // search
    let search = truncate_query(query);
    params.push(("search".to_string(), search));

    // per_page (FR-012)
    params.push(("per_page".to_string(), per_page.to_string()));

    // pagination (FR-008): use cursor=* deep paging when basic page limit
    // would be exceeded, otherwise use 1-indexed page.
    let estimated_offset = opts.page.saturating_mul(per_page);
    if estimated_offset >= BASIC_PAGINATION_LIMIT {
        params.push(("cursor".to_string(), "*".to_string()));
    } else {
        let page = opts.page.saturating_add(1);
        params.push(("page".to_string(), page.to_string()));
    }

    // filter (FR-004 site, FR-005 freshness)
    let filter = build_filter(opts);
    if !filter.is_empty() {
        params.push(("filter".to_string(), filter));
    }

    // mailto (FR-007)
    let mailto_trimmed = mailto.trim();
    if !mailto_trimmed.is_empty() {
        params.push(("mailto".to_string(), mailto_trimmed.to_string()));
    }

    (API_URL.to_string(), params)
}

/// Build the OpenAlex `filter` string from `site` and `freshness` options.
///
/// Returns an empty string when neither filter applies.
fn build_filter(opts: &SearchOptions) -> String {
    let mut parts: Vec<String> = Vec::new();

    // site → primary_location.source.host_organization (FR-004)
    let site = opts.site.trim();
    if !site.is_empty() {
        parts.push(format!("primary_location.source.host_organization:{site}"));
    }

    // freshness → from_/to_publication_date (FR-005)
    if let Some((from, to)) = freshness_to_date_range(opts.freshness) {
        parts.push(format!("from_publication_date:{from}"));
        parts.push(format!("to_publication_date:{to}"));
    }

    parts.join(",")
}

/// Map a [`Freshness`] value to a `(from_date, to_date)` pair in `YYYY-MM-DD`
/// format relative to today's UTC date.
///
/// Returns `None` for [`Freshness::Any`] (no date filter).
fn freshness_to_date_range(freshness: Freshness) -> Option<(String, String)> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let days = match freshness {
        Freshness::Day => 1,
        Freshness::Week => 7,
        Freshness::Month => 30,
        Freshness::Year => 365,
        Freshness::Any => return None,
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let to_ts = now;
    let from_ts = now - days * 86_400;

    Some((date_string(from_ts), date_string(to_ts)))
}

/// Convert a Unix timestamp (seconds) to a `YYYY-MM-DD` UTC date string.
fn date_string(secs: i64) -> String {
    let days = secs.div_euclid(86_400);
    // Civil-from-days algorithm (Howard Hinnant). days since 1970-01-01.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1_460 + doe / 36_524) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 }; // [1, 12]
    let year = if m <= 2 { y + 1 } else { y };
    format!("{year:04}-{m:02}-{d:02}")
}

/// Truncate a search query to [`MAX_QUERY_CHARS`] characters, respecting UTF-8
/// character boundaries.
#[must_use]
pub fn truncate_query(query: &str) -> String {
    let trimmed = query.trim();
    if trimmed.chars().count() <= MAX_QUERY_CHARS {
        return trimmed.to_string();
    }
    trimmed.chars().take(MAX_QUERY_CHARS).collect()
}

// ---------------------------------------------------------------------------
// Response parsing (pure, testable) — FR-001, FR-010, FR-011
// ---------------------------------------------------------------------------

/// Parse an OpenAlex JSON response into [`RawResult`]s.
///
/// Expects the shape `{ "results": [ … ] }`, where each work object contains
/// `id`, `title`, `relevance_score`, `abstract_inverted_index`,
/// `publication_year`, `cited_by_count`, `doi`, `open_access`, and
/// `primary_location`. Results are returned with `source` set to `"openalex"`.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::openalex::parse_response;
/// use serde_json::json;
///
/// let value = json!({
///     "results": [
///         {
///             "id": "https://openalex.org/W1",
///             "title": "Test Paper",
///             "relevance_score": 12.5,
///             "publication_year": 2024,
///             "cited_by_count": 42,
///             "doi": "https://doi.org/10.1000/test",
///             "primary_location": {
///                 "landing_page_url": "https://example.org/paper",
///                 "source": { "display_name": "Example Journal" }
///             },
///             "open_access": { "is_oa": true, "oa_url": "https://example.org/oa" }
///         }
///     ]
/// });
/// let results = parse_response(&value);
/// assert_eq!(results.len(), 1);
/// assert_eq!(results[0].source, "openalex");
/// assert_eq!(results[0].title, "Test Paper");
/// assert_eq!(results[0].url, "https://example.org/paper");
/// assert!(results[0].score.is_some());
/// assert!(results[0].snippet.contains("2024"));
/// assert!(results[0].snippet.contains("42"));
/// ```
#[must_use]
pub fn parse_response(value: &serde_json::Value) -> Vec<RawResult> {
    let results = match value.get("results").and_then(|r| r.as_array()) {
        Some(arr) => arr,
        None => return Vec::new(),
    };

    results
        .iter()
        .filter_map(|work| {
            let title = work
                .get("title")
                .and_then(|t| t.as_str())
                .unwrap_or_default()
                .to_string();

            // URL: primary_location.landing_page_url → id → doi (FR-010)
            let url = work
                .get("primary_location")
                .and_then(|pl| pl.get("landing_page_url"))
                .and_then(|u| u.as_str())
                .filter(|s| !s.is_empty())
                .map(String::from)
                .or_else(|| work.get("id").and_then(|i| i.as_str()).map(String::from))
                .or_else(|| work.get("doi").and_then(|d| d.as_str()).map(String::from))
                .unwrap_or_default();

            // score: relevance_score normalised to 0.0–1.0 (FR-011)
            let score = work
                .get("relevance_score")
                .and_then(|s| s.as_f64())
                .map(normalise_relevance);

            // snippet: reconstruct abstract + scholarly metadata (FR-001, FR-010)
            let snippet = build_snippet(work);

            if title.is_empty() && url.is_empty() {
                return None;
            }

            // authors: join `authorships[*].author.display_name` (FR-010)
            // so downstream research outputs can attribute the work without
            // fetching the paywalled landing page.
            let author = extract_authors(work);

            let mut result = RawResult::new(title, url, snippet, ENGINE_NAME);
            result.score = score;
            result.author = author;
            Some(result)
        })
        .collect()
}

/// Normalise an OpenAlex `relevance_score` to the 0.0–1.0 range (FR-011).
///
/// OpenAlex relevance scores are unbounded positive floats (typically 1–100+).
/// We apply a soft normalisation: divide by a scale factor and clamp to 1.0.
/// The scale factor of 30.0 maps typical scores (~5–30) into the 0.17–1.0
/// band, which fits the consensus ranker's expected range.
fn normalise_relevance(score: f64) -> f64 {
    const SCALE: f64 = 30.0;
    (score / SCALE).clamp(0.0, 1.0)
}

/// Build a snippet for an OpenAlex work, combining the reconstructed abstract
/// with scholarly metadata (publication year, citation count, OA status,
/// source name) (FR-001, FR-010).
///
/// The abstract is reconstructed from the `abstract_inverted_index` (a map of
/// word → list of positions) and truncated to ~200 characters. Metadata is
/// appended as a compact suffix.
fn build_snippet(work: &serde_json::Value) -> String {
    let abstract_text = reconstruct_abstract(
        work.get("abstract_inverted_index")
            .and_then(|a| a.as_object()),
    );

    let mut parts: Vec<String> = Vec::new();
    if !abstract_text.is_empty() {
        parts.push(truncate_snippet(&abstract_text));
    }

    // Scholarly metadata suffix (FR-001).
    let meta = build_metadata_suffix(work);
    if !meta.is_empty() {
        parts.push(meta);
    }

    parts.join(" ")
}

/// Reconstruct an abstract from OpenAlex's inverted index representation.
///
/// The `abstract_inverted_index` maps each word to a list of positions. We
/// invert it into a position → word map and join the words in order.
fn reconstruct_abstract(inverted: Option<&serde_json::Map<String, serde_json::Value>>) -> String {
    let inverted = match inverted {
        Some(idx) => idx,
        None => return String::new(),
    };

    // Build a vec of (position, word) pairs, then sort by position.
    let mut words: Vec<(usize, String)> = Vec::new();
    for (word, positions) in inverted {
        if let Some(pos_arr) = positions.as_array() {
            for pos in pos_arr {
                if let Some(p) = pos.as_u64() {
                    words.push((p as usize, word.clone()));
                }
            }
        }
    }
    words.sort_by_key(|(p, _)| *p);
    let text: Vec<&str> = words.iter().map(|(_, w)| w.as_str()).collect();
    text.join(" ")
}

/// Extract author display names from an OpenAlex work's `authorships` array.
///
/// OpenAlex models each contributor as an entry in `authorships` with a nested
/// `author.display_name`. Names are joined with `, ` and returned as a single
/// string; returns `None` when the work exposes no authorship information.
fn extract_authors(work: &serde_json::Value) -> Option<String> {
    let names: Vec<&str> = work
        .get("authorships")
        .and_then(|a| a.as_array())?
        .iter()
        .filter_map(|a| a.get("author"))
        .filter_map(|a| a.get("display_name"))
        .filter_map(|n| n.as_str())
        .map(str::trim)
        .filter(|n| !n.is_empty())
        .collect();
    if names.is_empty() {
        None
    } else {
        Some(names.join(", "))
    }
}

/// Build a compact scholarly-metadata suffix for a work (FR-001).
///
/// Format: `(Year: YYYY | Cited: N | OA: yes/no | Source: <name>)`, omitting
/// any unavailable fields.
fn build_metadata_suffix(work: &serde_json::Value) -> String {
    let mut fields: Vec<String> = Vec::new();

    if let Some(year) = work.get("publication_year").and_then(|y| y.as_u64()) {
        fields.push(format!("Year: {year}"));
    }

    if let Some(cited) = work.get("cited_by_count").and_then(|c| c.as_u64()) {
        fields.push(format!("Cited: {cited}"));
    }

    if let Some(oa) = work
        .get("open_access")
        .and_then(|o| o.get("is_oa"))
        .and_then(|v| v.as_bool())
    {
        fields.push(format!("OA: {}", if oa { "yes" } else { "no" }));
    }

    if let Some(source) = work
        .get("primary_location")
        .and_then(|pl| pl.get("source"))
        .and_then(|s| s.get("display_name"))
        .and_then(|n| n.as_str())
    {
        fields.push(format!("Source: {source}"));
    }

    if fields.is_empty() {
        String::new()
    } else {
        format!("({})", fields.join(" | "))
    }
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

// ---------------------------------------------------------------------------
// Email masking helper (NFR-003)
// ---------------------------------------------------------------------------

/// Mask an email address for display.
///
/// Keeps the first two and last two characters; everything in between is
/// replaced with `*`. Strings shorter than six characters are fully masked.
#[must_use]
pub fn mask_email(email: &str) -> String {
    let len = email.chars().count();
    if len <= 6 {
        return "*".repeat(len);
    }
    let first: String = email.chars().take(2).collect();
    let last: String = email
        .chars()
        .rev()
        .take(2)
        .collect::<String>()
        .chars()
        .rev()
        .collect();
    format!("{first}*{}*{last}", "*".repeat(len.saturating_sub(6)))
}
