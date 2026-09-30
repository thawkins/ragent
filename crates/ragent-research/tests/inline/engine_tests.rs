//! Inline tests for `engine.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::analysis::NoopAnalysisEngine;
use crate::planner::HeuristicPlanner;
use crate::session::{NoopObserver, SessionEvent};
use crate::web_gatherer::{WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool};
use std::collections::VecDeque;
use std::sync::Mutex;

#[derive(Debug, Default)]
struct FakeSearch {
    hits: Mutex<VecDeque<Vec<WebSearchHit>>>,
}

#[async_trait]
impl WebSearchTool for FakeSearch {
    async fn search(&self, query: &str, _max_results: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        let mut hits = self.hits.lock().unwrap();
        let mut out = hits.pop_front().unwrap_or_default();
        // Keep synthetic hits from being discarded by the low-relevance
        // guard when tests leave the snippet blank.
        for hit in &mut out {
            if hit.snippet.is_empty() {
                hit.snippet = query.to_string();
            }
        }
        Ok(out)
    }
}

#[derive(Debug, Default)]
struct FakeFetch;

#[async_trait]
impl WebFetchTool for FakeFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        Ok(WebFetchedPage {
            published_at: None,
            url: url.to_string(),
            title: "fake".to_string(),
            body: Arc::from("body text ".repeat(30)),
            content_type: None,
            page_type: None,
            language: None,
            author: None,
        })
    }
}

fn engine_with_fake(hits: Vec<Vec<WebSearchHit>>) -> IterativeEngine {
    let search = Arc::new(FakeSearch {
        hits: Mutex::new(hits.into_iter().collect()),
    });
    let fetch: Arc<dyn WebFetchTool> = Arc::new(FakeFetch);
    let web = WebGatherer::new(search, fetch);
    IterativeEngine::new(
        Arc::new(HeuristicPlanner::new()),
        Some(web),
        Arc::new(NoopAnalysisEngine),
        Arc::new(SimpleCritic),
        EngineConfig {
            max_iterations: 2,
            max_sources_per_question: 2,
            max_concurrency: 2,
            force_deeper: false,
        },
    )
}

#[tokio::test]
async fn engine_plans_and_answers_pending_questions() {
    let engine = engine_with_fake(vec![]);
    let state = engine
        .run("Rust macros", Arc::new(NoopObserver))
        .await
        .unwrap();
    assert!(!state.plan.sub_questions.is_empty());
    assert!(
        state
            .plan
            .sub_questions
            .iter()
            .all(|sq| sq.status == SubQuestionStatus::Answered)
    );
}

#[tokio::test]
async fn engine_captures_sources_and_emits_events() {
    let hits = vec![vec![WebSearchHit {
        url: "https://rust-lang.org".to_string(),
        title: "Rust".to_string(),
        snippet: String::new(),
        matched_query: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        author: None,
    }]];
    let engine = engine_with_fake(hits);
    let state = engine.run("Rust", Arc::new(NoopObserver)).await.unwrap();
    assert!(!state.sources.is_empty());
}

#[derive(Default)]
struct CollectObserver {
    events: Mutex<Vec<SessionEvent>>,
}

impl SessionObserver for CollectObserver {
    fn on_event(&self, event: SessionEvent) {
        self.events.lock().unwrap().push(event);
    }
}

#[tokio::test]
async fn engine_emits_plan_updated_and_iteration_events() {
    let engine = engine_with_fake(vec![]);
    let observer = Arc::new(CollectObserver::default());
    engine.run("async Rust", observer.clone()).await.unwrap();
    let events = observer.events.lock().unwrap().clone();
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SessionEvent::PlanUpdated { .. }))
    );
    assert!(
        events
            .iter()
            .any(|e| matches!(e, SessionEvent::IterationCompleted { .. }))
    );
}

#[tokio::test]
async fn engine_adds_gap_when_no_sources() {
    let engine = engine_with_fake(vec![]);
    let state = engine
        .run("obscure topic", Arc::new(NoopObserver))
        .await
        .unwrap();
    assert!(!state.gaps.is_empty());
    assert!(
        state
            .gaps
            .iter()
            .any(|g| g.description.contains("No direct source"))
    );
}
