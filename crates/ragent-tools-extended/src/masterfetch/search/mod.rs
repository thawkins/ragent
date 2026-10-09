//! Search subsystem for the masterfetch toolset.
//!
//! This module hosts the keyless multi-engine web search implementation
//! (`mf_search`), comprising:
//!
//! - [`engine`] - the [`SearchEngine`] trait, [`RawResult`] / [`EngineReport`]
//!   structs, search options, and URL-normalisation helpers for dedup
//!   (T-012, FR-008, NFR-003).
//! - [`langsearch`] - LangSearch API-backed backend (T-003).
//! - [`tavily`] - Tavily API-backed backend (T-001, T-002).
//! - [`perplexity`] - Perplexity Sonar API-backed backend.
//! - [`serper`] - Serper (Google Search) API-backed backend.
//! - [`openalex`] - OpenAlex keyless scholarly-works backend (spec `openalex`).
//! - [`wikipedia`] - Wikipedia REST API keyless encyclopedia backend (spec
//!   `wikisearch`).
//! - [`exa`] - Exa Search API-backed backend (spec `exasearch`).
//! - [`consensus`] - merge, dedup, consensus boost, and ranking (T-015).
//! - [`SearchOrchestrator`] - run all backends in parallel, merge, cache
//!   (T-016).
//!
//! # Requirements
//!
//! - **FR-008** - keyless multi-engine web search: multiple backends in
//!   parallel, merge + dedup by normalised URL, rank with cross-engine
//!   consensus. No API keys required.
//! - **FR-009** - response signals: `relevance_score`, `fetch_relevance`,
//!   `engines_consensus`, `related_queries`, `fetch_hint`.
//! - **FR-010** - filters: `site`, `exclude_sites`, `freshness`,
//!   `max_results`, `page`.
//! - **FR-023** - no API keys, tokens, or accounts for `mf_search`.
//! - **NFR-001** - search completes within 15 seconds; backends run in
//!   parallel.
//! - **NFR-003** - pure types and injectable trait, testable without network.
//!
//! # Design
//!
//! The [`SearchEngine`] trait is `async` and `Send + Sync` so that multiple
//! backends can be queried concurrently with `futures::join_all`. The
//! [`SearchOrchestrator`] runs all enabled backends in parallel, collects
//! [`EngineReport`]s, passes them to [`consensus::merge_and_rank`], and
//! caches the result for 5 minutes.
//!
//! For testing, the orchestrator accepts a `Vec<Box<dyn SearchEngine>>`,
//! enabling mock engines without network I/O (NFR-003).

pub mod consensus;
pub mod engine;
pub mod exa;
pub mod langsearch;
pub mod openalex;
pub mod perplexity;
pub mod serper;
pub mod tavily;
pub mod wikipedia;

// Re-export commonly used types at the module level.
pub use consensus::{ConsensusResult, MergeOutput, merge_and_rank, merge_and_rank_with_cap};
pub use engine::{
    DEFAULT_SEARCH_MAX_RETRIES, DEFAULT_SEARCH_RETRY_DELAY, EngineReport, Freshness, RawResult,
    SearchEngine, SearchEngineError, SearchOptions, blocked_engine_names, collect_all_results,
    count_engines_with_results, count_total_results, dedup_results_by_url, normalise_result_url,
    report_is_transient, search_with_retry,
};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Search-result cache TTL: 5 minutes (NFR-001).
pub const SEARCH_CACHE_TTL: Duration = Duration::from_mins(5);

/// Per-engine timeout. If a single backend does not respond within this
/// duration it is dropped from the merge and reported as an error. This keeps
/// the overall search bounded even if one engine hangs. The shared HTTP client
/// already has a 30-second timeout; this per-engine timeout matches it so a
/// slow engine is given a full window to respond before being dropped.
pub const ENGINE_TIMEOUT: Duration = Duration::from_secs(30);

/// Stagger between the start of consecutive engine futures in one parallel
/// batch.
///
/// All engine futures fire at `t=0` today, so any backend with a rate limiter
/// that penalizes bursts (Wikipedia's Action API, OpenAlex) gets hit by its
/// own request *and* by every other engine's at once whenever the caller runs
/// a fan-out. A short start-up stagger keeps the engines parallel overall
/// while spreading the first-request spikes.
pub const ENGINE_STAGGER: Duration = Duration::from_millis(120);

/// Canonical display name of the OpenAlex scholarly-works backend.
///
/// Aliases [`openalex::ENGINE_NAME`] so the string literal exists in exactly
/// one place while cross-engine code can import it from the `search` module
/// root.
pub const ENGINE_OPENALEX: &str = openalex::ENGINE_NAME;

/// Canonical display name of the Wikipedia encyclopedia backend.
///
/// Aliases [`wikipedia::ENGINE_NAME`].
pub const ENGINE_WIKIPEDIA: &str = wikipedia::ENGINE_NAME;

/// The set of academically-classified search engine names.
///
/// Today this is just OpenAlex (the scholarly-works catalog). Future scholarly
/// backends (for example arXiv or Semantic Scholar) are added here so that
/// research "no papers" engine exclusion and scholarly-hit classification pick
/// them up automatically.
pub const ACADEMIC_ENGINES: &[&str] = &[ENGINE_OPENALEX];

/// Returns `true` when `name` is an academically-classified search engine.
///
/// Matching is exact (case-sensitive) against [`ACADEMIC_ENGINES`], mirroring
/// the exact-match semantics of [`SearchEngine::name`] and the research-layer
/// `is_scholarly_hit` check (FR-003).
#[must_use]
pub fn is_academic_engine(name: &str) -> bool {
    ACADEMIC_ENGINES.contains(&name)
}

/// Run one [`SearchEngine`] with timeout, transient retry and staggered start.
///
/// This is the single per-engine call path used by both
/// [`SearchOrchestrator::search`] and [`SearchOrchestrator::search_per_engine`]
/// so every surface (TUI `/websearch`, CLI `mf_search`, research web-gather)
/// inherits the same resilience behaviour from the engine level instead of
/// re-implementing retries per surface. The `stagger_index`-th engine is
/// started `stagger_index * ENGINE_STAGGER` after the batch begins so
/// bursty keyless backends (Wikipedia, OpenAlex) don't all fire at once.
pub async fn run_engine_with_resilience(
    engine: Arc<dyn SearchEngine>,
    query: &str,
    opts: &SearchOptions,
    stagger_index: usize,
) -> EngineReport {
    if stagger_index > 0 {
        tokio::time::sleep(ENGINE_STAGGER * stagger_index as u32).await;
    }
    match tokio::time::timeout(
        ENGINE_TIMEOUT,
        search_with_retry(
            &engine,
            query,
            opts,
            DEFAULT_SEARCH_MAX_RETRIES,
            DEFAULT_SEARCH_RETRY_DELAY,
        ),
    )
    .await
    {
        Ok(report) => report,
        Err(_) => {
            // Only the error path needs the owned engine name.
            let name = engine.name().to_string();
            tracing::warn!(
                engine = %name,
                timeout_secs = ENGINE_TIMEOUT.as_secs(),
                "search engine timed out, dropping from merge"
            );
            EngineReport::error(
                name,
                format!("engine timed out after {}s", ENGINE_TIMEOUT.as_secs()),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// SearchOutput
// ---------------------------------------------------------------------------

/// The complete output of a search query.
///
/// Returned by [`SearchOrchestrator::search`]. Wraps the consensus
/// [`MergeOutput`] with search-level metadata: the original query, whether the
/// result was served from cache, the total duration, and the list of engines
/// used.
#[derive(Debug, Clone)]
pub struct SearchOutput {
    /// The original search query.
    pub query: String,
    /// Ranked results from the consensus merge.
    pub merge: MergeOutput,
    /// Whether the result was served from the 5-minute search cache.
    pub cached: bool,
    /// Total search duration in milliseconds (0 for cache hits).
    pub duration_ms: u64,
    /// Names of the engines that were queried.
    pub engines_used: Vec<String>,
    /// The search options that were applied.
    pub options: SearchOptions,
}

// ---------------------------------------------------------------------------
// SearchOrchestrator
// ---------------------------------------------------------------------------

/// Search orchestrator: runs all enabled backends in parallel, merges results
/// via consensus, and caches the output for 5 minutes.
///
/// # Requirements
///
/// - **FR-008** - multiple backends in parallel.
/// - **FR-009** - response signals.
/// - **FR-010** - filters via [`SearchOptions`].
/// - **NFR-001** - parallel execution for low latency.
/// - **NFR-003** - injectable engines for testing.
///
/// # Examples
///
/// Create an orchestrator with custom backends:
///
/// ```no_run
/// use ragent_tools_extended::masterfetch::search::{
///     SearchOrchestrator, SearchOptions,
/// };
///
/// # async fn demo() {
/// let orchestrator = SearchOrchestrator::new();
/// let opts = SearchOptions::new(10);
/// let output = orchestrator.search("rust async", &opts).await;
/// assert_eq!(output.query, "rust async");
/// # }
/// ```
pub struct SearchOrchestrator {
    /// The search backends to query in parallel.
    engines: Vec<Arc<dyn SearchEngine>>,
    /// In-memory search-result cache (query-key -> (output, timestamp)).
    cache: Mutex<HashMap<String, CacheEntry>>,
}

/// A cached search result with its insertion timestamp.
#[derive(Debug, Clone)]
struct CacheEntry {
    output: SearchOutput,
    inserted_at: Instant,
}

impl SearchOrchestrator {
    /// Create a new orchestrator with no backends. Callers should use
    /// [`MfSearchTool::build_orchestrator`] to obtain a fully wired
    /// orchestrator, or [`with_engines`](Self::with_engines) to supply custom
    /// backends.
    #[must_use]
    pub fn new() -> Self {
        Self::with_engines(Vec::new())
    }

    /// Create a new orchestrator with custom backends (for testing or for
    /// adding additional engines).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use ragent_tools_extended::masterfetch::search::{
    ///     SearchOrchestrator, SearchEngine, EngineReport, SearchOptions,
    /// };
    /// use std::sync::Arc;
    ///
    /// struct MockEngine;
    ///
    /// #[async_trait::async_trait]
    /// impl SearchEngine for MockEngine {
    ///     fn name(&self) -> &str { "mock" }
    ///     async fn search(&self, _q: &str, _o: &SearchOptions) -> EngineReport {
    ///         EngineReport::ok("mock", vec![])
    ///     }
    /// }
    ///
    /// let orchestrator = SearchOrchestrator::with_engines(vec![Arc::new(MockEngine)]);
    /// assert_eq!(orchestrator.engine_count(), 1);
    /// ```
    #[must_use]
    pub fn with_engines(engines: Vec<Arc<dyn SearchEngine>>) -> Self {
        Self {
            engines,
            cache: Mutex::new(HashMap::new()),
        }
    }

    /// Return the number of registered backends.
    #[must_use]
    pub fn engine_count(&self) -> usize {
        self.engines.len()
    }

    /// Return the names of all registered backends.
    #[must_use]
    pub fn engine_names(&self) -> Vec<&str> {
        self.engines.iter().map(|e| e.name()).collect()
    }

    /// Return a new orchestrator containing only the engine with the given
    /// name, or `None` if no registered engine matches.
    ///
    /// The returned orchestrator has its own (empty) cache. This is intended
    /// for the `mf_search` `engine` parameter, which restricts the search to
    /// a single backend.
    #[must_use]
    pub fn select_engine(&self, name: &str) -> Option<Self> {
        self.engines
            .iter()
            .find(|e| e.name() == name)
            .map(|e| Self::with_engines(vec![e.clone()]))
    }

    /// Return a new orchestrator containing every registered engine whose name
    /// is **not** in `names`, leaving the receiver unchanged.
    ///
    /// The returned orchestrator has its own (empty) cache. This is the
    /// complement of [`select_engine`](Self::select_engine) and is intended for
    /// the `mf_search` `exclude_engines` parameter, which removes a set of
    /// backends (for example the OpenAlex scholarly backend when the caller
    /// requests no papers) before any request is dispatched.
    ///
    /// Unknown names are ignored: an exclusion that does not match a registered
    /// engine simply excludes nothing. An empty `names` slice returns a copy
    /// with every engine retained, so the default (no exclusions) behaves
    /// exactly as the receiver.
    #[must_use]
    pub fn exclude_engines<S: AsRef<str>>(&self, names: &[S]) -> Self {
        let kept = self
            .engines
            .iter()
            .filter(|e| !names.iter().any(|n| n.as_ref() == e.name()))
            .cloned()
            .collect();
        Self::with_engines(kept)
    }

    /// Execute a search query across all backends in parallel, merge the
    /// results, and return the ranked output.
    ///
    /// # Arguments
    ///
    /// - `query` - the search query string (must not be empty).
    /// - `opts` - search options (filters: site, `exclude_sites`, freshness,
    ///   `max_results`, page).
    ///
    /// # Returns
    ///
    /// A [`SearchOutput`] with ranked results, related queries, and metadata.
    /// If the query was recently executed with the same options, the cached
    /// result is returned (`cached = true`).
    ///
    /// # Errors
    ///
    /// This method does not return `Err` - engine-level failures (rate
    /// limits, network errors) are captured in the `EngineReport`'s
    /// `engine_blocked` field and reflected in the output's
    /// `merge.blocked_engines`.
    pub async fn search(&self, query: &str, opts: &SearchOptions) -> SearchOutput {
        // Validate query.
        if query.trim().is_empty() {
            return SearchOutput {
                query: query.to_string(),
                merge: MergeOutput::default(),
                cached: false,
                duration_ms: 0,
                engines_used: Vec::new(), // no engines were queried
                options: opts.clone(),
            };
        }

        // Check cache.
        let cache_key = build_cache_key(query, opts);
        if let Some(cached) = self.check_cache(&cache_key) {
            return cached;
        }

        let start = Instant::now();

        // Build per-engine options: each backend receives `per_engine_results`
        // as its `max_results` request count, while the overall merge cap
        // remains `opts.max_results`.
        let per_engine_opts = {
            let mut pe = opts.clone();
            pe.max_results = opts.per_engine_results;
            pe
        };

        // Run all backends in parallel, each wrapped in a per-engine timeout,
        // one transient retry, and a staggered start (T-016). Engines that
        // exceed ENGINE_TIMEOUT are dropped from the merge.
        let futures: Vec<_> = self
            .engines
            .iter()
            .enumerate()
            .map(|(idx, engine)| {
                let engine = engine.clone();
                let query = query.to_string();
                let opts = per_engine_opts.clone();
                async move { run_engine_with_resilience(engine, &query, &opts, idx).await }
            })
            .collect();

        let reports = futures::future::join_all(futures).await;

        // Merge and rank with the overall cap.
        let merge = merge_and_rank_with_cap(&reports, query, opts.max_results);

        let elapsed = start.elapsed().as_millis() as u64;
        let engines_used: Vec<String> = self.engine_names().into_iter().map(String::from).collect();

        let output = SearchOutput {
            query: query.to_string(),
            merge,
            cached: false,
            duration_ms: elapsed,
            engines_used,
            options: opts.clone(),
        };

        // Store in cache.
        self.store_cache(cache_key, output.clone());

        output
    }

    /// Execute a search query against each registered backend and return the
    /// raw per-engine reports.
    ///
    /// Unlike [`search`](Self::search), this method does **not** merge,
    /// deduplicate, or cache results. It is intended for diagnostics such
    /// as the TUI `/websearch test` command, which needs to know how many
    /// results each engine returned individually.
    ///
    /// # Arguments
    ///
    /// - `query` - the search query string (must not be empty).
    /// - `opts` - search options applied to every backend.
    ///
    /// # Returns
    ///
    /// A vector of [`EngineReport`]s, one per registered backend, in the same
    /// order as [`engine_names`](Self::engine_names).
    pub async fn search_per_engine(&self, query: &str, opts: &SearchOptions) -> Vec<EngineReport> {
        if query.trim().is_empty() {
            return Vec::new();
        }

        // Same per-engine call path as `search` (T-016): timeout, one
        // transient retry, staggered start. Diagnostics and the live
        // `/websearch test` probe get the same resilience as merged search.
        let futures: Vec<_> = self
            .engines
            .iter()
            .enumerate()
            .map(|(idx, engine)| {
                let engine = engine.clone();
                let query = query.to_string();
                let opts = opts.clone();
                async move { run_engine_with_resilience(engine, &query, &opts, idx).await }
            })
            .collect();

        futures::future::join_all(futures).await
    }

    /// Lock the search cache, recovering from mutex poisoning.
    ///
    /// The cache contains only plain data (no invariants to violate), so
    /// poisoning recovery is always safe.
    fn cache_lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, CacheEntry>> {
        self.cache.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Clear the search-result cache.
    pub fn clear_cache(&self) {
        let mut cache = self.cache_lock();
        cache.clear();
    }

    /// Return the number of entries in the search cache.
    #[must_use]
    pub fn cache_size(&self) -> usize {
        self.cache_lock().len()
    }

    /// Check the cache for a fresh entry. Returns `Some(output)` if the cache
    /// has a fresh entry for the key, `None` otherwise.
    fn check_cache(&self, key: &str) -> Option<SearchOutput> {
        let mut cache = self.cache_lock();
        if let Some(entry) = cache.get(key) {
            if entry.inserted_at.elapsed() < SEARCH_CACHE_TTL {
                let mut output = entry.output.clone();
                output.cached = true;
                return Some(output);
            }
            // Expired - remove.
            cache.remove(key);
        }
        None
    }

    /// Store a search output in the cache, evicting expired entries to
    /// prevent unbounded memory growth in long-running sessions.
    fn store_cache(&self, key: String, output: SearchOutput) {
        let mut cache = self.cache_lock();
        // Purge expired entries before inserting so the cache does not
        // grow unboundedly with single-use queries.
        cache.retain(|_, e| e.inserted_at.elapsed() < SEARCH_CACHE_TTL);
        cache.insert(
            key,
            CacheEntry {
                output,
                inserted_at: Instant::now(),
            },
        );
    }
}

impl Default for SearchOrchestrator {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Cache key construction (pure, testable)
// ---------------------------------------------------------------------------

/// Build a cache key from the query and search options.
///
/// The key incorporates all filter parameters so that different filters
/// produce different cache entries.
#[must_use]
pub fn build_cache_key(query: &str, opts: &SearchOptions) -> String {
    format!(
        "{}|{}|{}|{}|{:?}|{}|{}",
        query.trim().to_ascii_lowercase(),
        opts.max_results,
        opts.per_engine_results,
        opts.site,
        opts.exclude_sites.join(","),
        opts.freshness,
        opts.page,
    )
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "../../../tests/inline/mod_tests.rs"]
mod tests;
