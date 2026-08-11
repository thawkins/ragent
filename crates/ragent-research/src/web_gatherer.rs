//! Web-gathering phase for the research system (FR-006, FR-007).
//!
//! This module implements the orchestration logic that turns a research
//! topic into a list of [`Source::Web`] entries. The actual HTTP calls
//! are made through the [`WebSearchTool`] and [`WebFetchTool`] trait
//! abstractions so the gatherer can be unit-tested without network access
//! and reused from any integration context (TUI agent loop, CLI, HTTP
//! endpoint, tests).
//!
//! ## Flow
//!
//! 1. [`WebGatherer::gather`] issues a [`WebSearchTool::search`] for the
//!    topic and collects up to `max_results` candidate URLs.
//! 2. For each candidate URL it calls [`WebFetchTool::fetch`] to obtain
//!    the page body and title.
//! 3. Each captured page becomes a [`Source::Web`] entry with a synthetic
//!    supporting-file path of the form `sources/web-NN.md` (zero-padded,
//!    starting at 01) — the actual supporting-file write is done by the IO
//!    layer (T-015) once we have an item directory on disk; this module
//!    only returns the captured metadata.
//! 4. If the search or fetch tools return zero results the gatherer
//!    returns an empty `Vec` (FR-006: graceful degradation).
//!
//! ## Reuse, not reimplementation
//!
//! Per the spec constraints, the gatherer does **not** reimplement search
//! or fetch — it delegates entirely to the provided `WebSearchTool` /
//! `WebFetchTool` implementations. In production these wrap the existing
//! `websearch` and `webfetch` tools in `crates/ragent-tools-extended`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::StreamExt;

use crate::document::{MAX_SOURCE_BODY_BYTES, fence_source_body, truncate_body_to_bytes};
use crate::gather_log::GatherLog;
use crate::source::Source;
use std::time::Duration;

mod classify;
mod decomposer;
mod relevance;
mod title;

pub use classify::{WebSourceKind, classify_web_source};
pub use decomposer::{HeuristicQueryDecomposer, LlmQueryDecomposer, QueryDecomposer};
use relevance::compute_relevance_label;
use title::clean_web_source_title;

/// Maximum number of focused sub-queries the research decomposer will
/// produce for a single topic. Increasing this raises the web-search
/// parallelism and usually increases the number of distinct sources found,
/// while staying within typical LLM output budgets for a JSON array.
pub(crate) const MAX_DECOMPOSED_QUERIES: usize = 10;

/// Default maximum number of web sources to capture per research item. The
/// earlier 15-source cap was too restrictive for broad topics; a larger
/// default lets the decomposer's parallel queries surface a much wider
/// set of candidate URLs before the synthesis phase.
/// Default cap on the number of web sources captured when the caller does not
/// supply an explicit `max_web_results` (FR-011).
pub const DEFAULT_MAX_WEB_RESULTS: usize = 250;

/// Default per-fetch wall-clock timeout. Pages that take longer than this are
/// treated as a fetch failure so a single slow URL cannot stall the whole
/// gather pass (Milestone B-004).
pub const DEFAULT_FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// Default upper bound on the number of concurrent page fetches issued during
/// the capture phase of [`WebGatherer::gather_with_observer`]. 10 is a safe
/// middle ground: fast enough to keep wall-clock latency low when a search
/// returns many candidate URLs, while staying well clear of OS file-descriptor
/// limits and typical search-provider rate ceilings.  Override with the
/// `--fetch-concurrently N` CLI flag or [`WebGatherer::with_fetch_concurrency`].
pub const DEFAULT_FETCH_CONCURRENCY: usize = 10;

/// Default maximum number of retry attempts for a failed sub-query search
/// (Milestone H-002). Retries use exponential backoff. `0` would disable
/// retries entirely; 2 gives a short burst of retries before giving up.
pub const DEFAULT_SEARCH_MAX_RETRIES: u32 = 2;

/// Default base delay in milliseconds for the first search-retry backoff
/// (Milestone H-002). Subsequent retries double this value (200 ms, 400 ms, …).
pub const DEFAULT_SEARCH_RETRY_BASE_DELAY_MS: u64 = 200;

/// Default number of consecutive search-tool failures after which the
/// circuit-breaker opens (Milestone H-003). Once open, no further search
/// calls are issued for the remainder of the gather pass. `0` disables the
/// circuit-breaker entirely.
pub const DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD: u32 = 3;

/// Minimum extracted content length (in characters) for a fetched page to be
/// accepted as a web source. Pages whose cleaned body is shorter than this are
/// rejected so near-empty extractions (paywalls, JS-only renders, soft 404s)
/// do not pollute the synthesis prompt. The value matches the preview length
/// surfaced in the progress display so a captured source always has at least
/// a full preview's worth of content.
pub const MIN_EXTRACTABLE_CONTENT_CHARS: usize = 256;

/// Cap a captured web body at the same byte budget used by the supporting
/// file renderer so the body stored on the `Source` matches what ends up on
/// disk. Keeps runaway pages from blowing up the synthesis prompt.
fn fence_captured_body(body: &str) -> String {
    fence_source_body(body)
}

/// Cap a captured web body to at most `max_bytes`. Kept for downstream
/// callers that want an explicit cap helper (Milestone B-002).
#[allow(dead_code)]
fn truncate_captured_body(body: &str, max_bytes: usize) -> String {
    truncate_body_to_bytes(body, max_bytes)
}

/// Result of a decomposed web-gathering pass.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatherResult {
    /// Sub-queries that were actually issued to the search tool.
    pub queries: Vec<String>,
    /// Captured web sources, already deduplicated by URL and limited to the
    /// caller's `max_results` budget.
    pub sources: Vec<Source>,
    /// Count of captured PDF documents.
    pub pdf_count: usize,
    /// Count of captured YouTube video URLs.
    pub youtube_count: usize,
    /// Number of candidate web sources that were fetched but excluded because
    /// their relevance score was too low.
    pub excluded_count: usize,
}

impl GatherResult {
    /// Empty result with no queries and no sources.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            queries: Vec::new(),
            sources: Vec::new(),
            pdf_count: 0,
            youtube_count: 0,
            excluded_count: 0,
        }
    }
}

/// Search-result row returned by a [`WebSearchTool`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebSearchHit {
    /// Page URL.
    pub url: String,
    /// Page title as reported by the search provider (may be empty).
    pub title: String,
    /// One- or two-line snippet (may be empty).
    pub snippet: String,
    /// The actual sub-query string that returned this hit. Used by the
    /// gatherer to compute a deterministic relevance note and to annotate the
    /// source in the RESEARCH.md References Index.
    pub matched_query: String,
    /// Name of the agent tool that issued the search (e.g. `"mf_search"` or
    /// `"websearch"`). This lets the research output show *which* search tool
    /// produced the source.
    pub search_tool: String,
    /// Name(s) of the backend search engine(s) that returned this hit. For
    /// `mf_search` this is a comma-separated list like `"duckduckgo, brave"`;
    /// for `websearch` it is `"tavily"`.
    pub search_engine: String,
}
/// Page body returned by a [`WebFetchTool`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebFetchedPage {
    /// Page URL — must match the URL passed in.
    pub url: String,
    /// Resolved page title (may be empty if the page lacked a title).
    pub title: String,
    /// Rendered text body of the page, in UTF-8. HTML tags should already
    /// have been stripped by the implementation.
    pub body: String,
    /// Publication date parsed from the page's embedded metadata, when the
    /// fetcher was able to determine one. `None` when the page did not expose
    /// a parseable publication date.
    pub published_at: Option<DateTime<Utc>>,
    /// HTTP `Content-Type` reported by the fetcher, when available. Used by
    /// the research layer to classify PDFs and other media types.
    pub content_type: Option<String>,
    /// Page-type classification reported by the fetcher (e.g. `article`,
    /// `docs`). Currently informational; `content_type` drives media
    /// classification.
    pub page_type: Option<String>,
    /// Detected human language of the page body, when the fetcher reported
    /// one. `None` when language detection was unavailable.
    pub language: Option<String>,
}

/// Trait abstracting the existing `websearch` tool.
///
/// Production wiring delegates to the real tool from
/// `ragent-tools-extended`; tests provide an in-memory fake.
#[async_trait]
pub trait WebSearchTool: Send + Sync {
    /// Run a web search for `query` and return up to `max_results` hits.
    async fn search(&self, query: &str, max_results: usize) -> anyhow::Result<Vec<WebSearchHit>>;
}

/// Trait abstracting the existing `webfetch` tool.
#[async_trait]
pub trait WebFetchTool: Send + Sync {
    /// Fetch `url` and return the rendered page body.
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage>;

    /// Fetch `url` and cap the returned body at `max_bytes`.
    ///
    /// The default implementation delegates to [`Self::fetch`] and then
    /// truncates the body, so implementations that already stream or cap data
    /// can override this for better memory behaviour. The research gatherer
    /// always calls this method to enforce [`MAX_SOURCE_BODY_BYTES`] at the
    /// boundary (Milestone B-002).
    async fn fetch_with_limit(
        &self,
        url: &str,
        max_bytes: usize,
    ) -> anyhow::Result<WebFetchedPage> {
        let mut page = self.fetch(url).await?;
        if page.body.len() > max_bytes {
            page.body = truncate_body_to_bytes(&page.body, max_bytes);
        }
        Ok(page)
    }
}

/// Errors emitted by [`WebGatherer`].
#[derive(Debug, thiserror::Error)]
pub enum WebGatherError {
    /// The configured search limit was zero — there is nothing to gather.
    #[error("web gatherer called with max_results = 0")]
    ZeroLimit,
    /// An empty topic was supplied.
    #[error("web gatherer called with an empty topic")]
    EmptyTopic,
}

/// Diagnostic events emitted by [`WebGatherer`] during a gather pass.
///
/// These are surfaced to the UI so users can see *why* no web sources were
/// captured (missing API key, network failure, fetch timeout, etc.).
#[derive(Debug, Clone)]
pub enum GatherEvent {
    /// The query decomposition step produced these sub-queries.
    QueriesDecomposed {
        /// Sub-queries that will be issued to the search tool.
        queries: Vec<String>,
    },
    /// A single candidate page was fetched and captured as a source.
    /// Emitted inline as each fetch succeeds so the UI can show
    /// successfully retrieved URLs as they arrive, rather than only
    /// seeing failures during the gather and successes at the end.
    SourceCaptured {
        /// URL of the captured page.
        url: String,
        /// Page title (may be empty).
        title: String,
        /// Search tool that produced this hit.
        search_tool: String,
        /// Backend search engine(s) that returned this URL.
        search_engine: String,
        /// First [`MIN_EXTRACTABLE_CONTENT_CHARS`] characters of the
        /// extracted page body, so the progress display can preview the
        /// captured content alongside the URL and title.
        body_preview: String,
        /// Detected human language of the page body in uppercase (e.g.
        /// `"ENGLISH"`, `"FRENCH"`), or `"UNKNOWN"` when language
        /// detection was unavailable.
        language: String,
    },
    /// The underlying search tool returned an error.
    SearchFailed {
        /// Error message from the search tool.
        error: String,
    },
    /// A single page fetch failed after the search produced a candidate URL.
    FetchFailed {
        /// URL that could not be fetched.
        url: String,
        /// Error message from the fetch tool.
        error: String,
    },
    /// Search succeeded but returned zero hits.
    SearchReturnedNoHits,
    /// A sub-query search failed and will be retried after a short backoff
    /// (Milestone H-002). Emitted before each retry attempt so the retry
    /// count is observable in the UI.
    SearchRetrying {
        /// Sub-query being retried.
        query: String,
        /// 1-based retry attempt number (1 = first retry after the initial
        /// failure, 2 = second retry, …).
        attempt: u32,
        /// Error from the previous failed attempt.
        error: String,
    },
    /// The search circuit-breaker has opened after too many consecutive
    /// search-tool failures (Milestone H-003). No further search calls will
    /// be issued for the remainder of this gather pass; the gatherer falls
    /// back to no hits.
    SearchCircuitOpen {
        /// Number of consecutive failures that triggered the breaker.
        consecutive_failures: u32,
    },
}

/// Observer receiving [`GatherEvent`]s from [`WebGatherer`].
pub trait GatherObserver: Send + Sync {
    /// Receive a diagnostic event.
    fn on_event(&self, event: GatherEvent);
}

/// Orchestrates a single web-gathering pass for one research topic.
///
/// `WebGatherer` is cheap to clone (internally an `Arc` pair) so the TUI
/// and CLI can hold one instance and call [`gather`] many times.
#[derive(Clone)]
pub struct WebGatherer {
    search: Arc<dyn WebSearchTool>,
    fetch: Arc<dyn WebFetchTool>,
    decomposer: Option<Arc<dyn QueryDecomposer>>,
    /// Upper bound on the number of concurrent page fetches issued during the
    /// capture phase of [`gather_with_observer`]. Defaults to
    /// [`DEFAULT_FETCH_CONCURRENCY`]; override via [`with_fetch_concurrency`].
    fetch_concurrency: usize,
    /// Wall-clock timeout applied to each individual page fetch. Pages that
    /// take longer are treated as a fetch failure (Milestone B-004).
    fetch_timeout: Duration,
    /// When `true`, every fetched page is retained regardless of its
    /// relevance score, disabling the default filter that discards
    /// "Low"/"Very low" sources. Defaults to `false`.
    keep_low_relevance: bool,
    /// Maximum number of retry attempts for a failed sub-query search
    /// (Milestone H-002). Retries use exponential backoff with a base delay of
    /// [`Self::search_retry_base_delay_ms`]. Defaults to
    /// [`DEFAULT_SEARCH_MAX_RETRIES`] (2). `0` disables retries.
    search_max_retries: u32,
    /// Base delay in milliseconds for the first retry backoff
    /// (Milestone H-002). Subsequent retries double the delay. Defaults to
    /// [`DEFAULT_SEARCH_RETRY_BASE_DELAY_MS`] (200 ms).
    search_retry_base_delay_ms: u64,
    /// Number of consecutive search-tool failures after which the
    /// circuit-breaker opens (Milestone H-003). Once open, no further search
    /// calls are issued for the remainder of the gather pass. Defaults to
    /// [`DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD`] (3). `0` disables the
    /// circuit-breaker entirely.
    search_circuit_breaker_threshold: u32,
    /// JSONL URL log (`log/research-<name>-<ts>-<rand>-web.jsonl`) recording
    /// every search hit as `considered`/`captured`/`rejected` with a reason.
    /// `None` disables logging. Set via [`with_gather_log`].
    gather_log: Option<Arc<Mutex<GatherLog>>>,
}

impl std::fmt::Debug for WebGatherer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebGatherer")
            .field("has_decomposer", &self.decomposer.is_some())
            .field("fetch_concurrency", &self.fetch_concurrency)
            .field("fetch_timeout_ms", &self.fetch_timeout.as_millis())
            .field("keep_low_relevance", &self.keep_low_relevance)
            .field("search_max_retries", &self.search_max_retries)
            .field(
                "search_circuit_breaker_threshold",
                &self.search_circuit_breaker_threshold,
            )
            .field("has_gather_log", &self.gather_log.is_some())
            .finish_non_exhaustive()
    }
}

impl WebGatherer {
    /// Construct a new gatherer from a search tool and a fetch tool.
    ///
    /// The fetch-phase concurrency defaults to [`DEFAULT_FETCH_CONCURRENCY`]
    /// (10); override it with [`WebGatherer::with_fetch_concurrency`].
    pub fn new(search: Arc<dyn WebSearchTool>, fetch: Arc<dyn WebFetchTool>) -> Self {
        Self {
            search,
            fetch,
            decomposer: None,
            fetch_concurrency: DEFAULT_FETCH_CONCURRENCY,
            fetch_timeout: DEFAULT_FETCH_TIMEOUT,
            keep_low_relevance: false,
            search_max_retries: DEFAULT_SEARCH_MAX_RETRIES,
            search_retry_base_delay_ms: DEFAULT_SEARCH_RETRY_BASE_DELAY_MS,
            search_circuit_breaker_threshold: DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD,
            gather_log: None,
        }
    }

    /// Attach a JSONL gather log that records every search hit considered
    /// during [`gather_with_observer`] and whether it was captured or
    /// rejected (with the rejection reason). Log entries are appended as
    /// hits stream in and each concurrent fetch resolves. The log file is
    /// created eagerly (even when a pass yields no hits) and flushed when
    /// the gatherer is dropped. Failures to open the log are reported via
    /// the observer and tracing, never propagated.
    pub fn with_gather_log(mut self, log: GatherLog) -> Self {
        self.gather_log = Some(Arc::new(Mutex::new(log)));
        self
    }

    /// Append one per-URL outcome record to the gather log, when configured.
    ///
    /// Best-effort: lock-poisoning is recovered and write failures are
    /// surfaced via `tracing::warn` so a logging problem can never fail a
    /// gather pass. An empty `reason` is recorded as `None` (captured URLs
    /// have no rejection reason).
    #[allow(clippy::too_many_arguments)]
    fn log_url_outcome(
        &self,
        url: &str,
        query: &str,
        title: &str,
        search_tool: &str,
        search_engine: &str,
        status: &str,
        reason: &str,
        detail: Option<&serde_json::Value>,
    ) {
        let Some(log) = &self.gather_log else {
            return;
        };
        let reason = if reason.is_empty() {
            None
        } else {
            Some(reason)
        };
        if let Err(e) = log.lock().unwrap_or_else(|p| p.into_inner()).log_url(
            url,
            query,
            status,
            title,
            search_tool,
            search_engine,
            reason,
            detail,
        ) {
            tracing::warn!(error = %e, url, "research: web URL log write failed");
        }
    }

    /// Attach a query decomposer.  When present, [`gather_with_observer`]
    /// decomposes the topic into parallel sub-queries and deduplicates the
    /// combined results.
    pub fn with_decomposer(mut self, decomposer: Arc<dyn QueryDecomposer>) -> Self {
        self.decomposer = Some(decomposer);
        self
    }

    /// Override the fetch-phase concurrency limit.
    ///
    /// Controls how many candidate page fetches are issued in parallel during
    /// [`gather_with_observer`].  Values of `0` are clamped up to `1` so the
    /// stream always makes progress.  Larger values reduce wall-clock latency
    /// when a search returns many hits, at the cost of more in-flight HTTP
    /// connections and memory.  The default is [`DEFAULT_FETCH_CONCURRENCY`]
    /// (10).
    #[must_use]
    pub fn with_fetch_concurrency(mut self, n: usize) -> Self {
        self.fetch_concurrency = n.max(1);
        self
    }

    /// Override the per-fetch wall-clock timeout.
    ///
    /// Pages that take longer than this are treated as a fetch failure and
    /// skipped, so one slow URL cannot stall the whole gather pass. The default
    /// is [`DEFAULT_FETCH_TIMEOUT`] (30 seconds). A zero duration is treated
    /// as the default.
    #[must_use]
    pub fn with_fetch_timeout(mut self, timeout: Duration) -> Self {
        self.fetch_timeout = if timeout.is_zero() {
            DEFAULT_FETCH_TIMEOUT
        } else {
            timeout
        };
        self
    }

    /// Keep low-relevance web sources instead of filtering them out.
    ///
    /// When enabled, [`gather_with_observer`] retains every fetched page
    /// regardless of its query-match relevance score, disabling the default
    /// filter that discards "Low"/"Very low" sources.
    #[must_use]
    pub fn with_keep_low_relevance(mut self, keep: bool) -> Self {
        self.keep_low_relevance = keep;
        self
    }

    /// Override the maximum number of retry attempts for a failed sub-query
    /// search (Milestone H-002). Retries use exponential backoff with a base
    /// delay of [`Self::search_retry_base_delay_ms`]. Setting this to `0`
    /// disables retries entirely (a single attempt is made). The default is
    /// [`DEFAULT_SEARCH_MAX_RETRIES`] (2).
    #[must_use]
    pub fn with_search_max_retries(mut self, n: u32) -> Self {
        self.search_max_retries = n;
        self
    }

    /// Override the base delay in milliseconds for the first search-retry
    /// backoff (Milestone H-002). Subsequent retries double this value
    /// (e.g. 200 ms → 400 ms → 800 ms). The default is
    /// [`DEFAULT_SEARCH_RETRY_BASE_DELAY_MS`] (200 ms). A value of `0` makes
    /// retries immediate with no delay.
    #[must_use]
    pub fn with_search_retry_base_delay_ms(mut self, ms: u64) -> Self {
        self.search_retry_base_delay_ms = ms;
        self
    }

    /// Override the number of consecutive search-tool failures after which
    /// the circuit-breaker opens (Milestone H-003). Once open, no further
    /// search calls are issued for the remainder of the gather pass and the
    /// gatherer falls back to no hits. Setting this to `0` disables the
    /// circuit-breaker entirely. The default is
    /// [`DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD`] (3).
    #[must_use]
    pub fn with_search_circuit_breaker_threshold(mut self, n: u32) -> Self {
        self.search_circuit_breaker_threshold = n;
        self
    }

    /// Fetch a single URL and return it as a [`Source::Web`] plus the raw
    /// [`WebFetchedPage`].
    ///
    /// Used by `--from-url` to capture a user-supplied page as the primary
    /// research subject *before* the normal web-search phase runs. The body is
    /// fenced via [`fence_captured_body`] so it stays within the same byte
    /// budget as pages captured during gathering. The `body_path` is set to
    /// `web-01.md` (index 0); the manager renumbers supporting files by
    /// position at write time, so this is purely a metadata hint.
    ///
    /// # Errors
    ///
    /// Returns the underlying fetch error when the page cannot be retrieved.
    pub async fn fetch_url_as_source(&self, url: &str) -> anyhow::Result<(Source, WebFetchedPage)> {
        let page = tokio::time::timeout(
            self.fetch_timeout,
            self.fetch.fetch_with_limit(url, MAX_SOURCE_BODY_BYTES),
        )
        .await
        .map_err(|_| anyhow::anyhow!("fetch timed out after {}s", self.fetch_timeout.as_secs()))?
        .map_err(|e| anyhow::anyhow!("failed to fetch seed URL {url}: {e}"))?;
        let body = fence_captured_body(&page.body);
        let title = clean_web_source_title(&page.title, url);
        let media_type = classify_web_source(url, page.content_type.as_deref())
            .as_str()
            .to_string();
        let source = Source::Web {
            url: page.url.clone(),
            title,
            captured_at: chrono::Utc::now(),
            published_at: page.published_at,
            body_path: web_body_path(0),
            body,
            relevance: "User-supplied seed URL".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: page.content_type.clone(),
            page_type: page.page_type.clone(),
            media_type,
            language: page.language.clone(),
        };
        Ok((source, page))
    }

    /// Gather up to `max_results` web sources for `topic`.
    ///
    /// Returns an empty `Vec` (not an error) when:
    ///
    /// - The search tool returns no hits (FR-006 graceful degradation).
    /// - Every fetch call fails for transient reasons (logged at info,
    ///   not surfaced as an error to the caller — the local-gathering
    ///   phase can still produce a useful RESEARCH.md).
    ///
    /// Returns a [`WebGatherError`] only for programmer mistakes such as
    /// `max_results == 0` or `topic.is_empty()`.
    pub async fn gather(
        &self,
        topic: &str,
        max_results: usize,
    ) -> Result<Vec<Source>, WebGatherError> {
        let result = self.gather_with_observer(topic, max_results, None).await?;
        Ok(result.sources)
    }

    /// Decide whether a search hit is worth fetching, based only on the
    /// query, title, snippet, and URL. Low-relevance hits are dropped before
    /// any full-page fetch, saving bandwidth and prompt budget
    /// (Milestone B-001). When `keep_low_relevance` is enabled the hit is
    /// retained but its label is still computed for later reporting.
    fn filter_hit(&self, query: &str, hit: &WebSearchHit) -> Option<(String, bool)> {
        let (label, retained) = compute_relevance_label(query, &hit.title, &hit.snippet, &hit.url);
        if retained || self.keep_low_relevance {
            Some((label, retained))
        } else {
            None
        }
    }

    /// Gather web sources with an optional observer for diagnostic events.
    ///
    /// When a decomposer is configured the topic is first split into focused
    /// sub-queries; each sub-query is issued in parallel, results are
    /// deduplicated by URL, pre-filtered by title/snippet relevance, and up to
    /// `max_results` unique pages are fetched **concurrently** up to
    /// [`WebGatherer::fetch_concurrency`] at a time (default
    /// [`DEFAULT_FETCH_CONCURRENCY`], 10). Each fetch is also bounded by
    /// [`MAX_SOURCE_BODY_BYTES`] and [`WebGatherer::fetch_timeout`].
    /// [`GatherEvent`] diagnostics (`SourceCaptured` / `FetchFailed`) fire in
    /// fetch-completion order so the UI can render each page as soon as it
    /// arrives; the returned `sources` vector is re-sorted into the original
    /// search-ranking order so the `web-NN.md` supporting-file names track hit
    /// position rather than completion timing. The returned [`GatherResult`]
    /// lists the sub-queries that were used so the caller can persist them in
    /// `RESEARCH.md`.
    pub async fn gather_with_observer(
        &self,
        topic: &str,
        max_results: usize,
        observer: Option<&dyn GatherObserver>,
    ) -> Result<GatherResult, WebGatherError> {
        if max_results == 0 {
            return Err(WebGatherError::ZeroLimit);
        }
        if topic.trim().is_empty() {
            return Err(WebGatherError::EmptyTopic);
        }

        tracing::info!(topic, max_results, "research: starting web-gathering phase");

        if let Some(log) = &self.gather_log
            && let Err(e) = log
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .log_event(&serde_json::json!({
                    "event": "gather_start",
                    "topic": topic,
                    "max_results": max_results,
                }))
        {
            tracing::warn!(error = %e, "research: failed to write gather-start to web URL log");
        }

        // Determine the set of sub-queries.  If no decomposer is configured
        // we still treat the original topic as a single query so callers see
        // a consistent [`GatherResult`].
        let queries: Vec<String> = match &self.decomposer {
            Some(d) => match d.decompose(topic).await {
                Ok(qs) if !qs.is_empty() => qs,
                Ok(_) => {
                    tracing::warn!("research: decomposer returned empty queries; using topic");
                    vec![topic.to_string()]
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        "research: query decomposition failed; falling back to single query"
                    );
                    vec![topic.to_string()]
                }
            },
            None => vec![topic.to_string()],
        };

        if let Some(obs) = observer {
            obs.on_event(GatherEvent::QueriesDecomposed {
                queries: queries.clone(),
            });
        }
        if let Some(log) = &self.gather_log
            && let Err(e) =
                log.lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .log_event(&serde_json::json!({
                        "event": "queries_decomposed",
                        "queries": queries,
                    }))
        {
            tracing::warn!(error = %e, "research: failed to write queries to web URL log");
        }

        // Run each sub-query in parallel with bounded concurrency. Each
        // future owns its query string so we don't borrow `queries`.
        //
        // Milestone H-002/H-003: each sub-query search is retried up to
        // `search_max_retries` times with exponential backoff on transient
        // failures. A circuit-breaker tracks consecutive failures across all
        // sub-queries; once it opens, no further search calls are issued and
        // the gatherer falls back to no hits.
        //
        // The retry/circuit-breaker state is shared across futures via
        // `Arc<AtomicU32>` / `Arc<AtomicBool>` so it works correctly under
        // `buffer_unordered` parallelism.
        let search_tool = self.search.clone();
        let max_retries = self.search_max_retries;
        let base_delay_ms = self.search_retry_base_delay_ms;
        let circuit_threshold = self.search_circuit_breaker_threshold;
        let consecutive_failures = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let circuit_tripped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let search_futures: Vec<_> = queries
            .iter()
            .map(|q| {
                let q = q.clone();
                let tool = search_tool.clone();
                let cf = consecutive_failures.clone();
                let ct = circuit_tripped.clone();
                async move {
                    // Circuit-breaker check: if already tripped, skip this
                    // search entirely and return a marker error.
                    if ct.load(std::sync::atomic::Ordering::Relaxed) {
                        return SearchCallOutcome::CircuitOpen;
                    }
                    // Retry loop with exponential backoff.
                    let mut attempt: u32 = 0;
                    let mut last_error;
                    loop {
                        match tool.search(&q, max_results).await {
                            Ok(hits) => {
                                // Success: reset the consecutive-failure counter.
                                cf.store(0, std::sync::atomic::Ordering::Relaxed);
                                return SearchCallOutcome::Ok {
                                    hits,
                                    retries: attempt,
                                };
                            }
                            Err(e) => {
                                last_error = e.to_string();
                                let count =
                                    cf.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1;
                                // Circuit-breaker: trip when threshold reached.
                                if circuit_threshold > 0 && count >= circuit_threshold {
                                    ct.store(true, std::sync::atomic::Ordering::Relaxed);
                                }
                                if attempt >= max_retries {
                                    return SearchCallOutcome::Err {
                                        error: last_error.clone(),
                                        retries: attempt,
                                    };
                                }
                                attempt += 1;
                                let delay_ms = base_delay_ms.saturating_mul(1u64 << (attempt - 1));
                                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                                tracing::warn!(
                                    query = %q,
                                    attempt,
                                    error = %last_error,
                                    "research: retrying sub-query search after transient failure"
                                );
                            }
                        }
                    }
                }
            })
            .collect();
        let mut results = futures::stream::iter(search_futures)
            .buffer_unordered(4)
            .enumerate();

        let mut hits_by_url: Vec<(String, WebSearchHit)> = Vec::new();
        let mut seen_urls: HashSet<String> = HashSet::new();
        let mut any_search_error: Option<String> = None;
        let mut excluded_count = 0usize;
        let mut considered_count = 0usize;
        let log_rejected = |url: &str,
                            query: &str,
                            title: &str,
                            search_tool: &str,
                            search_engine: &str,
                            reason: &str,
                            detail: Option<&serde_json::Value>| {
            self.log_url_outcome(
                url,
                query,
                title,
                search_tool,
                search_engine,
                "rejected",
                reason,
                detail,
            );
        };
        let log_captured = |url: &str,
                            query: &str,
                            title: &str,
                            search_tool: &str,
                            search_engine: &str,
                            detail: Option<&serde_json::Value>| {
            self.log_url_outcome(
                url,
                query,
                title,
                search_tool,
                search_engine,
                "captured",
                "",
                detail,
            );
        };
        let mut circuit_open_emitted = false;

        while let Some((idx, outcome)) = results.next().await {
            let query = queries
                .get(idx)
                .cloned()
                .unwrap_or_else(|| topic.to_string());
            match outcome {
                SearchCallOutcome::Ok { hits, retries: _ } => {
                    for mut hit in hits {
                        let url_key = hit.url.to_lowercase();
                        if !seen_urls.insert(url_key) {
                            continue;
                        }
                        considered_count += 1;
                        hit.matched_query = query.clone();
                        self.log_url_outcome(
                            &hit.url,
                            &query,
                            &hit.title,
                            &hit.search_tool,
                            &hit.search_engine,
                            "considered",
                            "",
                            None,
                        );
                        // Pre-filter by title/snippet relevance before any
                        // expensive full-page fetch (B-001).
                        if let Some((label, retained)) = self.filter_hit(&query, &hit) {
                            if !retained {
                                // Retained because keep_low_relevance is on.
                                tracing::info!(
                                    query = %query,
                                    url = %hit.url,
                                    relevance = %label,
                                    "research: retaining low-relevance hit due to --use-low-relevance"
                                );
                            }
                            hit.matched_query = format!("{query} [{label}]");
                            hits_by_url.push((query.clone(), hit));
                        } else {
                            excluded_count += 1;
                            let reason =
                                format!("title/snippet relevance too low for query {query}");
                            tracing::info!(
                                query = %query,
                                url = %hit.url,
                                "research: skipping search hit due to low title/snippet relevance"
                            );
                            log_rejected(
                                &hit.url,
                                &query,
                                &hit.title,
                                &hit.search_tool,
                                &hit.search_engine,
                                &reason,
                                None,
                            );
                            if let Some(obs) = observer {
                                obs.on_event(GatherEvent::FetchFailed {
                                    url: hit.url.clone(),
                                    error: reason,
                                });
                            }
                        }
                    }
                }
                SearchCallOutcome::Err { error, retries } => {
                    // Emit retry events for each retry that was attempted.
                    if let Some(obs) = observer {
                        for r in 1..=retries {
                            obs.on_event(GatherEvent::SearchRetrying {
                                query: query.clone(),
                                attempt: r,
                                error: error.clone(),
                            });
                        }
                    }
                    tracing::warn!(
                        query = %query,
                        error = %error,
                        retries,
                        "research: sub-query search failed after retries"
                    );
                    any_search_error = Some(format!("{query}: {error}"));
                }
                SearchCallOutcome::CircuitOpen => {
                    // The circuit-breaker tripped before this sub-query
                    // started. Emit the circuit-open event once.
                    if !circuit_open_emitted {
                        let cf = consecutive_failures.load(std::sync::atomic::Ordering::Relaxed);
                        if let Some(obs) = observer {
                            obs.on_event(GatherEvent::SearchCircuitOpen {
                                consecutive_failures: cf,
                            });
                        }
                        circuit_open_emitted = true;
                        tracing::warn!(
                            consecutive_failures = cf,
                            "research: search circuit-breaker open; skipping remaining sub-queries"
                        );
                    }
                    any_search_error = Some(format!("{query}: search circuit-breaker open"));
                }
            }
        }

        if hits_by_url.is_empty() {
            if let Some(err) = any_search_error {
                if let Some(obs) = observer {
                    obs.on_event(GatherEvent::SearchFailed { error: err });
                }
            } else if let Some(obs) = observer {
                obs.on_event(GatherEvent::SearchReturnedNoHits);
            }
            tracing::info!("research: websearch returned 0 hits");
            if let Some(log) = &self.gather_log
                && let Err(e) =
                    log.lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .log_event(&serde_json::json!({
                            "event": "gather_summary",
                            "queries": queries,
                            "considered": considered_count,
                            "captured": 0,
                            "rejected": excluded_count,
                        }))
            {
                tracing::warn!(error = %e, "research: failed to write gather summary to web URL log");
            }
            return Ok(GatherResult {
                queries,
                sources: Vec::new(),
                pdf_count: 0,
                youtube_count: 0,
                excluded_count,
            });
        }

        // Fetch each unique candidate concurrently up to `fetch_concurrency`
        // at a time. `SourceCaptured` / `FetchFailed` events fire in
        // completion order (so the UI renders pages as they arrive); the
        // collected `(index, Option<Source>)` pairs are re-sorted into the
        // original search-ranking order afterwards so `web-NN.md` supporting
        // file names track hit position rather than completion timing.
        let fetch_concurrency = self.fetch_concurrency.max(1);
        let fetch_tool = self.fetch.clone();
        let fetch_timeout = self.fetch_timeout;
        // Renumber retained hits densely so the supporting-file names have no
        // gaps, while preserving the original search-ranking order.
        let candidates: Vec<(usize, String, WebSearchHit)> = hits_by_url
            .into_iter()
            .take(max_results)
            .enumerate()
            .map(|(index, (query, hit))| (index, query, hit))
            .collect();
        let fetch_futures = candidates.into_iter().map(|(index, query, hit)| {
            let fetch_tool = fetch_tool.clone();
            async move {
                let result = tokio::time::timeout(
                    fetch_timeout,
                    fetch_tool.fetch_with_limit(&hit.url, MAX_SOURCE_BODY_BYTES),
                )
                .await;
                (index, query, hit, result)
            }
        });
        let mut collected: Vec<(usize, Option<Source>)> = Vec::with_capacity(max_results);
        let mut stream = futures::stream::iter(fetch_futures).buffer_unordered(fetch_concurrency);
        while let Some((index, query, hit, result)) = stream.next().await {
            match result {
                Ok(Ok(page)) => {
                    let title = clean_web_source_title(&page.title, &hit.title);
                    let body_path = web_body_path(index);
                    let body = fence_captured_body(&page.body);
                    let (relevance, retained) =
                        compute_relevance_label(&query, &title, &hit.snippet, &page.url);
                    if !retained && !self.keep_low_relevance {
                        excluded_count += 1;
                        tracing::info!(
                            query = %query,
                            url = %page.url,
                            relevance = %relevance,
                            "research: skipping web source due to low relevance"
                        );
                        if let Some(obs) = observer {
                            obs.on_event(GatherEvent::FetchFailed {
                                url: page.url.clone(),
                                error: format!("relevance too low ({relevance})"),
                            });
                        }
                        collected.push((index, None));
                        log_rejected(
                            &page.url,
                            &query,
                            &title,
                            &hit.search_tool,
                            &hit.search_engine,
                            &format!("relevance too low ({relevance})"),
                            Some(&serde_json::json!({"relevance": relevance})),
                        );
                        continue;
                    }
                    // Reject pages whose extracted content is shorter than the
                    // minimum extractable content length. Near-empty
                    // extractions (paywalls, JS-only renders, soft 404s, empty
                    // PDFs) add noise to the synthesis prompt without
                    // contributing usable evidence.
                    let content_chars = page.body.chars().count();
                    if content_chars < MIN_EXTRACTABLE_CONTENT_CHARS {
                        excluded_count += 1;
                        let error = format!(
                            "extracted content too short ({content_chars} < {MIN_EXTRACTABLE_CONTENT_CHARS} chars)"
                        );
                        tracing::info!(
                            query = %query,
                            url = %page.url,
                            content_chars,
                            "research: skipping web source — extracted content below minimum"
                        );
                        if let Some(obs) = observer {
                            obs.on_event(GatherEvent::FetchFailed {
                                url: page.url.clone(),
                                error: error.clone(),
                            });
                        }
                        collected.push((index, None));
                        log_rejected(
                            &page.url,
                            &query,
                            &title,
                            &hit.search_tool,
                            &hit.search_engine,
                            &error,
                            Some(&serde_json::json!({"content_chars": content_chars})),
                        );
                        continue;
                    }
                    let body_preview: String = page
                        .body
                        .lines()
                        .filter(|l| !l.trim_start().starts_with("```"))
                        .collect::<Vec<_>>()
                        .join("\n")
                        .chars()
                        .take(MIN_EXTRACTABLE_CONTENT_CHARS)
                        .collect();
                    tracing::info!(
                        query = %query,
                        url = %page.url,
                        title = %title,
                        body_path = %body_path.display(),
                        body_chars = body.chars().count(),
                        relevance = %relevance,
                        "research: captured web source"
                    );
                    if let Some(obs) = observer {
                        obs.on_event(GatherEvent::SourceCaptured {
                            url: page.url.clone(),
                            title: title.clone(),
                            search_tool: hit.search_tool.clone(),
                            search_engine: hit.search_engine.clone(),
                            body_preview,
                            language: page
                                .language
                                .as_deref()
                                .map(str::to_uppercase)
                                .unwrap_or_else(|| "UNKNOWN".to_string()),
                        });
                    }
                    log_captured(
                        &page.url,
                        &query,
                        &title,
                        &hit.search_tool,
                        &hit.search_engine,
                        Some(&serde_json::json!({
                            "relevance": relevance,
                            "content_chars": content_chars,
                        })),
                    );
                    collected.push((
                        index,
                        Some(Source::Web {
                            url: page.url.clone(),
                            title,
                            captured_at: Utc::now(),
                            published_at: page.published_at,
                            body_path,
                            body,
                            relevance,
                            search_tool: hit.search_tool,
                            search_engine: hit.search_engine,
                            content_type: page.content_type.clone(),
                            page_type: page.page_type.clone(),
                            media_type: classify_web_source(
                                &page.url,
                                page.content_type.as_deref(),
                            )
                            .as_str()
                            .to_string(),
                            language: page.language.clone(),
                        }),
                    ));
                }
                Ok(Err(e)) => {
                    if let Some(obs) = observer {
                        obs.on_event(GatherEvent::FetchFailed {
                            url: hit.url.clone(),
                            error: e.to_string(),
                        });
                    }
                    tracing::warn!(
                        query = %query,
                        url = %hit.url,
                        error = %e,
                        "research: webfetch failed; skipping"
                    );
                    excluded_count += 1;
                    log_rejected(
                        &hit.url,
                        &query,
                        &hit.title,
                        &hit.search_tool,
                        &hit.search_engine,
                        &e.to_string(),
                        None,
                    );
                    collected.push((index, None));
                }
                Err(_) => {
                    let error = format!("fetch timed out after {}s", fetch_timeout.as_secs());
                    if let Some(obs) = observer {
                        obs.on_event(GatherEvent::FetchFailed {
                            url: hit.url.clone(),
                            error: error.clone(),
                        });
                    }
                    tracing::warn!(
                        query = %query,
                        url = %hit.url,
                        "research: webfetch timed out; skipping"
                    );
                    excluded_count += 1;
                    log_rejected(
                        &hit.url,
                        &query,
                        &hit.title,
                        &hit.search_tool,
                        &hit.search_engine,
                        &error,
                        None,
                    );
                    collected.push((index, None));
                }
            }
        }
        // Restore search-ranking order so `web-NN.md` numbers track hit
        // position rather than fetch-completion timing.
        collected.sort_by_key(|(index, _)| *index);
        let mut pdf_count = 0usize;
        let mut youtube_count = 0usize;
        let sources: Vec<Source> = collected
            .into_iter()
            .filter_map(|(_, src)| {
                if let Some(Source::Web {
                    url, content_type, ..
                }) = src.as_ref()
                {
                    match classify_web_source(url, content_type.as_deref()) {
                        WebSourceKind::Pdf => pdf_count += 1,
                        WebSourceKind::YouTube => youtube_count += 1,
                        WebSourceKind::Page => {}
                    }
                }
                src
            })
            .collect();
        tracing::info!(
            count = sources.len(),
            pdf_count,
            youtube_count,
            excluded_count,
            "research: web-gathering phase complete"
        );
        if let Some(log) = &self.gather_log
            && let Err(e) =
                log.lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .log_event(&serde_json::json!({
                        "event": "gather_summary",
                        "queries": queries,
                        "considered": considered_count,
                        "captured": sources.len(),
                        "rejected": excluded_count,
                    }))
        {
            tracing::warn!(error = %e, "research: failed to write gather summary to web URL log");
        }
        Ok(GatherResult {
            queries,
            sources,
            pdf_count,
            youtube_count,
            excluded_count,
        })
    }
}

/// Outcome of a single sub-query search call, including retry/circuit-breaker
/// state (Milestone H-002/H-003).
enum SearchCallOutcome {
    /// The search succeeded. `retries` records how many retries were needed
    /// (0 = succeeded on the first attempt).
    #[allow(dead_code)]
    Ok {
        /// Search hits returned by the tool.
        hits: Vec<WebSearchHit>,
        /// Number of retries before success.
        retries: u32,
    },
    /// The search failed after all retries were exhausted.
    Err {
        /// Last error message.
        error: String,
        /// Number of retries attempted.
        retries: u32,
    },
    /// The circuit-breaker was already open when this sub-query started, so no
    /// search call was made.
    CircuitOpen,
}

/// Compute a deterministic relevance note for a captured web source.
///
/// The score is based only on the search query that produced the hit and the
/// hit's title, snippet, and URL domain, so it adds zero LLM cost and is fully
/// reproducible. It returns a short human-readable string like:
///
/// - "High — title + snippet match query"
/// - "Medium — snippet matches query"
/// - "Low — weak match"
/// - "Very high — exact title match"
///
/// Compute the zero-padded supporting-file path for the Nth web source.
///
/// Index 0 → `web-01.md`, index 1 → `web-02.md`, etc. The path is
/// relative to the research item directory (`research/<name>/`).
fn web_body_path(index: usize) -> PathBuf {
    PathBuf::from(format!("sources/web-{:02}.md", index + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    /// Generate a body string of at least [`MIN_EXTRACTABLE_CONTENT_CHARS`]
    /// characters so fake fetched pages pass the minimum-content-length guard
    /// in [`WebGatherer::gather_with_observer`]. The `prefix` is repeated and
    /// padded so callers can still recognise their test content in assertions.
    fn body256(prefix: &str) -> String {
        let mut s = String::new();
        while s.chars().count() < MIN_EXTRACTABLE_CONTENT_CHARS {
            if !s.is_empty() {
                s.push(' ');
            }
            s.push_str(prefix);
        }
        s
    }

    /// In-memory `WebSearchTool` for tests.
    #[derive(Default)]
    struct FakeSearch {
        hits: Vec<WebSearchHit>,
        calls: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl WebSearchTool for FakeSearch {
        async fn search(
            &self,
            query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
            self.calls.lock().unwrap().push(query.to_string());
            // Tag each returned hit with the query that produced it so the
            // gatherer's relevance computation has realistic metadata.
            let mut out = self.hits.clone();
            for hit in &mut out {
                hit.matched_query = query.to_string();
            }
            Ok(out)
        }
    }

    /// In-memory `WebFetchTool` for tests. Each URL maps to an optional
    /// `WebFetchedPage`; missing URLs produce an error.
    #[derive(Default)]
    struct FakeFetch {
        pages: std::collections::HashMap<String, WebFetchedPage>,
        fail_urls: Vec<String>,
        calls: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl WebFetchTool for FakeFetch {
        async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
            self.calls.lock().unwrap().push(url.to_string());
            if self.fail_urls.iter().any(|u| u == url) {
                anyhow::bail!("simulated fetch failure for {url}");
            }
            self.pages
                .get(url)
                .cloned()
                .ok_or_else(|| anyhow::anyhow!("no fake page registered for {url}"))
        }
    }

    #[test]
    fn relevance_exact_title_match_unchanged() {
        let (label, retained) = compute_relevance_label(
            "Rust async runtime",
            "Rust async runtime",
            "some unrelated snippet",
            "https://example.com/foo",
        );
        assert_eq!(label, "Very high — exact title match");
        assert!(retained);
    }

    #[test]
    fn relevance_low_when_no_terms_match() {
        let (label, retained) = compute_relevance_label(
            "quantum computing",
            "Rust async runtime",
            "tokio and futures",
            "https://example.com/rust",
        );
        assert!(label.starts_with("Very low"));
        assert!(!retained);
    }

    fn gatherer_with(
        hits: Vec<WebSearchHit>,
        pages: std::collections::HashMap<String, WebFetchedPage>,
        fail_urls: Vec<String>,
    ) -> (WebGatherer, Arc<FakeSearch>, Arc<FakeFetch>) {
        let search = Arc::new(FakeSearch {
            hits,
            calls: Mutex::new(Vec::new()),
        });
        let fetch = Arc::new(FakeFetch {
            pages,
            fail_urls,
            calls: Mutex::new(Vec::new()),
        });
        let g = WebGatherer::new(search.clone(), fetch.clone());
        (g, search, fetch)
    }

    #[tokio::test]
    async fn gather_prefilters_low_relevance_hits_before_fetch() {
        // Both hits will be fetched if we don't pre-filter, but the second has
        // a title/snippet that does not match the query at all.
        let hits = vec![
            WebSearchHit {
                url: "https://good.example".into(),
                title: "Rust async runtime guide".into(),
                snippet: " Tokio and async Rust performance".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "test".into(),
            },
            WebSearchHit {
                url: "https://bad.example".into(),
                title: "completely unrelated shopping page".into(),
                snippet: "buy shoes and gadgets here".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "test".into(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://good.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://good.example".into(),
                title: "Rust async runtime guide".into(),
                body: body256("body good").into(),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        pages.insert(
            "https://bad.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://bad.example".into(),
                title: "completely unrelated shopping page".into(),
                body: body256("body bad").into(),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let (g, _, fetch) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("Rust async runtime", 5).await.unwrap();
        assert_eq!(
            sources.len(),
            1,
            "low-relevance hit should be pre-filtered before fetch"
        );
        if let Source::Web { url, .. } = &sources[0] {
            assert_eq!(url, "https://good.example");
        }
        let calls = fetch.calls.lock().unwrap();
        assert!(
            !calls.contains(&"https://bad.example".to_string()),
            "bad URL must not be fetched"
        );
    }

    #[tokio::test]
    async fn gather_keep_low_relevance_disables_prefilter() {
        let hits = vec![WebSearchHit {
            url: "https://bad.example".into(),
            title: "completely unrelated page".into(),
            snippet: "buy shoes".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://bad.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://bad.example".into(),
                title: "completely unrelated page".into(),
                body: body256("body").into(),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let g = g.with_keep_low_relevance(true);
        let sources = g.gather("Rust async runtime", 5).await.unwrap();
        assert_eq!(sources.len(), 1, "--use-low-relevance keeps the hit");
    }

    #[tokio::test]
    async fn gather_caps_huge_body_at_max_source_body_bytes() {
        use crate::document::MAX_SOURCE_BODY_BYTES;
        let hits = vec![WebSearchHit {
            url: "https://huge.example".into(),
            title: "Huge page".into(),
            snippet: "Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://huge.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://huge.example".into(),
                title: "Huge page".into(),
                body: "x".repeat(MAX_SOURCE_BODY_BYTES + 1024),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("Rust async runtime", 5).await.unwrap();
        assert_eq!(sources.len(), 1);
        if let Source::Web { body, .. } = &sources[0] {
            assert!(
                body.len() <= MAX_SOURCE_BODY_BYTES + 128,
                "body should be capped near MAX_SOURCE_BODY_BYTES, got {} bytes",
                body.len()
            );
            assert!(body.contains("truncated"));
        }
    }

    #[tokio::test]
    async fn gather_times_out_slow_fetch() {
        struct SlowFetch;
        #[async_trait]
        impl WebFetchTool for SlowFetch {
            async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
                tokio::time::sleep(std::time::Duration::from_secs(60)).await;
                Ok(WebFetchedPage {
                    published_at: None,
                    url: _url.to_string(),
                    title: "slow".into(),
                    body: "slow body".into(),
                    content_type: None,
                    page_type: None,
                    language: None,
                })
            }
        }
        let hits = vec![WebSearchHit {
            url: "https://slow.example".into(),
            title: "Slow".into(),
            snippet: "Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
        }];
        let g = WebGatherer::new(
            Arc::new(FakeSearch {
                hits,
                calls: Mutex::new(Vec::new()),
            }),
            Arc::new(SlowFetch),
        )
        .with_fetch_timeout(std::time::Duration::from_millis(100));
        #[derive(Default)]
        struct CollectEvents(Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        let obs = CollectEvents::default();
        let result = g
            .gather_with_observer("Rust async runtime", 5, Some(&obs))
            .await
            .unwrap();
        assert!(result.sources.is_empty(), "slow fetch should time out");
        let events = obs.0.lock().unwrap();
        assert!(
            events.iter().any(|e| matches!(
                e,
                GatherEvent::FetchFailed { url, error } if url == "https://slow.example" && error.contains("timed out")
            )),
            "expected timeout FetchFailed event, got {events:?}"
        );
    }

    #[tokio::test]
    async fn gather_preserves_search_ranking_order_with_prefilter_gap() {
        // Three hits: the middle one is low relevance and should be dropped.
        // The remaining two must still be numbered web-01 and web-02 in
        // search-ranking order.
        let hits = vec![
            WebSearchHit {
                url: "https://first.example".into(),
                title: "First Rust async page".into(),
                snippet: "Rust async runtime".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "test".into(),
            },
            WebSearchHit {
                url: "https://low.example".into(),
                title: "unrelated shopping".into(),
                snippet: "buy shoes".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "test".into(),
            },
            WebSearchHit {
                url: "https://second.example".into(),
                title: "Second Rust async page".into(),
                snippet: "Rust async runtime".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "test".into(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        for url in ["https://first.example", "https://second.example"] {
            pages.insert(
                url.into(),
                WebFetchedPage {
                    published_at: None,
                    url: url.into(),
                    title: format!("Title {url}"),
                    body: body256("body").into(),
                    content_type: None,
                    page_type: None,
                    language: None,
                },
            );
        }
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("Rust async runtime", 5).await.unwrap();
        assert_eq!(sources.len(), 2);
        assert_eq!(
            sources[0].body_path(),
            Some(PathBuf::from("sources/web-01.md").as_path())
        );
        assert_eq!(
            sources[1].body_path(),
            Some(PathBuf::from("sources/web-02.md").as_path())
        );
        if let Source::Web { url, .. } = &sources[0] {
            assert_eq!(url, "https://first.example");
        }
        if let Source::Web { url, .. } = &sources[1] {
            assert_eq!(url, "https://second.example");
        }
    }

    #[tokio::test]
    async fn gather_returns_empty_vec_when_search_returns_no_hits() {
        let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let sources = g.gather("rust async", 5).await.unwrap();
        assert!(sources.is_empty());
    }

    #[tokio::test]
    async fn gather_returns_empty_vec_when_search_tool_errors() {
        struct AlwaysFailSearch;
        #[async_trait]
        impl WebSearchTool for AlwaysFailSearch {
            async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
                anyhow::bail!("network down")
            }
        }
        struct OkFetch;
        #[async_trait]
        impl WebFetchTool for OkFetch {
            async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: "u".into(),
                    title: "t".into(),
                    body: body256("b").into(),
                    content_type: None,
                    page_type: None,
                    language: None,
                })
            }
        }
        let g = WebGatherer::new(Arc::new(AlwaysFailSearch), Arc::new(OkFetch));
        let sources = g.gather("topic", 5).await.unwrap();
        assert!(
            sources.is_empty(),
            "search failure must not surface as an error"
        );
    }

    #[tokio::test]
    async fn gather_creates_web_source_per_hit_with_sequential_body_paths() {
        let hits = vec![
            WebSearchHit {
                url: "https://a.example".into(),
                title: "A".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
            WebSearchHit {
                url: "https://b.example".into(),
                title: "B".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
            WebSearchHit {
                url: "https://c.example".into(),
                title: "C".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://a.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://a.example".into(),
                title: "A — resolved".into(),
                body: body256("body a").into(),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        pages.insert(
            "https://b.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://b.example".into(),
                title: "B — resolved".into(),
                body: body256("body b").into(),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        pages.insert(
            "https://c.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://c.example".into(),
                title: String::new(), // empty title should fall back to search hit title
                body: body256("body c").into(),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("topic", 5).await.unwrap();
        assert_eq!(sources.len(), 3);

        for (i, src) in sources.iter().enumerate() {
            let Source::Web {
                published_at: None,
                url,
                title,
                body_path,
                ..
            } = src
            else {
                panic!("expected Source::Web, got {src:?}");
            };
            assert_eq!(
                body_path.as_path(),
                PathBuf::from(format!("sources/web-{:02}.md", i + 1)).as_path()
            );
            assert!(!url.is_empty());
            assert!(!title.is_empty());
        }
        // The third source had an empty page title, so it should have
        // fallen back to the search-hit title "C".
        if let Source::Web { title, .. } = &sources[2] {
            assert_eq!(title, "C");
        }
    }
    #[tokio::test]
    async fn gather_skips_individual_fetch_failures() {
        let hits = vec![
            WebSearchHit {
                url: "https://ok".into(),
                title: "OK".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
            WebSearchHit {
                url: "https://bad".into(),
                title: "Bad".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://ok".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://ok".into(),
                title: "OK".into(),
                body: body256("b").into(),

                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, vec!["https://bad".into()]);
        let sources = g.gather("topic", 5).await.unwrap();
        assert_eq!(
            sources.len(),
            1,
            "failed fetch should be skipped, not abort"
        );
        if let Source::Web { url, .. } = &sources[0] {
            assert_eq!(url, "https://ok");
        }
    }

    #[tokio::test]
    async fn gather_suppresses_failed_youtube_fetch_with_reason() {
        // A YouTube hit whose fetch adapter errors out (e.g. transcript
        // extraction failed because no caption tracks are available) must not
        // produce a source: it is suppressed with the adapter's error message
        // surfaced in the FetchFailed event, the video never enters the
        // research corpus, and `youtube_count` stays at zero.
        let hits = vec![WebSearchHit {
            url: "https://www.youtube.com/watch?v=abc".into(),
            title: "Some Video".into(),
            snippet: "topic Rust async Tokio runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
        }];
        // No page is registered, so the URL must be in `fail_urls`; the real
        // adapter bails instead of returning a placeholder body.
        let (g, _, _) = gatherer_with(
            hits,
            std::collections::HashMap::new(),
            vec!["https://www.youtube.com/watch?v=abc".into()],
        );

        #[derive(Default)]
        struct CollectEvents(std::sync::Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        let obs = CollectEvents::default();
        g.gather_with_observer("topic", 5, Some(&obs))
            .await
            .unwrap();

        let events = obs.0.lock().unwrap();
        assert!(
            events.iter().any(|e| matches!(
                e,
                GatherEvent::FetchFailed { url, error }
                    if url == "https://www.youtube.com/watch?v=abc"
                        && error.contains("simulated fetch failure")
            )),
            "expected FetchFailed with the adapter error, got {events:?}"
        );
        assert!(
            !events.iter().any(|e| matches!(
                e,
                GatherEvent::SourceCaptured { url, .. }
                    if url == "https://www.youtube.com/watch?v=abc"
            )),
            "failed youtube fetch must not be captured as a source, got {events:?}"
        );
    }
    #[tokio::test]
    async fn gather_respects_max_results() {
        let hits = vec![
            WebSearchHit {
                url: "https://1".into(),
                title: "1".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
            WebSearchHit {
                url: "https://2".into(),
                title: "2".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
            WebSearchHit {
                url: "https://3".into(),
                title: "3".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        for u in ["https://1", "https://2", "https://3"] {
            pages.insert(
                u.into(),
                WebFetchedPage {
                    published_at: None,
                    url: u.into(),
                    title: u.into(),
                    body: body256("b").into(),

                    content_type: None,
                    page_type: None,
                    language: None,
                },
            );
        }
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("topic", 2).await.unwrap();
        assert_eq!(sources.len(), 2, "must not exceed max_results");
    }
    #[tokio::test]
    async fn gather_rejects_zero_max_results() {
        let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let err = g.gather("topic", 0).await.unwrap_err();
        assert!(matches!(err, WebGatherError::ZeroLimit));
    }

    #[tokio::test]
    async fn gather_rejects_empty_topic() {
        let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let err = g.gather("   ", 5).await.unwrap_err();
        assert!(matches!(err, WebGatherError::EmptyTopic));
    }

    #[tokio::test]
    async fn gather_records_search_call() {
        let (g, search, _) =
            gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let _ = g.gather("rust async", 5).await.unwrap();
        let calls = search.calls.lock().unwrap();
        assert_eq!(calls.as_slice(), &["rust async".to_string()]);
    }
    #[test]
    fn web_body_path_zero_pads_and_uses_one_based_index() {
        assert_eq!(web_body_path(0), PathBuf::from("sources/web-01.md"));
        assert_eq!(web_body_path(8), PathBuf::from("sources/web-09.md"));
        assert_eq!(web_body_path(9), PathBuf::from("sources/web-10.md"));
    }

    #[tokio::test]
    async fn gather_with_observer_emits_search_failed_on_search_error() {
        struct FailSearch;
        #[async_trait]
        impl WebSearchTool for FailSearch {
            async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
                anyhow::bail!("api key missing")
            }
        }
        struct OkFetch;
        #[async_trait]
        impl WebFetchTool for OkFetch {
            async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: "u".into(),
                    title: "t".into(),
                    body: body256("b").into(),

                    content_type: None,
                    page_type: None,
                    language: None,
                })
            }
        }
        #[derive(Default)]
        struct CollectEvents(Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        let g =
            WebGatherer::new(Arc::new(FailSearch), Arc::new(OkFetch)).with_search_max_retries(0);
        let obs = CollectEvents::default();
        let result = g
            .gather_with_observer("topic", 5, Some(&obs))
            .await
            .unwrap();
        assert!(result.sources.is_empty());
        assert_eq!(result.queries, vec!["topic".to_string()]);
        let events = obs.0.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[0], GatherEvent::QueriesDecomposed { queries } if queries == &["topic".to_string()])
        );
        assert!(
            matches!(&events[1],
                GatherEvent::SearchFailed { error } if error.contains("api key missing")
            ),
            "got {:?}",
            events[1]
        );
    }
    #[tokio::test]
    async fn gather_with_observer_emits_no_hits_when_search_is_empty() {
        #[derive(Default)]
        struct CollectEvents(Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        let (g, _, _) = gatherer_with(Vec::new(), std::collections::HashMap::new(), Vec::new());
        let obs = CollectEvents::default();
        let result = g
            .gather_with_observer("rust async", 5, Some(&obs))
            .await
            .unwrap();
        assert!(result.sources.is_empty());
        assert_eq!(result.queries, vec!["rust async".to_string()]);
        let events = obs.0.lock().unwrap();
        assert_eq!(events.len(), 2);
        assert!(
            matches!(&events[0], GatherEvent::QueriesDecomposed { queries } if queries == &["rust async".to_string()])
        );
        assert!(matches!(events[1], GatherEvent::SearchReturnedNoHits));
    }
    #[tokio::test]
    async fn gather_with_observer_emits_fetch_failed_for_each_bad_url() {
        let hits = vec![
            WebSearchHit {
                url: "https://ok".into(),
                title: "OK".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
            WebSearchHit {
                url: "https://bad".into(),
                title: "Bad".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://ok".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://ok".into(),
                title: "OK".into(),
                body: body256("b").into(),

                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, vec!["https://bad".into()]);
        #[derive(Default)]
        struct CollectEvents(Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        let obs = CollectEvents::default();
        let result = g
            .gather_with_observer("topic", 5, Some(&obs))
            .await
            .unwrap();
        assert_eq!(result.sources.len(), 1);
        let events = obs.0.lock().unwrap();
        assert!(
                events.iter().any(|e| matches!(
                    e,
                    GatherEvent::FetchFailed { url, error } if url == "https://bad" && error.contains("simulated fetch failure")
                )),
                "got {:?}",
                  *events
              );
    }

    #[tokio::test]
    async fn gather_with_decomposer_runs_parallel_sub_queries_and_dedupes() {
        struct RecordingSearch {
            responses: std::collections::HashMap<String, Vec<WebSearchHit>>,
            calls: Mutex<Vec<String>>,
        }
        #[async_trait]
        impl WebSearchTool for RecordingSearch {
            async fn search(
                &self,
                query: &str,
                _max_results: usize,
            ) -> anyhow::Result<Vec<WebSearchHit>> {
                self.calls.lock().unwrap().push(query.to_string());
                Ok(self.responses.get(query).cloned().unwrap_or_default())
            }
        }
        struct OkFetch;
        #[async_trait]
        impl WebFetchTool for OkFetch {
            async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: url.to_string(),
                    title: format!("title-{url}"),
                    body: body256(&format!("body-{url}")),
                    content_type: None,
                    page_type: None,
                    language: None,
                })
            }
        }

        let responses = std::collections::HashMap::from([
            (
                "Rust async".to_string(),
                vec![WebSearchHit {
                    url: "https://a.example".into(),
                    title: "A".into(),
                    snippet: "topic Rust async Tokio runtime".into(),
                    matched_query: String::new(),
                    search_tool: String::new(),
                    search_engine: String::new(),
                }],
            ),
            (
                "Tokio runtime".to_string(),
                vec![
                    WebSearchHit {
                        url: "https://a.example".into(), // duplicate URL
                        title: "A2".into(),
                        snippet: "topic Rust async Tokio runtime".into(),
                        matched_query: String::new(),
                        search_tool: String::new(),
                        search_engine: String::new(),
                    },
                    WebSearchHit {
                        url: "https://b.example".into(),
                        title: "B".into(),
                        snippet: "topic Rust async Tokio runtime".into(),
                        matched_query: String::new(),
                        search_tool: String::new(),
                        search_engine: String::new(),
                    },
                ],
            ),
        ]);
        let search = Arc::new(RecordingSearch {
            responses,
            calls: Mutex::new(Vec::new()),
        });
        let gatherer = WebGatherer::new(search.clone(), Arc::new(OkFetch))
            .with_decomposer(Arc::new(HeuristicQueryDecomposer));

        let result = gatherer
            .gather_with_observer("Rust async and Tokio runtime", 5, None)
            .await
            .unwrap();

        // Both sub-queries plus the catch-all full topic were issued.
        let calls = search.calls.lock().unwrap();
        assert!(calls.contains(&"Rust async".to_string()));
        assert!(calls.contains(&"Tokio runtime".to_string()));
        assert!(calls.contains(&"Rust async and Tokio runtime".to_string()));

        // The duplicate https://a.example URL is fetched only once.
        assert_eq!(
            result.sources.len(),
            2,
            "dedup should leave two unique URLs"
        );
        assert_eq!(result.queries.len(), 3);
    }

    #[tokio::test]
    async fn llm_decomposer_parses_json_queries() {
        use ragent_llm::llm::{ChatRequest, LlmClient, StreamEvent};
        use ragent_llm::providers::ProviderRegistry;
        use std::pin::Pin;

        struct JsonReplyClient {
            text: String,
        }

        #[async_trait]
        impl LlmClient for JsonReplyClient {
            async fn chat(
                &self,
                _request: ChatRequest,
            ) -> anyhow::Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>>
            {
                let events = vec![
                    StreamEvent::TextDelta {
                        text: self.text.clone(),
                    },
                    StreamEvent::Finish {
                        reason: ragent_llm::llm::LlmFinishReason::Stop,
                    },
                ];
                Ok(Box::pin(futures::stream::iter(events)))
            }
        }

        struct JsonProvider;

        #[async_trait]
        impl ragent_llm::provider::Provider for JsonProvider {
            fn id(&self) -> &'static str {
                "json"
            }

            fn name(&self) -> &'static str {
                "JSON"
            }

            fn default_models(&self) -> Vec<ragent_llm::provider::ModelInfo> {
                Vec::new()
            }

            async fn create_client(
                &self,
                _api_key: &str,
                _base_url: Option<&str>,
                _options: &std::collections::HashMap<String, serde_json::Value>,
            ) -> anyhow::Result<Box<dyn LlmClient>> {
                Ok(Box::new(JsonReplyClient {
                            text: r#"{"queries":["Rust async internals","Tokio runtime","Rust async and Tokio runtime"]}"#.into(),
                        }))
            }

            fn set_event_bus(&self, _event_bus: Option<Arc<ragent_types::event::EventBus>>) {}

            fn as_any_static(&self) -> &dyn std::any::Any {
                self
            }
        }

        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(JsonProvider));
        let decomposer = LlmQueryDecomposer::new(Arc::new(registry), "json", "json-model");
        let queries = decomposer
            .decompose("Rust async and Tokio runtime")
            .await
            .unwrap();
        assert_eq!(
            queries,
            vec![
                "Rust async internals".to_string(),
                "Tokio runtime".to_string(),
                "Rust async and Tokio runtime".to_string(),
            ]
        );
    }
    #[tokio::test]
    async fn llm_decomposer_falls_back_to_heuristic_on_bad_json() {
        use ragent_llm::llm::{ChatRequest, LlmClient, StreamEvent};
        use ragent_llm::providers::ProviderRegistry;
        use std::pin::Pin;

        struct BadJsonClient;

        #[async_trait]
        impl LlmClient for BadJsonClient {
            async fn chat(
                &self,
                _request: ChatRequest,
            ) -> anyhow::Result<Pin<Box<dyn futures::Stream<Item = StreamEvent> + Send>>>
            {
                let events = vec![
                    StreamEvent::TextDelta {
                        text: "not json".into(),
                    },
                    StreamEvent::Finish {
                        reason: ragent_llm::llm::LlmFinishReason::Stop,
                    },
                ];
                Ok(Box::pin(futures::stream::iter(events)))
            }
        }

        struct BadJsonProvider;

        #[async_trait]
        impl ragent_llm::provider::Provider for BadJsonProvider {
            fn id(&self) -> &'static str {
                "badjson"
            }

            fn name(&self) -> &'static str {
                "Bad JSON"
            }

            fn default_models(&self) -> Vec<ragent_llm::provider::ModelInfo> {
                Vec::new()
            }

            async fn create_client(
                &self,
                _api_key: &str,
                _base_url: Option<&str>,
                _options: &std::collections::HashMap<String, serde_json::Value>,
            ) -> anyhow::Result<Box<dyn LlmClient>> {
                Ok(Box::new(BadJsonClient))
            }

            fn set_event_bus(&self, _event_bus: Option<Arc<ragent_types::event::EventBus>>) {}

            fn as_any_static(&self) -> &dyn std::any::Any {
                self
            }
        }

        let mut registry = ProviderRegistry::new();
        registry.register(Box::new(BadJsonProvider));
        let decomposer = LlmQueryDecomposer::new(Arc::new(registry), "badjson", "badjson-model");
        let queries = decomposer
            .decompose("Rust async and Tokio runtime")
            .await
            .unwrap();
        assert!(queries.contains(&"Rust async".to_string()));
        assert!(queries.contains(&"Tokio runtime".to_string()));
        assert!(queries.contains(&"Rust async and Tokio runtime".to_string()));
    }

    /// A fetch tool that sleeps for a fixed duration before returning, and
    /// tracks the maximum number of concurrently in-flight `fetch` calls via
    /// an [`AtomicUsize`].
    struct ConcurrencyTrackingFetch {
        delay: std::time::Duration,
        in_flight: Arc<std::sync::atomic::AtomicUsize>,
        max_in_flight: Arc<std::sync::atomic::AtomicUsize>,
    }

    #[async_trait]
    impl WebFetchTool for ConcurrencyTrackingFetch {
        async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
            let prev = self
                .in_flight
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            // Track the high-water mark of concurrent in-flight fetches.
            let now = prev + 1;
            let mut max = self.max_in_flight.load(std::sync::atomic::Ordering::SeqCst);
            while now > max {
                match self.max_in_flight.compare_exchange(
                    max,
                    now,
                    std::sync::atomic::Ordering::SeqCst,
                    std::sync::atomic::Ordering::SeqCst,
                ) {
                    Ok(_) => break,
                    Err(actual) => max = actual,
                }
            }
            tokio::time::sleep(self.delay).await;
            self.in_flight
                .fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
            Ok(WebFetchedPage {
                published_at: None,
                url: _url.to_string(),
                title: format!("title-{_url}"),
                body: body256(&format!("body-{_url}")),
                content_type: None,
                page_type: None,
                language: None,
            })
        }
    }

    /// `with_fetch_concurrency(0)` is clamped up to `1` so the stream always
    /// makes progress; the field reflects the clamped value.
    #[test]
    fn with_fetch_concurrency_clamps_zero_to_one() {
        let search: Arc<dyn WebSearchTool> = Arc::new(FakeSearch::default());
        let fetch: Arc<dyn WebFetchTool> = Arc::new(ConcurrencyTrackingFetch {
            delay: std::time::Duration::ZERO,
            in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            max_in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });
        let g = WebGatherer::new(search, fetch).with_fetch_concurrency(0);
        assert_eq!(g.fetch_concurrency, 1);
        let g = g.with_fetch_concurrency(7);
        assert_eq!(g.fetch_concurrency, 7);
    }

    /// The fetch phase of [`WebGatherer::gather_with_observer`] issues up to
    /// `fetch_concurrency` page fetches concurrently. With 6 candidate URLs
    /// and `fetch_concurrency = 6`, all six fetches should be in flight at
    /// once (high-water mark == 6); with `fetch_concurrency = 2` the
    /// high-water mark is capped at 2.
    #[tokio::test]
    async fn gather_fetches_pages_concurrently_up_to_fetch_concurrency() {
        let hits: Vec<WebSearchHit> = (0..6)
            .map(|i| WebSearchHit {
                url: format!("https://h{i}.example"),
                title: format!("H{i}"),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
            })
            .collect();

        // fetch_concurrency = 6 → high-water mark should reach 6.
        let in_flight = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max_in_flight = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let search: Arc<dyn WebSearchTool> = Arc::new(FakeSearch {
            hits: hits.clone(),
            calls: Mutex::new(Vec::new()),
        });
        let fetch: Arc<dyn WebFetchTool> = Arc::new(ConcurrencyTrackingFetch {
            delay: std::time::Duration::from_millis(40),
            in_flight,
            max_in_flight: max_in_flight.clone(),
        });
        let g = WebGatherer::new(search, fetch).with_fetch_concurrency(6);
        let sources = g.gather("topic", 6).await.unwrap();
        assert_eq!(sources.len(), 6, "all six hits should be captured");
        assert_eq!(
            max_in_flight.load(std::sync::atomic::Ordering::SeqCst),
            6,
            "all 6 fetches should have been in flight simultaneously"
        );

        // fetch_concurrency = 2 → high-water mark should be capped at 2.
        let in_flight2 = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let max_in_flight2 = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let search2: Arc<dyn WebSearchTool> = Arc::new(FakeSearch {
            hits,
            calls: Mutex::new(Vec::new()),
        });
        let fetch2: Arc<dyn WebFetchTool> = Arc::new(ConcurrencyTrackingFetch {
            delay: std::time::Duration::from_millis(40),
            in_flight: in_flight2,
            max_in_flight: max_in_flight2.clone(),
        });
        let g2 = WebGatherer::new(search2, fetch2).with_fetch_concurrency(2);
        let sources2 = g2.gather("topic", 6).await.unwrap();
        assert_eq!(sources2.len(), 6);
        let max2 = max_in_flight2.load(std::sync::atomic::Ordering::SeqCst);
        assert!(
            max2 <= 2,
            "fetch_concurrency=2 should cap in-flight at 2, got {max2}"
        );
        assert_eq!(
            max2, 2,
            "with 6 hits and concurrency 2 the high-water mark should reach 2"
        );
    }

    /// The default `fetch_concurrency` on a freshly-constructed
    /// [`WebGatherer`] is [`DEFAULT_FETCH_CONCURRENCY`].
    #[test]
    fn default_fetch_concurrency_is_ten() {
        let search: Arc<dyn WebSearchTool> = Arc::new(FakeSearch::default());
        let fetch: Arc<dyn WebFetchTool> = Arc::new(ConcurrencyTrackingFetch {
            delay: std::time::Duration::ZERO,
            in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
            max_in_flight: Arc::new(std::sync::atomic::AtomicUsize::new(0)),
        });
        let g = WebGatherer::new(search, fetch);
        assert_eq!(g.fetch_concurrency, DEFAULT_FETCH_CONCURRENCY);
        assert_eq!(DEFAULT_FETCH_CONCURRENCY, 10);
    }

    /// `fetch_url_as_source` classifies the media type from the fetched page's
    /// content type so PDF and YouTube seed URLs are reported correctly.
    #[tokio::test]
    async fn fetch_url_as_source_classifies_pdf_and_youtube_media_types() {
        struct TypedFetch {
            pages: std::collections::HashMap<String, WebFetchedPage>,
        }
        #[async_trait]
        impl WebFetchTool for TypedFetch {
            async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
                self.pages
                    .get(url)
                    .cloned()
                    .ok_or_else(|| anyhow::anyhow!("no fake page for {url}"))
            }
        }

        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://example.com/paper.pdf".into(),
            WebFetchedPage {
                url: "https://example.com/paper.pdf".into(),
                title: "Paper".into(),
                body: "extracted pdf text".into(),
                content_type: Some("application/pdf".into()),
                page_type: Some("pdf".into()),
                published_at: None,
                language: None,
            },
        );
        pages.insert(
            "https://www.youtube.com/watch?v=abc123".into(),
            WebFetchedPage {
                url: "https://www.youtube.com/watch?v=abc123".into(),
                title: "Video".into(),
                body: "transcript text".into(),
                content_type: Some("text/html; charset=utf-8".into()),
                page_type: Some("youtube".into()),
                published_at: None,
                language: None,
            },
        );

        let g = WebGatherer::new(
            Arc::new(FakeSearch::default()),
            Arc::new(TypedFetch { pages }),
        );

        let (pdf_source, _) = g
            .fetch_url_as_source("https://example.com/paper.pdf")
            .await
            .unwrap();
        if let Source::Web { media_type, .. } = &pdf_source {
            assert_eq!(media_type, "pdf");
        } else {
            panic!("expected Source::Web for PDF");
        }

        let (yt_source, _) = g
            .fetch_url_as_source("https://www.youtube.com/watch?v=abc123")
            .await
            .unwrap();
        if let Source::Web { media_type, .. } = &yt_source {
            assert_eq!(media_type, "youtube");
        } else {
            panic!("expected Source::Web for YouTube");
        }
    }

    /// `gather` copies the detected language from the fetched page into the
    /// web source so the References Index can render it.
    #[tokio::test]
    async fn gather_propagates_detected_language_to_source() {
        let hits = vec![WebSearchHit {
            url: "https://fr.example".into(),
            title: "Article".into(),
            snippet: "topic Rust async".into(),
            matched_query: String::new(),
            search_tool: String::new(),
            search_engine: String::new(),
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://fr.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://fr.example".into(),
                title: "Article".into(),
                body: body256("corps de texte").into(),
                content_type: None,
                page_type: None,
                language: Some("French".into()),
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("topic", 5).await.unwrap();
        assert_eq!(sources.len(), 1);
        if let Source::Web { language, .. } = &sources[0] {
            assert_eq!(language.as_deref(), Some("French"));
        } else {
            panic!("expected Source::Web");
        }
    }

    /// `fetch_url_as_source` copies the detected language from the fetched page
    /// into the returned web source.
    #[tokio::test]
    async fn fetch_url_as_source_propagates_detected_language() {
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://es.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://es.example".into(),
                title: "Página".into(),
                body: body256("cuerpo").into(),
                content_type: None,
                page_type: None,
                language: Some("Spanish".into()),
            },
        );
        let g = WebGatherer::new(
            Arc::new(FakeSearch::default()),
            Arc::new(FakeFetch {
                pages,
                ..Default::default()
            }),
        );
        let (source, _) = g.fetch_url_as_source("https://es.example").await.unwrap();
        if let Source::Web { language, .. } = &source {
            assert_eq!(language.as_deref(), Some("Spanish"));
        } else {
            panic!("expected Source::Web");
        }
    }
    #[tokio::test]
    async fn gather_counts_pdf_and_youtube_sources() {
        let hits = vec![
            WebSearchHit {
                url: "https://example.com/paper.pdf".into(),
                title: "PDF".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "test".into(),
            },
            WebSearchHit {
                url: "https://www.youtube.com/watch?v=abc123".into(),
                title: "YouTube".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "test".into(),
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://example.com/paper.pdf".into(),
            WebFetchedPage {
                url: "https://example.com/paper.pdf".into(),
                title: "PDF".into(),
                body: body256("pdf body").into(),
                content_type: Some("application/pdf".into()),
                page_type: Some("pdf".into()),
                published_at: None,
                language: None,
            },
        );
        pages.insert(
            "https://www.youtube.com/watch?v=abc123".into(),
            WebFetchedPage {
                url: "https://www.youtube.com/watch?v=abc123".into(),
                title: "YouTube".into(),
                body: body256("youtube transcript").into(),
                content_type: Some("text/html".into()),
                page_type: Some("youtube".into()),
                published_at: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let result = g.gather_with_observer("topic", 5, None).await.unwrap();
        assert_eq!(result.pdf_count, 1);
        assert_eq!(result.youtube_count, 1);
        assert_eq!(result.sources.len(), 2);
    }

    // ── Milestone H-002: search retry tests ───────────────────────────

    /// Search tool that fails the first N calls then succeeds.
    struct FailNTimes {
        fail_count: std::sync::atomic::AtomicU32,
        n: u32,
        hits: Vec<WebSearchHit>,
    }

    #[async_trait]
    impl WebSearchTool for FailNTimes {
        async fn search(&self, _query: &str, _max: usize) -> anyhow::Result<Vec<WebSearchHit>> {
            let count = self
                .fail_count
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if count < self.n {
                anyhow::bail!("transient failure #{count}");
            }
            Ok(self.hits.clone())
        }
    }

    #[tokio::test]
    async fn h002_search_retries_then_succeeds() {
        let hits = vec![WebSearchHit {
            url: "https://retry.example".into(),
            title: "Rust async runtime".into(),
            snippet: "Tokio and async Rust".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://retry.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://retry.example".into(),
                title: "Rust async runtime".into(),
                body: body256("body").into(),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let search = Arc::new(FailNTimes {
            fail_count: std::sync::atomic::AtomicU32::new(0),
            n: 2,
            hits: hits.clone(),
        });
        let fetch = Arc::new(FakeFetch {
            pages,
            fail_urls: Vec::new(),
            calls: Mutex::new(Vec::new()),
        });
        let g = WebGatherer::new(search, fetch)
            .with_search_max_retries(3)
            .with_search_retry_base_delay_ms(0);
        let result = g.gather("Rust async runtime", 5).await.unwrap();
        assert_eq!(result.len(), 1, "search should succeed after retries");
    }

    #[tokio::test]
    async fn h002_search_retries_exhausted_emits_search_failed() {
        struct AlwaysFail;
        #[async_trait]
        impl WebSearchTool for AlwaysFail {
            async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
                anyhow::bail!("persistent failure")
            }
        }
        struct OkFetch;
        #[async_trait]
        impl WebFetchTool for OkFetch {
            async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: "u".into(),
                    title: "t".into(),
                    body: body256("b").into(),
                    content_type: None,
                    page_type: None,
                    language: None,
                })
            }
        }
        #[derive(Default)]
        struct CollectEvents(Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        let g = WebGatherer::new(Arc::new(AlwaysFail), Arc::new(OkFetch))
            .with_search_max_retries(2)
            .with_search_retry_base_delay_ms(0);
        let obs = CollectEvents::default();
        let result = g
            .gather_with_observer("topic", 5, Some(&obs))
            .await
            .unwrap();
        assert!(result.sources.is_empty());
        let events = obs.0.lock().unwrap();
        // Should have: QueriesDecomposed, SearchRetrying x2, SearchFailed.
        let retry_count = events
            .iter()
            .filter(|e| matches!(e, GatherEvent::SearchRetrying { .. }))
            .count();
        assert_eq!(retry_count, 2, "expected 2 retry events");
        assert!(
            events.iter().any(|e| matches!(
                e,
                GatherEvent::SearchFailed { error } if error.contains("persistent failure")
            )),
            "expected SearchFailed event"
        );
    }

    // ── Milestone H-003: circuit-breaker tests ────────────────────────

    #[tokio::test]
    async fn h003_circuit_breaker_opens_after_threshold_failures() {
        struct AlwaysFail;
        #[async_trait]
        impl WebSearchTool for AlwaysFail {
            async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
                anyhow::bail!("circuit test failure")
            }
        }
        struct OkFetch;
        #[async_trait]
        impl WebFetchTool for OkFetch {
            async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: "u".into(),
                    title: "t".into(),
                    body: body256("b").into(),
                    content_type: None,
                    page_type: None,
                    language: None,
                })
            }
        }
        #[derive(Default)]
        struct CollectEvents(Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        // Use a decomposer that returns 5 sub-queries so the
        // circuit-breaker has a chance to trip mid-stream.
        struct FiveQueries;
        #[async_trait]
        impl QueryDecomposer for FiveQueries {
            async fn decompose(&self, _topic: &str) -> anyhow::Result<Vec<String>> {
                Ok((0..5).map(|i| format!("q{i}")).collect())
            }
        }
        let g = WebGatherer::new(Arc::new(AlwaysFail), Arc::new(OkFetch))
            .with_decomposer(Arc::new(FiveQueries))
            .with_search_max_retries(0)
            .with_search_circuit_breaker_threshold(3)
            .with_search_retry_base_delay_ms(0);
        let obs = CollectEvents::default();
        let result = g
            .gather_with_observer("topic", 5, Some(&obs))
            .await
            .unwrap();
        assert!(result.sources.is_empty());
        let events = obs.0.lock().unwrap();
        // CircuitOpen should be emitted at least once.
        assert!(
            events.iter().any(|e| matches!(
                e,
                GatherEvent::SearchCircuitOpen { consecutive_failures }
                    if *consecutive_failures >= 3
            )),
            "expected SearchCircuitOpen event with failures >= 3, got {events:?}"
        );
    }

    #[tokio::test]
    async fn h003_circuit_breaker_disabled_when_threshold_zero() {
        struct AlwaysFail;
        #[async_trait]
        impl WebSearchTool for AlwaysFail {
            async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
                anyhow::bail!("no circuit failure")
            }
        }
        struct OkFetch;
        #[async_trait]
        impl WebFetchTool for OkFetch {
            async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
                Ok(WebFetchedPage {
                    published_at: None,
                    url: "u".into(),
                    title: "t".into(),
                    body: body256("b").into(),
                    content_type: None,
                    page_type: None,
                    language: None,
                })
            }
        }
        #[derive(Default)]
        struct CollectEvents(Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        struct ThreeQueries;
        #[async_trait]
        impl QueryDecomposer for ThreeQueries {
            async fn decompose(&self, _topic: &str) -> anyhow::Result<Vec<String>> {
                Ok((0..3).map(|i| format!("q{i}")).collect())
            }
        }
        let g = WebGatherer::new(Arc::new(AlwaysFail), Arc::new(OkFetch))
            .with_decomposer(Arc::new(ThreeQueries))
            .with_search_max_retries(0)
            .with_search_circuit_breaker_threshold(0)
            .with_search_retry_base_delay_ms(0);
        let obs = CollectEvents::default();
        let _result = g
            .gather_with_observer("topic", 5, Some(&obs))
            .await
            .unwrap();
        let events = obs.0.lock().unwrap();
        // No circuit-open event should be emitted.
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, GatherEvent::SearchCircuitOpen { .. })),
            "circuit breaker should be disabled when threshold is 0"
        );
    }

    #[tokio::test]
    async fn gather_rejects_pages_below_min_extractable_content_chars() {
        let hits = vec![WebSearchHit {
            url: "https://short.example".into(),
            title: "Short page".into(),
            snippet: "Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://short.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://short.example".into(),
                title: "Short page".into(),
                // 100 chars — below the 256-char minimum.
                body: "x".repeat(100),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("Rust async runtime", 5).await.unwrap();
        assert!(
            sources.is_empty(),
            "page with < MIN_EXTRACTABLE_CONTENT_CHARS should be rejected"
        );
    }

    #[tokio::test]
    async fn gather_accepts_pages_at_min_extractable_content_chars() {
        let hits = vec![WebSearchHit {
            url: "https://exact.example".into(),
            title: "Exact page".into(),
            snippet: "Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://exact.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://exact.example".into(),
                title: "Exact page".into(),
                // Exactly 256 chars — at the minimum.
                body: "x".repeat(MIN_EXTRACTABLE_CONTENT_CHARS),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("Rust async runtime", 5).await.unwrap();
        assert_eq!(
            sources.len(),
            1,
            "page with exactly MIN_EXTRACTABLE_CONTENT_CHARS should be accepted"
        );
    }

    #[tokio::test]
    async fn gather_source_captured_event_carries_body_preview() {
        let hits = vec![WebSearchHit {
            url: "https://preview.example".into(),
            title: "Preview page".into(),
            snippet: "Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://preview.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://preview.example".into(),
                title: "Preview page".into(),
                body: "A".repeat(500),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        #[derive(Default)]
        struct CollectEvents(Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        let obs = CollectEvents::default();
        g.gather_with_observer("Rust async runtime", 5, Some(&obs))
            .await
            .unwrap();
        let events = obs.0.lock().unwrap();
        let captured = events.iter().find(|e| {
            matches!(
                e,
                GatherEvent::SourceCaptured { url, .. }
                    if url == "https://preview.example"
            )
        });
        assert!(captured.is_some(), "expected SourceCaptured event");
        if let Some(GatherEvent::SourceCaptured {
            body_preview,
            language,
            ..
        }) = captured
        {
            assert_eq!(
                body_preview.chars().count(),
                MIN_EXTRACTABLE_CONTENT_CHARS,
                "body_preview should be exactly MIN_EXTRACTABLE_CONTENT_CHARS chars"
            );
            assert!(
                body_preview.chars().all(|c| c == 'A'),
                "body_preview should contain the first 256 chars of the body"
            );
            // language is None in the fake page → "UNKNOWN"
            assert_eq!(
                language, "UNKNOWN",
                "language should be UNKNOWN when page.language is None"
            );
        }
    }

    #[tokio::test]
    async fn gather_emits_fetch_failed_for_short_content() {
        let hits = vec![WebSearchHit {
            url: "https://tiny.example".into(),
            title: "Tiny page".into(),
            snippet: "Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://tiny.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://tiny.example".into(),
                title: "Tiny page".into(),
                body: "tiny".into(),
                content_type: None,
                page_type: None,
                language: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        #[derive(Default)]
        struct CollectEvents(Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        let obs = CollectEvents::default();
        let result = g
            .gather_with_observer("Rust async runtime", 5, Some(&obs))
            .await
            .unwrap();
        assert!(result.sources.is_empty());
        assert_eq!(result.excluded_count, 1);
        let events = obs.0.lock().unwrap();
        assert!(
            events.iter().any(|e| matches!(
                e,
                GatherEvent::FetchFailed { url, error }
                    if url == "https://tiny.example"
                        && error.contains("too short")
            )),
            "expected FetchFailed with 'too short' message, got {:?}",
            *events
        );
    }
}
