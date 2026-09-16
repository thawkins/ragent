//! Tests for T-008: driving engine exclusion from `WebGatherer.disable_scholarly`
//! (spec `researchnoacc`; FR-006, FR-010, FR-015, NFR-001).
//!
//! When `--no-papers` is active the gatherer must steer the search tool away
//! from the academically-classified engines *before* any request is dispatched,
//! rather than querying them and discarding the hits afterwards. Non-academic
//! engines must keep running, and the hit-level scholarly filter stays in place
//! as defence in depth.

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ragent_research::search_budget::SharedQueryCache;
use ragent_research::{WebGatherer, WebSearchHit, WebSearchTool};

/// Topic chosen so the crafted hits pass the gatherer's lexical relevance
/// filter (the snippet repeats the query's terms).
const TOPIC: &str = "rust async runtime";

/// Build a hit whose snippet is long enough to clear the scholarly /
/// encyclopedia minimum-content guards so it is captured.
fn hit(url: &str, engine: &str) -> WebSearchHit {
    WebSearchHit {
        url: url.to_string(),
        title: format!("{TOPIC} {engine}"),
        snippet: format!(
            "{TOPIC} scheduling throughput improvement across engines {engine} \
             with a reusable executor that reduces contention in production"
        ),
        matched_query: TOPIC.to_string(),
        search_tool: "mf_search".to_string(),
        search_engine: engine.to_string(),
        author: None,
    }
}

/// Search double that distinguishes the base search from the exclusion-aware
/// search and records exactly what each call received.
#[derive(Default)]
struct RecordingSearch {
    /// Queries received by the base `search` method.
    base_calls: Mutex<Vec<String>>,
    /// `(query, exclusions)` received by `search_with_exclusions`.
    exclusion_calls: Mutex<Vec<(String, Vec<String>)>>,
}

impl RecordingSearch {
    fn base_call_count(&self) -> usize {
        self.base_calls.lock().unwrap().len()
    }

    fn exclusion_calls(&self) -> Vec<(String, Vec<String>)> {
        self.exclusion_calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl WebSearchTool for RecordingSearch {
    async fn search(&self, query: &str, _max: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        self.base_calls.lock().unwrap().push(query.to_string());
        // The unfiltered sweep returns both the academic and the general-web
        // hit, so a regression that skips exclusion becomes observable (an
        // academic source would appear in the result).
        Ok(vec![
            hit("https://openalex.example/work", "openalex"),
            hit("https://wiki.example/runtime", "wikipedia"),
        ])
    }

    async fn search_with_exclusions(
        &self,
        query: &str,
        _max: usize,
        exclude_engines: &[&str],
    ) -> anyhow::Result<Vec<WebSearchHit>> {
        self.exclusion_calls.lock().unwrap().push((
            query.to_string(),
            exclude_engines.iter().map(|s| s.to_string()).collect(),
        ));
        // A correctly steered backend never queried openalex, so only the
        // general-web hit exists to return.
        Ok(vec![hit("https://wiki.example/runtime", "wikipedia")])
    }
}

fn gatherer(search: Arc<RecordingSearch>) -> WebGatherer {
    WebGatherer::new(search, Arc::new(NoFetch))
}

/// The scholarly and encyclopedia hits are captured from their snippet without
/// a URL fetch, so the fetch tool is never expected to be called here.
struct NoFetch;

#[async_trait]
impl ragent_research::WebFetchTool for NoFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<ragent_research::WebFetchedPage> {
        anyhow::bail!("unexpected fetch of {url}: these hits should be synthesized")
    }
}

#[tokio::test]
async fn no_papers_excludes_academic_engines_before_search() {
    // FR-006 / NFR-001: with --no-papers set, the academic engine name must be
    // pushed through the trait so OpenAlex is never queried, and the base
    // (unfiltered) search must not be used at all.
    let search = Arc::new(RecordingSearch::default());
    let web = gatherer(search.clone()).with_disable_scholarly(true);

    let sources = web.gather(TOPIC, 5).await.expect("gather should succeed");

    assert_eq!(
        search.base_call_count(),
        0,
        "the unfiltered search must not run while --no-papers is active"
    );
    let exclusions = search.exclusion_calls();
    assert!(
        !exclusions.is_empty(),
        "the exclusion-aware search must be used"
    );
    for (query, engines) in &exclusions {
        assert_eq!(query, TOPIC);
        assert_eq!(
            engines,
            &vec!["openalex".to_string()],
            "the academic engine set must be forwarded verbatim"
        );
    }

    // FR-010: the non-academic engine still yields a captured source, and the
    // academic source is gone.
    assert_eq!(
        sources.len(),
        1,
        "only the wikipedia source should remain: {sources:?}"
    );
    assert!(
        sources
            .iter()
            .all(|s| !s.path_or_url().contains("openalex.example")),
        "no academic source may be captured"
    );
}

#[tokio::test]
async fn without_no_papers_search_is_unchanged() {
    // FR-011 / FR-015: the default path issues the plain search and does not
    // exclude anything, so academic and general-web engines both run.
    let search = Arc::new(RecordingSearch::default());
    let web = gatherer(search.clone());

    let sources = web.gather(TOPIC, 5).await.expect("gather should succeed");

    assert_eq!(
        search.base_call_count(),
        1,
        "the base search must run exactly once when nothing is excluded"
    );
    assert!(
        search.exclusion_calls().is_empty(),
        "no exclusion-aware search should be issued without --no-papers"
    );
    assert_eq!(
        sources.len(),
        2,
        "both academic and general-web sources must flow through when --no-papers is off: {sources:?}"
    );
    assert!(
        sources
            .iter()
            .any(|s| s.path_or_url().contains("openalex.example")),
        "academic hits must flow through normally when --no-papers is off"
    );
}

#[tokio::test]
async fn shared_cache_does_not_leak_filtered_results_into_unfiltered_gathers() {
    // Defence in depth: the exclusion set is folded into the cache key, so a
    // filtered sweep cannot satisfy an unfiltered one (or vice versa) when the
    // same cache is shared across a run.
    let search = Arc::new(RecordingSearch::default());
    let cache = Arc::new(SharedQueryCache::new());

    let filtered = gatherer(search.clone())
        .with_disable_scholarly(true)
        .with_query_cache(cache.clone());
    filtered
        .gather(TOPIC, 5)
        .await
        .expect("filtered gather should succeed");
    assert_eq!(search.exclusion_calls().len(), 1);

    let unfiltered = gatherer(search.clone()).with_query_cache(cache);
    let sources = unfiltered
        .gather(TOPIC, 5)
        .await
        .expect("unfiltered gather should succeed");

    assert_eq!(
        search.base_call_count(),
        1,
        "the unfiltered gather must not be served the cached filtered result"
    );
    assert!(
        sources
            .iter()
            .any(|s| s.path_or_url().contains("openalex.example")),
        "the unfiltered gather must see academic sources"
    );
}
