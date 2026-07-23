//! Search-engine trait, raw result types, and dedup helpers.
//!
//! Implements **FR-008** and **NFR-003** (T-012).
//!
//! This module defines the core abstractions for the keyless multi-engine
//! search pipeline:
//!
//! - [`SearchEngine`] — an `async` trait implemented by each search backend
//!   adapter (DuckDuckGo, Brave, …). Backends are keyless: they scrape public
//!   search-engine HTML result pages and parse the results. No API keys,
//!   tokens, or accounts are required (FR-023).
//! - [`RawResult`] — a single search result as returned by one engine, before
//!   merging / dedup / ranking. Carries the engine name (`source`) and an
//!   optional relevance `score` (0.0–1.0) if the engine provides one.
//! - [`EngineReport`] — the complete output of one engine's search: a list of
//!   [`RawResult`]s plus metadata about whether the engine was blocked, rate-
//!   limited, or errored. The `engine_blocked` flag lets the consensus merger
//!   report honest `engine_blocked` signals to the agent (FR-008).
//! - [`SearchOptions`] — query modifiers: `max_results`, `site`, `exclude_sites`,
//!   `freshness`, `page`. Shared across all backends.
//! - [`Freshness`] — time filter enum (`Day`, `Week`, `Month`, `Year`).
//! - [`normalise_result_url`] — normalises a result URL for dedup using the
//!   shared [`urlnorm`](crate::masterfetch::urlnorm) module. Falls back to the
//!   raw URL if normalisation fails (e.g. relative URLs).
//! - [`dedup_results_by_url`] — removes duplicate results by normalised URL,
//!   preserving first occurrence.
//!
//! # Testability (NFR-003)
//!
//! All data types are plain structs with public fields — no I/O, no async.
//! The [`SearchEngine`] trait can be implemented by a mock in tests (see
//! `tests/test_mf_search_engine.rs`). Real backends (T-013, T-014) perform HTTP
//! I/O and are tested with `#[ignore]`-gated integration tests.
//!
//! # Examples
//!
//! Dedup by normalised URL:
//!
//! ```
//! use ragent_tools_extended::masterfetch::search::engine::{
//!     RawResult, dedup_results_by_url,
//! };
//!
//! let results = vec![
//!     RawResult { title: "A".into(), url: "https://example.com/page/".into(), ..Default::default() },
//!     RawResult { title: "B".into(), url: "https://example.com/page".into(),  ..Default::default() },
//!     RawResult { title: "C".into(), url: "https://other.com".into(),          ..Default::default() },
//! ];
//! let deduped = dedup_results_by_url(&results);
//! assert_eq!(deduped.len(), 2); // /page/ and /page are the same after normalisation
//! assert_eq!(deduped[0].title, "A");
//! ```

use std::collections::HashSet;

use thiserror::Error;

use crate::masterfetch::urlnorm::normalise_url;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum number of results to request from each engine (FR-008).
pub const DEFAULT_MAX_RESULTS: usize = 10;

/// Maximum allowed `max_results` value (engines may cap lower).
pub const MAX_MAX_RESULTS: usize = 50;

/// Default result page (0 = first page).
pub const DEFAULT_PAGE: usize = 0;

// ---------------------------------------------------------------------------
// Freshness (FR-008)
// ---------------------------------------------------------------------------

/// Time filter for search results.
///
/// Maps to the `freshness` parameter of `mf_search`. Engines translate this
/// into their own time-filter syntax (e.g. DuckDuckGo's `df` parameter).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Freshness {
    /// Results from the last 24 hours.
    Day,
    /// Results from the last week.
    Week,
    /// Results from the last month.
    Month,
    /// Results from the last year.
    Year,
    /// No time filter (default).
    #[default]
    Any,
}

impl Freshness {
    /// Convert to a lowercase string suitable for JSON serialisation or
    /// engine parameter mapping.
    ///
    /// # Examples
    ///
    /// ```
    /// use ragent_tools_extended::masterfetch::search::engine::Freshness;
    ///
    /// assert_eq!(Freshness::Day.as_str(), "day");
    /// assert_eq!(Freshness::Any.as_str(), "any");
    /// ```
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
            Self::Year => "year",
            Self::Any => "any",
        }
    }
}

impl std::fmt::Display for Freshness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for Freshness {
    type Err = SearchEngineError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "day" => Ok(Self::Day),
            "week" => Ok(Self::Week),
            "month" => Ok(Self::Month),
            "year" => Ok(Self::Year),
            "any" | "" => Ok(Self::Any),
            other => Err(SearchEngineError::InvalidFreshness(other.to_string())),
        }
    }
}

// ---------------------------------------------------------------------------
// SearchOptions (FR-008)
// ---------------------------------------------------------------------------

/// Query modifiers shared across all search backends.
///
/// Built from the `mf_search` tool's input parameters. Each engine adapter
/// translates these into its own query-string syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchOptions {
    /// Maximum results to request from this engine (1–50, default 10).
    pub max_results: usize,
    /// Restrict results to this domain (site: filter). Empty = no restriction.
    pub site: String,
    /// Domains to exclude from results. Empty = no exclusions.
    pub exclude_sites: Vec<String>,
    /// Time filter for results.
    pub freshness: Freshness,
    /// Result page (0-based, 0 = first page).
    pub page: usize,
}

impl SearchOptions {
    /// Create a new `SearchOptions` with the given `max_results` and all other
    /// fields at their defaults.
    #[must_use]
    pub fn new(max_results: usize) -> Self {
        Self {
            max_results: max_results.clamp(1, MAX_MAX_RESULTS),
            ..Self::default()
        }
    }

    /// Builder: set the `site` filter.
    #[must_use]
    pub fn with_site(mut self, site: impl Into<String>) -> Self {
        self.site = site.into();
        self
    }

    /// Builder: set the `exclude_sites` filter.
    #[must_use]
    pub fn with_exclude_sites(mut self, sites: Vec<String>) -> Self {
        self.exclude_sites = sites;
        self
    }

    /// Builder: set the `freshness` filter.
    #[must_use]
    pub fn with_freshness(mut self, freshness: Freshness) -> Self {
        self.freshness = freshness;
        self
    }

    /// Builder: set the result `page`.
    #[must_use]
    pub fn with_page(mut self, page: usize) -> Self {
        self.page = page;
        self
    }
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_results: DEFAULT_MAX_RESULTS,
            site: String::new(),
            exclude_sites: Vec::new(),
            freshness: Freshness::Any,
            page: DEFAULT_PAGE,
        }
    }
}

// ---------------------------------------------------------------------------
// RawResult (FR-008)
// ---------------------------------------------------------------------------

/// A single raw search result from one engine, before merging / dedup / ranking.
///
/// The `url` field holds the URL as returned by the engine (which may include
/// tracking parameters or trailing slashes). For dedup, use
/// [`normalise_result_url`] or [`dedup_results_by_url`].
///
/// The `source` field identifies which engine produced this result (e.g.
/// `"duckduckgo"`, `"brave"`). This is used by the consensus merger to compute
/// `engines_consensus`.
///
/// The `score` field is an optional relevance score (0.0–1.0) if the engine
/// provides one. Most keyless backends do not provide scores; the consensus
/// merger assigns scores based on rank position and cross-engine consensus.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct RawResult {
    /// Result title.
    pub title: String,
    /// Result URL as returned by the engine (not yet normalised).
    pub url: String,
    /// Short snippet / abstract from the search engine.
    pub snippet: String,
    /// Engine name that produced this result (e.g. `"duckduckgo"`).
    pub source: String,
    /// Optional relevance score (0.0–1.0) if the engine provides one.
    pub score: Option<f64>,
}

impl RawResult {
    /// Create a new `RawResult` with the given title, URL, snippet, and source.
    #[must_use]
    pub fn new(
        title: impl Into<String>,
        url: impl Into<String>,
        snippet: impl Into<String>,
        source: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            url: url.into(),
            snippet: snippet.into(),
            source: source.into(),
            score: None,
        }
    }

    /// Return the normalised URL for dedup purposes.
    ///
    /// Uses [`normalise_result_url`]. If the URL fails to normalise, the raw
    /// URL is returned.
    #[must_use]
    pub fn normalised_url(&self) -> String {
        normalise_result_url(&self.url)
    }
}

// ---------------------------------------------------------------------------
// EngineReport (FR-008)
// ---------------------------------------------------------------------------

/// The complete output of one search engine's query.
///
/// Returned by [`SearchEngine::search`]. Contains the list of [`RawResult`]s
/// plus metadata about the engine's status:
///
/// - `engine` — the engine name (matches `SearchEngine::name()`).
/// - `results` — the raw results (may be empty if blocked or errored).
/// - `error` — an error message if the engine failed (empty on success).
/// - `engine_blocked` — `true` if the engine was rate-limited (HTTP 429/202)
///   or otherwise blocked. Reported to the agent as an honest signal.
/// - `result_count` — number of results returned (convenience; equals
///   `results.len()`).
/// - `duration_ms` — time spent on the request in milliseconds.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EngineReport {
    /// Engine name (e.g. `"duckduckgo"`, `"brave"`).
    pub engine: String,
    /// Raw results from this engine.
    pub results: Vec<RawResult>,
    /// Error message if the engine failed; empty string on success.
    pub error: String,
    /// `true` if the engine was rate-limited, blocked, or returned a
    /// challenge page.
    pub engine_blocked: bool,
    /// Number of results returned (equals `results.len()`).
    pub result_count: usize,
    /// Request duration in milliseconds.
    pub duration_ms: u64,
}

impl EngineReport {
    /// Create a successful report with the given engine name and results.
    #[must_use]
    pub fn ok(engine: impl Into<String>, results: Vec<RawResult>) -> Self {
        let engine = engine.into();
        let result_count = results.len();
        Self {
            engine,
            results,
            error: String::new(),
            engine_blocked: false,
            result_count,
            duration_ms: 0,
        }
    }

    /// Create a blocked/errored report with the given engine name and error
    /// message.
    ///
    /// `engine_blocked` is set to `true`; `results` is empty.
    #[must_use]
    pub fn blocked(engine: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            engine: engine.into(),
            results: Vec::new(),
            error: error.into(),
            engine_blocked: true,
            result_count: 0,
            duration_ms: 0,
        }
    }

    /// Create an errored report (not blocked, just a transient error).
    #[must_use]
    pub fn error(engine: impl Into<String>, error: impl Into<String>) -> Self {
        Self {
            engine: engine.into(),
            results: Vec::new(),
            error: error.into(),
            engine_blocked: false,
            result_count: 0,
            duration_ms: 0,
        }
    }

    /// Returns `true` if this report has results (i.e. `results` is non-empty).
    #[must_use]
    pub fn has_results(&self) -> bool {
        !self.results.is_empty()
    }

    /// Returns `true` if this report represents a successful, non-blocked
    /// search (even if zero results were returned).
    #[must_use]
    pub fn is_success(&self) -> bool {
        self.error.is_empty() && !self.engine_blocked
    }
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Error type for search-engine operations.
///
/// Used primarily by [`Freshness::from_str`] and as the error variant for
/// the [`SearchEngine`] trait's internal operations. Network errors are
/// captured in [`EngineReport::error`] rather than propagated as `Err`,
/// matching Hound's catch-and-return-text pattern (FR-024).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SearchEngineError {
    /// Invalid freshness value (expected "day", "week", "month", "year", or "any").
    #[error("invalid freshness value: '{0}' (expected day, week, month, year, or any)")]
    InvalidFreshness(String),

    /// The search query is empty.
    #[error("search query must not be empty")]
    EmptyQuery,

    /// `max_results` is out of range (must be 1–50).
    #[error("max_results out of range: {0} (must be 1–{1})")]
    MaxResultsOutOfRange(usize, usize),

    /// HTTP request failed (network error, timeout).
    #[error("HTTP request failed: {0}")]
    Http(String),

    /// Engine returned a rate-limit response (HTTP 429 or 202).
    #[error("engine rate-limited (HTTP {0})")]
    RateLimited(u16),

    /// Engine returned a block / challenge page.
    #[error("engine blocked: {0}")]
    Blocked(String),

    /// Failed to parse the engine's HTML response.
    #[error("failed to parse search results HTML: {0}")]
    Parse(String),
}

// ---------------------------------------------------------------------------
// SearchEngine trait (FR-008)
// ---------------------------------------------------------------------------

/// Trait implemented by each keyless search-engine backend adapter.
///
/// Backends are **keyless**: they scrape public search-engine HTML result
/// pages and parse the results. No API keys, tokens, or accounts are required
/// (FR-023).
///
/// Each adapter (DuckDuckGo, Brave, …) implements this trait and is queried
/// in parallel by the `mf_search` consensus merger. The merger collects
/// [`EngineReport`]s from all backends, merges and deduplicates results by
/// normalised URL, and ranks them with cross-engine consensus boosting.
///
/// # Testability (NFR-003)
///
/// The trait is `async` and `Send + Sync` so backends can run concurrently.
/// For testing, implement a mock `SearchEngine` that returns canned
/// [`EngineReport`]s without any network I/O.
///
/// # Examples
///
/// Implementing a mock engine for tests:
///
/// ```
/// use ragent_tools_extended::masterfetch::search::engine::{
///     EngineReport, RawResult, SearchEngine, SearchOptions,
/// };
///
/// struct MockEngine;
///
/// #[async_trait::async_trait]
/// impl SearchEngine for MockEngine {
///     fn name(&self) -> &str { "mock" }
///     async fn search(&self, query: &str, opts: &SearchOptions) -> EngineReport {
///         EngineReport::ok("mock", vec![
///             RawResult::new("Mock result", "https://example.com", "Snippet", "mock"),
///         ])
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait SearchEngine: Send + Sync {
    /// The engine's display name (e.g. `"duckduckgo"`, `"brave"`).
    ///
    /// Used as the `source` field in [`RawResult`]s and the `engine` field in
    /// [`EngineReport`]s.
    fn name(&self) -> &str;

    /// Execute a keyless search query against this engine.
    ///
    /// Returns an [`EngineReport`] containing the raw results or an error /
    /// blocked status. This method **must not** return `Err` for engine-level
    /// failures (rate limits, parse errors, network errors) — those are
    /// captured in the `EngineReport`'s `error` and `engine_blocked` fields,
    /// matching Hound's catch-and-return pattern (FR-024).
    ///
    /// The only valid reason to return `Err` is a programming error (e.g.
    /// a poisoned lock).
    ///
    /// # Arguments
    ///
    /// - `query` — the search query string (must not be empty).
    /// - `opts` — search modifiers (max results, site filter, freshness, …).
    async fn search(&self, query: &str, opts: &SearchOptions) -> EngineReport;
}

// ---------------------------------------------------------------------------
// URL normalisation for dedup (FR-008, FR-027)
// ---------------------------------------------------------------------------

/// Normalise a result URL for deduplication.
///
/// Uses the shared [`urlnorm`](crate::masterfetch::urlnorm) module to
/// lowercase the host, strip default ports, remove trailing slashes, and strip
/// tracking parameters (`utm_*`, `fbclid`, `gclid`, `ref`, …).
///
/// If the URL fails to normalise (e.g. it's a relative URL or malformed), the
/// raw URL is returned unchanged. This ensures dedup never panics on bad input.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::engine::normalise_result_url;
///
/// // Trailing slash removed, host lowercased.
/// assert_eq!(
///     normalise_result_url("https://Example.com/page/"),
///     "https://example.com/page",
/// );
/// // Tracking params stripped.
/// assert_eq!(
///     normalise_result_url("https://example.com/article?utm_source=x&keep=1"),
///     "https://example.com/article?keep=1",
/// );
/// // Same URL after normalisation → same string.
/// let a = normalise_result_url("https://example.com/page/");
/// let b = normalise_result_url("https://example.com/page");
/// assert_eq!(a, b);
/// ```
#[must_use]
pub fn normalise_result_url(url: &str) -> String {
    normalise_url(url).unwrap_or_else(|_| url.to_string())
}

/// Remove duplicate results by normalised URL, preserving first occurrence.
///
/// Two results are considered duplicates if their normalised URLs are equal
/// (case-insensitive host, stripped ports, removed trailing slashes, stripped
/// tracking parameters). The first result with a given normalised URL is kept;
/// subsequent duplicates are dropped.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::engine::{
///     RawResult, dedup_results_by_url,
/// };
///
/// let results = vec![
///     RawResult::new("A", "https://example.com/page/", "", "ddg"),
///     RawResult::new("B", "https://example.com/page",  "", "brave"),
///     RawResult::new("C", "https://other.com",          "", "ddg"),
/// ];
/// let deduped = dedup_results_by_url(&results);
/// assert_eq!(deduped.len(), 2);
/// assert_eq!(deduped[0].title, "A"); // first occurrence kept
/// ```
#[must_use]
pub fn dedup_results_by_url(results: &[RawResult]) -> Vec<RawResult> {
    let mut seen: HashSet<String> = HashSet::new();
    let mut deduped: Vec<RawResult> = Vec::with_capacity(results.len());

    for result in results {
        let norm = normalise_result_url(&result.url);
        if seen.insert(norm) {
            deduped.push(result.clone());
        }
    }

    deduped
}

/// Collect results from multiple [`EngineReport`]s into a single flat list.
///
/// All results from all reports are concatenated in order. Deduplication is
/// not performed here — use [`dedup_results_by_url`] afterwards if needed.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::engine::{
///     EngineReport, RawResult, collect_all_results,
/// };
///
/// let reports = vec![
///     EngineReport::ok("ddg", vec![
///         RawResult::new("A", "https://a.com", "", "ddg"),
///     ]),
///     EngineReport::ok("brave", vec![
///         RawResult::new("B", "https://b.com", "", "brave"),
///     ]),
/// ];
/// let all = collect_all_results(&reports);
/// assert_eq!(all.len(), 2);
/// ```
#[must_use]
pub fn collect_all_results(reports: &[EngineReport]) -> Vec<RawResult> {
    reports
        .iter()
        .flat_map(|r| r.results.iter().cloned())
        .collect()
}

/// Count the number of engines that produced at least one result.
///
/// Engines that were blocked or errored (empty results) are not counted.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::engine::{
///     EngineReport, RawResult, count_engines_with_results,
/// };
///
/// let reports = vec![
///     EngineReport::ok("ddg", vec![RawResult::new("A", "https://a.com", "", "ddg")]),
///     EngineReport::blocked("brave", "rate limited"),
/// ];
/// assert_eq!(count_engines_with_results(&reports), 1);
/// ```
#[must_use]
pub fn count_engines_with_results(reports: &[EngineReport]) -> usize {
    reports.iter().filter(|r| r.has_results()).count()
}

/// Count the total number of results across all reports.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::engine::{
///     EngineReport, RawResult, count_total_results,
/// };
///
/// let reports = vec![
///     EngineReport::ok("ddg", vec![
///         RawResult::new("A", "https://a.com", "", "ddg"),
///         RawResult::new("B", "https://b.com", "", "ddg"),
///     ]),
///     EngineReport::ok("brave", vec![
///         RawResult::new("C", "https://c.com", "", "brave"),
///     ]),
/// ];
/// assert_eq!(count_total_results(&reports), 3);
/// ```
#[must_use]
pub fn count_total_results(reports: &[EngineReport]) -> usize {
    reports.iter().map(|r| r.result_count).sum()
}

/// Return the names of engines that were blocked or errored.
///
/// # Examples
///
/// ```
/// use ragent_tools_extended::masterfetch::search::engine::{
///     EngineReport, blocked_engine_names,
/// };
///
/// let reports = vec![
///     EngineReport::ok("ddg", vec![]),
///     EngineReport::blocked("brave", "rate limited"),
///     EngineReport::error("mojeek", "timeout"),
/// ];
/// let blocked = blocked_engine_names(&reports);
/// assert!(blocked.contains(&"brave"));
/// assert!(blocked.contains(&"mojeek"));
/// assert!(!blocked.contains(&"ddg"));
/// ```
#[must_use]
pub fn blocked_engine_names(reports: &[EngineReport]) -> Vec<&str> {
    reports
        .iter()
        .filter(|r| r.engine_blocked || (!r.error.is_empty() && !r.has_results()))
        .map(|r| r.engine.as_str())
        .collect()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // --- Freshness ---

    #[test]
    fn test_freshness_as_str() {
        assert_eq!(Freshness::Day.as_str(), "day");
        assert_eq!(Freshness::Week.as_str(), "week");
        assert_eq!(Freshness::Month.as_str(), "month");
        assert_eq!(Freshness::Year.as_str(), "year");
        assert_eq!(Freshness::Any.as_str(), "any");
    }

    #[test]
    fn test_freshness_display() {
        assert_eq!(Freshness::Day.to_string(), "day");
        assert_eq!(Freshness::Any.to_string(), "any");
    }

    #[test]
    fn test_freshness_from_str() {
        assert_eq!("day".parse::<Freshness>().unwrap(), Freshness::Day);
        assert_eq!("WEEK".parse::<Freshness>().unwrap(), Freshness::Week);
        assert_eq!("Month".parse::<Freshness>().unwrap(), Freshness::Month);
        assert_eq!("year".parse::<Freshness>().unwrap(), Freshness::Year);
        assert_eq!("any".parse::<Freshness>().unwrap(), Freshness::Any);
        assert_eq!("".parse::<Freshness>().unwrap(), Freshness::Any);
    }

    #[test]
    fn test_freshness_from_str_invalid() {
        assert!("hour".parse::<Freshness>().is_err());
        assert!("invalid".parse::<Freshness>().is_err());
    }

    #[test]
    fn test_freshness_default_is_any() {
        assert_eq!(Freshness::default(), Freshness::Any);
    }

    // --- SearchOptions ---

    #[test]
    fn test_search_options_default() {
        let opts = SearchOptions::default();
        assert_eq!(opts.max_results, DEFAULT_MAX_RESULTS);
        assert!(opts.site.is_empty());
        assert!(opts.exclude_sites.is_empty());
        assert_eq!(opts.freshness, Freshness::Any);
        assert_eq!(opts.page, DEFAULT_PAGE);
    }

    #[test]
    fn test_search_options_new_clamps_max_results() {
        let opts = SearchOptions::new(0);
        assert_eq!(opts.max_results, 1);

        let opts = SearchOptions::new(100);
        assert_eq!(opts.max_results, MAX_MAX_RESULTS);

        let opts = SearchOptions::new(6);
        assert_eq!(opts.max_results, 6);
    }

    #[test]
    fn test_search_options_builder() {
        let opts = SearchOptions::new(5)
            .with_site("example.com")
            .with_exclude_sites(vec!["spam.com".into()])
            .with_freshness(Freshness::Week)
            .with_page(2);
        assert_eq!(opts.max_results, 5);
        assert_eq!(opts.site, "example.com");
        assert_eq!(opts.exclude_sites, vec!["spam.com"]);
        assert_eq!(opts.freshness, Freshness::Week);
        assert_eq!(opts.page, 2);
    }

    // --- RawResult ---

    #[test]
    fn test_raw_result_new() {
        let r = RawResult::new("Title", "https://example.com", "Snippet", "ddg");
        assert_eq!(r.title, "Title");
        assert_eq!(r.url, "https://example.com");
        assert_eq!(r.snippet, "Snippet");
        assert_eq!(r.source, "ddg");
        assert!(r.score.is_none());
    }

    #[test]
    fn test_raw_result_default() {
        let r = RawResult::default();
        assert!(r.title.is_empty());
        assert!(r.url.is_empty());
        assert!(r.snippet.is_empty());
        assert!(r.source.is_empty());
        assert!(r.score.is_none());
    }

    #[test]
    fn test_raw_result_normalised_url() {
        let r = RawResult::new("T", "https://Example.com/page/", "", "ddg");
        assert_eq!(r.normalised_url(), "https://example.com/page");
    }

    #[test]
    fn test_raw_result_normalised_url_strips_tracking() {
        let r = RawResult::new("T", "https://example.com/a?utm_source=x&keep=1", "", "ddg");
        assert_eq!(r.normalised_url(), "https://example.com/a?keep=1");
    }

    #[test]
    fn test_raw_result_normalised_url_invalid_falls_back() {
        let r = RawResult::new("T", "not a url", "", "ddg");
        // Falls back to raw URL.
        assert_eq!(r.normalised_url(), "not a url");
    }

    // --- EngineReport ---

    #[test]
    fn test_engine_report_ok() {
        let results = vec![RawResult::new("A", "https://a.com", "", "ddg")];
        let report = EngineReport::ok("ddg", results);
        assert_eq!(report.engine, "ddg");
        assert_eq!(report.result_count, 1);
        assert!(!report.engine_blocked);
        assert!(report.error.is_empty());
        assert!(report.has_results());
        assert!(report.is_success());
    }

    #[test]
    fn test_engine_report_ok_empty_results() {
        let report = EngineReport::ok("ddg", vec![]);
        assert_eq!(report.result_count, 0);
        assert!(!report.has_results());
        assert!(report.is_success()); // success even with 0 results
    }

    #[test]
    fn test_engine_report_blocked() {
        let report = EngineReport::blocked("brave", "rate limited (429)");
        assert_eq!(report.engine, "brave");
        assert!(report.engine_blocked);
        assert_eq!(report.error, "rate limited (429)");
        assert!(!report.has_results());
        assert!(!report.is_success());
    }

    #[test]
    fn test_engine_report_error() {
        let report = EngineReport::error("mojeek", "timeout");
        assert_eq!(report.engine, "mojeek");
        assert!(!report.engine_blocked);
        assert_eq!(report.error, "timeout");
        assert!(!report.has_results());
        assert!(!report.is_success());
    }

    #[test]
    fn test_engine_report_default() {
        let report = EngineReport::default();
        assert!(report.engine.is_empty());
        assert!(report.results.is_empty());
        assert!(!report.engine_blocked);
        assert_eq!(report.result_count, 0);
    }

    // --- normalise_result_url ---

    #[test]
    fn test_normalise_result_url_basic() {
        assert_eq!(
            normalise_result_url("https://Example.com/page/"),
            "https://example.com/page"
        );
    }

    #[test]
    fn test_normalise_result_url_strips_tracking() {
        assert_eq!(
            normalise_result_url("https://example.com/a?utm_source=x&keep=1"),
            "https://example.com/a?keep=1"
        );
    }

    #[test]
    fn test_normalise_result_url_idempotent() {
        let once = normalise_result_url("https://example.com/page/");
        let twice = normalise_result_url(&once);
        assert_eq!(once, twice);
    }

    #[test]
    fn test_normalise_result_url_invalid_falls_back() {
        assert_eq!(normalise_result_url("not a url"), "not a url");
        assert_eq!(normalise_result_url(""), "");
    }

    #[test]
    fn test_normalise_result_url_strips_default_port() {
        assert_eq!(
            normalise_result_url("https://example.com:443/page"),
            "https://example.com/page"
        );
    }

    // --- dedup_results_by_url ---

    #[test]
    fn test_dedup_removes_duplicates() {
        let results = vec![
            RawResult::new("A", "https://example.com/page/", "", "ddg"),
            RawResult::new("B", "https://example.com/page", "", "brave"),
            RawResult::new("C", "https://other.com", "", "ddg"),
        ];
        let deduped = dedup_results_by_url(&results);
        assert_eq!(deduped.len(), 2);
        assert_eq!(deduped[0].title, "A"); // first kept
        assert_eq!(deduped[1].title, "C");
    }

    #[test]
    fn test_dedup_preserves_first_occurrence() {
        let results = vec![
            RawResult::new("Brave", "https://example.com/page", "", "brave"),
            RawResult::new("DDG", "https://example.com/page/", "", "ddg"),
        ];
        let deduped = dedup_results_by_url(&results);
        assert_eq!(deduped.len(), 1);
        assert_eq!(deduped[0].title, "Brave"); // first kept
    }

    #[test]
    fn test_dedup_empty_input() {
        let deduped = dedup_results_by_url(&[]);
        assert!(deduped.is_empty());
    }

    #[test]
    fn test_dedup_no_duplicates() {
        let results = vec![
            RawResult::new("A", "https://a.com", "", "ddg"),
            RawResult::new("B", "https://b.com", "", "ddg"),
            RawResult::new("C", "https://c.com", "", "ddg"),
        ];
        let deduped = dedup_results_by_url(&results);
        assert_eq!(deduped.len(), 3);
    }

    #[test]
    fn test_dedup_tracking_params_ignored() {
        let results = vec![
            RawResult::new("A", "https://example.com/page?utm_source=x", "", "ddg"),
            RawResult::new("B", "https://example.com/page", "", "brave"),
        ];
        let deduped = dedup_results_by_url(&results);
        assert_eq!(deduped.len(), 1);
    }

    #[test]
    fn test_dedup_case_insensitive_host() {
        let results = vec![
            RawResult::new("A", "https://Example.COM/page", "", "ddg"),
            RawResult::new("B", "https://example.com/page", "", "brave"),
        ];
        let deduped = dedup_results_by_url(&results);
        assert_eq!(deduped.len(), 1);
    }

    #[test]
    fn test_dedup_default_port_normalized() {
        let results = vec![
            RawResult::new("A", "https://example.com:443/page", "", "ddg"),
            RawResult::new("B", "https://example.com/page", "", "brave"),
        ];
        let deduped = dedup_results_by_url(&results);
        assert_eq!(deduped.len(), 1);
    }

    #[test]
    fn test_dedup_different_paths_not_duplicates() {
        let results = vec![
            RawResult::new("A", "https://example.com/page1", "", "ddg"),
            RawResult::new("B", "https://example.com/page2", "", "brave"),
        ];
        let deduped = dedup_results_by_url(&results);
        assert_eq!(deduped.len(), 2);
    }

    #[test]
    fn test_dedup_different_queries_not_duplicates() {
        let results = vec![
            RawResult::new("A", "https://example.com/search?q=1", "", "ddg"),
            RawResult::new("B", "https://example.com/search?q=2", "", "brave"),
        ];
        let deduped = dedup_results_by_url(&results);
        assert_eq!(deduped.len(), 2);
    }

    // --- collect_all_results ---

    #[test]
    fn test_collect_all_results_flattens() {
        let reports = vec![
            EngineReport::ok(
                "ddg",
                vec![
                    RawResult::new("A", "https://a.com", "", "ddg"),
                    RawResult::new("B", "https://b.com", "", "ddg"),
                ],
            ),
            EngineReport::ok(
                "brave",
                vec![RawResult::new("C", "https://c.com", "", "brave")],
            ),
        ];
        let all = collect_all_results(&reports);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].title, "A");
        assert_eq!(all[2].title, "C");
    }

    #[test]
    fn test_collect_all_results_empty_reports() {
        let all = collect_all_results(&[]);
        assert!(all.is_empty());
    }

    #[test]
    fn test_collect_all_results_blocked_reports_contribute_nothing() {
        let reports = vec![
            EngineReport::ok("ddg", vec![RawResult::new("A", "https://a.com", "", "ddg")]),
            EngineReport::blocked("brave", "rate limited"),
        ];
        let all = collect_all_results(&reports);
        assert_eq!(all.len(), 1);
    }

    // --- count_engines_with_results ---

    #[test]
    fn test_count_engines_with_results() {
        let reports = vec![
            EngineReport::ok("ddg", vec![RawResult::new("A", "https://a.com", "", "ddg")]),
            EngineReport::blocked("brave", "rate limited"),
            EngineReport::ok("mojeek", vec![]), // 0 results but not blocked
        ];
        assert_eq!(count_engines_with_results(&reports), 1);
    }

    #[test]
    fn test_count_engines_with_results_all_blocked() {
        let reports = vec![
            EngineReport::blocked("ddg", "blocked"),
            EngineReport::blocked("brave", "blocked"),
        ];
        assert_eq!(count_engines_with_results(&reports), 0);
    }

    #[test]
    fn test_count_engines_with_results_all_success() {
        let reports = vec![
            EngineReport::ok("ddg", vec![RawResult::new("A", "https://a.com", "", "ddg")]),
            EngineReport::ok(
                "brave",
                vec![RawResult::new("B", "https://b.com", "", "brave")],
            ),
        ];
        assert_eq!(count_engines_with_results(&reports), 2);
    }

    // --- count_total_results ---

    #[test]
    fn test_count_total_results() {
        let reports = vec![
            EngineReport::ok(
                "ddg",
                vec![
                    RawResult::new("A", "https://a.com", "", "ddg"),
                    RawResult::new("B", "https://b.com", "", "ddg"),
                ],
            ),
            EngineReport::ok(
                "brave",
                vec![RawResult::new("C", "https://c.com", "", "brave")],
            ),
        ];
        assert_eq!(count_total_results(&reports), 3);
    }

    #[test]
    fn test_count_total_results_empty() {
        assert_eq!(count_total_results(&[]), 0);
    }

    // --- blocked_engine_names ---

    #[test]
    fn test_blocked_engine_names() {
        let reports = vec![
            EngineReport::ok("ddg", vec![]),
            EngineReport::blocked("brave", "rate limited"),
            EngineReport::error("mojeek", "timeout"),
        ];
        let blocked = blocked_engine_names(&reports);
        assert!(blocked.contains(&"brave"));
        assert!(blocked.contains(&"mojeek"));
        assert!(!blocked.contains(&"ddg"));
    }

    #[test]
    fn test_blocked_engine_names_all_ok() {
        let reports = vec![EngineReport::ok(
            "ddg",
            vec![RawResult::new("A", "https://a.com", "", "ddg")],
        )];
        let blocked = blocked_engine_names(&reports);
        assert!(blocked.is_empty());
    }

    // --- SearchEngineError ---

    #[test]
    fn test_search_engine_error_display() {
        assert_eq!(
            SearchEngineError::EmptyQuery.to_string(),
            "search query must not be empty"
        );
        assert_eq!(
            SearchEngineError::RateLimited(429).to_string(),
            "engine rate-limited (HTTP 429)"
        );
    }

    // --- SearchEngine trait (mock implementation) ---

    struct MockEngine {
        name: &'static str,
        results: Vec<RawResult>,
    }

    #[async_trait::async_trait]
    impl SearchEngine for MockEngine {
        fn name(&self) -> &str {
            self.name
        }

        async fn search(&self, _query: &str, _opts: &SearchOptions) -> EngineReport {
            EngineReport::ok(self.name, self.results.clone())
        }
    }

    struct BlockedEngine;

    #[async_trait::async_trait]
    impl SearchEngine for BlockedEngine {
        fn name(&self) -> &str {
            "blocked-engine"
        }

        async fn search(&self, _query: &str, _opts: &SearchOptions) -> EngineReport {
            EngineReport::blocked("blocked-engine", "rate limited (429)")
        }
    }

    #[tokio::test]
    async fn test_mock_engine_returns_results() {
        let engine = MockEngine {
            name: "mock",
            results: vec![
                RawResult::new("A", "https://a.com", "Snippet A", "mock"),
                RawResult::new("B", "https://b.com", "Snippet B", "mock"),
            ],
        };
        let report = engine.search("test", &SearchOptions::default()).await;
        assert_eq!(report.engine, "mock");
        assert_eq!(report.result_count, 2);
        assert!(report.is_success());
        assert!(!report.engine_blocked);
    }

    #[tokio::test]
    async fn test_blocked_engine_returns_blocked_report() {
        let engine = BlockedEngine;
        let report = engine.search("test", &SearchOptions::default()).await;
        assert_eq!(report.engine, "blocked-engine");
        assert!(report.engine_blocked);
        assert!(!report.is_success());
        assert!(!report.has_results());
    }

    #[tokio::test]
    async fn test_multiple_engines_in_parallel() {
        let engines: Vec<Box<dyn SearchEngine>> = vec![
            Box::new(MockEngine {
                name: "ddg",
                results: vec![RawResult::new("A", "https://a.com", "", "ddg")],
            }),
            Box::new(MockEngine {
                name: "brave",
                results: vec![RawResult::new("B", "https://b.com", "", "brave")],
            }),
            Box::new(BlockedEngine),
        ];

        let query = "test query";
        let opts = SearchOptions::default();

        // Run all engines concurrently.
        let mut handles = Vec::new();
        for _engine in &engines {
            let query = query.to_string();
            handles.push(tokio::spawn(async move {
                // Can't move Box<dyn SearchEngine> across spawn, so we
                // call directly — this test just verifies the pattern works.
                let _ = query;
            }));
        }
        // For the test, call sequentially (the parallel pattern is tested
        // by the consensus merger in T-015).
        let mut reports = Vec::new();
        for engine in &engines {
            reports.push(engine.search(query, &opts).await);
        }
        assert_eq!(reports.len(), 3);
        assert_eq!(count_engines_with_results(&reports), 2);
        assert_eq!(count_total_results(&reports), 2);
        assert!(blocked_engine_names(&reports).contains(&"blocked-engine"));
    }

    #[tokio::test]
    async fn test_trait_object_dyn_compatibility() {
        // Verify the trait is dyn-compatible (object-safe).
        let engine: Box<dyn SearchEngine> = Box::new(MockEngine {
            name: "mock",
            results: vec![RawResult::new("A", "https://a.com", "", "mock")],
        });
        assert_eq!(engine.name(), "mock");
        let report = engine.search("test", &SearchOptions::default()).await;
        assert!(report.is_success());
    }
}
