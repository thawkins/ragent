//! Search-engine trait, raw result types, and dedup helpers.
//!
//! Implements **FR-008** and **NFR-003** (T-012).
//!
//! This module defines the core abstractions for the multi-engine search
//! pipeline. Two of the seven registered backends (OpenAlex, Wikipedia) are
//! keyless; the remaining five (Tavily, Exa, Serper, Perplexity, LangSearch)
//! are API-key-backed services:
//!
//! - [`SearchEngine`] - an `async` trait implemented by each search backend
//!   adapter (OpenAlex, Wikipedia, Tavily, Exa, Serper, Perplexity,
//!   LangSearch). Each adapter returns results from its respective API; some
//!   are keyless (OpenAlex, Wikipedia), others require an API key (FR-023).
//! - [`RawResult`] - a single search result as returned by one engine, before
//!   merging / dedup / ranking. Carries the engine name (`source`) and an
//!   optional relevance `score` (0.0-1.0) if the engine provides one.
//! - [`EngineReport`] - the complete output of one engine's search: a list of
//!   [`RawResult`]s plus metadata about whether the engine was blocked, rate-
//!   limited, or errored. The `engine_blocked` flag lets the consensus merger
//!   report honest `engine_blocked` signals to the agent (FR-008).
//! - [`SearchOptions`] - query modifiers: `max_results`, `site`, `exclude_sites`,
//!   `freshness`, `page`. Shared across all backends.
//! - [`Freshness`] - time filter enum (`Day`, `Week`, `Month`, `Year`).
//! - [`normalise_result_url`] - normalises a result URL for dedup using the
//!   shared [`urlnorm`](crate::masterfetch::urlnorm) module. Falls back to the
//!   raw URL if normalisation fails (e.g. relative URLs).
//! - [`dedup_results_by_url`] - removes duplicate results by normalised URL,
//!   preserving first occurrence.
//!
//! # Testability (NFR-003)
//!
//! All data types are plain structs with public fields - no I/O, no async.
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

use std::sync::Arc;
// PERF-080: FxHash for short non-adversarial URL keys.
use rustc_hash::FxHashSet as HashSet;
use thiserror::Error;

use crate::masterfetch::urlnorm::normalise_url;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default maximum number of results to request from each engine (FR-008).
///
/// Also the shared default for the `max_results` tool input
/// ([`crate::masterfetch::tools::search_tool`]) so the orchestrator default and
/// the tool default cannot drift (ANTIPAT M5.9 / 3.7).
pub const DEFAULT_MAX_RESULTS: usize = 10;

/// Maximum allowed `max_results` value - the overall merge cap across all
/// engines (engines may cap lower).
pub const MAX_MAX_RESULTS: usize = 500;

/// Default per-engine result cap. Each engine is asked for at most this many
/// results before the consensus merger deduplicates and caps to
/// `max_results`.
pub const DEFAULT_PER_ENGINE_RESULTS: usize = 75;

/// Maximum allowed `per_engine_results` value.
pub const MAX_PER_ENGINE_RESULTS: usize = 200;

/// Default result page (0 = first page).
pub const DEFAULT_PAGE: usize = 0;

// ---------------------------------------------------------------------------
// Freshness (FR-008)
// ---------------------------------------------------------------------------

/// Time filter for search results.
///
/// Maps to the `freshness` parameter of `mf_search`. Engines translate this
/// into their own time-filter syntax (e.g. `DuckDuckGo`'s `df` parameter).
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
    pub const fn as_str(&self) -> &'static str {
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
    /// Maximum results to return after merge/dedup (1-500, default 10).
    /// This is the overall cap applied by `merge_and_rank_with_cap`.
    pub max_results: usize,
    /// Maximum results to request from each individual engine
    /// (1-200, default 75). Each backend receives an opts copy with
    /// `max_results` set to this value.
    pub per_engine_results: usize,
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
    /// Create a new `SearchOptions` with the given `max_results` (the overall
    /// merge cap) and all other fields at their defaults. `per_engine_results`
    /// defaults to [`DEFAULT_PER_ENGINE_RESULTS`] (75).
    #[must_use]
    pub fn new(max_results: usize) -> Self {
        Self {
            max_results: max_results.clamp(1, MAX_MAX_RESULTS),
            per_engine_results: DEFAULT_PER_ENGINE_RESULTS,
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
    pub const fn with_freshness(mut self, freshness: Freshness) -> Self {
        self.freshness = freshness;
        self
    }

    /// Builder: set the result `page`.
    #[must_use]
    pub const fn with_page(mut self, page: usize) -> Self {
        self.page = page;
        self
    }

    /// Builder: set the per-engine result cap (1-200).
    #[must_use]
    pub fn with_per_engine_results(mut self, n: usize) -> Self {
        self.per_engine_results = n.clamp(1, MAX_PER_ENGINE_RESULTS);
        self
    }
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            max_results: DEFAULT_MAX_RESULTS,
            per_engine_results: DEFAULT_PER_ENGINE_RESULTS,
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
/// The `score` field is an optional relevance score (0.0-1.0) if the engine
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
    /// Optional relevance score (0.0-1.0) if the engine provides one.
    pub score: Option<f64>,
    /// Author name when the engine exposes one in the result payload (e.g.
    /// Exa's `author` field or OpenAlex's `authorships[*].author.display_name`).
    /// `None` for engines that never provide author metadata (DuckDuckGo,
    /// Brave, Wikipedia).
    pub author: Option<String>,
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
            author: None,
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
/// - `engine` - the engine name (matches `SearchEngine::name()`).
/// - `results` - the raw results (may be empty if blocked or errored).
/// - `error` - an error message if the engine failed (empty on success).
/// - `engine_blocked` - `true` if the engine was rate-limited (HTTP 429/202)
///   or otherwise blocked. Reported to the agent as an honest signal.
/// - `result_count` - number of results returned (convenience; equals
///   `results.len()`).
/// - `duration_ms` - time spent on the request in milliseconds.
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
    pub const fn has_results(&self) -> bool {
        !self.results.is_empty()
    }

    /// Returns `true` if this report represents a successful, non-blocked
    /// search (even if zero results were returned).
    #[must_use]
    pub const fn is_success(&self) -> bool {
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

    /// `max_results` is out of range (must be 1-500).
    #[error("max_results out of range: {0} (must be 1-{1})")]
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

/// Trait implemented by each search-engine backend adapter (OpenAlex,
/// Wikipedia, Tavily, Exa, Serper, Perplexity, LangSearch).
///
/// Adapters query their respective backend API (keyless or API-key-backed)
/// and return results as [`EngineReport`]s.
///
/// Each adapter (OpenAlex, Wikipedia, ...) implements this trait and is queried
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
    /// failures (rate limits, parse errors, network errors) - those are
    /// captured in the `EngineReport`'s `error` and `engine_blocked` fields,
    /// matching Hound's catch-and-return pattern (FR-024).
    ///
    /// The only valid reason to return `Err` is a programming error (e.g.
    /// a poisoned lock).
    ///
    /// # Arguments
    ///
    /// - `query` - the search query string (must not be empty).
    /// - `opts` - search modifiers (max results, site filter, freshness, ...).
    async fn search(&self, query: &str, opts: &SearchOptions) -> EngineReport;
}

// ---------------------------------------------------------------------------
// URL normalisation for dedup (FR-008, FR-027)
// ---------------------------------------------------------------------------

/// Normalise a result URL for deduplication.
///
/// Uses the shared [`urlnorm`](crate::masterfetch::urlnorm) module to
/// lowercase the host, strip default ports, remove trailing slashes, and strip
/// tracking parameters (`utm_*`, `fbclid`, `gclid`, `ref`, ...).
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
/// // Same URL after normalisation -> same string.
/// let a = normalise_result_url("https://example.com/page/");
/// let b = normalise_result_url("https://example.com/page");
/// assert_eq!(a, b);
/// ```
#[must_use]
pub fn normalise_result_url(url: &str) -> String {
    normalise_url(url).unwrap_or_else(|_| url.to_string())
}

/// Remove quote characters from a search query.
///
/// Several search backends reject quoted-phrase syntax on restricted (free)
/// accounts - Serper returns HTTP 400 "Query pattern not allowed for free
/// accounts", which blocks that engine for the whole query. Stripping both
/// ASCII quotes and smart/typographic quotes (`'` `"` `'` `"` `"` `<<` `>>`)
/// keeps every engine reachable; the unquoted multi-term query still matches
/// the same terms without phrase semantics. Callers that embed quoted phrases
/// (e.g. a TUI echoing a quoted slash-command argument) therefore degrade
/// gracefully instead of silently losing engines.
#[must_use]
pub fn strip_disallowed_quotes(query: &str) -> String {
    const DISALLOWED: [char; 8] = [
        '"', '\'', '\u{2018}', '\u{2019}', '\u{201C}', '\u{201D}', '\u{00AB}', '\u{00BB}',
    ];
    // Single pass, one allocation: drop disallowed quote characters and
    // collapse whitespace runs, trimming leading/trailing whitespace.
    let mut out = String::with_capacity(query.len());
    let mut pending_space = false;
    for ch in query.chars() {
        if ch.is_whitespace() {
            pending_space = !out.is_empty();
            continue;
        }
        if DISALLOWED.contains(&ch) {
            continue;
        }
        if pending_space {
            out.push(' ');
            pending_space = false;
        }
        out.push(ch);
    }
    out
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
    let mut seen: HashSet<String> = HashSet::default();
    // No `Vec::with_capacity(results.len())` here: `results` is parsed from raw
    // upstream JSON, so a hostile/buggy engine can return an arbitrarily large
    // array and force a large up-front allocation before any dedup happens
    // (ANTIPAT 4.4). Dedup typically shrinks the list anyway, so growing from
    // empty is both safer and rarely less efficient.
    let mut deduped: Vec<RawResult> = Vec::new();

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
/// not performed here - use [`dedup_results_by_url`] afterwards if needed.
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

/// Returns `true` if this report represents a failed engine call: either
/// explicitly blocked, or errored with no results.
///
/// This is the source-of-truth predicate for the "blocked-or-errored"
/// concept. Used by both the engine module's `blocked_engine_names` and
/// the consensus merger's inline filter (see consensus.rs).
#[must_use]
pub const fn report_is_failed(r: &EngineReport) -> bool {
    r.engine_blocked || (!r.error.is_empty() && !r.has_results())
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
        .filter(|r| report_is_failed(r))
        .map(|r| r.engine.as_str())
        .collect()
}

/// Default number of search-attempt retries for transient engine failures.
///
/// A search attempt is transient when the engine reported an error that is
/// likely to succeed on retry: Wikipedia-style rate limiting (HTTP 429),
/// server errors (HTTP 5xx), or request-level transport failures (timeout,
/// connect, request builder). Wikimedia's shared limiter windows outlast a
/// single 1s backoff, so two retries with short exponential backoff are the
/// engine-level default (T-016).
pub const DEFAULT_SEARCH_MAX_RETRIES: u32 = 2;

/// Default delay before the first transient search retry. Subsequent retries
/// double this base (1s, 2s for [`DEFAULT_SEARCH_MAX_RETRIES`] == 2).
pub const DEFAULT_SEARCH_RETRY_DELAY: std::time::Duration = std::time::Duration::from_secs(1);

/// Classify whether an [`EngineReport`] represents a transient failure worth
/// retrying once inside the engine call path.
///
/// Transient when:
/// - the engine is Wikipedia and the message mentions `429` / `rate-limit`
///   (Wikimedia's shared limiter window closes quickly), or
/// - the message mentions an HTTP `5xx` status, or
/// - the report is an *error* report (`engine_blocked == false`) whose message
///   starts with `HTTP request failed:` (per the engine adapters' convention)
///   and mentions a timeout/connect/request-class transport problem.
///
/// NOT transient: quota-style blocks (HTTP 402/403/432, missing key) and the
/// OpenAlex daily-budget 429, which does not reset until midnight UTC and
/// would only be hammered by pointless retries.
#[must_use]
pub fn report_is_transient(report: &EngineReport) -> bool {
    if !report.results.is_empty() {
        return false;
    }
    let msg = report.error.as_str();
    if msg.is_empty() {
        return false;
    }
    let lower = msg.to_ascii_lowercase();
    if msg.contains("429") || lower.contains("rate-limit") {
        // OpenAlex's budget-based 429 is a daily quota, not a burst window -
        // the provider payload itself says "Insufficient budget"; do not
        // retry it, but DO retry Wikipedia's plain per-IP limiter.
        let openalex_daily_quota = report.engine == "openalex" && msg.contains("budget");
        if !openalex_daily_quota {
            return true;
        }
    }
    if msg.contains("HTTP 5") {
        // HTTP 500/502/503/504 etc.
        return true;
    }
    if !report.engine_blocked && msg.starts_with("HTTP request failed:") {
        return lower.contains("timeout")
            || lower.contains("timed out")
            || lower.contains("connect")
            || lower.contains("request error")
            || lower.contains("error sending request");
    }
    false
}

/// Search one engine with up to `max_retries` transient retries.
///
/// The first attempt runs immediately; when the returned report classifies as
/// transient (see [`report_is_transient`]) the call waits `retry_delay` and
/// tries the engine again. Non-transient results are returned as-is. This is
/// the engine-level retry home for every `mf_search` caller (TUI, CLI, tools,
/// research), so no caller implements its own retry loop (spec T-016).
pub async fn search_with_retry(
    engine: &Arc<dyn SearchEngine>,
    query: &str,
    opts: &SearchOptions,
    max_retries: u32,
    retry_delay: std::time::Duration,
) -> EngineReport {
    let mut attempt: u32 = 0;
    loop {
        let report = engine.search(query, opts).await;
        if attempt >= max_retries || !report_is_transient(&report) {
            return report;
        }
        attempt += 1;
        // Exponential backoff: retry_delay, 2*retry_delay, 4*retry_delay, ....
        // The exponent is capped at 31 so an out-of-range `max_retries` cannot
        // overflow the shift (which would panic in debug builds).
        let delay = retry_delay.saturating_mul(1u32 << (attempt - 1).min(31));
        tracing::warn!(
            engine = %report.engine,
            attempt,
            delay_ms = delay.as_millis(),
            error = %report.error,
            "search engine transiently failed; retrying"
        );
        tokio::time::sleep(delay).await;
    }
}

// ---------------------------------------------------------------------------
// Shared API-engine helpers
// ---------------------------------------------------------------------------

/// Maximum number of characters [`truncate_snippet`] keeps before appending an
/// ellipsis.
pub const SNIPPET_MAX_CHARS: usize = 200;

/// Resolve an engine's HTTP client, falling back to the shared masterfetch
/// default client when none was injected via `with_client`.
///
/// Every API-key-backed engine stores an optional client for mock-server tests;
/// centralising the fallback keeps the adapters from drifting apart.
pub(crate) fn engine_http_client(
    client: &Option<reqwest::Client>,
) -> Result<reqwest::Client, String> {
    if let Some(c) = client {
        return Ok(c.clone());
    }
    // Reuse the process-wide shared client singleton (connection pool + TLS
    // session cache) instead of rebuilding an equivalent client on every
    // engine call (ANTIPAT M5.9 / 3.2).
    crate::masterfetch::http::shared_client()
        .cloned()
        .map_err(|e| format!("failed to build HTTP client: {e}"))
}

/// Truncate a query to at most `max` characters, respecting UTF-8 character
/// boundaries.
#[must_use]
pub fn truncate_query_to(query: &str, max: usize) -> String {
    if query.chars().count() <= max {
        query.to_string()
    } else {
        query.chars().take(max).collect()
    }
}

/// Truncate a snippet to [`SNIPPET_MAX_CHARS`] characters, appending an
/// ellipsis when the input was longer.
#[must_use]
pub fn truncate_snippet(snippet: &str) -> String {
    if snippet.chars().count() <= SNIPPET_MAX_CHARS {
        return snippet.to_string();
    }
    // Keep the result within `SNIPPET_MAX_CHARS` including the 3-char ellipsis.
    let head = SNIPPET_MAX_CHARS.saturating_sub(3);
    let truncated: String = snippet.chars().take(head).collect();
    format!("{truncated}...")
}

/// Truncate a snippet to at most `max` bytes (rounded down to a char
/// boundary), appending an ellipsis when the input was longer.
///
/// Some engines (OpenAlex, Wikipedia) budget snippets in bytes rather than
/// characters; they share this one implementation instead of forking it
/// (see `ANTIPAT.md` M3.8).
#[must_use]
pub fn truncate_snippet_bytes(snippet: &str, max: usize) -> String {
    if snippet.len() <= max {
        return snippet.to_string();
    }
    let end = snippet
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|&i| i <= max)
        .last()
        .unwrap_or(0);
    format!("{}...", &snippet[..end])
}

/// Mask a sensitive API key for display.
///
/// Keeps the first two and last two characters; everything in between is
/// replaced with `*`. Strings of six characters or fewer are fully masked.
#[must_use]
pub fn mask_api_key(key: &str) -> String {
    let len = key.chars().count();
    if len <= 6 {
        return "*".repeat(len);
    }
    let first: String = key.chars().take(2).collect();
    let last: String = key.chars().skip(len - 2).collect();
    format!("{first}*{}*{last}", "*".repeat(len.saturating_sub(6)))
}

/// Common pre-flight guard shared by the API-key-backed engines.
///
/// Returns `Some(report)` when the call must be short-circuited (empty query or
/// missing key) and `None` when the engine should proceed.
pub(crate) fn api_engine_preflight(
    engine: &str,
    query: &str,
    api_key: &str,
    missing_key_msg: &str,
) -> Option<EngineReport> {
    if query.trim().is_empty() {
        return Some(EngineReport::error(
            engine,
            "search query must not be empty",
        ));
    }
    if api_key.is_empty() {
        return Some(EngineReport::blocked(engine, missing_key_msg));
    }
    None
}

/// Shared post-response tail for the JSON API engines.
///
/// Maps a finished HTTP response into an [`EngineReport`]: status check, body
/// read, JSON parse, dedup, truncation to `max_results`, and duration stamping.
/// `on_error` supplies the blocked message for a non-success status so each
/// engine keeps its provider-specific wording.
pub(crate) async fn finish_json_search(
    engine: &str,
    started: std::time::Instant,
    response: reqwest::Response,
    max_results: usize,
    parse: impl FnOnce(&serde_json::Value) -> Vec<RawResult>,
    on_error: impl FnOnce(reqwest::StatusCode) -> String,
) -> EngineReport {
    let status = response.status();
    if !status.is_success() {
        tracing::warn!(status = %status, engine = engine, "api returned error status");
        return EngineReport::blocked(engine, on_error(status));
    }
    // ANTIPAT 4.1: cap the body so a hostile/buggy upstream (or a gzip bomb,
    // which the shared client decompresses) cannot exhaust memory.
    let text = match crate::masterfetch::http::read_body_capped(
        response,
        crate::masterfetch::http::MAX_RESPONSE_BODY_BYTES,
    )
    .await
    {
        Ok(t) => t,
        Err(e) => {
            return EngineReport::error(engine, format!("failed to read response body: {e}"));
        }
    };
    let value: serde_json::Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            return EngineReport::error(engine, format!("failed to parse response JSON: {e}"));
        }
    };
    let mut results = parse(&value);
    results = dedup_results_by_url(&results);
    results.truncate(max_results);
    let elapsed = started.elapsed().as_millis() as u64;
    let mut report = EngineReport::ok(engine, results);
    report.duration_ms = elapsed;
    report
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "../tests/inline/engine_tests.rs"]
mod tests;
