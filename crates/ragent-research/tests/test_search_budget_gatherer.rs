//! Gatherer-level integration tests for the run-scoped search budget and
//! shared query cache (`crates/ragent-research/src/search_budget.rs`).
//!
//! The budget/cache live on the cloned [`WebGatherer`], so these tests
//! verify the supervisor/competitive wiring semantics: two researchers
//! sharing one `Arc<SearchBudget>` draw from a single pool, and two
//! gatherers sharing one `Arc<SharedQueryCache>` never re-issue an identical
//! query.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ragent_research::search_budget::{SearchBudget, SharedQueryCache};
use ragent_research::web_gatherer::{WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool};

/// Search tool that returns one unique hit per call and records every query.
#[derive(Default)]
struct CountingSearch {
    calls: Mutex<Vec<String>>,
}

impl CountingSearch {
    fn call_count(&self) -> usize {
        self.calls.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    fn queries(&self) -> Vec<String> {
        self.calls.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }
}

#[async_trait]
impl WebSearchTool for CountingSearch {
    async fn search(&self, query: &str, _max_results: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        self.calls
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(query.to_string());
        Ok(vec![WebSearchHit {
            url: format!("https://example.com/{}", query.replace(' ', "-")),
            title: format!("hit for {query}"),
            snippet: String::new(),
            matched_query: query.to_string(),
            search_tool: "test".to_string(),
            search_engine: "test".to_string(),
            author: None,
        }])
    }
}

/// Fetch tool returning a large-enough body for every URL so hits are
/// accepted as sources.
struct AlwaysFetch;

#[async_trait]
impl ragent_research::web_gatherer::WebFetchTool for AlwaysFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        Ok(WebFetchedPage {
            url: url.to_string(),
            title: url.to_string(),
            body: "x".repeat(ragent_research::web_gatherer::MIN_EXTRACTABLE_CONTENT_CHARS * 2),
            published_at: None,
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        })
    }
}

fn gatherer_with(
    search: Arc<CountingSearch>,
    budget: Option<Arc<SearchBudget>>,
    cache: Option<Arc<SharedQueryCache>>,
) -> WebGatherer {
    let g = WebGatherer::new(search, Arc::new(AlwaysFetch))
        .with_decomposer(Arc::new(ragent_research::HeuristicQueryDecomposer));
    let g = match budget {
        Some(b) => g.with_search_budget(b),
        None => g,
    };
    match cache {
        Some(c) => g.with_query_cache(c),
        None => g,
    }
}

#[tokio::test]
async fn test_gatherer_budget_exhaustion_degrades_to_partial_results() {
    let search = Arc::new(CountingSearch::default());
    // No decomposer is registered below, so the topic is a single query;
    // instead simulate a multi-query pass by giving the budget room for
    // exactly one call and issuing two gather passes.
    let budget = Arc::new(SearchBudget::new(Some(1)));
    let g = gatherer_with(search.clone(), Some(budget.clone()), None);

    let first = g.gather("first topic", 10).await.unwrap();
    assert_eq!(first.len(), 1, "first pass should capture its hit");
    assert_eq!(budget.used(), 1);

    // Budget is now exhausted: the second pass must skip its search (no new
    // tool call) and return a partial (empty) result instead of an error.
    let second = g.gather("second topic", 10).await.unwrap();
    assert_eq!(second.len(), 0, "budget exhausted: no new sources");
    assert_eq!(
        search.call_count(),
        1,
        "no search call after budget exhausted"
    );
}

#[tokio::test]
async fn test_gatherer_shared_cache_serves_identical_query_once() {
    let search = Arc::new(CountingSearch::default());
    let cache = Arc::new(SharedQueryCache::new());
    // Two "researchers" = two gatherer clones sharing the same cache, exactly
    // like the supervisor hands its web gatherer to every researcher.
    let researcher_a = gatherer_with(search.clone(), None, Some(cache.clone()));
    let researcher_b = gatherer_with(search.clone(), None, Some(cache.clone()));

    let a = researcher_a.gather("identical topic", 10).await.unwrap();
    let b = researcher_b.gather("identical topic", 10).await.unwrap();

    assert_eq!(a.len(), 1);
    assert_eq!(b.len(), 1, "cache hit still yields the source");
    assert_eq!(
        search.call_count(),
        1,
        "identical query must hit the run cache, not the search tool"
    );
    assert_eq!(search.queries(), vec!["identical topic".to_string()]);
}

#[tokio::test]
async fn test_gatherer_unbudgeted_run_issues_all_searches() {
    // Guard against regressions: with no budget and no cache the gatherer
    // must behave exactly as before (one call per gather pass).
    let search = Arc::new(CountingSearch::default());
    let g = gatherer_with(search.clone(), None, None);
    let _ = g.gather("topic one", 10).await.unwrap();
    let _ = g.gather("topic two", 10).await.unwrap();
    assert_eq!(search.call_count(), 2);
}
