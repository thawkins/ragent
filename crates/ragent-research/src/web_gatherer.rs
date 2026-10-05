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
//!    starting at 01) - the actual supporting-file write is done by the IO
//!    layer (T-015) once we have an item directory on disk; this module
//!    only returns the captured metadata.
//! 4. If the search or fetch tools return zero results the gatherer
//!    returns an empty `Vec` (FR-006: graceful degradation).
//!
//! ## Reuse, not reimplementation
//!
//! Per the spec constraints, the gatherer does **not** reimplement search
//! or fetch - it delegates entirely to the provided `WebSearchTool` /
//! `WebFetchTool` implementations. In production these wrap the existing
//! `websearch` and `webfetch` tools in `crates/ragent-tools-extended`.

use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use futures::StreamExt;

use ragent_tools_extended::masterfetch::language::detect_language_best_effort;
use ragent_tools_extended::masterfetch::search::{
    ACADEMIC_ENGINES, ENGINE_WIKIPEDIA, is_academic_engine,
};

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
pub use relevance::PreparedQuery;
use title::clean_web_source_title;

/// Maximum number of focused sub-queries the research decomposer will
/// produce for a single topic. Increasing this raises the web-search
/// parallelism and usually increases the number of distinct sources found.
/// 20 stays within what current models reliably emit as a JSON array of
/// sub-queries while roughly doubling the achievable source fan-out over
/// the earlier 10-sub-query cap on broad topics.
pub(crate) const MAX_DECOMPOSED_QUERIES: usize = 20;

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

/// Upper bound on the number of concurrent sub-query searches issued during the
/// search fan-out phase of [`WebGatherer::gather_with_observer`].
///
/// Sub-queries are independent, so a handful can run in parallel to overlap
/// provider latency; 4 keeps the fan-out well clear of search-provider rate
/// ceilings and keyless-backend burst penalties without serialising the sweep.
/// Named here rather than inline so the magic number is visible alongside
/// [`DEFAULT_FETCH_CONCURRENCY`] (ANTIPAT F-22).
const SEARCH_FANOUT_CONCURRENCY: usize = 4;

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
/// (Milestone H-002). Subsequent retries double this value (200 ms, 400 ms, ...).
pub const DEFAULT_SEARCH_RETRY_BASE_DELAY_MS: u64 = 200;

/// Hard ceiling on the number of retry attempts accepted by
/// [`WebGatherer::with_search_max_retries`].
///
/// ANTIPAT F-02: `--search-max-retries` is a CLI value forwarded straight into
/// the setter. The backoff is `base * 2^(attempt - 1)`, so an unclamped value of
/// 64 or more panicked on `1u64 << 64`, and values in the 40-63 range slept for
/// days. 10 retries (a 200 ms .. 102 s schedule) is far beyond any useful
/// transient-failure recovery and keeps the schedule bounded.
pub const MAX_SEARCH_RETRIES: u32 = 10;

/// Hard ceiling on a single retry backoff delay.
///
/// The exponential schedule is additionally capped here so a large (but
/// accepted) retry count cannot stall a gather pass indefinitely.
pub const MAX_SEARCH_RETRY_DELAY_MS: u64 = 60_000;

/// Per-query search allowance used when the gatherer runs uncapped
/// (`volume_cap == None`). Large enough to accept engine-max result pages
/// (OpenAlex returns up to 75 per query) without truncating anything; the
/// per-fetch timeout and fetch concurrency remain the real volume bounds.
const UNCAPTED_MAX_RESULTS: usize = 500;

/// Fetch-budget allowance used when the gatherer runs uncapped: effectively
/// unbounded, so every retained candidate is fetched.
const UNCAPTED_FETCH_BUDGET: usize = usize::MAX;

/// Resolve the volume policy for a sweep into
/// `(per_query_search_allowance, fetch_budget)`.
///
/// Uncapped runs (the default - tiered/supervisor) ask each search for
/// engine-max results and fetch every retained candidate. Capped (competitive)
/// runs restrict each sub-query to `cap` hits and scale the fetch budget by the
/// sub-query count, so the cap bounds per-query volume rather than the whole
/// sweep. Keeping both allowances in one place stops them from drifting apart.
fn volume_policy(volume_cap: Option<usize>, sub_query_count: usize) -> (usize, usize) {
    match volume_cap {
        Some(cap) => (
            cap.max(1),
            cap.saturating_mul(sub_query_count.max(1)).max(cap),
        ),
        None => (UNCAPTED_MAX_RESULTS, UNCAPTED_FETCH_BUDGET),
    }
}

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

/// The set of encyclopedia-classified search engine names.
///
/// Today this is just Wikipedia. It is kept next to the scholarly
/// classification (which reads the shared [`ACADEMIC_ENGINES`] vocabulary from
/// `masterfetch::search`) so both special-cased capture paths derive their
/// engine membership from one table and cannot drift (ANTIPAT F-14).
const ENCYCLOPEDIA_ENGINES: &[&str] = &[ENGINE_WIKIPEDIA];

/// Classification of a contributing search engine for the special-cased
/// capture paths (ANTIPAT F-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum EngineClass {
    /// Scholarly catalog (e.g. OpenAlex): captured from a reconstructed
    /// abstract in the search snippet, without a URL fetch.
    Academic,
    /// Encyclopedia (e.g. Wikipedia): captured from a REST page summary in the
    /// search snippet, without a URL fetch.
    Encyclopedia,
    /// General web engine: fetched as HTML via the normal fetch path.
    General,
}

/// Classify `engine` from the single engine-classification table.
///
/// Scholarly membership delegates to the shared
/// [`is_academic_engine`] / [`ACADEMIC_ENGINES`] vocabulary in
/// `masterfetch::search`; encyclopedia membership reads [`ENCYCLOPEDIA_ENGINES`].
/// Both the default scholarly-exclusion set (the academic branch above) and the
/// hit-level [`is_scholarly_hit`] / [`is_encyclopedia_hit`] predicates derive
/// from this one table, so they cannot drift apart (ANTIPAT F-14).
fn classify_engine(engine: &str) -> EngineClass {
    if is_academic_engine(engine) {
        EngineClass::Academic
    } else if ENCYCLOPEDIA_ENGINES.contains(&engine) {
        EngineClass::Encyclopedia
    } else {
        EngineClass::General
    }
}

/// Returns `true` when *every* contributing search engine for the hit classifies
/// as `class`.
///
/// Both special-cased capture paths (scholarly abstracts, encyclopedia
/// summaries) require the engine to be the only contributor: when a general web
/// engine (LangSearch, Tavily) also returned the URL, a fetchable HTML page
/// exists and the normal fetch path is preferred so the richer page body is
/// captured instead of the concise engine-provided snippet.
fn is_sole_engine_class(hit: &WebSearchHit, class: EngineClass) -> bool {
    hit.search_engine
        .split(',')
        .map(str::trim)
        .all(|e| !e.is_empty() && classify_engine(e) == class)
}

/// Returns `true` for scholarly search-engine hits that carry a reconstructed
/// abstract in their snippet and should be captured as self-contained sources
/// without a URL fetch.
///
/// Scholarly backends (currently OpenAlex) rank results by their own
/// `relevance_score`; the lexical title/snippet pre-filter is a heuristic for
/// unranked HTML scrapers (LangSearch, Tavily) and would reject most scholarly
/// titles because they do not lexically overlap with the (often rephrased)
/// research sub-query. Scholarly hits are therefore exempt from the lexical
/// filter and from the URL fetch - their snippet is the evidence.
fn is_scholarly_hit(hit: &WebSearchHit) -> bool {
    is_sole_engine_class(hit, EngineClass::Academic)
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
/// and from the URL fetch - their snippet (the REST API page summary) is the
/// evidence.
fn is_encyclopedia_hit(hit: &WebSearchHit) -> bool {
    is_sole_engine_class(hit, EngineClass::Encyclopedia)
}

/// Collect the sorted, de-duplicated set of contributing search engines from
/// the comma-separated `search_engine` fields of the given strings.
///
/// Used by both gather paths (`gather_from_vault` and `gather_with_observer`)
/// so `GatherResult.engines` has one deterministic ordering regardless of the
/// origin path (previously the vault path left HashSet iteration order while
/// the observer path sorted).
fn collect_engines<'a, I>(fields: I) -> Vec<String>
where
    I: IntoIterator<Item = &'a str>,
{
    let mut engines: Vec<String> = fields
        .into_iter()
        .flat_map(|f| f.split(','))
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    engines.sort();
    engines
}

/// Build a self-contained [`WebFetchedPage`] for a search hit without fetching
/// its URL (used for scholarly abstracts and encyclopedia summaries).
fn synthesize_hit_page(hit: &WebSearchHit, page_type: &str) -> WebFetchedPage {
    WebFetchedPage {
        url: hit.url.clone(),
        title: hit.title.clone(),
        body: Arc::from(hit.snippet.as_str()),
        published_at: None,
        content_type: None,
        page_type: Some(page_type.to_string()),
        language: detect_language_best_effort(&hit.snippet),
        // Snippets carry an engine-provided author list (e.g. OpenAlex
        // `authorships`) - the page is never fetched, so the hit's author is
        // the only source.
        author: hit.author.clone(),
    }
}

/// Return `Some(body)` for language detection, or `None` when the body is too
/// large and too uniform for language detection to be meaningful. A multi-MB
/// body made of a single repeated byte (benchmarks, cap tests) sends lingua
/// down a pathological path that can take minutes - such bodies carry no
/// linguistic signal anyway, so detection is skipped.
fn detectable_body(body: &str) -> Option<&str> {
    // lingua's n-gram builder walks the input char-by-char and takes minutes
    // on multi-kilobyte strings made of a single repeated symbol (benchmark /
    // cap-test filler bodies), so skip detection when the first 4096
    // characters are all the same character - such bodies carry no linguistic
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

/// Per-engine width-sweep accounting row (T-006). One row per backend search
/// engine, so the progress table can show how each engine's hits fared
/// through deduplication, pre-filtering, fetching, and capture.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct EngineSweepStat {
    /// Engine name (e.g. `langsearch`, `openalex`).
    pub engine: String,
    /// Unique URLs credited to this engine that survived deduplication.
    pub considered: usize,
    /// URLs credited to this engine that were captured as sources.
    pub captured: usize,
    /// URLs credited to this engine that were rejected by a gather filter or
    /// a fetch failure/timeout.
    pub excluded: usize,
    /// Breakdown of `excluded` by reason; the counts sum to `excluded` for
    /// this engine.
    pub excluded_by_reason: std::collections::BTreeMap<ExclusionReason, usize>,
    /// Breakdown of the `ExclusionReason::FetchFailed` bucket by fine-grained
    /// fetch-failure cause; the counts sum to this engine's `fetch` reason
    /// count.
    pub failed_by_kind: std::collections::BTreeMap<FetchFailureKind, usize>,
}

/// Why a considered web candidate was excluded (T-006 progress table).
///
/// The variants group the individual gather-filter messages into the
/// categories shown as columns of the per-engine summary table, so the
/// progress detail reports *why* candidates were dropped rather than only how
/// many.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExclusionReason {
    /// Scholarly engine (e.g. OpenAlex) filtered out when papers are disabled.
    ScholarlyEngine,
    /// PDF web source skipped because `--use-pdf` is off.
    PdfDisabled,
    /// Pre-fetch title/snippet or post-fetch relevance below the threshold.
    LowRelevance,
    /// Extracted content shorter than the minimum usable length.
    ContentTooShort,
    /// Page fetch failed or timed out.
    FetchFailed,
}

impl ExclusionReason {
    /// Every reason, in the column order used by the progress table.
    pub const ALL: [Self; 5] = [
        Self::ScholarlyEngine,
        Self::PdfDisabled,
        Self::LowRelevance,
        Self::ContentTooShort,
        Self::FetchFailed,
    ];

    /// Short fixed-width column label for the progress table header.
    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::ScholarlyEngine => "papers",
            Self::PdfDisabled => "pdf",
            Self::LowRelevance => "relev",
            Self::ContentTooShort => "short",
            Self::FetchFailed => "fetch",
        }
    }
}

/// Fine-grained cause of a page fetch failure (T-006 progress table).
///
/// Splits the single [`ExclusionReason::FetchFailed`] bucket into the distinct
/// ways a fetch can fail, so the per-engine summary reports *why* the network
/// stage dropped a candidate rather than only that it did.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FetchFailureKind {
    /// The per-fetch wall-clock timeout elapsed.
    Timeout,
    /// Transport/HTTP-layer error with no more specific classification.
    Network,
    /// The URL was rejected before any request (SSRF guard or robots.txt).
    SecurityBlocked,
    /// The server answered with an error status.
    HttpStatus,
    /// The page required authentication or was paywalled.
    AuthWall,
    /// The page was a JavaScript shell with no usable static content.
    JsShell,
    /// Content was retrieved but extraction failed (readability, empty PDF,
    /// missing transcript, or a body below the minimum length).
    Extraction,
}

impl FetchFailureKind {
    /// Every kind, in the column order used by the progress table.
    pub const ALL: [Self; 7] = [
        Self::Timeout,
        Self::Network,
        Self::SecurityBlocked,
        Self::HttpStatus,
        Self::AuthWall,
        Self::JsShell,
        Self::Extraction,
    ];

    /// Short fixed-width column label for the progress table header.
    #[must_use]
    pub const fn short_label(self) -> &'static str {
        match self {
            Self::Timeout => "t/o",
            Self::Network => "net",
            Self::SecurityBlocked => "blk",
            Self::HttpStatus => "http",
            Self::AuthWall => "wall",
            Self::JsShell => "js",
            Self::Extraction => "extr",
        }
    }
}

/// Typed fetch-failure cause attached to a fetch error so the gatherer can
/// classify it without parsing the message.
///
/// Fetch adapters return this (boxed through `anyhow`) in place of a bare
/// string; the gatherer downcasts to it and falls back to
/// [`FetchFailureKind::Network`] for untyped errors, so in-memory test doubles
/// that return plain `anyhow` errors keep working unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FetchFailure {
    /// Fine-grained cause of the failure.
    pub kind: FetchFailureKind,
    /// Human-readable detail (surfaced in events and the gather log).
    pub detail: String,
}

impl FetchFailure {
    /// Build a failure with `kind` and a human-readable `detail`.
    #[must_use]
    pub fn new(kind: FetchFailureKind, detail: impl Into<String>) -> Self {
        Self {
            kind,
            detail: detail.into(),
        }
    }
}

impl std::fmt::Display for FetchFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.detail)
    }
}

impl std::error::Error for FetchFailure {}

/// Credit one URL outcome to every engine listed in a comma-separated engine
/// CSV (consensus hits credit each contributing engine, so per-engine sums
/// may exceed the global unique-URL counts).
fn bump_engine_stats(
    stats: &mut std::collections::BTreeMap<String, EngineSweepStat>,
    engines_csv: &str,
    update: impl Fn(&mut EngineSweepStat),
) {
    for engine in engines_csv
        .split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty())
    {
        let entry = stats
            .entry(engine.to_string())
            .or_insert_with(|| EngineSweepStat {
                engine: engine.to_string(),
                considered: 0,
                captured: 0,
                excluded: 0,
                excluded_by_reason: std::collections::BTreeMap::new(),
                failed_by_kind: std::collections::BTreeMap::new(),
            });
        update(entry);
    }
}

/// Credit one excluded URL to every engine listed in a comma-separated engine
/// CSV, incrementing each engine's `excluded` total, its per-reason bucket, and
/// the global per-reason tally. Keeps every count in lockstep at each
/// exclusion site so the progress table always balances.
fn bump_engine_exclusions(
    stats: &mut std::collections::BTreeMap<String, EngineSweepStat>,
    global_by_reason: &mut std::collections::BTreeMap<ExclusionReason, usize>,
    engines_csv: &str,
    reason: ExclusionReason,
) {
    bump_engine_stats(stats, engines_csv, |s| {
        s.excluded += 1;
        *s.excluded_by_reason.entry(reason).or_insert(0) += 1;
    });
    *global_by_reason.entry(reason).or_insert(0) += 1;
}

/// Credit one fetch failure to every engine listed in a comma-separated engine
/// CSV, alongside the [`ExclusionReason::FetchFailed`] exclusion itself.
///
/// `global_by_kind` tallies the cause once per failed candidate URL (one URL has
/// one cause), while each engine's `failed_by_kind` repeats the cause for every
/// engine credited with the hit, matching the consensus-crediting convention of
/// [`bump_engine_exclusions`].
fn bump_engine_fetch_failure(
    stats: &mut std::collections::BTreeMap<String, EngineSweepStat>,
    global_by_reason: &mut std::collections::BTreeMap<ExclusionReason, usize>,
    global_by_kind: &mut std::collections::BTreeMap<FetchFailureKind, usize>,
    engines_csv: &str,
    kind: FetchFailureKind,
) {
    // One pass over the engine CSV bumps the exclusion, its reason bucket, and
    // the fine-grained failure bucket together.
    bump_engine_stats(stats, engines_csv, |s| {
        s.excluded += 1;
        *s.excluded_by_reason
            .entry(ExclusionReason::FetchFailed)
            .or_insert(0) += 1;
        *s.failed_by_kind.entry(kind).or_insert(0) += 1;
    });
    *global_by_reason
        .entry(ExclusionReason::FetchFailed)
        .or_insert(0) += 1;
    *global_by_kind.entry(kind).or_insert(0) += 1;
}

/// Extract the fine-grained failure cause from a fetch error.
///
/// Returns the [`FetchFailure::kind`] when the adapter attached a typed
/// [`FetchFailure`] (downcast through the `anyhow` chain), otherwise
/// classifies the message text so the common HTTP-status and
/// walled-page cases still land in the right column. Falls back to
/// [`FetchFailureKind::Network`] for unclassified errors.
fn classify_fetch_failure(err: &anyhow::Error) -> FetchFailureKind {
    if let Some(failure) = err.downcast_ref::<FetchFailure>() {
        return failure.kind;
    }
    classify_fetch_message(&err.to_string())
}

/// Heuristic classification of a fetch-error message string.
///
/// Used when the fetch adapter did not return a typed [`FetchFailure`] (e.g. a
/// test double or the legacy `webfetch` path). Order matters: the walled-page
/// and HTTP-status signals are checked before the generic network fallback.
fn classify_fetch_message(message: &str) -> FetchFailureKind {
    let m = message.to_ascii_lowercase();
    if m.contains("timed out") || m.contains("timeout") || m.contains("elapsed") {
        return FetchFailureKind::Timeout;
    }
    if m.contains("paywall") {
        return FetchFailureKind::AuthWall;
    }
    if m.contains("auth_wall") || m.contains("requires login") || m.contains("authentication") {
        return FetchFailureKind::AuthWall;
    }
    if m.contains("js_shell") || m.contains("javascript-rendered") {
        return FetchFailureKind::JsShell;
    }
    if m.contains("ssrf") || m.contains("robots.txt") {
        return FetchFailureKind::SecurityBlocked;
    }
    if m.contains("readability") || m.contains("content_ok = false") {
        return FetchFailureKind::Extraction;
    }
    if let Some(status) = extract_http_status(&m)
        && status >= 400
    {
        return FetchFailureKind::HttpStatus;
    }
    FetchFailureKind::Network
}

/// Find an HTTP status code in a lowercased error message.
///
/// Only tokens that directly follow an explicit `status` / `http` context are
/// considered, so a bare 3-digit number elsewhere in the message (a byte count,
/// a duration, a URL path segment) is not mistaken for a status code.
fn extract_http_status(message: &str) -> Option<u16> {
    for marker in ["status", "http"] {
        let mut search = message;
        while let Some(pos) = search.find(marker) {
            let after = search[pos + marker.len()..].trim_start();
            // Allow an optional "code" word between the marker and the number.
            let after = after.strip_prefix("code").map_or(after, str::trim_start);
            // Parse exactly three digits without allocating: only a 3-digit
            // ASCII run (a fourth digit or a non-digit beyond it both exclude
            // the run) counts as a status code.
            let digit_bytes: usize = after.bytes().take_while(u8::is_ascii_digit).take(4).count();
            if digit_bytes == 3 {
                let code = u16::from(after.as_bytes()[0] - b'0') * 100
                    + u16::from(after.as_bytes()[1] - b'0') * 10
                    + u16::from(after.as_bytes()[2] - b'0');
                if (100..=599).contains(&code) {
                    return Some(code);
                }
            }
            search = &search[pos + marker.len()..];
        }
    }
    None
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
    /// Page URL - must match the URL passed in.
    pub url: String,
    /// Resolved page title (may be empty if the page lacked a title).
    pub title: String,
    /// Rendered text body of the page, in UTF-8. HTML tags should already
    /// have been stripped by the implementation.
    ///
    /// PERF-077: stored behind an `Arc<str>` so the CPU-bound language-detection
    /// `spawn_blocking` closure can take a refcount clone instead of copying the
    /// whole page body onto a worker thread.
    pub body: Arc<str>,
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
    ///
    /// This is the base search without engine steering. Callers that need to
    /// skip specific engines should use
    /// [`WebSearchTool::search_with_exclusions`] instead.
    async fn search(&self, query: &str, max_results: usize) -> anyhow::Result<Vec<WebSearchHit>>;

    /// Run a web search for `query` while excluding the named engines.
    ///
    /// `exclude_engines` carries engine names drawn from the `mf_search`
    /// engine vocabulary (e.g. `"openalex"`, `"wikipedia"`) that must **not**
    /// be queried. The default implementation ignores the exclusions and
    /// delegates to [`WebSearchTool::search`], so implementations that cannot
    /// steer engine selection (and in-memory test doubles) keep working
    /// unchanged.
    ///
    /// Production implementations translate the names into `mf_search`'s
    /// `exclude_engines` parameter, so an excluded engine is never queried and
    /// consumes no search budget (FR-004, FR-006). Unknown names are ignored.
    async fn search_with_exclusions(
        &self,
        query: &str,
        max_results: usize,
        exclude_engines: &[&str],
    ) -> anyhow::Result<Vec<WebSearchHit>> {
        if !exclude_engines.is_empty() {
            tracing::debug!(
                "search tool does not support engine exclusions; falling back to plain search"
            );
        }
        self.search(query, max_results).await
    }
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
            page.body = Arc::from(truncate_body_to_bytes(&page.body, max_bytes));
        }
        Ok(page)
    }
}

/// Errors emitted by [`WebGatherer`].
#[derive(Debug, thiserror::Error)]
pub enum WebGatherError {
    /// The configured search limit was zero - there is nothing to gather.
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
        /// failure, 2 = second retry, ...).
        attempt: u32,
        /// Error from the previous failed attempt.
        error: String,
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
        /// Number of unique candidate URLs considered after deduplication.
        considered: usize,
        /// Number of sources ultimately captured.
        captured: usize,
        /// Number of candidates excluded by relevance or content-length
        /// filters.
        excluded: usize,
        /// Per-engine breakdown of the sweep (T-006). Sorted by engine name.
        /// Consensus hits credit every engine in the source CSV, so
        /// per-engine sums may exceed the global unique-URL counts.
        per_engine: Vec<EngineSweepStat>,
        /// Global exclusion tally by reason, summing to `excluded` (one entry
        /// per excluded candidate URL, independent of consensus crediting).
        /// Drives the reason columns on the summary totals row.
        excluded_by_reason: std::collections::BTreeMap<ExclusionReason, usize>,
        /// Global fetch-failure tally by fine-grained cause, summing to the
        /// global `ExclusionReason::FetchFailed` count (one entry per failed
        /// candidate URL). Drives the fetch-failure columns on the totals row.
        failed_by_kind: std::collections::BTreeMap<FetchFailureKind, usize>,
        /// Unique URLs that passed pre-filtering but were dropped because the
        /// fetch budget (competitive runs only: the volume cap scaled by the
        /// sub-query count) was already full. Always 0 on uncapped runs.
        /// These count as considered but are neither captured nor excluded,
        /// so `considered == captured + excluded + capped + cancelled` holds.
        capped: usize,
        /// Fetches cancelled when the phase deadline truncated the pass.
        /// Structurally 0 - the fetch stage is not deadline-bounded and
        /// in-flight fetches are never cancelled; the field is retained so
        /// the balance invariant stays checkable if truncation behavior
        /// changes. Also neither captured nor excluded.
        cancelled: usize,
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
        /// Configured budget limit. FUNC-082: `None` means an unlimited budget
        /// (this variant is not normally emitted then); it is never reported as
        /// a misleading `0`.
        limit: Option<usize>,
    },
    /// End-of-pass summary of the search-provider request counts recorded by
    /// the attached [`ProviderCallStats`] counter. Emitted once per gather
    /// pass, only when a counter is attached.
    ProviderCallsSummary {
        /// `(search tool, call count)` pairs, sorted by tool name.
        tool_calls: Vec<(String, usize)>,
    },
    /// One or more sub-query searches failed while others succeeded, so the
    /// gather pass completed with only partial coverage. Without this signal a
    /// partially-failed sweep is indistinguishable from a complete one
    /// (FUNC-030).
    SearchPartiallyFailed {
        /// Number of sub-queries that failed after retries.
        failed: usize,
        /// Total sub-queries issued.
        total: usize,
        /// The last failure, for diagnosis.
        error: String,
    },
    /// A captured source could not be written to the vault (FR-016). The source
    /// is still counted as captured, but the persistence failure is surfaced so
    /// the vault divergence is visible (FUNC-030).
    VaultStoreFailed {
        /// URL that failed to persist.
        url: String,
        /// Error from the vault store.
        error: String,
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
/// and CLI can hold one instance and call `gather` many times.
#[derive(Clone)]
pub struct WebGatherer {
    search: Arc<dyn WebSearchTool>,
    fetch: Arc<dyn WebFetchTool>,
    decomposer: Option<Arc<dyn QueryDecomposer>>,
    /// Upper bound on the number of concurrent page fetches issued during the
    /// capture phase of `gather_with_observer`. Defaults to
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
    /// `Self::search_retry_base_delay_ms`. Defaults to
    /// [`DEFAULT_SEARCH_MAX_RETRIES`] (2). `0` disables retries.
    search_max_retries: u32,
    /// Base delay in milliseconds for the first retry backoff
    /// (Milestone H-002). Subsequent retries double the delay. Defaults to
    /// [`DEFAULT_SEARCH_RETRY_BASE_DELAY_MS`] (200 ms).
    search_retry_base_delay_ms: u64,
    /// JSONL URL log (`log/research/research-<name>-<ts>-<rand>-web.jsonl`) recording
    /// every search hit as `considered`/`captured`/`rejected` with a reason.
    /// `None` disables logging. Set via [`with_gather_log`].
    gather_log: Option<Arc<Mutex<GatherLog>>>,
    /// Optional persistent source vault (Milestone T-004). When configured,
    /// `gather_with_observer` searches the vault before issuing any web search
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
    /// Optional wall-clock deadline for the *search stage* of the gather pass.
    /// When set, [`WebGatherer::gather_with_observer`] stops issuing searches
    /// once the deadline passes and proceeds to the fetch stage with whatever
    /// was captured. The fetch stage is never deadline-bounded: fetches are
    /// started for every candidate and in-flight fetches are never cancelled
    /// on deadline. `None` (the default) means no deadline.
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
    /// Optional volume cap for competitive (comparison) runs. `Some(n)` makes
    /// `n` the per-query search allowance and `n * sub-queries` the fetch
    /// budget (the original T-006 semantics). `None` (the default) runs
    /// uncapped: every sub-query search asks for engine-max results and every
    /// retained candidate is fetched - the per-fetch timeout and fetch
    /// concurrency remain the real volume bounds.
    volume_cap: Option<usize>,
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
            volume_cap: None,
        }
    }

    /// Set an optional wall-clock deadline for the *search stage* of the
    /// gather pass.
    ///
    /// When the deadline passes, no new search work is started (FR-008): the
    /// decomposer call and the wait for each sub-query search result are
    /// bounded by the remaining budget, and truncation breaks the search
    /// loop before any further search is polled. Everything captured up to
    /// that point is returned so the caller can proceed to analysis/synthesis
    /// with whatever was gathered.
    ///
    /// The fetch stage is NOT deadline-bounded: every candidate produced by
    /// the search stage is fetched to completion, in-flight fetches are
    /// never cancelled on deadline, and each fetch is individually capped by
    /// `fetch_timeout`, so the stage remains bounded without one.
    #[must_use]
    pub fn with_phase_deadline(mut self, deadline: Option<Instant>) -> Self {
        self.phase_deadline = deadline;
        self
    }

    /// Attach a persistent source vault (Milestone T-004). When a vault is
    /// configured, `gather_with_observer` searches it before issuing any web
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
    /// during `gather_with_observer` and whether it was captured or
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

    /// Attach a query decomposer. When present, `gather_with_observer`
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
    /// `gather_with_observer`. Values of `0` are clamped up to `1` so the
    /// stream always makes progress. Larger values reduce wall-clock latency
    /// when a search returns many hits, at the cost of more in-flight HTTP
    /// connections and memory. The default is [`DEFAULT_FETCH_CONCURRENCY`]
    /// (10).
    ///
    /// This bounds the maximum in-flight HTTP connections at any moment; it
    /// does not gate on the phase deadline (the fetch stage is not
    /// deadline-bounded and fetches are never cancelled on deadline).
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
    /// This per-fetch cap is the bound that keeps the fetch stage finite now
    /// that the stage is not deadline-bounded: every fetch runs to completion
    /// or to this timeout, whichever comes first.
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
    /// When enabled, `gather_with_observer` retains every fetched page
    /// regardless of its query-match relevance score, disabling the default
    /// filter that discards "Low"/"Very low" sources.
    #[must_use]
    pub fn with_keep_low_relevance(mut self, keep: bool) -> Self {
        self.keep_low_relevance = keep;
        self
    }

    /// Disable scholarly search engines (e.g. OpenAlex) during gathering.
    ///
    /// When enabled, `gather_with_observer` filters out hits from
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
    /// delay of `Self::search_retry_base_delay_ms`. Setting this to `0`
    /// disables retries entirely (a single attempt is made). The default is
    /// [`DEFAULT_SEARCH_MAX_RETRIES`] (2).
    ///
    /// The value is clamped to [`MAX_SEARCH_RETRIES`] (ANTIPAT F-02): the
    /// backoff shift and the total sleeping time are both bounded by it.
    #[must_use]
    pub fn with_search_max_retries(mut self, n: u32) -> Self {
        self.search_max_retries = n.min(MAX_SEARCH_RETRIES);
        self
    }

    /// The effective sub-query search retry budget.
    ///
    /// Always bounded by [`MAX_SEARCH_RETRIES`] (ANTIPAT F-02).
    #[must_use]
    pub const fn search_max_retries(&self) -> u32 {
        self.search_max_retries
    }

    /// Override the base delay in milliseconds for the first search-retry
    /// backoff (Milestone H-002). Subsequent retries double this value
    /// (e.g. 200 ms -> 400 ms -> 800 ms). The default is
    /// [`DEFAULT_SEARCH_RETRY_BASE_DELAY_MS`] (200 ms). A value of `0` makes
    /// retries immediate with no delay.
    #[must_use]
    pub fn with_search_retry_base_delay_ms(mut self, ms: u64) -> Self {
        self.search_retry_base_delay_ms = ms;
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

    /// Set the volume cap for competitive (comparison) runs. `Some(n)` makes
    /// `n` the per-query search allowance and `n * sub-queries` the fetch
    /// budget. `None` (the default) runs uncapped: every sub-query search
    /// asks for engine-max results and every retained candidate is fetched -
    /// the per-fetch timeout and fetch concurrency remain the real volume
    /// bounds.
    #[must_use]
    pub fn with_volume_cap(mut self, cap: Option<usize>) -> Self {
        self.volume_cap = cap;
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
    /// fenced via [`fence_source_body`] so it stays within the same byte
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
        let body = fence_source_body(&page.body);
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
            // Off-load the blocking vault write to the blocking pool so a slow
            // disk does not park a tokio worker (FUNC-017).
            if let Err(e) = vault.store_async(&new_source).await {
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
    ///   not surfaced as an error to the caller - the local-gathering
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
    fn filter_hit(&self, prepared: &PreparedQuery, hit: &WebSearchHit) -> Option<(String, bool)> {
        let (label, retained) = prepared.label(&hit.title, &hit.snippet, &hit.url);
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
        // run them concurrently (the reads are independent - sequential
        // `spawn_blocking(..).await` calls would serialize the whole batch
        // behind the slowest file). `join_all` preserves hit order, so the
        // per-index body-path mapping below is unchanged.
        let bodies: Vec<Result<String, anyhow::Error>> =
            futures::future::join_all(hits.iter().map(|hit| {
                let vault = Arc::clone(&vault);
                let source_id = hit.source_id.clone();
                async move {
                    // JoinError -> anyhow; the inner Result is already
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
            let relevance = format!("Vaulted - reused from {}", hit.run_tag);
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

        let engines = collect_engines(sources.iter().map(|s| s.search_engine()));
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
    /// `WebGatherer::fetch_concurrency` at a time (default
    /// [`DEFAULT_FETCH_CONCURRENCY`], 10). Each fetch is also bounded by
    /// [`MAX_SOURCE_BODY_BYTES`] and `WebGatherer::fetch_timeout`.
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
    /// When [`WebGatherer::with_phase_deadline`] set a deadline, the awaits in
    /// the *search stage* are bounded by the remaining budget: the decomposer
    /// call and each search-result wait. After the deadline elapses no new
    /// search is started; the search loop breaks and everything captured so
    /// far proceeds to the fetch stage.
    ///
    /// The fetch stage is NOT deadline-bounded: every candidate produced by
    /// the search stage is fetched to completion and in-flight fetches are
    /// never cancelled on deadline; each fetch is individually capped by
    /// `fetch_timeout`, so the stage never runs unbounded.
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

        // Phase deadline (H-001 / --web-time): when set, the *search stage*
        // of the gather pass becomes best-effort. Once the deadline passes we
        // stop issuing new searches and proceed to fetching. The fetch stage
        // is not deadline-bounded: every candidate produced by the search
        // stage is fetched to completion and in-flight fetches are never
        // cancelled on deadline.
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
        // the bounded waits in the search stage (decomposer, search loop), but
        // the `PhaseTimedOut` event fires exactly once, from the single
        // terminal site at the end of the gather pass, carrying the final
        // captured count. Interim sites only set the flag and break their
        // loops.
        //
        // No-new-search guarantee (FR-008): both await points in the search
        // stage - (a) the decomposer call and (b) each `results.next()` in the
        // search loop - are wrapped in `tokio::time::timeout(remaining(), ..)`
        // via `next_bounded!` or an explicit call. Once the deadline elapses,
        // `truncated` is set and the search loop breaks before polling any
        // further search, so no new search is initiated after the deadline.
        // The fetch stage below is deliberately NOT bounded this way: no
        // fetch start is gated on the deadline and in-flight fetches are
        // never cancelled.
        let mut truncated = false;
        let remaining = || {
            deadline.map_or_else(
                || std::time::Duration::from_secs(u64::MAX / 2),
                |d| d.saturating_duration_since(std::time::Instant::now()),
            )
        };
        // Await the next stream item, but bound the wait by the phase
        // deadline so a stalled search cannot outlive the budget.
        //
        // This macro is the mechanism behind the no-new-search guarantee
        // (FR-008): every stream wait in the search loop is
        // deadline-bounded, so after the deadline elapses the loop sees
        // `truncated` and breaks instead of polling more work. The fetch
        // loop deliberately does NOT use this macro - fetches run to
        // completion regardless of the deadline.
        macro_rules! next_bounded {
            // Only mark truncation when the deadline actually elapsed: a
            // `Duration::ZERO` timeout on an unready stream also yields
            // Err(Elapsed) (e.g. when no deadline is configured), which must
            // not be mistaken for a deadline hit.
            ($stream:expr) => {
                match tokio::time::timeout(remaining(), $stream.next()).await {
                    Ok(item) => item,
                    Err(_) => {
                        if deadline.is_some() {
                            truncated = true;
                        }
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
                            if deadline.is_some() {
                                truncated = true;
                            }
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
        // Milestone H-002: each sub-query search is retried up to
        // `search_max_retries` times with exponential backoff on transient
        // failures.
        let search_tool = self.search.clone();
        // Clamp the retry budget at the use site as well as in the setter so a
        // directly-constructed gatherer cannot reintroduce the shift-overflow
        // panic (ANTIPAT F-02).
        let max_retries = self.search_max_retries.min(MAX_SEARCH_RETRIES);
        let base_delay_ms = self.search_retry_base_delay_ms;
        // Volume policy: uncapped runs (tiered/supervisor default) ask each
        // search for engine-max results and fetch every retained candidate;
        // competitive runs with a volume cap restrict every sub-query to `cap`
        // hits and scale the fetch budget by the sub-query count. Resolved once
        // and reused by the search and fetch stages below.
        let (per_query_allowance, fetch_budget) = volume_policy(self.volume_cap, queries.len());
        let search_budget = self.search_budget.clone();
        let query_cache = self.query_cache.clone();
        let provider_stats = self.provider_stats.clone();
        // T-008 (FR-006, FR-010, NFR-001): when scholarly exclusion is active, steer
        // the search tool away from every academically-classified engine
        // (OpenAlex) *before* any request is dispatched, so no search budget or
        // metered backend call is spent on them. The names come from the single
        // authoritative vocabulary in `masterfetch::search` (FR-003). The
        // hit-level scholarly filter in the capture loop below is kept as
        // defence in depth for tools that cannot steer engine selection
        // (FR-015) - and to guarantee non-academic engines still run untouched.
        // Share the exclusion slice across every sub-query future so the set is
        // cloned once rather than once per sub-query. Build the `Arc` slice
        // directly - no intermediate `Vec` - via the same branch the log uses.
        let exclude_engines: Arc<[&str]> = if self.disable_scholarly {
            tracing::info!(
                engines = ?ACADEMIC_ENGINES,
                "research: scholarly search engines excluded for this sweep (--papers not set)"
            );
            Arc::from(ACADEMIC_ENGINES)
        } else {
            Arc::from(&[][..])
        };
        let search_futures: Vec<_> = queries
            .iter()
            .map(|q| {
                let q = q.clone();
                let tool = search_tool.clone();
                let budget = search_budget.clone();
                let cache = query_cache.clone();
                let stats = provider_stats.clone();
                let exclude = Arc::clone(&exclude_engines);
                async move {
                    // Run-scoped search budget: reserve one call before any
                    // provider request. Exhaustion skips the search entirely.
                    if let Some(budget) = &budget
                        && !budget.try_acquire()
                    {
                        return SearchCallOutcome::BudgetExhausted;
                    }
                    // Shared query cache: an identical query already answered
                    // this run is served without a provider call. The exclusion
                    // set is folded into the cache key so an exclusion-aware result
                    // can never satisfy a later unfiltered query (or vice versa)
                    // within the same run.
                    let cache_key = if exclude.is_empty() {
                        q.clone()
                    } else {
                        format!("{q}\u{1f}exclude={}", exclude.join(","))
                    };
                    if let Some(cache) = &cache
                        && let Some(hits) = cache.get(&cache_key)
                    {
                        return SearchCallOutcome::Ok { hits };
                    }
                    // Retry loop with exponential backoff.
                    let mut attempt: u32 = 0;
                    let mut last_error;
                    loop {
                        let search_result = if exclude.is_empty() {
                            tool.search(&q, per_query_allowance).await
                        } else {
                            tool.search_with_exclusions(&q, per_query_allowance, &exclude)
                                .await
                        };
                        match search_result {
                            Ok(hits) => {
                                // Record one logical search call (retries
                                // included) and memoise the result.
                                if let Some(stats) = &stats {
                                    stats.record(&hit_search_tool(&hits));
                                }
                                // PERF-077: share the hit vector behind an Arc
                                // so caching is a refcount hand-off rather than
                                // a deep clone of every hit.
                                let hits: Arc<[WebSearchHit]> = hits.into();
                                if let Some(cache) = &cache {
                                    cache.insert(&cache_key, Arc::clone(&hits));
                                }
                                return SearchCallOutcome::Ok { hits };
                            }
                            Err(e) => {
                                last_error = e.to_string();
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
                                let delay_ms = base_delay_ms
                                    .saturating_mul(
                                        1u64.checked_shl(attempt - 1).unwrap_or(u64::MAX),
                                    )
                                    .min(MAX_SEARCH_RETRY_DELAY_MS);
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
            .buffer_unordered(SEARCH_FANOUT_CONCURRENCY)
            .enumerate();

        // PERF-077: the sub-query text is shared by every hit under it via
        // `Arc<str>` so each hit stores a refcount bump rather than its own
        // `String` copy of the query.
        let mut hits_by_url: Vec<(Arc<str>, WebSearchHit)> = Vec::new();
        // Per-sub-query prepared relevance query, built lazily on first use so
        // each query's normalisation and morphological variants are computed
        // exactly once per gather pass (PERF-068).
        let mut prepared_queries: std::collections::HashMap<Arc<str>, PreparedQuery> =
            std::collections::HashMap::new();
        let mut seen_urls: HashSet<String> = HashSet::new();
        let mut any_search_error: Option<String> = None;
        // FUNC-030: count sub-query search failures so a partially-failed sweep
        // is distinguishable from a complete one, not only when every
        // sub-query fails.
        let mut failed_sub_queries = 0usize;
        let mut excluded_count = 0usize;
        let mut considered_count = 0usize;
        // Per-engine accounting (T-006). All bump sites run on the gather
        // task itself (search loop + fetch dispatch loop), so a plain map
        // needs no locking.
        let mut per_engine: std::collections::BTreeMap<String, EngineSweepStat> =
            std::collections::BTreeMap::new();
        // Global per-reason exclusion tally for the summary totals row. Kept
        // alongside the per-engine breakdown so the totals row reports the
        // same reasons even when consensus crediting inflates engine sums.
        let mut excluded_by_reason: std::collections::BTreeMap<ExclusionReason, usize> =
            std::collections::BTreeMap::new();
        // Global fetch-failure tally by fine-grained cause, so the `fetch`
        // reason column on the totals row breaks down into the individual
        // fetch failure modes.
        let mut failed_by_kind: std::collections::BTreeMap<FetchFailureKind, usize> =
            std::collections::BTreeMap::new();
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
            // PERF-077: share one `Arc<str>` per sub-query with every hit it
            // yields, instead of cloning the query `String` per hit below.
            let query: Arc<str> =
                Arc::from(queries.get(idx).map_or(topic, std::string::String::as_str));
            match outcome {
                SearchCallOutcome::Ok { hits } => {
                    // `hits` is an `Arc<[WebSearchHit]>`; clone it into an owned
                    // `Vec` so each hit can be mutated in place below. The Arc
                    // keeps the cache entry alive without a deep copy.
                    for mut hit in hits.iter().cloned() {
                        let url_key = hit.url.to_lowercase();
                        if !seen_urls.insert(url_key) {
                            continue;
                        }
                        // Classify once so the two branches below can reuse the
                        // result instead of re-splitting the engine CSV twice.
                        let is_scholarly = is_scholarly_hit(&hit);
                        let is_encyclopedia = is_encyclopedia_hit(&hit);
                        // Count every deduplicated hit as considered so the
                        // summary balances exactly:
                        // considered = captured + excluded + capped + cancelled.
                        considered_count += 1;
                        bump_engine_stats(&mut per_engine, &hit.search_engine, |s| {
                            s.considered += 1
                        });
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
                        // Filter out scholarly hits unless --papers is set.
                        if self.disable_scholarly && is_scholarly {
                            excluded_count += 1;
                            bump_engine_exclusions(
                                &mut per_engine,
                                &mut excluded_by_reason,
                                &hit.search_engine,
                                ExclusionReason::ScholarlyEngine,
                            );
                            let reason = "scholarly engine excluded (--papers not set)";
                            tracing::info!(
                                query = %query,
                                url = %hit.url,
                                "research: skipping scholarly hit (--papers not set)"
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
                            bump_engine_exclusions(
                                &mut per_engine,
                                &mut excluded_by_reason,
                                &hit.search_engine,
                                ExclusionReason::PdfDisabled,
                            );
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
                        // Scholarly hits (e.g. OpenAlex) are already ranked by
                        // the source engine's own relevance score and carry a
                        // reconstructed abstract in the snippet. The lexical
                        // pre-filter is a heuristic for unranked HTML scrapers
                        // and would wrongly reject scholarly titles that do not
                        // lexically overlap the (often rephrased) sub-query, so
                        // these hits bypass it entirely.
                        if is_scholarly {
                            hit.matched_query =
                                format!("{query} [Scholarly - engine-ranked abstract]");
                            hits_by_url.push((Arc::clone(&query), hit));
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
                                format!("{query} [Encyclopedia - engine-ranked summary]");
                            hits_by_url.push((Arc::clone(&query), hit));
                            continue;
                        }
                        // Pre-filter by title/snippet relevance before any
                        // expensive full-page fetch (B-001). The query is
                        // normalised once per sub-query and reused for every
                        // hit under it (PERF-068).
                        let prepared = prepared_queries
                            .entry(Arc::clone(&query))
                            .or_insert_with(|| PreparedQuery::new(&query));
                        if let Some((label, retained)) = self.filter_hit(prepared, &hit) {
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
                            hits_by_url.push((Arc::clone(&query), hit));
                        } else {
                            excluded_count += 1;
                            bump_engine_exclusions(
                                &mut per_engine,
                                &mut excluded_by_reason,
                                &hit.search_engine,
                                ExclusionReason::LowRelevance,
                            );
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
                                query: query.to_string(),
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
                    failed_sub_queries += 1;
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
                                    limit: budget.limit(),
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

        // FUNC-030: surface a partial search-failure even when some hits were
        // collected, so the caller knows coverage is incomplete.
        if failed_sub_queries > 0
            && !hits_by_url.is_empty()
            && let Some(obs) = observer
        {
            obs.on_event(GatherEvent::SearchPartiallyFailed {
                failed: failed_sub_queries,
                total: queries.len(),
                error: any_search_error
                    .clone()
                    .unwrap_or_else(|| "sub-query search failed".to_string()),
            });
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
                            "excluded_by_reason": excluded_by_reason.clone(),
                            "failed_by_kind": failed_by_kind.clone(),
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
                    per_engine: Vec::new(),
                    excluded_by_reason,
                    failed_by_kind,
                    capped: 0,
                    cancelled: 0,
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
        // Concurrency bound: only `fetch_concurrency` fetch futures are polled
        // at any moment; `buffer_unordered` does not start a new fetch until
        // an in-flight one resolves. The fetch loop is NOT deadline-bounded:
        // every queued candidate is started and every in-flight fetch runs to
        // completion or its own `fetch_timeout`, never cancelled on the phase
        // deadline.
        let fetch_concurrency = self.fetch_concurrency.max(1);
        let fetch_tool = self.fetch.clone();
        let fetch_timeout = self.fetch_timeout;
        // Renumber retained hits densely so the supporting-file names have no
        // gaps, while preserving the original search-ranking order.
        // Collect the set of contributing engines before consuming hits_by_url.
        let engines = collect_engines(
            hits_by_url
                .iter()
                .map(|(_, hit)| hit.search_engine.as_str()),
        );
        // Fetch-budget accounting (T-006): hits beyond the fetch budget are
        // neither fetched nor counted as excluded, so the summary reports
        // them separately to keep the balance
        // considered == captured + excluded + capped + cancelled.
        //
        // Volume policy: uncapped runs (the default - tiered/supervisor)
        // fetch every retained candidate; the per-fetch `fetch_timeout` caps
        // each page and `fetch_concurrency` bounds parallelism, so a separate
        // fetch cap would only discard candidates the search already paid for
        // (observed: 42 retained / 35 capped / 7 captured on a 7-query
        // sweep). Capped (competitive) runs scale the per-query allowance by
        // the sub-query count so the cap bounds per-query volume, not the
        // whole sweep. `fetch_budget` is resolved once alongside the search
        // allowance above.
        let candidate_total = hits_by_url.len();
        let capped_count = candidate_total.saturating_sub(fetch_budget);
        let fetch_futures = hits_by_url
            .into_iter()
            .take(fetch_budget)
            .enumerate()
            .map(|(index, (query, hit))| (index, query, hit))
            .map(|(index, query, hit)| {
                let fetch_tool = fetch_tool.clone();
                async move {
                    // Scholarly and encyclopedia hits are captured as
                    // self-contained sources from the snippet - the underlying
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
                            // PERF-077: clone the body `Arc`, not the page
                            // bytes, so language detection adds no page-sized
                            // allocation and the page keeps its own copy.
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
        let mut collected: Vec<(usize, Option<Source>)> =
            Vec::with_capacity(candidate_total.min(UNCAPTED_MAX_RESULTS));
        let mut stream = futures::stream::iter(fetch_futures).buffer_unordered(fetch_concurrency);
        // The fetch stage is deliberately NOT deadline-bounded: once the
        // search stage produced the candidate set, every candidate fetch is
        // started regardless of the phase deadline and in-flight fetches are
        // never cancelled on deadline - each fetch is still individually
        // capped by `fetch_timeout`, so the stage stays bounded.
        while let Some((index, query, hit, result, language_fallback)) = stream.next().await {
            match result {
                Ok(Ok(page)) => {
                    let scholarly = page.page_type.as_deref() == Some("scholarly");
                    let encyclopedia = page.page_type.as_deref() == Some("encyclopedia");
                    let mut title = clean_web_source_title(&page.title, &hit.title);
                    let body_path = web_body_path(index);
                    let body = fence_source_body(&page.body);
                    // Classify the media type once per page; three render
                    // sites below (SourceCaptured event, vault store, final
                    // Source::Web) previously re-classified independently.
                    let media_type = classify_web_source(&page.url, page.content_type.as_deref())
                        .as_str()
                        .to_string();
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
                        ("Scholarly - engine-ranked abstract".to_string(), true)
                    } else if encyclopedia {
                        ("Encyclopedia - engine-ranked summary".to_string(), true)
                    } else {
                        // Post-fetch relevance, reusing the prepared query built
                        // by the pre-filter (PERF-068).
                        let prepared = prepared_queries
                            .entry(Arc::clone(&query))
                            .or_insert_with(|| PreparedQuery::new(&query));
                        prepared.label(&title, &hit.snippet, &page.url)
                    };
                    if !retained && !self.keep_low_relevance {
                        excluded_count += 1;
                        bump_engine_exclusions(
                            &mut per_engine,
                            &mut excluded_by_reason,
                            &hit.search_engine,
                            ExclusionReason::LowRelevance,
                        );
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
                    // lower threshold - their body is the concise reconstructed
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
                        bump_engine_exclusions(
                            &mut per_engine,
                            &mut excluded_by_reason,
                            &hit.search_engine,
                            ExclusionReason::ContentTooShort,
                        );
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
                            "research: skipping web source - extracted content below minimum"
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
                            media_type: media_type.clone(),
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
                            media_type: media_type.clone(),
                            content_type: page.content_type.clone(),
                            body_text: body.clone(),
                            summary_text: summary_text.clone(),
                        };
                        // Off-load the blocking vault write to the blocking pool
                        // so a slow disk does not park a tokio worker (FUNC-017).
                        if let Err(e) = vault.store_async(&new_source).await {
                            tracing::warn!(
                                error = %e,
                                url = %page.url,
                                "research: failed to store source in vault"
                            );
                            // FUNC-030: surface the persistence failure so a
                            // captured-but-unstored source is not silently
                            // counted as fully captured.
                            if let Some(obs) = observer {
                                obs.on_event(GatherEvent::VaultStoreFailed {
                                    url: page.url.clone(),
                                    error: e.to_string(),
                                });
                            }
                        }
                    }
                    // Credit the capture before `hit.search_engine` is moved
                    // into the source below, so no clone is needed.
                    bump_engine_stats(&mut per_engine, &hit.search_engine, |s| s.captured += 1);
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
                            media_type: media_type.clone(),
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
                    bump_engine_fetch_failure(
                        &mut per_engine,
                        &mut excluded_by_reason,
                        &mut failed_by_kind,
                        &hit.search_engine,
                        classify_fetch_failure(&e),
                    );
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
                    bump_engine_fetch_failure(
                        &mut per_engine,
                        &mut excluded_by_reason,
                        &mut failed_by_kind,
                        &hit.search_engine,
                        FetchFailureKind::Timeout,
                    );
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
        // Emit the timeout diagnostic when the configured deadline has passed
        // by the end of the pass. The deadline's own state is the ground
        // truth: the unbounded fetch stage can outlive it without any
        // bounded wait expiring.
        if deadline.is_some_and(|d| d <= std::time::Instant::now()) {
            emit_deadline_event(observer, sources.len());
        }
        // Cancellation accounting (T-006): the fetch stage never cancels
        // fetches for the phase deadline, so this is structurally 0; the
        // derived counter is kept so the WidthSweepSummary balance invariant
        // `considered == captured + excluded + capped + cancelled` still
        // holds if a future truncation site is added to the fetch stage.
        let cancelled_count =
            considered_count.saturating_sub(sources.len() + excluded_count + capped_count);
        let per_engine_stats: Vec<EngineSweepStat> = per_engine.into_values().collect();
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
                        "capped": capped_count,
                        "cancelled": cancelled_count,
                        "per_engine": per_engine_stats,
                        "excluded_by_reason": excluded_by_reason.clone(),
                        "failed_by_kind": failed_by_kind.clone(),
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
                per_engine: per_engine_stats,
                excluded_by_reason,
                failed_by_kind,
                capped: capped_count,
                cancelled: cancelled_count,
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

/// Outcome of a single sub-query search call, including retry state
/// (Milestone H-002).
enum SearchCallOutcome {
    /// The search succeeded (retry counts are only tracked on the `Err`
    /// variant, where they drive `SearchRetrying` events).
    Ok {
        /// Search hits returned by the tool, shared behind an `Arc` so the
        /// cache insert is a refcount hand-off (PERF-077).
        hits: Arc<[WebSearchHit]>,
    },
    /// The search failed after all retries were exhausted.
    Err {
        /// Last error message.
        error: String,
        /// Number of retries attempted.
        retries: u32,
    },
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
/// - "High - title + snippet match query"
/// - "Medium - snippet matches query"
/// - "Low - weak match"
/// - "Very high - exact title match"
///
/// Compute the zero-padded supporting-file path for the Nth web source.
///
/// Index 0 -> `web-01.md`, index 1 -> `web-02.md`, etc. The path is
/// relative to the research item directory (`research/<name>/`).
fn web_body_path(index: usize) -> PathBuf {
    PathBuf::from(format!("sources/web-{:02}.md", index + 1))
}

#[cfg(test)]
#[path = "../tests/inline/web_gatherer_tests.rs"]
mod tests;
