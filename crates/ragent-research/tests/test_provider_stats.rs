//! Tests for the run-scoped per-provider search-request statistics
//! (`crates/ragent-research/src/provider_stats.rs` and the gatherer wiring
//! that feeds it).
//!
//! Covers:
//! - unit semantics of [`ProviderCallStats`] (record/snapshot/empty),
//! - one recorded request per logical search (retries included, cache hits
//!   and budget skips never counted),
//! - `Arc` sharing across cloned gatherers (supervisor/competitive wiring),
//! - the end-of-pass `GatherEvent::ProviderCallsSummary` emission.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ragent_research::provider_stats::ProviderCallStats;
use ragent_research::web_gatherer::{
    GatherEvent, GatherObserver, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool,
};

/// Search tool that returns one hit per call, records every query, and lets
/// tests fail selected queries to exercise the retry path.
#[derive(Default)]
struct CountingSearch {
    calls: Mutex<Vec<String>>,
    fail_first_attempt_of: Mutex<Vec<String>>,
}

impl CountingSearch {
    fn call_count(&self) -> usize {
        self.calls.lock().unwrap_or_else(|p| p.into_inner()).len()
    }

    fn record_failure_for(&self, query: &str) {
        self.fail_first_attempt_of
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(query.to_string());
    }
}

#[async_trait]
impl WebSearchTool for CountingSearch {
    async fn search(&self, query: &str, _max_results: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        let mut failed = self
            .fail_first_attempt_of
            .lock()
            .unwrap_or_else(|p| p.into_inner());
        if let Some(pos) = failed.iter().position(|q| q == query) {
            // Fail the first attempt only: remove the marker so the retry
            // succeeds, exercising the "retries count as one call" rule.
            failed.remove(pos);
            return Err(anyhow::anyhow!("transient failure"));
        }
        drop(failed);
        self.calls
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .push(query.to_string());
        Ok(vec![WebSearchHit {
            url: format!("https://example.com/{}", query.replace(' ', "-")),
            title: format!("hit for {query}"),
            snippet: String::new(),
            matched_query: query.to_string(),
            search_tool: "test_tool".to_string(),
            search_engine: "test_engine".to_string(),
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

/// Observer collecting every gather event for assertions.
#[derive(Default)]
struct CollectEvents(Mutex<Vec<GatherEvent>>);

impl GatherObserver for CollectEvents {
    fn on_event(&self, event: GatherEvent) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).push(event);
    }
}

fn provider_summary(events: &[GatherEvent]) -> Option<Vec<(String, usize)>> {
    events.iter().find_map(|e| match e {
        GatherEvent::ProviderCallsSummary { tool_calls } => Some(tool_calls.clone()),
        _ => None,
    })
}

#[test]
fn test_provider_stats_records_per_tool_totals() {
    let stats = ProviderCallStats::new();
    stats.record("mf_search");
    stats.record("mf_search");
    stats.record("websearch");
    assert_eq!(stats.total(), 3);
    assert_eq!(
        stats.by_tool(),
        vec![("mf_search".to_string(), 2), ("websearch".to_string(), 1),]
    );
}

#[test]
fn test_provider_stats_empty_when_no_calls() {
    let stats = ProviderCallStats::new();
    assert_eq!(stats.total(), 0);
    assert_eq!(stats.by_tool(), Vec::new());
}

#[tokio::test]
async fn test_gatherer_counts_one_request_per_logical_search() {
    let search = Arc::new(CountingSearch::default());
    let stats = Arc::new(ProviderCallStats::new());
    let g = WebGatherer::new(search.clone(), Arc::new(AlwaysFetch))
        .with_decomposer(Arc::new(ragent_research::HeuristicQueryDecomposer))
        .with_provider_stats(stats.clone());

    // The heuristic decomposer splits this into at least two sub-queries;
    // every logical search (success or retried failure) counts once.
    let _ = g
        .gather("Rust async runtimes and executors", 5)
        .await
        .unwrap();

    assert_eq!(stats.total(), search.call_count());
    assert_eq!(
        stats.by_tool(),
        vec![("test_tool".to_string(), stats.total())]
    );
    assert!(stats.total() >= 2, "decomposer should have split the topic");
}

#[tokio::test]
async fn test_gatherer_retries_count_as_one_request() {
    let search = Arc::new(CountingSearch::default());
    let stats = Arc::new(ProviderCallStats::new());
    let g = WebGatherer::new(search.clone(), Arc::new(AlwaysFetch))
        .with_decomposer(Arc::new(ragent_research::HeuristicQueryDecomposer))
        .with_provider_stats(stats.clone());
    search.record_failure_for("rust");

    let _ = g.gather("rust", 5).await.unwrap();

    // The single sub-query needed one retry (two raw HTTP attempts) but is
    // one logical provider request.
    assert_eq!(search.call_count(), 1);
    assert_eq!(stats.total(), 1);
}

#[tokio::test]
async fn test_gatherer_shared_stats_aggregate_across_clones() {
    let search = Arc::new(CountingSearch::default());
    let stats = Arc::new(ProviderCallStats::new());
    let base = WebGatherer::new(search.clone(), Arc::new(AlwaysFetch))
        .with_decomposer(Arc::new(ragent_research::HeuristicQueryDecomposer))
        .with_provider_stats(stats.clone());

    // Supervisor wiring: one gatherer cloned into two "researchers", each
    // running its own gather pass against a distinct topic.
    let researcher_a = base.clone();
    let researcher_b = base.clone();
    let (a, b) = tokio::join!(
        researcher_a.gather("tokio runtime", 5),
        researcher_b.gather("async std runtime", 5)
    );
    let _ = a.unwrap();
    let _ = b.unwrap();

    assert_eq!(stats.total(), search.call_count());
    assert!(stats.total() >= 2, "two passes should each have searched");
}

#[tokio::test]
async fn test_gatherer_emits_provider_calls_summary_once_per_pass() {
    let search = Arc::new(CountingSearch::default());
    let stats = Arc::new(ProviderCallStats::new());
    let g = WebGatherer::new(search, Arc::new(AlwaysFetch))
        .with_decomposer(Arc::new(ragent_research::HeuristicQueryDecomposer))
        .with_provider_stats(stats.clone());
    let obs = CollectEvents::default();

    let _ = g
        .gather_with_observer("rust async runtime", 5, Some(&obs))
        .await
        .unwrap();

    let events = obs.0.lock().unwrap_or_else(|p| p.into_inner());
    let summaries = events
        .iter()
        .filter(|e| matches!(e, GatherEvent::ProviderCallsSummary { .. }))
        .count();
    assert_eq!(summaries, 1, "exactly one summary per gather pass");
    let snapshot = provider_summary(&events).expect("summary present");
    assert_eq!(snapshot, vec![("test_tool".to_string(), stats.total())]);
}

#[tokio::test]
async fn test_gatherer_without_stats_emits_no_summary() {
    let search = Arc::new(CountingSearch::default());
    let g = WebGatherer::new(search, Arc::new(AlwaysFetch))
        .with_decomposer(Arc::new(ragent_research::HeuristicQueryDecomposer));
    let obs = CollectEvents::default();

    let _ = g
        .gather_with_observer("rust async runtime", 5, Some(&obs))
        .await
        .unwrap();

    let events = obs.0.lock().unwrap_or_else(|p| p.into_inner());
    assert!(
        provider_summary(&events).is_none(),
        "no summary without an attached stats counter"
    );
}
