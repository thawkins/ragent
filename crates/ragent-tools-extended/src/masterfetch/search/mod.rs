//! Search subsystem for the masterfetch toolset.
//!
//! This module hosts the keyless multi-engine web search implementation
//! (`mf_search`), comprising:
//!
//! - [`engine`] — the [`SearchEngine`] trait, [`RawResult`] / [`EngineReport`]
//!   structs, search options, and URL-normalisation helpers for dedup
//!   (T-012, FR-008, NFR-003).
//! - [`duckduckgo`] — `DuckDuckGo` keyless backend (T-013).
//! - [`brave`] — Brave keyless backend (T-014).
//! - [`langsearch`] — LangSearch API-backed backend (T-003).
//! - [`tavily`] — Tavily API-backed backend (T-001, T-002).
//! - [`perplexity`] — Perplexity Sonar API-backed backend.
//! - [`consensus`] — merge, dedup, consensus boost, and ranking (T-015).
//! - [`SearchOrchestrator`] — run all backends in parallel, merge, cache
//!   (T-016).
//!
//! # Requirements
//!
//! - **FR-008** — keyless multi-engine web search: multiple backends in
//!   parallel, merge + dedup by normalised URL, rank with cross-engine
//!   consensus. No API keys required.
//! - **FR-009** — response signals: `relevance_score`, `fetch_relevance`,
//!   `engines_consensus`, `related_queries`, `fetch_hint`.
//! - **FR-010** — filters: `site`, `exclude_sites`, `freshness`,
//!   `max_results`, `page`.
//! - **FR-023** — no API keys, tokens, or accounts for `mf_search`.
//! - **NFR-001** — search completes within 15 seconds; backends run in
//!   parallel.
//! - **NFR-003** — pure types and injectable trait, testable without network.
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

pub mod brave;
pub mod consensus;
pub mod duckduckgo;
pub mod engine;
pub mod langsearch;
pub mod perplexity;
pub mod tavily;

// Re-export commonly used types at the module level.
pub use consensus::{ConsensusResult, MergeOutput, merge_and_rank, merge_and_rank_with_cap};
pub use engine::{
    EngineReport, Freshness, RawResult, SearchEngine, SearchEngineError, SearchOptions,
    blocked_engine_names, collect_all_results, count_engines_with_results, count_total_results,
    dedup_results_by_url, normalise_result_url,
};

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Search-result cache TTL: 5 minutes (NFR-001).
pub const SEARCH_CACHE_TTL: Duration = Duration::from_mins(5);

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
/// - **FR-008** — multiple backends in parallel.
/// - **FR-009** — response signals.
/// - **FR-010** — filters via [`SearchOptions`].
/// - **NFR-001** — parallel execution for low latency.
/// - **NFR-003** — injectable engines for testing.
///
/// # Examples
///
/// Create an orchestrator with default backends (`DuckDuckGo` + Brave):
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
    /// In-memory search-result cache (query-key → (output, timestamp)).
    cache: Mutex<HashMap<String, CacheEntry>>,
}

/// A cached search result with its insertion timestamp.
#[derive(Debug, Clone)]
struct CacheEntry {
    output: SearchOutput,
    inserted_at: Instant,
}

impl SearchOrchestrator {
    /// Create a new orchestrator with the default backends (`DuckDuckGo` +
    /// Brave).
    #[must_use]
    pub fn new() -> Self {
        let engines: Vec<Arc<dyn SearchEngine>> = vec![
            Arc::new(duckduckgo::DuckDuckGoEngine::new()),
            Arc::new(brave::BraveEngine::new()),
        ];
        Self::with_engines(engines)
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

    /// Execute a search query across all backends in parallel, merge the
    /// results, and return the ranked output.
    ///
    /// # Arguments
    ///
    /// - `query` — the search query string (must not be empty).
    /// - `opts` — search options (filters: site, `exclude_sites`, freshness,
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
    /// This method does not return `Err` — engine-level failures (rate
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
                engines_used: self.engine_names().into_iter().map(String::from).collect(),
                options: opts.clone(),
            };
        }

        // Check cache.
        let cache_key = build_cache_key(query, opts);
        if let Some(cached) = self.check_cache(&cache_key) {
            return cached;
        }

        let start = Instant::now();

        // Run all backends in parallel.
        let futures: Vec<_> = self
            .engines
            .iter()
            .map(|engine| {
                let engine = engine.clone();
                let query = query.to_string();
                let opts = opts.clone();
                async move { engine.search(&query, &opts).await }
            })
            .collect();

        let reports = futures::future::join_all(futures).await;

        // Merge and rank.
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
    /// - `query` — the search query string (must not be empty).
    /// - `opts` — search options applied to every backend.
    ///
    /// # Returns
    ///
    /// A vector of [`EngineReport`]s, one per registered backend, in the same
    /// order as [`engine_names`](Self::engine_names).
    pub async fn search_per_engine(&self, query: &str, opts: &SearchOptions) -> Vec<EngineReport> {
        if query.trim().is_empty() {
            return Vec::new();
        }

        let futures: Vec<_> = self
            .engines
            .iter()
            .map(|engine| {
                let engine = engine.clone();
                let query = query.to_string();
                let opts = opts.clone();
                async move { engine.search(&query, &opts).await }
            })
            .collect();

        futures::future::join_all(futures).await
    }

    /// Clear the search-result cache.
    pub fn clear_cache(&self) {
        let mut cache = self.cache.lock().expect("search cache mutex poisoned");
        cache.clear();
    }

    /// Return the number of entries in the search cache.
    #[must_use]
    pub fn cache_size(&self) -> usize {
        self.cache
            .lock()
            .expect("search cache mutex poisoned")
            .len()
    }

    /// Check the cache for a fresh entry. Returns `Some(output)` if the cache
    /// has a fresh entry for the key, `None` otherwise.
    fn check_cache(&self, key: &str) -> Option<SearchOutput> {
        let mut cache = self.cache.lock().expect("search cache mutex poisoned");
        if let Some(entry) = cache.get(key) {
            if entry.inserted_at.elapsed() < SEARCH_CACHE_TTL {
                let mut output = entry.output.clone();
                output.cached = true;
                return Some(output);
            }
            // Expired — remove.
            cache.remove(key);
        }
        None
    }

    /// Store a search output in the cache.
    fn store_cache(&self, key: String, output: SearchOutput) {
        let mut cache = self.cache.lock().expect("search cache mutex poisoned");
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
        "{}|{}|{}|{}|{:?}|{}",
        query.trim().to_ascii_lowercase(),
        opts.max_results,
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
mod tests {
    use super::*;

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

    struct BlockedMockEngine;

    #[async_trait::async_trait]
    impl SearchEngine for BlockedMockEngine {
        fn name(&self) -> &'static str {
            "blocked"
        }
        async fn search(&self, _query: &str, _opts: &SearchOptions) -> EngineReport {
            EngineReport::blocked("blocked", "rate-limited")
        }
    }

    #[test]
    fn test_build_cache_key_includes_query() {
        let opts = SearchOptions::default();
        let key1 = build_cache_key("rust", &opts);
        let key2 = build_cache_key("python", &opts);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_build_cache_key_case_insensitive_query() {
        let opts = SearchOptions::default();
        let key1 = build_cache_key("Rust", &opts);
        let key2 = build_cache_key("rust", &opts);
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_build_cache_key_includes_max_results() {
        let opts1 = SearchOptions::new(5);
        let opts2 = SearchOptions::new(10);
        let key1 = build_cache_key("rust", &opts1);
        let key2 = build_cache_key("rust", &opts2);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_build_cache_key_includes_site() {
        let opts1 = SearchOptions::default().with_site("github.com");
        let opts2 = SearchOptions::default().with_site("stackoverflow.com");
        let key1 = build_cache_key("rust", &opts1);
        let key2 = build_cache_key("rust", &opts2);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_build_cache_key_includes_freshness() {
        let opts1 = SearchOptions::default().with_freshness(Freshness::Day);
        let opts2 = SearchOptions::default().with_freshness(Freshness::Week);
        let key1 = build_cache_key("rust", &opts1);
        let key2 = build_cache_key("rust", &opts2);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_build_cache_key_includes_page() {
        let opts1 = SearchOptions::default().with_page(0);
        let opts2 = SearchOptions::default().with_page(1);
        let key1 = build_cache_key("rust", &opts1);
        let key2 = build_cache_key("rust", &opts2);
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_build_cache_key_trims_query() {
        let opts = SearchOptions::default();
        let key1 = build_cache_key("  rust  ", &opts);
        let key2 = build_cache_key("rust", &opts);
        assert_eq!(key1, key2);
    }

    #[tokio::test]
    async fn test_orchestrator_with_mock_engines() {
        let engines: Vec<Arc<dyn SearchEngine>> = vec![
            Arc::new(MockEngine {
                name: "ddg",
                results: vec![RawResult::new("A", "https://a.com", "Snip A", "ddg")],
            }),
            Arc::new(MockEngine {
                name: "brave",
                results: vec![RawResult::new("B", "https://b.com", "Snip B", "brave")],
            }),
        ];
        let orchestrator = SearchOrchestrator::with_engines(engines);

        let output = orchestrator.search("test", &SearchOptions::default()).await;

        assert_eq!(output.query, "test");
        assert!(!output.cached);
        assert_eq!(output.merge.total_engines, 2);
        assert_eq!(output.merge.results.len(), 2);
        assert!(output.engines_used.contains(&"ddg".to_string()));
        assert!(output.engines_used.contains(&"brave".to_string()));
    }

    #[tokio::test]
    async fn test_orchestrator_empty_query_returns_empty() {
        let orchestrator = SearchOrchestrator::with_engines(vec![]);
        let output = orchestrator.search("", &SearchOptions::default()).await;

        assert_eq!(output.query, "");
        assert!(output.merge.results.is_empty());
        assert!(!output.cached);
    }

    #[tokio::test]
    async fn test_orchestrator_blocked_engine_reported() {
        let engines: Vec<Arc<dyn SearchEngine>> = vec![
            Arc::new(MockEngine {
                name: "ddg",
                results: vec![RawResult::new("A", "https://a.com", "", "ddg")],
            }),
            Arc::new(BlockedMockEngine),
        ];
        let orchestrator = SearchOrchestrator::with_engines(engines);

        let output = orchestrator.search("test", &SearchOptions::default()).await;

        assert!(
            output
                .merge
                .blocked_engines
                .contains(&"blocked".to_string())
        );
        assert_eq!(output.merge.results.len(), 1);
    }

    #[tokio::test]
    async fn test_orchestrator_cache_hit() {
        let engines: Vec<Arc<dyn SearchEngine>> = vec![Arc::new(MockEngine {
            name: "ddg",
            results: vec![RawResult::new("A", "https://a.com", "", "ddg")],
        })];
        let orchestrator = SearchOrchestrator::with_engines(engines);

        // First call — cache miss.
        let output1 = orchestrator.search("rust", &SearchOptions::default()).await;
        assert!(!output1.cached);
        assert_eq!(orchestrator.cache_size(), 1);

        // Second call — cache hit.
        let output2 = orchestrator.search("rust", &SearchOptions::default()).await;
        assert!(output2.cached);
        assert_eq!(output2.merge.results.len(), output1.merge.results.len());
    }

    #[tokio::test]
    async fn test_orchestrator_cache_miss_different_query() {
        let engines: Vec<Arc<dyn SearchEngine>> = vec![Arc::new(MockEngine {
            name: "ddg",
            results: vec![RawResult::new("A", "https://a.com", "", "ddg")],
        })];
        let orchestrator = SearchOrchestrator::with_engines(engines);

        let _ = orchestrator.search("rust", &SearchOptions::default()).await;
        let output = orchestrator
            .search("python", &SearchOptions::default())
            .await;
        assert!(!output.cached);
        assert_eq!(orchestrator.cache_size(), 2);
    }

    #[tokio::test]
    async fn test_orchestrator_cache_miss_different_options() {
        let engines: Vec<Arc<dyn SearchEngine>> = vec![Arc::new(MockEngine {
            name: "ddg",
            results: vec![RawResult::new("A", "https://a.com", "", "ddg")],
        })];
        let orchestrator = SearchOrchestrator::with_engines(engines);

        let _ = orchestrator.search("rust", &SearchOptions::new(5)).await;
        let output = orchestrator.search("rust", &SearchOptions::new(10)).await;
        assert!(!output.cached);
    }

    #[tokio::test]
    async fn test_orchestrator_clear_cache() {
        let engines: Vec<Arc<dyn SearchEngine>> = vec![Arc::new(MockEngine {
            name: "ddg",
            results: vec![RawResult::new("A", "https://a.com", "", "ddg")],
        })];
        let orchestrator = SearchOrchestrator::with_engines(engines);

        let _ = orchestrator.search("rust", &SearchOptions::default()).await;
        assert_eq!(orchestrator.cache_size(), 1);

        orchestrator.clear_cache();
        assert_eq!(orchestrator.cache_size(), 0);

        // Next search should be a cache miss.
        let output = orchestrator.search("rust", &SearchOptions::default()).await;
        assert!(!output.cached);
    }

    #[tokio::test]
    async fn test_orchestrator_max_results_cap() {
        let engines: Vec<Arc<dyn SearchEngine>> = vec![Arc::new(MockEngine {
            name: "ddg",
            results: (0..20)
                .map(|i| RawResult::new(format!("R{i}"), format!("https://r{i}.com"), "", "ddg"))
                .collect(),
        })];
        let orchestrator = SearchOrchestrator::with_engines(engines);

        let output = orchestrator.search("test", &SearchOptions::new(5)).await;

        assert!(output.merge.results.len() <= 5);
    }

    #[test]
    fn test_orchestrator_engine_count() {
        let engines: Vec<Arc<dyn SearchEngine>> = vec![
            Arc::new(MockEngine {
                name: "a",
                results: vec![],
            }),
            Arc::new(MockEngine {
                name: "b",
                results: vec![],
            }),
        ];
        let orchestrator = SearchOrchestrator::with_engines(engines);
        assert_eq!(orchestrator.engine_count(), 2);
    }

    #[test]
    fn test_orchestrator_engine_names() {
        let engines: Vec<Arc<dyn SearchEngine>> = vec![
            Arc::new(MockEngine {
                name: "alpha",
                results: vec![],
            }),
            Arc::new(MockEngine {
                name: "beta",
                results: vec![],
            }),
        ];
        let orchestrator = SearchOrchestrator::with_engines(engines);
        let names = orchestrator.engine_names();
        assert!(names.contains(&"alpha"));
        assert!(names.contains(&"beta"));
    }

    #[test]
    fn test_search_cache_ttl_is_5_minutes() {
        assert_eq!(SEARCH_CACHE_TTL, Duration::from_mins(5));
    }
}
