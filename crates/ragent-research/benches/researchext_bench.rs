use async_trait::async_trait;
use criterion::{Criterion, criterion_group, criterion_main};
use ragent_research::analysis::NoopAnalysisEngine;
use ragent_research::engine::{EngineConfig, IterativeEngine};
use ragent_research::planner::HeuristicPlanner;
use ragent_research::session::NoopObserver;
use ragent_research::web_gatherer::{
    WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool,
};
use std::sync::Arc;

/// No-op search tool that always returns one deterministic hit.
struct FixedSearch;

#[async_trait]
impl WebSearchTool for FixedSearch {
    async fn search(&self, _query: &str, _max_results: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        Ok(vec![WebSearchHit {
            url: "https://example.com/fake".into(),
            title: "Fake source".into(),
            snippet: "snippet".into(),
            matched_query: String::new(),
            search_tool: "mf_search".to_string(),
            search_engine: "duckduckgo, brave".to_string(),
        }])
    }
}

/// No-op fetch tool that returns deterministic body text.
struct FixedFetch;

#[async_trait]
impl WebFetchTool for FixedFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        Ok(WebFetchedPage {
            published_at: None,
            url: url.to_string(),
            title: "Fake source".into(),
            body: "This is the body of the fake source used for benchmarking.".into(),
            content_type: None,
            page_type: None,
            language: None,
        })
    }
}

fn researchext_benchmark(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let web = WebGatherer::new(Arc::new(FixedSearch), Arc::new(FixedFetch));
    let engine = IterativeEngine::new(
        Arc::new(HeuristicPlanner::new()),
        Some(web),
        Arc::new(NoopAnalysisEngine),
        Arc::new(ragent_research::engine::SimpleCritic),
        EngineConfig {
            max_iterations: 2,
            max_sources_per_question: 2,
            max_concurrency: 2,
            force_deeper: false,
        },
    );

    c.bench_function("iterative_engine_2_iterations", |b| {
        b.iter(|| {
            let engine = engine.clone();
            rt.block_on(engine.run(
                "Rust async runtimes benchmark topic",
                Arc::new(NoopObserver),
            ))
        });
    });
}

criterion_group!(benches, researchext_benchmark);
criterion_main!(benches);
