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

use ragent_tools_extended::masterfetch::language::detect_language_best_effort;

use crate::document::{MAX_SOURCE_BODY_BYTES, fence_source_body, truncate_body_to_bytes};
use crate::gather_log::GatherLog;
use crate::open_access::{
    DEFAULT_OA_MIN_FULL_TEXT_CHARS, OpenAccessClient, RecoveredOpenAccess, ReqwestOpenAccessClient,
    recover_open_access,
};
use crate::provider_stats::ProviderCallStats;
use crate::search_budget::{SearchBudget, SharedQueryCache};
use crate::source::Source;
use crate::source_vault::SourceVault;
use std::time::{Duration, Instant};

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

/// Default maximum number of web sources to capture per research item
/// (FR-011). The earlier 15-source cap was too restrictive for broad topics; a
/// larger default lets the decomposer's parallel queries surface a much wider
/// set of candidate URLs before the synthesis phase.
pub const DEFAULT_MAX_WEB_RESULTS: usize = 500;

/// Default per-fetch wall-clock timeout. Pages that take longer than this are
/// treated as a fetch failure so a single slow URL cannot stall the whole
/// gather pass (Milestone B-004).
pub const DEFAULT_FETCH_TIMEOUT: Duration = Duration::from_secs(30);

/// Default upper bound on the number of concurrent page fetches issued during
/// the capture phase of [`WebGatherer::gather_with_observer`]. 10 is a safe
/// middle ground: fast enough to keep wall-clock latency low when a search
/// returns many candidate URLs, while staying well clear of OS file-descriptor
/// limits and typical search-provider rate ceilings. Override with the
/// `--fetch-concurrently N` CLI flag or [`WebGatherer::with_fetch_concurrency`].
pub const DEFAULT_FETCH_CONCURRENCY: usize = 10;

/// Default wall-clock timeout for a single open-access recovery lookup
/// (Unpaywall / Europe PMC). The lookup is awaited inside the fetch dispatch
/// loop, so an un-timed call against a stalled OA API would freeze the
/// entire gather pass; this bound keeps the pause proportional to one lookup.
pub const OA_LOOKUP_TIMEOUT: Duration = Duration::from_secs(15);

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

/// Minimum content length (in characters) for a *scholarly* source captured
/// directly from the search-engine snippet.
///
/// Scholarly backends (e.g. OpenAlex) reconstruct the work's abstract into
/// the snippet and rank results by their own `relevance_score`. Such hits are
/// captured as self-contained sources without a URL fetch (the DOI/landing
/// page is typically a paywalled redirect that readability cannot extract), so
/// the much shorter, information-dense abstract replaces the full page body.
/// The threshold is correspondingly lower than [`MIN_EXTRACTABLE_CONTENT_CHARS`]
/// and only rejects works that expose no abstract at all.
pub const MIN_SCHOLARLY_CONTENT_CHARS: usize = 80;

/// Minimum content length (in characters) for an *encyclopedia* source
/// captured directly from the search-engine snippet.
///
/// Encyclopedia backends (e.g. Wikipedia) return a concise page summary via
/// their REST API; the snippet carries that summary as clean, extracted text.
/// Such hits are captured as self-contained sources without a URL fetch (the
/// full Wikipedia article HTML is large and readability extraction on it can
/// fail or produce inconsistent results), so the shorter, information-dense
/// summary replaces the full page body. The threshold matches
/// [`MIN_SCHOLARLY_CONTENT_CHARS`] and only rejects summaries that expose no
/// extract at all.
pub const MIN_ENCYCLOPEDIA_CONTENT_CHARS: usize = 80;

/// Returns `true` for scholarly search-engine hits that carry a reconstructed
/// abstract in their snippet and should be captured as self-contained sources
/// without a URL fetch.
///
/// Scholarly backends (currently OpenAlex) rank results by their own
/// `relevance_score`; the lexical title/snippet pre-filter is a heuristic for
/// unranked HTML scrapers (LangSearch, Tavily) and would reject most scholarly
/// titles because they do not lexically overlap with the (often rephrased)
/// research sub-query. Scholarly hits are therefore exempt from the lexical
/// filter and from the URL fetch — their snippet is the evidence.
///
/// A hit is scholarly only when OpenAlex is the *sole* contributing engine.
/// When the same URL is also returned by a general web engine (LangSearch,
/// Tavily), the page is a fetchable HTML page and the normal fetch path is
/// preferred so the richer page body is captured instead of the concise
/// abstract.
fn is_scholarly_hit(hit: &WebSearchHit) -> bool {
    let engines: Vec<&str> = hit.search_engine.split(',').map(str::trim).collect();
    !engines.is_empty() && engines.iter().all(|e| *e == "openalex")
}

/// Returns `true` for encyclopedia search-engine hits that carry a page
/// summary in their snippet and should be captured as self-contained sources
/// without a URL fetch.
///
/// Encyclopedia backends (currently Wikipedia) rank results by their own
/// search relevance; the lexical title/snippet pre-filter is a heuristic for
/// unranked HTML scrapers (LangSearch, Tavily) and would reject most
/// encyclopedia titles because they use proper names and technical terms
/// that do not lexically overlap with the (often rephrased) research
/// sub-query. Encyclopedia hits are therefore exempt from the lexical filter
/// and from the URL fetch — their snippet (the REST API page summary) is the
/// evidence.
///
/// A hit is encyclopedia only when Wikipedia is the *sole* contributing
/// engine. When the same URL is also returned by a general web engine
/// (LangSearch, Tavily), the page is a fetchable HTML page and the normal fetch
/// path is preferred so the richer page body is captured instead of the
/// concise summary.
fn is_encyclopedia_hit(hit: &WebSearchHit) -> bool {
    let engines: Vec<&str> = hit.search_engine.split(',').map(str::trim).collect();
    !engines.is_empty() && engines.iter().all(|e| *e == "wikipedia")
}

/// Build a self-contained [`WebFetchedPage`] for a search hit without fetching
/// its URL (used for scholarly abstracts and encyclopedia summaries).
fn synthesize_hit_page(hit: &WebSearchHit, page_type: &str) -> WebFetchedPage {
    WebFetchedPage {
        url: hit.url.clone(),
        title: hit.title.clone(),
        body: hit.snippet.clone(),
        published_at: None,
        content_type: None,
        page_type: Some(page_type.to_string()),
        language: detect_language_best_effort(&hit.snippet),
        // Snippets carry an engine-provided author list (e.g. OpenAlex
        // `authorships`) — the page is never fetched, so the hit's author is
        // the only source.
        author: hit.author.clone(),
    }
}

/// Return `Some(body)` for language detection, or `None` when the body is too
/// large and too uniform for language detection to be meaningful. A multi-MB
/// body made of a single repeated byte (benchmarks, cap tests) sends lingua
/// down a pathological path that can take minutes — such bodies carry no
/// linguistic signal anyway, so detection is skipped.
fn detectable_body(body: &str) -> Option<&str> {
    // lingua's n-gram builder walks the input char-by-char and takes minutes
    // on multi-kilobyte strings made of a single repeated symbol (benchmark /
    // cap-test filler bodies), so skip detection when the first 4096
    // characters are all the same character — such bodies carry no linguistic
    // signal by definition. Heterogeneous short bodies are unaffected.
    let probe: String = body.chars().take(4096).collect();
    if probe.len() >= 256 {
        let mut chars = probe.chars();
        if let Some(first) = chars.next()
            && chars.all(|c| c == first)
        {
            return None;
        }
    }
    Some(body)
}

/// Cap a captured web body at the same byte budget used by the supporting
/// file renderer so the body stored on the `Source` matches what ends up on
/// disk. Keeps runaway pages from blowing up the synthesis prompt.
fn fence_captured_body(body: &str) -> String {
    fence_source_body(body)
}

/// Build a short preview of `body` for progress display: strip fenced-code
/// lines, then take the first `MIN_EXTRACTABLE_CONTENT_CHARS` characters.
///
/// Implemented as a single streaming pass so we avoid the intermediate
/// `Vec<&str>` + `String::join` allocations of the previous multi-pass chain
/// (PERF-WEB-02, PERF-WEB-04).
fn body_preview_of(body: &str) -> String {
    let mut out = String::with_capacity(MIN_EXTRACTABLE_CONTENT_CHARS);
    let mut char_count = 0usize;
    for line in body.lines() {
        if line.trim_start().starts_with("```") {
            continue;
        }
        if out.is_empty() {
            out.push_str(line);
        } else {
            out.push('\n');
            char_count += 1;
            out.push_str(line);
        }
        char_count += line.chars().count();
        // Track char count incrementally instead of re-counting the full
        // string on every iteration (avoids O(n^2) for long lines).
        if char_count >= MIN_EXTRACTABLE_CONTENT_CHARS {
            break;
        }
    }
    // Trim to exact cap if we overshot on the last push.
    if out.chars().count() > MIN_EXTRACTABLE_CONTENT_CHARS {
        out.truncate(
            out.char_indices()
                .nth(MIN_EXTRACTABLE_CONTENT_CHARS)
                .map_or(out.len(), |(i, _)| i),
        );
    }
    out
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
    /// Number of unique candidate URLs that passed the title/snippet pre-filter
    /// and were considered for fetching during the width sweep.
    pub considered_count: usize,
    /// Backend search engines that contributed at least one hit, sorted and
    /// deduplicated (e.g. `["langsearch", "openalex", "tavily", "wikipedia"]`).
    pub engines: Vec<String>,
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
            considered_count: 0,
            engines: Vec::new(),
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
    /// `mf_search` this is a comma-separated list like `"openalex, wikipedia"`;
    /// for `websearch` it is `"tavily"`.
    pub search_engine: String,
    /// Author name when the search engine exposed one in its result payload
    /// (e.g. OpenAlex's joined `authorships` names or Exa's `author` field).
    /// `None` when no author metadata was available at search time; the
    /// fetcher may still extract an author from the fetched page metadata.
    pub author: Option<String>,
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
    /// Author name extracted from the page's embedded metadata, when the
    /// fetcher was able to determine one. `None` when the page did not expose
    /// parseable author information.
    pub author: Option<String>,
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
        /// Open-access recovery metadata when the full text was recovered
        /// from a legal OA copy instead of fetched from the original URL
        /// (FR-015).
        oa_recovery: Option<Box<crate::open_access::RecoveredOpenAccess>>,
        /// Classified content type of the captured page (`"page"`, `"pdf"`,
        /// or `"youtube"`) so the UI can aggregate captures by file type
        /// without re-classifying the URL.
        media_type: String,
    },
    /// The underlying search tool returned an error.
    SearchFailed {
        /// Error message from the search tool.
        error: String,
    },
    /// A single page fetch failed after the search produced a candidate URL.
    /// Reserved for genuine network/transport errors and timeouts; policy
    /// exclusions (low relevance, too-short body, disabled PDFs) are
    /// reported as [`GatherEvent::SourceExcluded`] instead so the UI can
    /// distinguish "the network failed" from "the page was filtered out".
    FetchFailed {
        /// URL that could not be fetched.
        url: String,
        /// Error message from the fetch tool.
        error: String,
    },
    /// A candidate was deliberately excluded by a gather policy rather than
    /// failing on the network: pre-fetch relevance filter, post-fetch
    /// relevance filter, minimum-content threshold, or PDF sources disabled.
    /// Surfaced separately so fetch-failure counters stay meaningful.
    SourceExcluded {
        /// URL of the excluded candidate.
        url: String,
        /// Human-readable exclusion reason.
        reason: String,
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
    /// Width-sweep summary emitted after all parallel sub-query searches and
    /// candidate fetches have resolved. Carries aggregate statistics so the
    /// tier router and the UI can display which `mf_search` backends
    /// contributed to the run (T-006, FR-005).
    WidthSweepSummary {
        /// Sub-queries that were issued in parallel.
        queries: Vec<String>,
        /// Unique backend search engines that returned at least one hit.
        engines: Vec<String>,
        /// Number of unique candidate URLs considered after deduplication and
        /// pre-filtering.
        considered: usize,
        /// Number of sources ultimately captured.
        captured: usize,
        /// Number of candidates excluded by relevance or content-length
        /// filters.
        excluded: usize,
    },
    /// The vault already contained enough sources to satisfy the query for the
    /// current tier, so no new web searches were issued (FR-016, T-021).
    VaultSufficient {
        /// Number of matching sources found in the vault.
        count: usize,
        /// Minimum required to satisfy the tier.
        required: usize,
        /// Tier that was requested for this run.
        tier: String,
    },
    /// The web-gathering phase is about to start with an active phase
    /// deadline (FR-009). Emitted exactly once at the start of
    /// [`WebGatherer::gather_with_observer`] when a deadline is configured, so
    /// UI layers can render a live countdown. Never emitted when the deadline
    /// is disabled (`--web-time 0`).
    PhaseStarted {
        /// Effective deadline for this phase, in seconds.
        deadline_secs: u64,
    },
    /// The optional phase deadline passed before the gather pass finished.
    /// Everything captured so far is returned as a partial [`GatherResult`]
    /// and the run proceeds to analysis/synthesis with those sources.
    PhaseTimedOut {
        /// Configured deadline in seconds.
        deadline_secs: u64,
        /// Number of sources already captured when the deadline fired.
        captured: usize,
    },
    /// The run-scoped search budget was exhausted mid-gather, so remaining
    /// sub-queries were skipped without issuing further search calls. Emitted
    /// once per gather pass. The pass degrades to the hits captured so far.
    SearchBudgetExhausted {
        /// Search calls consumed by this run when the budget ran out.
        used: usize,
        /// Configured budget limit.
        limit: usize,
    },
    /// End-of-pass summary of the search-provider request counts recorded by
    /// the attached [`ProviderCallStats`] counter. Emitted once per gather
    /// pass, only when a counter is attached.
    ProviderCallsSummary {
        /// `(search tool, call count)` pairs, sorted by tool name.
        tool_calls: Vec<(String, usize)>,
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
    /// When `true`, hits from scholarly search engines (e.g. OpenAlex) are
    /// filtered out during gathering so only general web search results are
    /// captured. Defaults to `false`.
    disable_scholarly: bool,
    /// When `true`, PDF documents returned by web search or supplied via
    /// `--from-url` are captured as web sources. Defaults to `false`; most
    /// PDFs require additional extraction time and are often paywalled or
    /// large, so they are skipped unless explicitly enabled.
    allow_pdf_web_sources: bool,
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
    /// Optional persistent source vault (Milestone T-004). When configured,
    /// [`gather_with_observer`] searches the vault before issuing any web search
    /// and returns matching stored sources directly, satisfying FR-009.
    vault: Option<Arc<SourceVault>>,
    /// Minimum number of vaulted sources that satisfies the query for the
    /// current tier. When the vault lookup returns at least this many sources,
    /// the gatherer skips new web searches entirely (FR-016, T-021). `None`
    /// disables the sufficient-source check and falls back to the FR-009
    /// behavior of using any vault match.
    sufficient_sources: Option<usize>,
    /// When `true`, the gatherer attempts to recover a legal open-access
    /// full-text copy via Unpaywall and Europe PMC for scholarly sources
    /// whose body is shorter than [`Self::oa_min_full_text_chars`] (FR-010).
    open_access_recovery: bool,
    /// Contact email required by Unpaywall's API terms.
    contact_email: Option<String>,
    /// Minimum body length (in characters) that triggers OA recovery for
    /// scholarly sources. Defaults to [`DEFAULT_OA_MIN_FULL_TEXT_CHARS`].
    oa_min_full_text_chars: usize,
    /// HTTP client used for Unpaywall/Europe PMC queries.
    oa_client: Option<Arc<dyn OpenAccessClient>>,
    /// Optional wall-clock deadline for the whole gather pass. When set,
    /// [`WebGatherer::gather_with_observer`] stops issuing searches and stops
    /// waiting for fetch completions once the deadline passes, returning
    /// everything captured so far as a partial [`GatherResult`] instead of
    /// blocking indefinitely. `None` (the default) means no deadline.
    phase_deadline: Option<Instant>,
    /// Optional LLM page summarizer (T-012 / T-013). When configured, each
    /// captured web page body is summarized before it is stored in the vault
    /// and before it is handed off to the synthesis pipeline. The original
    /// full body is still written to the vault so citations can trace back to
    /// the captured source (FR-003, FR-018).
    summarizer: Option<Arc<dyn crate::page_summarizer::PageSummarizer>>,
    /// Optional run-scoped search budget. When configured, each sub-query
    /// search reserves one call from the shared counter before issuing any
    /// provider request; once exhausted, remaining sub-queries are skipped.
    search_budget: Option<Arc<SearchBudget>>,
    /// Optional shared query-result cache. Successful search results are
    /// memoised by normalized query text so identical sub-queries across
    /// parallel researchers issue only one provider call.
    query_cache: Option<Arc<SharedQueryCache>>,
    /// Optional run-scoped per-provider search-request counter. When
    /// configured, each logical search call is recorded and an end-of-pass
    /// [`GatherEvent::ProviderCallsSummary`] is emitted.
    provider_stats: Option<Arc<ProviderCallStats>>,
}

impl std::fmt::Debug for WebGatherer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebGatherer")
            .field("has_decomposer", &self.decomposer.is_some())
            .field("fetch_concurrency", &self.fetch_concurrency)
            .field("fetch_timeout_ms", &self.fetch_timeout.as_millis())
            .field("keep_low_relevance", &self.keep_low_relevance)
            .field("disable_scholarly", &self.disable_scholarly)
            .field("allow_pdf_web_sources", &self.allow_pdf_web_sources)
            .field("search_max_retries", &self.search_max_retries)
            .field(
                "search_circuit_breaker_threshold",
                &self.search_circuit_breaker_threshold,
            )
            .field("has_gather_log", &self.gather_log.is_some())
            .field("has_vault", &self.vault.is_some())
            .field("has_summarizer", &self.summarizer.is_some())
            .field("open_access_recovery", &self.open_access_recovery)
            .field("oa_min_full_text_chars", &self.oa_min_full_text_chars)
            .field("has_contact_email", &self.contact_email.is_some())
            .field("has_phase_deadline", &self.phase_deadline.is_some())
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
            disable_scholarly: false,
            allow_pdf_web_sources: false,
            search_max_retries: DEFAULT_SEARCH_MAX_RETRIES,
            search_retry_base_delay_ms: DEFAULT_SEARCH_RETRY_BASE_DELAY_MS,
            search_circuit_breaker_threshold: DEFAULT_SEARCH_CIRCUIT_BREAKER_THRESHOLD,
            gather_log: None,
            vault: None,
            sufficient_sources: None,
            open_access_recovery: false,
            contact_email: None,
            oa_min_full_text_chars: DEFAULT_OA_MIN_FULL_TEXT_CHARS,
            oa_client: Some(Arc::new(ReqwestOpenAccessClient::new(None))),
            phase_deadline: None,
            summarizer: None,
            search_budget: None,
            query_cache: None,
            provider_stats: None,
        }
    }

    /// Set an optional wall-clock deadline for the whole gather pass.
    ///
    /// When the deadline passes, no new work is started (FR-008): the
    /// decomposer call, the wait for each sub-query search result, and the
    /// wait for each in-flight fetch completion are all bounded by the
    /// remaining budget, and truncation breaks the search and fetch loops
    /// before any further search or fetch is polled. Because fetches already
    /// in flight are cancelled on drop, the worst-case overshoot beyond the
    /// deadline is the completion of at most one bounded wait — a fetch
    /// future that has already been polled and resolves just as the deadline
    /// elapses is still recorded; nothing newer is initiated. Everything
    /// captured up to that point is returned as a partial [`GatherResult`] so
    /// the caller can proceed to analysis/synthesis with whatever was
    /// gathered.
    #[must_use]
    pub fn with_phase_deadline(mut self, deadline: Option<Instant>) -> Self {
        self.phase_deadline = deadline;
        self
    }

    /// Attach a persistent source vault (Milestone T-004). When a vault is
    /// configured, [`gather_with_observer`] searches it before issuing any web
    /// search calls; if the vault contains sources matching the topic, those
    /// sources are returned directly and the web search phase is skipped.
    /// This satisfies FR-009 and avoids re-fetching sources that have already
    /// been captured for this run.
    #[must_use]
    pub fn with_vault(mut self, vault: Arc<SourceVault>) -> Self {
        self.vault = Some(vault);
        self
    }

    /// Attach an optional LLM page summarizer (T-012 / T-013). When set, each
    /// captured web page body is summarized before it enters the vault and the
    /// synthesis pipeline. The original full body is still persisted in the vault
    /// so citations can resolve to the captured source (FR-018).
    #[must_use]
    pub fn with_summarizer(
        mut self,
        summarizer: Arc<dyn crate::page_summarizer::PageSummarizer>,
    ) -> Self {
        self.summarizer = Some(summarizer);
        self
    }

    /// Set the minimum number of vaulted sources that satisfies the query for
    /// the current tier (FR-016, T-021). When the vault lookup returns at least
    /// this many matching sources, no new web searches are issued.
    ///
    /// A value of `0` disables the sufficient-source check and restores the
    /// FR-009 behavior of using any vault match.
    #[must_use]
    pub fn with_sufficient_sources(mut self, n: usize) -> Self {
        self.sufficient_sources = if n == 0 { None } else { Some(n) };
        self
    }

    /// Enable or disable open-access recovery (FR-010) and set the contact
    /// email required by Unpaywall.
    #[must_use]
    pub fn with_open_access_recovery(
        mut self,
        enabled: bool,
        contact_email: Option<String>,
    ) -> Self {
        self.open_access_recovery = enabled;
        self.contact_email = contact_email.clone();
        self.oa_client = Some(Arc::new(ReqwestOpenAccessClient::new(contact_email)));
        self
    }

    /// Override the minimum full-text length that triggers OA recovery.
    #[must_use]
    pub fn with_oa_min_full_text_chars(mut self, n: usize) -> Self {
        self.oa_min_full_text_chars = n.max(1);
        self
    }

    /// Replace the OA HTTP client. Used by tests to inject a fake client.
    #[must_use]
    pub fn with_oa_client(mut self, client: Arc<dyn OpenAccessClient>) -> Self {
        self.oa_client = Some(client);
        self
    }

    /// Attach a JSONL gather log that records every search hit considered
    /// during [`gather_with_observer`] and whether it was captured or
    /// rejected (with the rejection reason). Log entries are appended as
    /// hits stream in and each concurrent fetch resolves. The log file is
    /// created eagerly (even when a pass yields no hits) and flushed when
    /// the gatherer is dropped. Failures to open the log are reported via
    /// the observer and tracing, never propagated.
    #[must_use]
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
        let lock = log.lock().unwrap_or_else(|p| p.into_inner());
        let result = lock.log_url(
            url,
            query,
            status,
            title,
            search_tool,
            search_engine,
            reason,
            detail,
        );
        if let Err(e) = result {
            tracing::warn!(error = %e, url, "research: web URL log write failed");
        }
    }

    /// Attach a query decomposer. When present, [`gather_with_observer`]
    /// decomposes the topic into parallel sub-queries and deduplicates the
    /// combined results.
    #[must_use]
    pub fn with_decomposer(mut self, decomposer: Arc<dyn QueryDecomposer>) -> Self {
        self.decomposer = Some(decomposer);
        self
    }

    /// Override the fetch-phase concurrency limit.
    ///
    /// Controls how many candidate page fetches are issued in parallel during
    /// [`gather_with_observer`]. Values of `0` are clamped up to `1` so the
    /// stream always makes progress. Larger values reduce wall-clock latency
    /// when a search returns many hits, at the cost of more in-flight HTTP
    /// connections and memory. The default is [`DEFAULT_FETCH_CONCURRENCY`]
    /// (10).
    ///
    /// This also bounds the deadline overshoot (FR-008): at most
    /// `fetch_concurrency` fetches are ever in flight, and truncation by the
    /// phase deadline cancels the rest, so the phase never starts a fetch
    /// after the deadline.
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
    ///
    /// Together with the phase deadline (FR-008) this bounds the worst-case
    /// overshoot: a fetch already in flight when the deadline elapses runs at
    /// most to its own timeout, and no new fetch is started after truncation.
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

    /// Disable scholarly search engines (e.g. OpenAlex) during gathering.
    ///
    /// When enabled, [`gather_with_observer`] filters out hits from
    /// scholarly backends so only general web search results are captured.
    #[must_use]
    pub fn with_disable_scholarly(mut self, disable: bool) -> Self {
        self.disable_scholarly = disable;
        self
    }

    /// Allow PDF documents from the web to be captured as sources.
    ///
    /// When `false` (the default), any search hit whose URL or declared
    /// `Content-Type` indicates a PDF is rejected before the expensive fetch
    /// and extraction pass, and any `--from-url` seed that resolves to a PDF
    /// is reported as a fetch failure. When `true`, PDFs are treated like
    /// normal pages and are fetched/extracted by the underlying `webfetch`
    /// tool.
    #[must_use]
    pub fn with_allow_pdf_web_sources(mut self, allow: bool) -> Self {
        self.allow_pdf_web_sources = allow;
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

    /// Attach a run-scoped search budget. Each sub-query search reserves one
    /// call from the shared counter before issuing any provider request; once
    /// exhausted, remaining sub-queries are skipped and the pass degrades to
    /// the hits captured so far.
    #[must_use]
    pub fn with_search_budget(mut self, budget: Arc<SearchBudget>) -> Self {
        self.search_budget = Some(budget);
        self
    }

    /// Attach a shared query-result cache. Successful search results are
    /// memoised by normalized query text so identical sub-queries across
    /// parallel researchers issue only one provider call.
    #[must_use]
    pub fn with_query_cache(mut self, cache: Arc<SharedQueryCache>) -> Self {
        self.query_cache = Some(cache);
        self
    }

    /// Attach a run-scoped per-provider search-request counter. Each logical
    /// search call is recorded and an end-of-pass
    /// [`GatherEvent::ProviderCallsSummary`] is emitted.
    #[must_use]
    pub fn with_provider_stats(mut self, stats: Arc<ProviderCallStats>) -> Self {
        self.provider_stats = Some(stats);
        self
    }

    /// Access the attached run-scoped per-provider search-request counter, if
    /// any. Cloning the `Arc` shares the same totals.
    #[must_use]
    pub fn provider_stats(&self) -> Option<Arc<ProviderCallStats>> {
        self.provider_stats.clone()
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
        if !self.allow_pdf_web_sources && classify_web_source(url, None) == WebSourceKind::Pdf {
            return Err(anyhow::anyhow!(
                "PDF web source excluded; use --use-pdf to enable"
            ));
        }
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
            .to_string(); // Fall back to an aggressive best-guess detector when the fetcher did
        // not report a language, so `--from-url` seed sources still get a
        // language label in the References Index.
        let language = page
            .language
            .clone()
            .or_else(|| detectable_body(&body).and_then(detect_language_best_effort));
        let captured_at = chrono::Utc::now();
        let mut summary_text: Option<String> = None;
        let body_for_source: String = if let Some(sum) = self.summarizer.as_ref() {
            match sum.summarize_page(&page.url, &page.body).await {
                Ok(page_summary) => {
                    summary_text = Some(page_summary.summary.clone());
                    page_summary.summary
                }
                Err(e) => {
                    tracing::warn!(
                        error = %e,
                        url = %page.url,
                        "research: seed URL summarization failed; using full body"
                    );
                    body.clone()
                }
            }
        } else {
            body.clone()
        };
        if let Some(vault) = self.vault.as_ref() {
            let new_source = crate::source_vault::NewVaultSource {
                url: page.url.clone(),
                title: title.clone(),
                fetch_timestamp: Some(captured_at),
                search_tool: String::new(),
                search_engine: String::new(),
                media_type: media_type.clone(),
                content_type: page.content_type.clone(),
                body_text: body.clone(),
                summary_text: summary_text.clone(),
            };
            if let Err(e) = vault.store(&new_source) {
                tracing::warn!(
                    error = %e,
                    url = %page.url,
                    "research: failed to store seed URL source in vault"
                );
            }
        }
        let source = Source::Web {
            url: page.url.clone(),
            title,
            captured_at,
            published_at: page.published_at,
            body_path: web_body_path(0),
            body: body_for_source,
            relevance: "User-supplied seed URL".into(),
            search_tool: String::new(),
            search_engine: String::new(),
            content_type: page.content_type.clone(),
            page_type: page.page_type.clone(),
            media_type,
            language,
            author: page.author.clone(),
            oa_recovery: None,
        };
        Ok((source, page))
    }

    /// Gather up to `max_results` web sources for `topic`.
    ///
    /// Returns an empty `Vec` (not an error) when:
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

    /// Search the configured [`SourceVault`] for sources matching `topic` and
    /// convert any matches into `Source::Web` entries.
    ///
    /// Returns `Ok(Some(GatherResult))` when the vault contains at least one
    /// matching source, signalling that the caller should skip the web search
    /// phase entirely. Returns `Ok(None)` when the vault is empty or configured
    /// but has no matches for this topic. Errors are returned so the caller can
    /// decide whether to fall back to web search.
    async fn gather_from_vault(
        &self,
        vault: Arc<SourceVault>,
        topic: &str,
        max_results: usize,
        observer: Option<&dyn GatherObserver>,
    ) -> anyhow::Result<Option<GatherResult>> {
        let topic = topic.to_string();
        let limit = max_results;
        let hits = {
            let vault = Arc::clone(&vault);
            let topic = topic.clone();
            tokio::task::spawn_blocking(move || vault.search(&topic, limit))
                .await
                .map_err(|e| anyhow::anyhow!("vault search task panicked: {e}"))??
        };

        if hits.is_empty() {
            return Ok(None);
        }

        let mut pdf_count = 0usize;
        let mut youtube_count = 0usize;
        let mut sources = Vec::with_capacity(hits.len());

        // M-026: `read_content` does blocking `fs::read_to_string` + a
        // `Mutex<Connection>`; off-load every read to the blocking pool and
        // run them concurrently (the reads are independent — sequential
        // `spawn_blocking(..).await` calls would serialize the whole batch
        // behind the slowest file). `join_all` preserves hit order, so the
        // per-index body-path mapping below is unchanged.
        let bodies: Vec<Result<String, anyhow::Error>> =
            futures::future::join_all(hits.iter().map(|hit| {
                let vault = Arc::clone(&vault);
                let source_id = hit.source_id.clone();
                async move {
                    // JoinError → anyhow; the inner Result is already
                    // `Result<String, SourceVaultError>` which converts via
                    // anyhow's blanket `From` impl.
                    match tokio::task::spawn_blocking(move || vault.read_content(&source_id)).await
                    {
                        Ok(inner) => inner.map_err(anyhow::Error::from),
                        Err(e) => Err(anyhow::anyhow!("vault read_content task panicked: {e}")),
                    }
                }
            }))
            .await;

        for (index, hit) in hits.into_iter().enumerate() {
            let body = match &bodies[index] {
                Ok(b) => b.clone(),
                Err(e) => {
                    tracing::warn!(source_id = %hit.source_id, error = %e, "research: failed to read vaulted source content; skipping");
                    continue;
                }
            };
            let relevance = format!("Vaulted — reused from {}", hit.run_tag);
            let body_preview = body_preview_of(&body);
            let kind = classify_web_source(&hit.url, None);
            match kind {
                WebSourceKind::Pdf => pdf_count += 1,
                WebSourceKind::YouTube => youtube_count += 1,
                WebSourceKind::Page => {}
            }
            let source = Source::Web {
                url: hit.url.clone(),
                title: hit.title,
                captured_at: hit.fetch_timestamp,
                published_at: None,
                body_path: web_body_path(index),
                body,
                relevance,
                search_tool: hit.search_tool,
                search_engine: hit.search_engine,
                content_type: None,
                page_type: None,
                media_type: kind.as_str().to_string(),
                language: None,
                author: None,
                oa_recovery: None,
            };
            if let Some(obs) = observer {
                obs.on_event(GatherEvent::SourceCaptured {
                    url: hit.url.clone(),
                    title: source.title().to_string(),
                    search_tool: source.search_tool().to_string(),
                    search_engine: source.search_engine().to_string(),
                    body_preview,
                    language: "UNKNOWN".to_string(),
                    oa_recovery: None,
                    media_type: source.media_type().to_string(),
                });
            }
            self.log_url_outcome(
                &hit.url,
                &topic,
                source.title(),
                source.search_tool(),
                source.search_engine(),
                "captured",
                "",
                Some(&serde_json::json!({"origin": "vault"})),
            );
            sources.push(source);
        }

        if sources.is_empty() {
            return Ok(None);
        }

        let engines: Vec<String> = sources
            .iter()
            .flat_map(|s| s.search_engine().split(','))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        let considered_count = sources.len();
        Ok(Some(GatherResult {
            queries: Vec::new(),
            sources,
            pdf_count,
            youtube_count,
            excluded_count: 0,
            considered_count,
            engines,
        }))
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
    ///
    /// # Deadline behaviour (`--web-time`, FR-008)
    ///
    /// When [`WebGatherer::with_phase_deadline`] set a deadline, every await
    /// point in this method is bounded by the remaining budget: the decomposer
    /// call, each search-result wait, and each fetch-completion wait. After
    /// the deadline elapses no new search or fetch is started; the loops break
    /// and everything captured so far is returned as a partial result. The
    /// worst-case overshoot beyond the deadline is the completion of at most
    /// the fetches already in flight at truncation — in-flight requests are
    /// cancelled on drop, and each is itself capped by `fetch_timeout` — so
    /// the phase never runs unbounded past the deadline.
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

        // Phase deadline (H-001 / --web-time): when set, the gather pass
        // becomes best-effort. Once the deadline passes we stop issuing new
        // searches and stop waiting for fetch completions, returning
        // everything captured so far as a partial result so the session can
        // proceed to analysis/synthesis with whatever was gathered.
        let deadline = self.phase_deadline;
        let deadline_secs = deadline
            .map(|d| {
                d.saturating_duration_since(std::time::Instant::now())
                    .as_secs()
            })
            .unwrap_or(0);
        // Phase-start notification (FR-009): emitted exactly once, before any
        // search or fetch work, whenever a deadline is configured so UI layers
        // can start a live countdown. The payload carries the *configured*
        // budget (deadline - Instant::now() at the moment the phase begins),
        // floored at 1 so a just-created deadline never reports 0s and is
        // always distinguishable from the deadline-disabled sentinel.
        // `deadline_secs == 0` unambiguously means the deadline is disabled
        // and no phase-start event is emitted at all.
        if deadline.is_some()
            && let Some(obs) = observer
        {
            obs.on_event(GatherEvent::PhaseStarted {
                deadline_secs: deadline_secs.max(1),
            });
        }
        // Deadline emission is single-shot (FR-004): `truncated` may be set by
        // several bounded waits (decomposer, search loop, fetch loop), but the
        // `PhaseTimedOut` event fires exactly once, from the single terminal
        // site at the end of the gather pass, carrying the final captured
        // count. Interim sites only set the flag and break their loops.
        //
        // No-new-work guarantee (FR-008): all three await points below —
        // (a) the decomposer call, (b) each `results.next()` in the search
        // loop, and (c) each `stream.next()` in the fetch loop — are wrapped
        // in `tokio::time::timeout(remaining(), ..)` via `next_bounded!` or an
        // explicit call. Once the deadline elapses, `truncated` is set and
        // both loops break before polling any further search or fetch, so no
        // new search or fetch is initiated after the deadline. In-flight
        // futures are cancelled on drop, which cancels the underlying
        // request; the worst-case overshoot is therefore bounded by the
        // completion of the bounded waits already resolved at truncation
        // (at most one in-flight fetch, itself capped by `fetch_timeout`).
        let mut truncated = false;
        let remaining = || {
            deadline.map_or_else(
                || std::time::Duration::from_secs(u64::MAX / 2),
                |d| d.saturating_duration_since(std::time::Instant::now()),
            )
        };
        // Await the next stream item, but bound the wait by the phase
        // deadline so a stalled search/fetch cannot outlive the budget. The
        // abandoned futures are cancelled on drop, which cancels the
        // underlying request.
        //
        // This macro is the mechanism behind the no-new-work guarantee
        // (FR-008): every stream wait in the search and fetch loops is
        // deadline-bounded, so after the deadline elapses the loops see
        // `truncated` and break instead of polling more work.
        macro_rules! next_bounded {
            ($stream:expr) => {
                match tokio::time::timeout(remaining(), $stream.next()).await {
                    Ok(item) => item,
                    Err(_) => {
                        truncated = true;
                        None
                    }
                }
            };
        }
        // Single terminal emission site (FR-004): called once after the
        // gather loops unwind, with the final captured count. Interim
        // truncation sites set `truncated` and break without emitting.
        let emit_deadline_event = |observer: Option<&dyn GatherObserver>, captured: usize| {
            if let Some(obs) = observer {
                obs.on_event(GatherEvent::PhaseTimedOut {
                    deadline_secs,
                    captured,
                });
            }
            tracing::warn!(
                deadline_secs,
                captured,
                "research: web phase deadline reached; proceeding with sources gathered so far"
            );
        };

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

        // If a source vault is configured, try to satisfy the request from
        // already-captured sources before issuing any new web search (FR-009).
        // The vault is searched by the raw topic; matching sources are
        // converted back into `Source::Web` entries and returned directly.
        // Errors are logged and treated as "no matches" so a vault problem
        // never fails a gather pass.
        if let Some(vault) = &self.vault {
            match self
                .gather_from_vault(vault.clone(), topic, max_results, observer)
                .await
            {
                Ok(Some(result)) => {
                    // FR-016 (T-021): if the vault already contains enough
                    // sources for the requested tier, skip new web searches.
                    let required = self.sufficient_sources.unwrap_or(1);
                    if result.sources.len() >= required {
                        tracing::info!(
                            topic,
                            sources = result.sources.len(),
                            required,
                            "research: vault has sufficient sources; skipping web search"
                        );
                        if let Some(obs) = observer {
                            obs.on_event(GatherEvent::VaultSufficient {
                                count: result.sources.len(),
                                required,
                                tier: self
                                    .sufficient_sources
                                    .map(|_| "configured".to_string())
                                    .unwrap_or_else(|| "default".to_string()),
                            });
                        }
                        return Ok(result);
                    }
                    tracing::info!(
                        topic,
                        sources = result.sources.len(),
                        required,
                        "research: vault sources are below tier threshold; continuing to web search"
                    );
                }
                Ok(None) => {
                    tracing::info!(
                        topic,
                        "research: vault lookup returned no matches; continuing to web search"
                    );
                }
                Err(e) => {
                    tracing::warn!(error = %e, "research: vault lookup failed; continuing to web search");
                }
            }
        }

        // Determine the set of sub-queries. If no decomposer is configured
        // we still treat the original topic as a single query so callers see
        // a consistent [`GatherResult`].
        let queries: Vec<String> = if truncated {
            Vec::new()
        } else {
            match &self.decomposer {
                Some(d) => {
                    // Bound decomposition by the remaining phase budget; a
                    // stalled LLM decomposer must not consume the whole
                    // budget or outlive it. When the deadline elapses here,
                    // `truncated` short-circuits into an empty query set, so
                    // no sub-query search is issued at all (FR-008: no new
                    // work after the deadline).
                    match tokio::time::timeout(remaining(), d.decompose(topic)).await {
                        Ok(Ok(qs)) if !qs.is_empty() => qs,
                        Ok(Ok(_)) => {
                            tracing::warn!(
                                "research: decomposer returned empty queries; using topic"
                            );
                            vec![topic.to_string()]
                        }
                        Ok(Err(e)) => {
                            tracing::warn!(
                                error = %e,
                                "research: query decomposition failed; falling back to single query"
                            );
                            vec![topic.to_string()]
                        }
                        Err(_) => {
                            truncated = true;
                            Vec::new()
                        }
                    }
                }
                None => vec![topic.to_string()],
            }
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
        let search_budget = self.search_budget.clone();
        let query_cache = self.query_cache.clone();
        let provider_stats = self.provider_stats.clone();
        let consecutive_failures = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let circuit_tripped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let search_futures: Vec<_> = queries
            .iter()
            .map(|q| {
                let q = q.clone();
                let tool = search_tool.clone();
                let cf = consecutive_failures.clone();
                let ct = circuit_tripped.clone();
                let budget = search_budget.clone();
                let cache = query_cache.clone();
                let stats = provider_stats.clone();
                async move {
                    // Circuit-breaker check: if already tripped, skip this
                    // search entirely and return a marker error.
                    if ct.load(std::sync::atomic::Ordering::Relaxed) {
                        return SearchCallOutcome::CircuitOpen;
                    }
                    // Run-scoped search budget: reserve one call before any
                    // provider request. Exhaustion skips the search entirely.
                    if let Some(budget) = &budget
                        && !budget.try_acquire()
                    {
                        return SearchCallOutcome::BudgetExhausted;
                    }
                    // Shared query cache: an identical query already answered
                    // this run is served without a provider call.
                    if let Some(cache) = &cache
                        && let Some(hits) = cache.get(&q)
                    {
                        return SearchCallOutcome::Ok { hits, retries: 0 };
                    }
                    // Retry loop with exponential backoff.
                    let mut attempt: u32 = 0;
                    let mut last_error;
                    loop {
                        match tool.search(&q, max_results).await {
                            Ok(hits) => {
                                // Success: reset the consecutive-failure counter.
                                cf.store(0, std::sync::atomic::Ordering::Relaxed);
                                // Record one logical search call (retries
                                // included) and memoise the result.
                                if let Some(stats) = &stats {
                                    stats.record(&hit_search_tool(&hits));
                                }
                                if let Some(cache) = &cache {
                                    cache.insert(&q, hits.clone());
                                }
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
                                    // Record the logical call even on terminal
                                    // failure so the provider total reflects
                                    // the paid request.
                                    if let Some(stats) = &stats {
                                        stats.record(&hit_search_tool(&[]));
                                    }
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
        let mut budget_exhausted_emitted = false;

        while let Some((idx, outcome)) = next_bounded!(results) {
            // Deadline reached while waiting for the next sub-query search:
            // stop issuing further searches and move on with what we have.
            // No further search is polled, and none is newly started
            // (FR-008). The `PhaseTimedOut` event is emitted once at the
            // terminal site.
            if truncated {
                break;
            }
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
                        // Classify once so the two branches below can reuse the
                        // result instead of re-splitting the engine CSV twice.
                        let is_scholarly = is_scholarly_hit(&hit);
                        let is_encyclopedia = is_encyclopedia_hit(&hit);
                        // Filter out scholarly hits when --no-papers is set.
                        if self.disable_scholarly && is_scholarly {
                            excluded_count += 1;
                            let reason = "scholarly engine excluded by --no-papers";
                            tracing::info!(
                                query = %query,
                                url = %hit.url,
                                "research: skipping scholarly hit due to --no-papers"
                            );
                            log_rejected(
                                &hit.url,
                                &query,
                                &hit.title,
                                &hit.search_tool,
                                &hit.search_engine,
                                reason,
                                None,
                            );
                            continue;
                        }
                        // Filter out PDF web sources unless explicitly enabled.
                        if !self.allow_pdf_web_sources
                            && classify_web_source(&hit.url, None) == WebSourceKind::Pdf
                        {
                            excluded_count += 1;
                            let reason = "PDF web source excluded; use --use-pdf to enable";
                            tracing::info!(
                                query = %query,
                                url = %hit.url,
                                "research: skipping PDF web source (use --use-pdf to enable)"
                            );
                            log_rejected(
                                &hit.url,
                                &query,
                                &hit.title,
                                &hit.search_tool,
                                &hit.search_engine,
                                reason,
                                None,
                            );
                            if let Some(obs) = observer {
                                obs.on_event(GatherEvent::SourceExcluded {
                                    url: hit.url.clone(),
                                    reason: reason.to_string(),
                                });
                            }
                            continue;
                        }
                        considered_count += 1;
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
                        // Scholarly hits (e.g. OpenAlex) are already ranked by
                        // the source engine's own relevance score and carry a
                        // reconstructed abstract in the snippet. The lexical
                        // pre-filter is a heuristic for unranked HTML scrapers
                        // and would wrongly reject scholarly titles that do not
                        // lexically overlap the (often rephrased) sub-query, so
                        // these hits bypass it entirely.
                        if is_scholarly {
                            hit.matched_query =
                                format!("{query} [Scholarly — engine-ranked abstract]");
                            hits_by_url.push((query.clone(), hit));
                            continue;
                        }
                        // Encyclopedia hits (e.g. Wikipedia) are already ranked
                        // by the source engine's own search relevance and carry
                        // a clean page summary in the snippet. The lexical
                        // pre-filter is a heuristic for unranked HTML scrapers
                        // and would wrongly reject encyclopedia titles (proper
                        // names, technical terms) that do not lexically overlap
                        // the sub-query, so these hits bypass it entirely.
                        if is_encyclopedia {
                            hit.matched_query =
                                format!("{query} [Encyclopedia — engine-ranked summary]");
                            hits_by_url.push((query.clone(), hit));
                            continue;
                        }
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
                                obs.on_event(GatherEvent::SourceExcluded {
                                    url: hit.url.clone(),
                                    reason,
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
                SearchCallOutcome::BudgetExhausted => {
                    // The run-scoped search budget ran out before this
                    // sub-query started. Emit the budget-exhausted event once
                    // and skip the remaining sub-queries.
                    if !budget_exhausted_emitted {
                        if let Some(budget) = &self.search_budget {
                            if let Some(obs) = observer {
                                obs.on_event(GatherEvent::SearchBudgetExhausted {
                                    used: budget.used(),
                                    limit: budget.limit().unwrap_or(0),
                                });
                            }
                        }
                        budget_exhausted_emitted = true;
                        tracing::warn!(
                            "research: run search budget exhausted; skipping remaining sub-queries"
                        );
                    }
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
            if let Some(obs) = observer {
                obs.on_event(GatherEvent::WidthSweepSummary {
                    queries: queries.clone(),
                    engines: Vec::new(),
                    considered: considered_count,
                    captured: 0,
                    excluded: excluded_count,
                });
            }
            return Ok(GatherResult {
                queries,
                sources: Vec::new(),
                pdf_count: 0,
                youtube_count: 0,
                excluded_count,
                considered_count,
                engines: Vec::new(),
            });
        }

        // Fetch each unique candidate concurrently up to `fetch_concurrency`
        // at a time. `SourceCaptured` / `FetchFailed` events fire in
        // completion order (so the UI renders pages as they arrive); the
        // collected `(index, Option<Source>)` pairs are re-sorted into the
        // original search-ranking order afterwards so `web-NN.md` supporting
        // file names track hit position rather than completion timing.
        //
        // Overshoot bound (FR-008): only `fetch_concurrency` fetch futures
        // are polled at any moment; `buffer_unordered` does not start a new
        // fetch until an in-flight one resolves. When the deadline truncates
        // the loop, the stream (and with it every queued future) is dropped
        // and the in-flight requests are cancelled, so the phase's overshoot
        // past the deadline is at most the completion time of the bounded
        // wait that observed the deadline — never a fresh fetch.
        let fetch_concurrency = self.fetch_concurrency.max(1);
        let fetch_tool = self.fetch.clone();
        let fetch_timeout = self.fetch_timeout;
        // Renumber retained hits densely so the supporting-file names have no
        // gaps, while preserving the original search-ranking order.
        // Collect the set of contributing engines before consuming hits_by_url.
        let mut engines: Vec<String> = hits_by_url
            .iter()
            .flat_map(|(_, hit)| hit.search_engine.split(','))
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect();
        engines.sort();
        let fetch_futures = hits_by_url
            .into_iter()
            .take(max_results)
            .enumerate()
            .map(|(index, (query, hit))| (index, query, hit))
            .map(|(index, query, hit)| {
                let fetch_tool = fetch_tool.clone();
                async move {
                    // Scholarly and encyclopedia hits are captured as
                    // self-contained sources from the snippet — the underlying
                    // page is either a paywalled redirect (DOI/landing page)
                    // or a large article whose readability extraction is
                    // unreliable, so a URL fetch would drop or degrade the
                    // result. Synthesize a page directly from the hit instead.
                    let special_page_type: Option<&str> = if is_scholarly_hit(&hit) {
                        Some("scholarly")
                    } else if is_encyclopedia_hit(&hit) {
                        Some("encyclopedia")
                    } else {
                        None
                    };
                    let result: Result<
                        Result<WebFetchedPage, anyhow::Error>,
                        tokio::time::error::Elapsed,
                    > = if let Some(page_type) = special_page_type {
                        Ok(Ok(synthesize_hit_page(&hit, page_type)))
                    } else {
                        tokio::time::timeout(
                            fetch_timeout,
                            fetch_tool.fetch_with_limit(&hit.url, MAX_SOURCE_BODY_BYTES),
                        )
                        .await
                    };
                    // Best-effort language detection is CPU-bound lingua work:
                    // compute it here, concurrent with the other in-flight
                    // fetches, instead of serially in the dispatch loop where
                    // it stalls the whole event stream behind every page.
                    let language_fallback = match &result {
                        Ok(Ok(page)) => {
                            let body = page.body.clone();
                            tokio::task::spawn_blocking(move || {
                                detectable_body(&body).and_then(detect_language_best_effort)
                            })
                            .await
                            .ok()
                            .flatten()
                        }
                        _ => None,
                    };
                    (index, query, hit, result, language_fallback)
                }
            });
        let mut collected: Vec<(usize, Option<Source>)> = Vec::with_capacity(max_results);
        let mut stream = futures::stream::iter(fetch_futures).buffer_unordered(fetch_concurrency);
        while let Some((index, query, hit, result, language_fallback)) = next_bounded!(stream) {
            // Deadline reached while waiting for the next fetch completion:
            // abandon the remaining in-flight fetches (they are cancelled on
            // drop) and keep everything captured so far. No further fetch is
            // polled, and none is newly started (FR-008). The `PhaseTimedOut`
            // event is emitted once at the terminal site.
            if truncated {
                break;
            }
            match result {
                Ok(Ok(page)) => {
                    let scholarly = page.page_type.as_deref() == Some("scholarly");
                    let encyclopedia = page.page_type.as_deref() == Some("encyclopedia");
                    let mut title = clean_web_source_title(&page.title, &hit.title);
                    let body_path = web_body_path(index);
                    let body = fence_captured_body(&page.body);
                    // Use the fetcher's language when available; otherwise run
                    // an aggressive best-guess detector on the body so that
                    // stored `Source::Web.language` is rarely `None`.
                    let detected_language = page.language.clone().or(language_fallback);
                    // Scholarly and encyclopedia sources are already ranked by
                    // the source engine (e.g. OpenAlex's `relevance_score`,
                    // Wikipedia's search relevance); skip the lexical
                    // post-fetch filter, which would reject them for the same
                    // lack of query-term overlap as the pre-filter.
                    let (relevance, retained) = if scholarly {
                        ("Scholarly — engine-ranked abstract".to_string(), true)
                    } else if encyclopedia {
                        ("Encyclopedia — engine-ranked summary".to_string(), true)
                    } else {
                        compute_relevance_label(&query, &title, &hit.snippet, &page.url)
                    };
                    if !retained && !self.keep_low_relevance {
                        excluded_count += 1;
                        tracing::info!(
                            query = %query,
                            url = %page.url,
                            relevance = %relevance,
                            "research: skipping web source due to low relevance"
                        );
                        if let Some(obs) = observer {
                            obs.on_event(GatherEvent::SourceExcluded {
                                url: page.url.clone(),
                                reason: format!("relevance too low ({relevance})"),
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
                    // contributing usable evidence. Scholarly sources use a
                    // lower threshold — their body is the concise reconstructed
                    // abstract, not a full page.
                    let mut page = page;
                    let mut oa_recovery: Option<Box<RecoveredOpenAccess>> = None;
                    if self.open_access_recovery {
                        let is_short = page.body.chars().count() < self.oa_min_full_text_chars;
                        let is_scholarly_url = scholarly
                            || page.page_type.as_deref() == Some("scholarly")
                            || crate::open_access::extract_doi(&page.url).is_some();
                        if is_short
                            && is_scholarly_url
                            && let Some(client) = self.oa_client.clone()
                        {
                            let email = self.contact_email.as_deref();
                            // Bound the OA lookup so a stalled Unpaywall /
                            // Europe PMC API cannot freeze the whole dispatch
                            // loop (the loop blocks on every completion, so an
                            // un-timed lookup halts progress for every other
                            // in-flight fetch).
                            match tokio::time::timeout(
                                OA_LOOKUP_TIMEOUT,
                                recover_open_access(&page.url, email, client.as_ref()),
                            )
                            .await
                            {
                                Ok(Ok(Some(recovered))) => {
                                    let recovered_url = recovered.url.clone();
                                    let recovered_source = recovered.source.to_string();
                                    tracing::info!(
                                        url = %page.url,
                                        recovered_url = %recovered_url,
                                        source = %recovered_source,
                                        "research: recovering open-access full text"
                                    );
                                    let recovered_result = tokio::time::timeout(
                                        fetch_timeout,
                                        fetch_tool.fetch_with_limit(
                                            &recovered_url,
                                            MAX_SOURCE_BODY_BYTES,
                                        ),
                                    )
                                    .await;
                                    match recovered_result {
                                        Ok(Ok(recovered_page)) => {
                                            page.body = recovered_page.body;
                                            page.title = recovered_page.title;
                                            page.content_type = recovered_page.content_type.clone();
                                            page.page_type = recovered_page.page_type.clone();
                                            page.language = recovered_page.language.clone();
                                            page.author = recovered_page.author.clone();
                                            // Recompute the source title from the recovered page
                                            // so the References Index reflects the OA copy.
                                            title = clean_web_source_title(&page.title, &hit.title);
                                            oa_recovery = Some(Box::new(recovered));
                                        }
                                        Ok(Err(e)) => {
                                            tracing::warn!(
                                                url = %page.url,
                                                recovered_url = %recovered_url,
                                                error = %e,
                                                "research: failed to fetch recovered OA copy; keeping original"
                                            );
                                        }
                                        Err(_) => {
                                            tracing::warn!(
                                                url = %page.url,
                                                recovered_url = %recovered_url,
                                                "research: recovered OA fetch timed out; keeping original"
                                            );
                                        }
                                    }
                                }
                                Ok(Ok(None)) => {}
                                Ok(Err(e)) => {
                                    tracing::warn!(
                                        url = %page.url,
                                        error = %e,
                                        "research: OA recovery lookup failed"
                                    );
                                }
                                Err(_) => {
                                    tracing::warn!(
                                        url = %page.url,
                                        timeout_secs = OA_LOOKUP_TIMEOUT.as_secs(),
                                        "research: OA recovery lookup timed out; keeping original"
                                    );
                                }
                            }
                        }
                    }

                    let content_chars = page.body.chars().count();
                    let min_chars = if scholarly || encyclopedia {
                        if scholarly {
                            MIN_SCHOLARLY_CONTENT_CHARS
                        } else {
                            MIN_ENCYCLOPEDIA_CONTENT_CHARS
                        }
                    } else {
                        MIN_EXTRACTABLE_CONTENT_CHARS
                    };
                    if content_chars < min_chars {
                        excluded_count += 1;
                        let error = if scholarly {
                            format!(
                                "scholarly abstract too short ({content_chars} < {min_chars} chars)"
                            )
                        } else if encyclopedia {
                            format!(
                                "encyclopedia summary too short ({content_chars} < {min_chars} chars)"
                            )
                        } else {
                            format!(
                                "extracted content too short ({content_chars} < {min_chars} chars)"
                            )
                        };
                        tracing::info!(
                            query = %query,
                            url = %page.url,
                            content_chars,
                            "research: skipping web source — extracted content below minimum"
                        );
                        if let Some(obs) = observer {
                            obs.on_event(GatherEvent::SourceExcluded {
                                url: page.url.clone(),
                                reason: error.clone(),
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
                    let body_preview = body_preview_of(&page.body);
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
                            language: detected_language
                                .as_deref()
                                .map(str::to_uppercase)
                                .unwrap_or_else(|| "UNKNOWN".to_string()),
                            oa_recovery: oa_recovery.clone(),
                            media_type: classify_web_source(
                                &page.url,
                                page.content_type.as_deref(),
                            )
                            .as_str()
                            .to_string(),
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
                    let captured_at = Utc::now();
                    let mut summary_text: Option<String> = None;
                    let body_for_source: String = if let Some(sum) = self.summarizer.as_ref() {
                        match sum.summarize_page(&page.url, &page.body).await {
                            Ok(page_summary) => {
                                summary_text = Some(page_summary.summary.clone());
                                tracing::info!(
                                    url = %page.url,
                                    summary_chars = page_summary.summary.chars().count(),
                                    "research: summarized web source"
                                );
                                page_summary.summary
                            }
                            Err(e) => {
                                tracing::warn!(
                                    error = %e,
                                    url = %page.url,
                                    "research: page summarization failed; using full body"
                                );
                                body.clone()
                            }
                        }
                    } else {
                        body.clone()
                    };
                    if let Some(vault) = self.vault.as_ref() {
                        let new_source = crate::source_vault::NewVaultSource {
                            url: page.url.clone(),
                            title: title.clone(),
                            fetch_timestamp: Some(captured_at),
                            search_tool: hit.search_tool.clone(),
                            search_engine: hit.search_engine.clone(),
                            media_type: classify_web_source(
                                &page.url,
                                page.content_type.as_deref(),
                            )
                            .as_str()
                            .to_string(),
                            content_type: page.content_type.clone(),
                            body_text: body.clone(),
                            summary_text: summary_text.clone(),
                        };
                        if let Err(e) = vault.store(&new_source) {
                            tracing::warn!(
                                error = %e,
                                url = %page.url,
                                "research: failed to store source in vault"
                            );
                        }
                    }
                    collected.push((
                        index,
                        Some(Source::Web {
                            url: page.url.clone(),
                            title,
                            captured_at,
                            published_at: page.published_at,
                            body_path,
                            body: body_for_source,
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
                            language: detected_language.clone(),
                            author: page.author.clone(),
                            oa_recovery,
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
            truncated,
            "research: web-gathering phase complete"
        );
        if truncated {
            emit_deadline_event(observer, sources.len());
        }
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
        if let Some(obs) = observer {
            obs.on_event(GatherEvent::WidthSweepSummary {
                queries: queries.clone(),
                engines: engines.clone(),
                considered: considered_count,
                captured: sources.len(),
                excluded: excluded_count,
            });
        }
        // End-of-pass provider-call summary, emitted once when a counter is
        // attached so the session can surface per-tool request totals.
        if let Some(obs) = observer
            && let Some(stats) = &self.provider_stats
        {
            obs.on_event(GatherEvent::ProviderCallsSummary {
                tool_calls: stats.by_tool(),
            });
        }
        Ok(GatherResult {
            queries,
            sources,
            pdf_count,
            youtube_count,
            excluded_count,
            considered_count,
            engines,
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
    /// The run-scoped search budget was exhausted before this sub-query
    /// started, so no search call was made.
    BudgetExhausted,
}

/// Determine the search-tool name to attribute a logical search call to.
///
/// Prefers the `search_tool` field of the first returned hit; falls back to
/// `"unknown"` when the hit list is empty (e.g. a terminal failure where no
/// hits were captured) so the provider total still reflects the paid request.
fn hit_search_tool(hits: &[WebSearchHit]) -> String {
    hits.first()
        .map(|h| h.search_tool.clone())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
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
    #![allow(clippy::assert_is_empty)]
    use super::*;
    use crate::source_vault::{NewVaultSource, SourceVault};
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
                author: None,
            },
            WebSearchHit {
                url: "https://bad.example".into(),
                title: "completely unrelated shopping page".into(),
                snippet: "buy shoes and gadgets here".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "test".into(),
                author: None,
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://good.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://good.example".into(),
                title: "Rust async runtime guide".into(),
                body: body256("body good"),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
        pages.insert(
            "https://bad.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://bad.example".into(),
                title: "completely unrelated shopping page".into(),
                body: body256("body bad"),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
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
    async fn gather_captures_scholarly_hit_without_fetch() {
        // An OpenAlex hit carries a reconstructed abstract in the snippet and
        // is captured as a self-contained source. The DOI/landing-page URL is
        // never fetched (it would be a paywalled redirect readability cannot
        // extract), and the lexical relevance pre-filter is bypassed because
        // OpenAlex already ranked the result by its own relevance score.
        let abstract_text = "We present a novel approach to async runtime \
             scheduling in Rust using a work-stealing executor that improves \
             throughput by thirty percent on benchmarks.";
        let hits = vec![WebSearchHit {
            url: "https://doi.org/10.1000/rust-async".into(),
            title: "Work-Stealing Async Scheduling in Rust".into(),
            snippet: format!("{abstract_text} (Year: 2024 | Cited: 17 | OA: yes | Source: ACM)"),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "openalex".into(),
            author: Some("Ada Lovelace, Alan Turing".into()),
        }];
        let (g, _, fetch) = gatherer_with(hits, std::collections::HashMap::new(), Vec::new());
        let sources = g.gather("free keyless open search APIs", 5).await.unwrap();
        assert_eq!(sources.len(), 1, "scholarly hit should be captured");
        match &sources[0] {
            Source::Web {
                url,
                title,
                relevance,
                page_type,
                search_engine,
                language,
                author,
                ..
            } => {
                assert_eq!(url, "https://doi.org/10.1000/rust-async");
                assert_eq!(
                    author.as_deref(),
                    Some("Ada Lovelace, Alan Turing"),
                    "scholarly hit author should propagate to the captured source"
                );
                assert_eq!(title, "Work-Stealing Async Scheduling in Rust");
                assert_eq!(page_type.as_deref(), Some("scholarly"));
                assert_eq!(search_engine, "openalex");
                assert!(
                    relevance.contains("Scholarly"),
                    "scholarly sources use the engine-ranked relevance label"
                );
                assert_eq!(
                    language.as_deref(),
                    Some("English"),
                    "OpenAlex snippet should have its language detected"
                );
            }
            other => panic!("expected Source::Web, got {other:?}"),
        }
        let calls = fetch.calls.lock().unwrap();
        assert!(
            calls.is_empty(),
            "scholarly hit must not trigger a URL fetch (no network), got {calls:?}"
        );
    }

    #[tokio::test]
    async fn gather_filters_scholarly_hit_when_no_papers_set() {
        // With --no-papers, OpenAlex hits should be filtered out before
        // any fetch or capture.
        let hits = vec![WebSearchHit {
            url: "https://doi.org/10.1000/rust-async".into(),
            title: "Work-Stealing Async Scheduling in Rust".into(),
            snippet: format!(
                "We present a novel approach to async runtime \
                 scheduling in Rust using a work-stealing executor that improves \
                 throughput by thirty percent on benchmarks. \
                 (Year: 2024 | Cited: 17 | OA: yes | Source: ACM)"
            ),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "openalex".into(),
            author: None,
        }];
        let (g, _, fetch) = gatherer_with(hits, std::collections::HashMap::new(), Vec::new());
        let g = g.with_disable_scholarly(true);
        let sources = g.gather("free keyless open search APIs", 5).await.unwrap();
        assert_eq!(
            sources.len(),
            0,
            "scholarly hit should be filtered out by --no-papers"
        );
        let calls = fetch.calls.lock().unwrap();
        assert!(
            calls.is_empty(),
            "no fetch should be attempted for filtered scholarly hit"
        );
    }

    #[tokio::test]
    async fn gather_rejects_scholarly_hit_with_no_abstract() {
        // An OpenAlex work with no abstract produces a snippet shorter than
        // `MIN_SCHOLARLY_CONTENT_CHARS`; it should be rejected rather than
        // admitted as near-empty noise.
        let hits = vec![WebSearchHit {
            url: "https://doi.org/10.1000/no-abstract".into(),
            title: "Untitled Work With No Abstract".into(),
            snippet: "(Year: 2024 | Cited: 0 | OA: no)".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "openalex".into(),
            author: None,
        }];
        let (g, _, fetch) = gatherer_with(hits, std::collections::HashMap::new(), Vec::new());
        let sources = g.gather("anything at all", 5).await.unwrap();
        assert!(
            sources.is_empty(),
            "abstract-less scholarly hit should be rejected"
        );
        let calls = fetch.calls.lock().unwrap();
        assert!(calls.is_empty(), "no fetch should be attempted");
    }

    #[tokio::test]
    async fn gather_fetches_shared_engine_url_instead_of_treating_as_scholarly() {
        // When the same URL is returned by OpenAlex *and* a general web engine,
        // the page is a fetchable HTML page. The gatherer must take the normal
        // fetch path (capturing the richer page body) rather than treating it
        // as a scholarly snippet-only source.
        let hits = vec![WebSearchHit {
            url: "https://shared.example/paper".into(),
            title: "Rust async runtime".into(),
            snippet: "Rust async runtime tokio".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "langsearch, openalex".into(),
            author: None,
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://shared.example/paper".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://shared.example/paper".into(),
                title: "Rust async runtime".into(),
                body: body256("full page body for the shared URL"),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
        let (g, _, fetch) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("Rust async runtime", 5).await.unwrap();
        assert_eq!(sources.len(), 1, "shared-engine hit should be captured");
        match &sources[0] {
            Source::Web { page_type, .. } => {
                assert_ne!(
                    page_type.as_deref(),
                    Some("scholarly"),
                    "shared-engine URL must not be treated as scholarly"
                );
            }
            other => panic!("expected Source::Web, got {other:?}"),
        }
        let calls = fetch.calls.lock().unwrap();
        assert!(
            calls.contains(&"https://shared.example/paper".to_string()),
            "shared-engine URL must be fetched normally, got {calls:?}"
        );
    }

    #[tokio::test]
    async fn gather_captures_encyclopedia_hit_without_fetch() {
        // A Wikipedia hit carries a page summary in the snippet and is captured
        // as a self-contained source. The article URL is never fetched (the
        // full Wikipedia HTML is large and readability extraction on it can
        // fail), and the lexical relevance pre-filter is bypassed because
        // Wikipedia's search API already ranked the result by its own
        // relevance score.
        let summary = "DuckDuckGo is an internet search engine that emphasizes \
             protecting searchers' privacy and avoiding the filter bubble of \
             personalized search results.";
        let hits = vec![WebSearchHit {
            url: "https://en.wikipedia.org/wiki/DuckDuckGo".into(),
            title: "DuckDuckGo".into(),
            snippet: summary.to_string(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "wikipedia".into(),
            author: None,
        }];
        let (g, _, fetch) = gatherer_with(hits, std::collections::HashMap::new(), Vec::new());
        // Query terms that do NOT lexically overlap with "DuckDuckGo" — this
        // would be rejected by the pre-filter if the encyclopedia bypass were
        // not in place.
        let sources = g
            .gather("free keyless web search APIs no API key required", 5)
            .await
            .unwrap();
        assert_eq!(sources.len(), 1, "encyclopedia hit should be captured");
        match &sources[0] {
            Source::Web {
                url,
                title,
                relevance,
                page_type,
                search_engine,
                language,
                ..
            } => {
                assert_eq!(url, "https://en.wikipedia.org/wiki/DuckDuckGo");
                assert_eq!(title, "DuckDuckGo");
                assert_eq!(page_type.as_deref(), Some("encyclopedia"));
                assert_eq!(search_engine, "wikipedia");
                assert!(
                    relevance.contains("Encyclopedia"),
                    "encyclopedia sources use the engine-ranked relevance label"
                );
                assert_eq!(
                    language.as_deref(),
                    Some("English"),
                    "Wikipedia snippet should have its language detected"
                );
            }
            other => panic!("expected Source::Web, got {other:?}"),
        }
        let calls = fetch.calls.lock().unwrap();
        assert!(
            calls.is_empty(),
            "encyclopedia hit must not trigger a URL fetch (no network), got {calls:?}"
        );
    }

    #[tokio::test]
    async fn gather_rejects_encyclopedia_hit_with_no_summary() {
        // A Wikipedia article with no extract produces a snippet shorter than
        // `MIN_ENCYCLOPEDIA_CONTENT_CHARS`; it should be rejected rather than
        // admitted as near-empty noise.
        let hits = vec![WebSearchHit {
            url: "https://en.wikipedia.org/wiki/No_Summary".into(),
            title: "No Summary".into(),
            snippet: "[thumbnail: https://example.com/thumb.png]".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "wikipedia".into(),
            author: None,
        }];
        let (g, _, fetch) = gatherer_with(hits, std::collections::HashMap::new(), Vec::new());
        let sources = g.gather("anything at all", 5).await.unwrap();
        assert!(
            sources.is_empty(),
            "summary-less encyclopedia hit should be rejected"
        );
        let calls = fetch.calls.lock().unwrap();
        assert!(calls.is_empty(), "no fetch should be attempted");
    }

    #[tokio::test]
    async fn gather_fetches_shared_wikipedia_engine_url_instead_of_treating_as_encyclopedia() {
        // When the same URL is returned by Wikipedia *and* a general web engine,
        // the page is a fetchable HTML page. The gatherer must take the normal
        // fetch path (capturing the richer page body) rather than treating it
        // as an encyclopedia snippet-only source.
        let hits = vec![WebSearchHit {
            url: "https://en.wikipedia.org/wiki/Rust_(programming_language)".into(),
            title: "Rust (programming language)".into(),
            snippet: "Rust programming language".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "langsearch, wikipedia".into(),
            author: None,
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://en.wikipedia.org/wiki/Rust_(programming_language)".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://en.wikipedia.org/wiki/Rust_(programming_language)".into(),
                title: "Rust (programming language)".into(),
                body: body256("full page body for the shared Wikipedia URL"),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
        let (g, _, fetch) = gatherer_with(hits, pages, Vec::new());
        let sources = g.gather("Rust programming language", 5).await.unwrap();
        assert_eq!(sources.len(), 1, "shared-engine hit should be captured");
        match &sources[0] {
            Source::Web { page_type, .. } => {
                assert_ne!(
                    page_type.as_deref(),
                    Some("encyclopedia"),
                    "shared-engine URL must not be treated as encyclopedia"
                );
            }
            other => panic!("expected Source::Web, got {other:?}"),
        }
        let calls = fetch.calls.lock().unwrap();
        assert!(
            calls
                .contains(&"https://en.wikipedia.org/wiki/Rust_(programming_language)".to_string()),
            "shared-engine URL must be fetched normally, got {calls:?}"
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
            author: None,
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://bad.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://bad.example".into(),
                title: "completely unrelated page".into(),
                body: body256("body"),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
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
            author: None,
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
                author: None,
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
                    author: None,
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
            author: None,
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
                author: None,
            },
            WebSearchHit {
                url: "https://low.example".into(),
                title: "unrelated shopping".into(),
                snippet: "buy shoes".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "test".into(),
                author: None,
            },
            WebSearchHit {
                url: "https://second.example".into(),
                title: "Second Rust async page".into(),
                snippet: "Rust async runtime".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "test".into(),
                author: None,
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
                    body: body256("body"),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
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
                    body: body256("b"),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
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
                author: None,
            },
            WebSearchHit {
                url: "https://b.example".into(),
                title: "B".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
            },
            WebSearchHit {
                url: "https://c.example".into(),
                title: "C".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://a.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://a.example".into(),
                title: "A — resolved".into(),
                body: body256("body a"),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
        pages.insert(
            "https://b.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://b.example".into(),
                title: "B — resolved".into(),
                body: body256("body b"),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
        pages.insert(
            "https://c.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://c.example".into(),
                title: String::new(), // empty title should fall back to search hit title
                body: body256("body c"),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
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
                author: None,
            },
            WebSearchHit {
                url: "https://bad".into(),
                title: "Bad".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://ok".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://ok".into(),
                title: "OK".into(),
                body: body256("b"),

                content_type: None,
                page_type: None,
                language: None,
                author: None,
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
            author: None,
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
                author: None,
            },
            WebSearchHit {
                url: "https://2".into(),
                title: "2".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
            },
            WebSearchHit {
                url: "https://3".into(),
                title: "3".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
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
                    body: body256("b"),

                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
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
        let err = g.gather(" ", 5).await.unwrap_err();
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
                    body: body256("b"),

                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
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
        assert_eq!(events.len(), 3);
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
        assert_eq!(events.len(), 3);
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
                author: None,
            },
            WebSearchHit {
                url: "https://bad".into(),
                title: "Bad".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: String::new(),
                search_engine: String::new(),
                author: None,
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://ok".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://ok".into(),
                title: "OK".into(),
                body: body256("b"),

                content_type: None,
                page_type: None,
                language: None,
                author: None,
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
                    author: None,
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
                    author: None,
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
                        author: None,
                    },
                    WebSearchHit {
                        url: "https://b.example".into(),
                        title: "B".into(),
                        snippet: "topic Rust async Tokio runtime".into(),
                        matched_query: String::new(),
                        search_tool: String::new(),
                        search_engine: String::new(),
                        author: None,
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
                author: None,
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
                author: None,
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
                author: None,
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
                author: None,
            },
        );

        let g = WebGatherer::new(
            Arc::new(FakeSearch::default()),
            Arc::new(TypedFetch { pages }),
        )
        .with_allow_pdf_web_sources(true);

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
            author: None,
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://fr.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://fr.example".into(),
                title: "Article".into(),
                body: body256("corps de texte"),
                content_type: None,
                page_type: None,
                language: Some("French".into()),
                author: None,
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
                body: body256("cuerpo"),
                content_type: None,
                page_type: None,
                language: Some("Spanish".into()),
                author: None,
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
                author: None,
            },
            WebSearchHit {
                url: "https://www.youtube.com/watch?v=abc123".into(),
                title: "YouTube".into(),
                snippet: "topic Rust async Tokio runtime".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "test".into(),
                author: None,
            },
        ];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://example.com/paper.pdf".into(),
            WebFetchedPage {
                url: "https://example.com/paper.pdf".into(),
                title: "PDF".into(),
                body: body256("pdf body"),
                content_type: Some("application/pdf".into()),
                page_type: Some("pdf".into()),
                published_at: None,
                language: None,
                author: None,
            },
        );
        pages.insert(
            "https://www.youtube.com/watch?v=abc123".into(),
            WebFetchedPage {
                url: "https://www.youtube.com/watch?v=abc123".into(),
                title: "YouTube".into(),
                body: body256("youtube transcript"),
                content_type: Some("text/html".into()),
                page_type: Some("youtube".into()),
                published_at: None,
                language: None,
                author: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let result = g
            .with_allow_pdf_web_sources(true)
            .gather_with_observer("topic", 5, None)
            .await
            .unwrap();
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
            author: None,
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://retry.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://retry.example".into(),
                title: "Rust async runtime".into(),
                body: body256("body"),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
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
                    body: body256("b"),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
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
                    body: body256("b"),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
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
                    body: body256("b"),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
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
            author: None,
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
                author: None,
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
            author: None,
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
                author: None,
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
            author: None,
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://preview.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://preview.example".into(),
                title: "Preview page".into(),
                body: "Rust async runtime programming guide with Tokio tasks, futures, channels, and executors. ".repeat(20),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
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
                !body_preview.is_empty(),
                "body_preview should carry content"
            ); // When the fetcher does not report a language, the research layer
            // now runs an aggressive best-guess detector on the body. The
            // fake body is real English prose, so the detector must succeed.
            assert_ne!(
                language, "UNKNOWN",
                "language should be guessed when page.language is None"
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
            author: None,
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
                author: None,
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
                GatherEvent::SourceExcluded { url, reason }
                    if url == "https://tiny.example"
                        && reason.contains("too short")
            )),
            "expected SourceExcluded with 'too short' message, got {:?}",
            *events
        );
    }

    #[tokio::test]
    async fn gather_uses_vault_sources_when_available() {
        let tmp = tempfile::TempDir::new().unwrap();
        let vault_root = tmp.path().join("vault");
        let vault = SourceVault::open_with_root(&vault_root, "run-2026-001").unwrap();
        vault
            .store(&NewVaultSource {
                url: "https://vaulted.example/page".into(),
                title: "Vaulted Rust Async Page".into(),
                fetch_timestamp: Some(Utc::now()),
                search_tool: "mf_search".into(),
                search_engine: "langsearch".into(),
                media_type: "page".into(),
                content_type: None,
                body_text: body256("vaulted rust async runtime content"),
                summary_text: None,
            })
            .unwrap();

        // Search tool that would fail if called — proves vault short-circuits
        // the web-search phase.
        struct PanicSearch;
        #[async_trait]
        impl WebSearchTool for PanicSearch {
            async fn search(
                &self,
                _query: &str,
                _max_results: usize,
            ) -> anyhow::Result<Vec<WebSearchHit>> {
                panic!("search should not be called when vault has matches")
            }
        }
        struct PanicFetch;
        #[async_trait]
        impl WebFetchTool for PanicFetch {
            async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
                panic!("fetch should not be called when vault has matches")
            }
        }

        let g = WebGatherer::new(Arc::new(PanicSearch), Arc::new(PanicFetch))
            .with_vault(Arc::new(vault));
        let result = g
            .gather_with_observer("rust async runtime", 5, None)
            .await
            .unwrap();
        assert_eq!(
            result.sources.len(),
            1,
            "vaulted source should be returned directly"
        );
        if let Source::Web { url, body, .. } = &result.sources[0] {
            assert_eq!(url, "https://vaulted.example/page");
            assert!(
                body.contains("vaulted rust async runtime content"),
                "vault body should be present"
            );
        } else {
            panic!("expected Source::Web")
        }
    }

    #[tokio::test]
    async fn gather_falls_back_to_search_when_vault_has_no_matches() {
        let tmp = tempfile::TempDir::new().unwrap();
        let vault_root = tmp.path().join("vault");
        let vault = SourceVault::open_with_root(&vault_root, "run-2026-empty").unwrap();

        let hits = vec![WebSearchHit {
            url: "https://web.example".into(),
            title: "Web page".into(),
            snippet: "rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
            author: None,
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://web.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://web.example".into(),
                title: "Web page".into(),
                body: body256("fresh web content"),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
        let (search, fetch) = (
            Arc::new(FakeSearch {
                hits,
                calls: Mutex::new(Vec::new()),
            }),
            Arc::new(FakeFetch {
                pages,
                fail_urls: Vec::new(),
                calls: Mutex::new(Vec::new()),
            }),
        );
        let g = WebGatherer::new(search.clone(), fetch.clone()).with_vault(Arc::new(vault));
        let result = g.gather("rust async runtime", 5).await.unwrap();
        assert_eq!(
            result.len(),
            1,
            "should fall back to web search when vault has no matches"
        );
        assert_eq!(
            search.calls.lock().unwrap().len(),
            1,
            "search should be called once"
        );
        assert_eq!(
            fetch.calls.lock().unwrap().len(),
            1,
            "fetch should be called once"
        );
    }

    #[tokio::test]
    async fn gather_skips_search_when_vault_has_sufficient_sources() {
        let tmp = tempfile::TempDir::new().unwrap();
        let vault_root = tmp.path().join("vault");
        let vault = SourceVault::open_with_root(&vault_root, "run-2026-sufficient").unwrap();

        for i in 0..3 {
            vault
                .store(&NewVaultSource {
                    url: format!("https://vaulted.example/page-{i}"),
                    title: format!("Vaulted Rust Async Page {i}"),
                    fetch_timestamp: Some(Utc::now()),
                    search_tool: "mf_search".into(),
                    search_engine: "langsearch".into(),
                    media_type: "page".into(),
                    content_type: None,
                    body_text: body256(&format!("vaulted rust async runtime content {i}")),
                    summary_text: None,
                })
                .unwrap();
        }

        struct PanicSearch;
        #[async_trait]
        impl WebSearchTool for PanicSearch {
            async fn search(
                &self,
                _query: &str,
                _max_results: usize,
            ) -> anyhow::Result<Vec<WebSearchHit>> {
                panic!("search should not be called when vault has sufficient sources")
            }
        }
        struct PanicFetch;
        #[async_trait]
        impl WebFetchTool for PanicFetch {
            async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
                panic!("fetch should not be called when vault has sufficient sources")
            }
        }

        let g = WebGatherer::new(Arc::new(PanicSearch), Arc::new(PanicFetch))
            .with_vault(Arc::new(vault))
            .with_sufficient_sources(3);
        let result = g
            .gather_with_observer("rust async runtime", 5, None)
            .await
            .unwrap();
        assert_eq!(
            result.sources.len(),
            3,
            "all vaulted sources should be returned without new fetches"
        );
    }

    #[tokio::test]
    async fn gather_falls_back_to_search_when_vault_is_below_threshold() {
        let tmp = tempfile::TempDir::new().unwrap();
        let vault_root = tmp.path().join("vault");
        let vault = SourceVault::open_with_root(&vault_root, "run-2026-below").unwrap();
        vault
            .store(&NewVaultSource {
                url: "https://vaulted.example/page".into(),
                title: "Vaulted Rust Async Page".into(),
                fetch_timestamp: Some(Utc::now()),
                search_tool: "mf_search".into(),
                search_engine: "langsearch".into(),
                media_type: "page".into(),
                content_type: None,
                body_text: body256("vaulted rust async runtime content"),
                summary_text: None,
            })
            .unwrap();

        let hits = vec![WebSearchHit {
            url: "https://web.example".into(),
            title: "Web page".into(),
            snippet: "rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
            author: None,
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://web.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://web.example".into(),
                title: "Web page".into(),
                body: body256("fresh web content"),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
        let (search, fetch) = (
            Arc::new(FakeSearch {
                hits,
                calls: Mutex::new(Vec::new()),
            }),
            Arc::new(FakeFetch {
                pages,
                fail_urls: Vec::new(),
                calls: Mutex::new(Vec::new()),
            }),
        );
        let g = WebGatherer::new(search.clone(), fetch.clone())
            .with_vault(Arc::new(vault))
            .with_sufficient_sources(3);
        let result = g.gather("rust async runtime", 5).await.unwrap();
        assert_eq!(
            result.len(),
            1,
            "should fall back to web search when vault is below threshold"
        );
        assert_eq!(
            search.calls.lock().unwrap().len(),
            1,
            "search should be called once"
        );
        assert_eq!(
            fetch.calls.lock().unwrap().len(),
            1,
            "fetch should be called once"
        );
    }

    #[tokio::test]
    async fn gather_emits_vault_sufficient_event() {
        let tmp = tempfile::TempDir::new().unwrap();
        let vault_root = tmp.path().join("vault");
        let vault = SourceVault::open_with_root(&vault_root, "run-2026-event").unwrap();

        for i in 0..3 {
            vault
                .store(&NewVaultSource {
                    url: format!("https://vaulted.example/page-{i}"),
                    title: format!("Vaulted Rust Async Page {i}"),
                    fetch_timestamp: Some(Utc::now()),
                    search_tool: "mf_search".into(),
                    search_engine: "langsearch".into(),
                    media_type: "page".into(),
                    content_type: None,
                    body_text: body256(&format!("vaulted rust async runtime content {i}")),
                    summary_text: None,
                })
                .unwrap();
        }

        struct PanicSearch;
        #[async_trait]
        impl WebSearchTool for PanicSearch {
            async fn search(
                &self,
                _query: &str,
                _max_results: usize,
            ) -> anyhow::Result<Vec<WebSearchHit>> {
                panic!("search should not be called when vault is sufficient")
            }
        }
        struct PanicFetch;
        #[async_trait]
        impl WebFetchTool for PanicFetch {
            async fn fetch(&self, _url: &str) -> anyhow::Result<WebFetchedPage> {
                panic!("fetch should not be called when vault is sufficient")
            }
        }

        #[derive(Default)]
        struct CollectEvents(Mutex<Vec<GatherEvent>>);
        impl GatherObserver for CollectEvents {
            fn on_event(&self, event: GatherEvent) {
                self.0.lock().unwrap().push(event);
            }
        }
        let obs = CollectEvents::default();
        let g = WebGatherer::new(Arc::new(PanicSearch), Arc::new(PanicFetch))
            .with_vault(Arc::new(vault))
            .with_sufficient_sources(3);
        g.gather_with_observer("rust async runtime", 5, Some(&obs))
            .await
            .unwrap();

        let events = obs.0.lock().unwrap();
        let sufficient = events
            .iter()
            .find(|e| matches!(e, GatherEvent::VaultSufficient { required: 3, .. }));
        assert!(
            sufficient.is_some(),
            "expected VaultSufficient event, got {events:?}"
        );
    }

    /// A fake summarizer that returns deterministic text so we can assert the
    /// vault stores the summary alongside the original URL and timestamp (T-013).
    struct FakeSummarizer;

    #[async_trait]
    impl crate::page_summarizer::PageSummarizer for FakeSummarizer {
        async fn summarize_page(
            &self,
            url: &str,
            _body: &str,
        ) -> anyhow::Result<crate::page_summarizer::PageSummary> {
            Ok(crate::page_summarizer::PageSummary {
                url: url.to_string(),
                summary: "Summarized.".to_string(),
                summarized_at: chrono::Utc::now(),
            })
        }
    }

    #[tokio::test]
    async fn gather_stores_summarized_source_in_vault_with_url_and_timestamp() {
        let tmp = tempfile::TempDir::new().unwrap();
        let vault_root = tmp.path().join("vault");
        let vault = SourceVault::open_with_root(&vault_root, "run-2026-summary").unwrap();

        let hits = vec![WebSearchHit {
            url: "https://web.example".into(),
            title: "Web page".into(),
            snippet: "rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
            author: None,
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://web.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://web.example".into(),
                title: "Web page".into(),
                body: body256("fresh web content with many details"),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
        let (search, fetch) = (
            Arc::new(FakeSearch {
                hits,
                calls: Mutex::new(Vec::new()),
            }),
            Arc::new(FakeFetch {
                pages,
                fail_urls: Vec::new(),
                calls: Mutex::new(Vec::new()),
            }),
        );
        let summarizer: Arc<dyn crate::page_summarizer::PageSummarizer> = Arc::new(FakeSummarizer);
        let g = WebGatherer::new(search, fetch)
            .with_vault(Arc::new(vault.clone()))
            .with_summarizer(summarizer);
        let result = g.gather("rust async runtime", 5).await.unwrap();
        assert_eq!(result.len(), 1, "one source should be captured");
        assert_eq!(
            result[0].path_or_url(),
            "https://web.example",
            "source should carry the original URL"
        );

        let stored = vault.list(5).unwrap();
        assert_eq!(stored.len(), 1, "vault should contain one stored source");
        assert_eq!(stored[0].url, "https://web.example");
        assert!(
            stored[0]
                .body_text
                .contains("fresh web content with many details"),
            "vault should keep the original body text"
        );
        assert_eq!(
            stored[0].summary_text.as_deref(),
            Some("Summarized."),
            "vault should store the generated summary"
        );
        assert!(
            stored[0].fetch_timestamp > chrono::DateTime::UNIX_EPOCH,
            "vault should store a post-epoch fetch timestamp"
        );
    }

    /// Fake OA client that mimics Unpaywall/Europe PMC responses for a
    /// configurable recovered URL.
    struct FakeOaClient {
        recovered_url: String,
    }

    #[async_trait]
    impl OpenAccessClient for FakeOaClient {
        async fn fetch_text(&self, _url: &str) -> crate::open_access::Result<String> {
            Ok(String::new())
        }

        async fn fetch_json(&self, url: &str) -> crate::open_access::Result<serde_json::Value> {
            if url.contains("unpaywall.org") {
                Ok(serde_json::json!({
                    "is_oa": true,
                    "oa_status": "gold",
                    "best_oa_location": {
                        "url_for_pdf": self.recovered_url,
                        "url": self.recovered_url,
                        "license": "cc-by"
                    }
                }))
            } else {
                Ok(serde_json::json!({ "resultList": { "result": [] } }))
            }
        }
    }

    #[tokio::test]
    async fn gather_recovers_open_access_copy_for_short_scholarly_source() {
        let hits = vec![WebSearchHit {
            url: "https://doi.org/10.1234/example".into(),
            title: "Paywalled Paper".into(),
            snippet: "short abstract".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "langsearch".into(),
            author: None,
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://doi.org/10.1234/example".into(),
            WebFetchedPage {
                url: "https://doi.org/10.1234/example".into(),
                title: "Paywalled Paper".into(),
                body: "short abstract".into(),
                published_at: None,
                content_type: None,
                page_type: Some("scholarly".into()),
                language: None,
                author: None,
            },
        );
        pages.insert(
            "https://oa.example.com/full.pdf".into(),
            WebFetchedPage {
                url: "https://oa.example.com/full.pdf".into(),
                title: "Recovered Full Text".into(),
                body: body256("open access full text content here"),
                published_at: None,
                content_type: Some("application/pdf".into()),
                page_type: None,
                language: Some("English".into()),
                author: None,
            },
        );
        let fetch = Arc::new(FakeFetch {
            pages,
            fail_urls: Vec::new(),
            calls: Mutex::new(Vec::new()),
        });
        let oa_client = Arc::new(FakeOaClient {
            recovered_url: "https://oa.example.com/full.pdf".into(),
        });
        let gatherer = WebGatherer::new(
            Arc::new(FakeSearch {
                hits,
                calls: Mutex::new(Vec::new()),
            }),
            fetch,
        )
        .with_keep_low_relevance(true)
        .with_open_access_recovery(true, Some("oa@example.com".into()))
        .with_oa_min_full_text_chars(500)
        .with_oa_client(oa_client);
        let sources = gatherer.gather("some scholarly topic", 5).await.unwrap();
        assert_eq!(
            sources.len(),
            1,
            "short scholarly source should be recovered"
        );
        let src = &sources[0];
        assert_eq!(src.title(), "Recovered Full Text");
        assert_eq!(src.path_or_url(), "https://doi.org/10.1234/example");
        let recovery = src
            .oa_recovery()
            .expect("OA recovery metadata should be present");
        assert_eq!(recovery.url, "https://oa.example.com/full.pdf");
        assert_eq!(recovery.source.to_string(), "unpaywall");
    }

    #[tokio::test]
    async fn gather_does_not_recover_when_body_is_long_enough() {
        let hits = vec![WebSearchHit {
            url: "https://doi.org/10.1234/example".into(),
            title: "Open Paper".into(),
            snippet: "already full text".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "langsearch".into(),
            author: None,
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://doi.org/10.1234/example".into(),
            WebFetchedPage {
                url: "https://doi.org/10.1234/example".into(),
                title: "Open Paper".into(),
                body: body256("already long full text"),
                published_at: None,
                content_type: None,
                page_type: Some("scholarly".into()),
                language: None,
                author: None,
            },
        );
        let fetch = Arc::new(FakeFetch {
            pages,
            fail_urls: Vec::new(),
            calls: Mutex::new(Vec::new()),
        });
        let oa_client = Arc::new(FakeOaClient {
            recovered_url: "https://oa.example.com/full.pdf".into(),
        });
        let gatherer = WebGatherer::new(
            Arc::new(FakeSearch {
                hits,
                calls: Mutex::new(Vec::new()),
            }),
            fetch,
        )
        .with_keep_low_relevance(true)
        .with_open_access_recovery(true, Some("oa@example.com".into()))
        .with_oa_min_full_text_chars(200)
        .with_oa_client(oa_client);
        let sources = gatherer.gather("some scholarly topic", 5).await.unwrap();
        assert_eq!(sources.len(), 1);
        assert!(
            sources[0].oa_recovery().is_none(),
            "long source should not be recovered"
        );
    }

    #[tokio::test]
    async fn gather_does_not_recover_when_disabled() {
        let hits = vec![WebSearchHit {
            url: "https://doi.org/10.1234/example".into(),
            title: "Paywalled Paper".into(),
            snippet: "short".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "openalex".into(),
            author: None,
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://doi.org/10.1234/example".into(),
            WebFetchedPage {
                url: "https://doi.org/10.1234/example".into(),
                title: "Paywalled Paper".into(),
                body: "short".into(),
                published_at: None,
                content_type: None,
                page_type: Some("scholarly".into()),
                language: None,
                author: None,
            },
        );
        let fetch = Arc::new(FakeFetch {
            pages,
            fail_urls: Vec::new(),
            calls: Mutex::new(Vec::new()),
        });
        let oa_client = Arc::new(FakeOaClient {
            recovered_url: "https://oa.example.com/full.pdf".into(),
        });
        let gatherer = WebGatherer::new(
            Arc::new(FakeSearch {
                hits,
                calls: Mutex::new(Vec::new()),
            }),
            fetch,
        )
        .with_open_access_recovery(false, None)
        .with_oa_client(oa_client);
        let sources = gatherer.gather("some scholarly topic", 5).await.unwrap();
        assert!(
            sources.is_empty() || sources[0].oa_recovery().is_none(),
            "recovery disabled"
        );
    }

    /// T-006: the width sweep records which mf_search parallel backends
    /// contributed hits, and the aggregate `GatherResult` carries the
    /// deduplicated engine list plus considered/captured/excluded counts.
    #[tokio::test]
    async fn width_sweep_tracks_parallel_engines_and_considered_count() {
        let hits = vec![
            WebSearchHit {
                url: "https://langsearch.example".into(),
                title: "LangSearch result".into(),
                snippet: "topic Rust async runtime".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "langsearch".into(),
                author: None,
            },
            WebSearchHit {
                url: "https://tavily.example".into(),
                title: "Tavily result".into(),
                snippet: "topic Rust async runtime".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "tavily".into(),
                author: None,
            },
            WebSearchHit {
                url: "https://wikipedia.example".into(),
                title: "Wikipedia summary".into(),
                snippet: "Rust is a multi-paradigm, general-purpose programming language emphasizing performance and safety, especially safe concurrency.".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "wikipedia".into(),
                author: None,
            },
            WebSearchHit {
                url: "https://openalex.example/paper".into(),
                title: "OpenAlex paper".into(),
                snippet: "We evaluate asynchronous runtimes in Rust across a range of benchmark workloads and report detailed performance comparisons. (Year: 2024 | Cited: 5 | OA: yes)".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "openalex".into(),
                author: None,
            },
        ];
        let mut pages = std::collections::HashMap::new();
        for url in ["https://langsearch.example", "https://tavily.example"] {
            pages.insert(
                url.into(),
                WebFetchedPage {
                    published_at: None,
                    url: url.into(),
                    title: format!("title {url}"),
                    body: body256("body"),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
                },
            );
        }
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let result = g
            .gather_with_observer("Rust async runtime", 10, None)
            .await
            .unwrap();
        // LangSearch and Tavily pages are fetched; Wikipedia and OpenAlex are
        // captured as snippet-only sources.
        assert_eq!(
            result.sources.len(),
            4,
            "all four backend hits should be captured"
        );
        assert_eq!(
            result.engines,
            vec!["langsearch", "openalex", "tavily", "wikipedia"],
            "engines should be sorted and deduplicated"
        );
        assert_eq!(
            result.considered_count, 4,
            "all four unique URLs were considered"
        );
        assert_eq!(result.excluded_count, 0, "no sources were excluded");
    }

    /// T-006: when the same URL is returned by multiple parallel backends,
    /// the engine list on the source should reflect every contributing
    /// engine and the result engines should still be deduplicated.
    #[tokio::test]
    async fn width_sweep_merges_engines_for_shared_url() {
        let hits = vec![WebSearchHit {
            url: "https://shared.example".into(),
            title: "Shared result".into(),
            snippet: "topic Rust async runtime".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "langsearch, tavily".into(),
            author: None,
        }];
        let mut pages = std::collections::HashMap::new();
        pages.insert(
            "https://shared.example".into(),
            WebFetchedPage {
                published_at: None,
                url: "https://shared.example".into(),
                title: "Shared result".into(),
                body: body256("shared body"),
                content_type: None,
                page_type: None,
                language: None,
                author: None,
            },
        );
        let (g, _, _) = gatherer_with(hits, pages, Vec::new());
        let result = g
            .gather_with_observer("Rust async runtime", 5, None)
            .await
            .unwrap();
        assert_eq!(result.sources.len(), 1);
        assert_eq!(result.engines, vec!["langsearch", "tavily"]);
    }

    /// T-006: the width-sweep summary event is emitted after parallel
    /// searches resolve, carrying the contributing engine list and counts.
    #[tokio::test]
    async fn width_sweep_emits_summary_event() {
        let hits = vec![
            WebSearchHit {
                url: "https://langsearch.example".into(),
                title: "LangSearch result".into(),
                snippet: "topic Rust async runtime".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "langsearch".into(),
                author: None,
            },
            WebSearchHit {
                url: "https://tavily.example".into(),
                title: "Tavily result".into(),
                snippet: "topic Rust async runtime".into(),
                matched_query: String::new(),
                search_tool: "mf_search".into(),
                search_engine: "tavily".into(),
                author: None,
            },
        ];
        let mut pages = std::collections::HashMap::new();
        for url in ["https://langsearch.example", "https://tavily.example"] {
            pages.insert(
                url.into(),
                WebFetchedPage {
                    published_at: None,
                    url: url.into(),
                    title: format!("title {url}"),
                    body: body256("body"),
                    content_type: None,
                    page_type: None,
                    language: None,
                    author: None,
                },
            );
        }
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

        let events = obs.0.lock().unwrap();
        let summary = events.iter().find(|e| {
            matches!(
                e, GatherEvent::WidthSweepSummary { engines, .. } if engines.contains(&"langsearch".to_string())
            )
        });
        assert!(
            summary.is_some(),
            "expected WidthSweepSummary event, got {events:?}"
        );
        if let Some(GatherEvent::WidthSweepSummary {
            queries,
            engines,
            considered,
            captured,
            excluded,
        }) = summary
        {
            assert_eq!(queries, &["Rust async runtime".to_string()]);
            assert_eq!(engines, &["langsearch", "tavily"]);
            assert_eq!(*considered, 2);
            assert_eq!(*captured, result.sources.len());
            assert_eq!(*excluded, 0);
        }
    }

    /// PDF search hits are skipped by default, so they do not consume the
    /// fetch budget or trigger expensive PDF extraction.
    #[tokio::test]
    async fn gather_skips_pdf_web_sources_by_default() {
        let hits = vec![WebSearchHit {
            url: "https://example.com/paper.pdf".into(),
            title: "PDF".into(),
            snippet: "topic Rust async".into(),
            matched_query: String::new(),
            search_tool: "mf_search".into(),
            search_engine: "test".into(),
            author: None,
        }];
        let (g, _, _) = gatherer_with(hits, std::collections::HashMap::new(), Vec::new());
        let result = g.gather_with_observer("topic", 5, None).await.unwrap();
        assert_eq!(result.pdf_count, 0);
        assert!(result.sources.is_empty());
    }
}
