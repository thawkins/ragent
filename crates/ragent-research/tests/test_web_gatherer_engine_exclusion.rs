//! Engine-level exclusion tests for the research web gatherer (spec
//! `researchnoacc`, T-013; FR-006, FR-010, NFR-003).
//!
//! T-008 proved that `WebGatherer.disable_scholarly` forwards the academic
//! engine set through the `WebSearchTool` trait to a recording double. These
//! tests close the loop one layer down: the gatherer runs against a
//! `WebSearchTool` backed by the **real** `SearchOrchestrator` and counting
//! mock engines, so a regression anywhere in the
//! gatherer -> trait -> orchestrator -> engine chain is observable.
//!
//! The key guarantees under test:
//!
//! - NFR-001: with `--no-papers` active the excluded (OpenAlex) engine's
//!   `search` method is never invoked, so no request is dispatched.
//! - FR-010: the non-academic (Wikipedia) engine keeps running and its results
//!   still gather.
//! - NFR-003: with no exclusion the default path queries every engine exactly
//!   as before.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;

use ragent_research::{WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool};
use ragent_tools_extended::masterfetch::search::{
    EngineReport, RawResult, SearchEngine, SearchOptions, SearchOrchestrator, SearchOutput,
};

/// Topic repeated in every crafted snippet so the hits clear the gatherer
/// content-length guards when the engine-ranked capture paths apply.
const TOPIC: &str = "rust async runtime";

/// A mock search engine that counts how many times `search` is invoked.
///
/// The count is the decisive assertion: an engine that is excluded before the
/// search runs must register zero calls.
struct CountingEngine {
    name: &'static str,
    calls: Arc<AtomicUsize>,
    result: RawResult,
}

#[async_trait]
impl SearchEngine for CountingEngine {
    fn name(&self) -> &str {
        self.name
    }

    async fn search(&self, _query: &str, _opts: &SearchOptions) -> EngineReport {
        self.calls.fetch_add(1, Ordering::SeqCst);
        EngineReport::ok(self.name, vec![self.result.clone()])
    }
}

/// Build a mock engine returning one result sourced under `name` with a
/// snippet long enough to survive the scholarly/encyclopedia content guards.
fn counting_engine(
    name: &'static str,
    calls: Arc<AtomicUsize>,
    url: &str,
) -> Arc<dyn SearchEngine> {
    Arc::new(CountingEngine {
        name,
        calls,
        result: RawResult::new(
            format!("{TOPIC} {name}"),
            url,
            format!(
                "{TOPIC} scheduling throughput improvement across engines {name} \
                 with a reusable executor that reduces contention in production"
            ),
            name,
        ),
    })
}

/// A `WebSearchTool` backed by the real `SearchOrchestrator`.
///
/// The base path runs the orchestrator unchanged; the exclusion-aware path
/// applies the orchestrator's `exclude_engines` before searching, exactly as
/// the production `AgentWebSearchTool` maps the research exclusion onto the
/// `mf_search` `exclude_engines` parameter.
struct OrchestratorSearch {
    orchestrator: SearchOrchestrator,
}

fn hits_from_output(output: SearchOutput) -> Vec<WebSearchHit> {
    output
        .merge
        .results
        .into_iter()
        .map(|r| WebSearchHit {
            url: r.url,
            title: r.title,
            snippet: r.snippet,
            matched_query: String::new(),
            search_tool: "mf_search".to_string(),
            search_engine: r.source,
            author: r.author,
        })
        .collect()
}

#[async_trait]
impl WebSearchTool for OrchestratorSearch {
    async fn search(&self, query: &str, max_results: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        let output = self
            .orchestrator
            .search(query, &SearchOptions::new(max_results))
            .await;
        Ok(hits_from_output(output))
    }

    async fn search_with_exclusions(
        &self,
        query: &str,
        max_results: usize,
        exclude_engines: &[&str],
    ) -> anyhow::Result<Vec<WebSearchHit>> {
        let filtered = self.orchestrator.exclude_engines(exclude_engines);
        let output = filtered
            .search(query, &SearchOptions::new(max_results))
            .await;
        Ok(hits_from_output(output))
    }
}

/// The scholarly and encyclopedia hits are captured from their snippet without
/// a URL fetch, so the fetch tool is never expected to be called here.
struct NoFetch;

#[async_trait]
impl WebFetchTool for NoFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        anyhow::bail!("unexpected fetch of {url}: these hits should be synthesized")
    }
}

/// Build a gatherer wired to the real orchestrator plus the two counters.
fn gatherer() -> (WebGatherer, Arc<AtomicUsize>, Arc<AtomicUsize>) {
    let openalex_calls = Arc::new(AtomicUsize::new(0));
    let wikipedia_calls = Arc::new(AtomicUsize::new(0));
    let orchestrator = SearchOrchestrator::with_engines(vec![
        counting_engine(
            "openalex",
            Arc::clone(&openalex_calls),
            "https://openalex.example/work",
        ),
        counting_engine(
            "wikipedia",
            Arc::clone(&wikipedia_calls),
            "https://wiki.example/runtime",
        ),
    ]);
    let search = Arc::new(OrchestratorSearch { orchestrator });
    let web = WebGatherer::new(search, Arc::new(NoFetch));
    (web, openalex_calls, wikipedia_calls)
}

#[tokio::test]
async fn no_papers_never_queries_excluded_engine_and_still_gathers_web() {
    // FR-006 / NFR-001: with --no-papers the OpenAlex engine must never be
    // queried (zero `search` calls), while Wikipedia (FR-010) still runs and
    // its source is captured.
    let (web, openalex_calls, wikipedia_calls) = gatherer();
    let web = web.with_disable_scholarly(true);

    let sources = web.gather(TOPIC, 10).await.expect("gather should succeed");

    assert_eq!(
        openalex_calls.load(Ordering::SeqCst),
        0,
        "the excluded OpenAlex engine must never be queried (NFR-001)"
    );
    assert!(
        wikipedia_calls.load(Ordering::SeqCst) >= 1,
        "the non-academic Wikipedia engine must still run (FR-010)"
    );
    assert!(
        sources
            .iter()
            .any(|s| s.path_or_url().contains("wiki.example")),
        "Wikipedia sources must still gather: {sources:?}"
    );
    assert!(
        sources
            .iter()
            .all(|s| !s.path_or_url().contains("openalex.example")),
        "no OpenAlex source may be captured: {sources:?}"
    );
}

#[tokio::test]
async fn without_no_papers_every_engine_is_queried() {
    // NFR-003 baseline: with no exclusion the default path queries both engines
    // and both sources flow through unchanged.
    let (web, openalex_calls, wikipedia_calls) = gatherer();

    let sources = web.gather(TOPIC, 10).await.expect("gather should succeed");

    assert!(
        openalex_calls.load(Ordering::SeqCst) >= 1,
        "the OpenAlex engine must run when --no-papers is off"
    );
    assert!(
        wikipedia_calls.load(Ordering::SeqCst) >= 1,
        "the Wikipedia engine must run when --no-papers is off"
    );
    assert!(
        sources
            .iter()
            .any(|s| s.path_or_url().contains("openalex.example")),
        "OpenAlex sources must flow through by default: {sources:?}"
    );
    assert!(
        sources
            .iter()
            .any(|s| s.path_or_url().contains("wiki.example")),
        "Wikipedia sources must flow through by default: {sources:?}"
    );
}
