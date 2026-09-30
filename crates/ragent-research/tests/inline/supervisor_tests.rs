//! Inline tests for `supervisor.rs` - compiled inside the module tree
//! via the `#[path]` delegation left in the source file
//! (ANTIPAT M2 inline-test migration; no behaviour change).

use super::*;
use crate::analysis::{AnalysisEngine, AnalysisResult, SourceBody};
use crate::session::{NoopObserver, SessionObserver};
use crate::source_vault::SourceVault;
use crate::web_gatherer::{WebFetchTool, WebFetchedPage, WebSearchHit, WebSearchTool};
use async_trait::async_trait;
use std::sync::Mutex;

/// In-memory search tool that returns a fixed list of hits on every call.
struct FakeSearch {
    hits: Vec<WebSearchHit>,
}

#[async_trait]
impl WebSearchTool for FakeSearch {
    async fn search(&self, _query: &str, _max_results: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        Ok(self.hits.clone())
    }
}

/// In-memory fetch tool that returns deterministic page bodies.
struct FakeFetch;

#[async_trait]
impl WebFetchTool for FakeFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        let body = format!(
            "Comprehensive article about {url}. Tokio is an asynchronous runtime for the Rust \
             programming language. It provides the building blocks needed for writing network \
             applications. This text contains more than two hundred and fifty six characters so \
             that the minimum content length threshold used by the web gatherer is comfortably \
             satisfied and the source is not excluded during post-fetch filtering."
        );
        Ok(WebFetchedPage {
            url: url.to_string(),
            title: format!("Title for {url}"),
            body: body.into(),
            published_at: None,
            content_type: None,
            page_type: None,
            language: Some("english".to_string()),
            author: None,
        })
    }
}

/// Observer that records all events for inspection.
struct CollectEvents(Mutex<Vec<SessionEvent>>);

impl SessionObserver for CollectEvents {
    fn on_event(&self, event: SessionEvent) {
        self.0.lock().unwrap_or_else(|p| p.into_inner()).push(event);
    }
}

fn web_gatherer_with_search_hits(hits: Vec<WebSearchHit>) -> WebGatherer {
    WebGatherer::new(Arc::new(FakeSearch { hits }), Arc::new(FakeFetch))
        .with_keep_low_relevance(true)
}

#[tokio::test]
async fn iterative_researcher_node_captures_sources_and_emits_notes() {
    let hit = WebSearchHit {
        url: "https://example.com/tokio".to_string(),
        title: "Tokio async runtime".to_string(),
        snippet: "A runtime for writing reliable network applications".to_string(),
        matched_query: "Tokio async runtime".to_string(),
        search_tool: "fake".to_string(),
        search_engine: "fake".to_string(),
        author: None,
    };
    let web = web_gatherer_with_search_hits(vec![hit]);
    let analysis: Arc<dyn AnalysisEngine> = Arc::new(crate::analysis::NoopAnalysisEngine);
    let node = IterativeResearcherNode::new(Some(web), analysis);
    let observer = Arc::new(CollectEvents(Mutex::new(Vec::new())));

    let (sources, summary) = node
        .research("r1", "What is Tokio?", observer.clone())
        .await
        .expect("research should succeed");

    assert!(!sources.is_empty(), "researcher should capture sources");
    assert!(
        summary.contains("Researcher r1: What is Tokio?"),
        "summary should identify researcher and sub-topic"
    );
    assert!(
        summary.contains("Captured sources"),
        "summary should include captured sources section"
    );

    let events = observer.0.lock().unwrap();
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::ResearcherProgress { id, status, .. } if id == "r1" && status == "done"
        )),
        "should emit final researcher progress event"
    );
    assert!(
        events.iter().any(|e| matches!(
            e,
            SessionEvent::ResearcherNote { id, .. } if id == "r1"
        )),
        "should emit structured notes for captured sources"
    );
}

#[tokio::test]
async fn iterative_researcher_node_returns_empty_summary_when_web_disabled() {
    let analysis: Arc<dyn AnalysisEngine> = Arc::new(crate::analysis::NoopAnalysisEngine);
    let node = IterativeResearcherNode::new(None, analysis);
    let observer: Arc<dyn SessionObserver> = Arc::new(NoopObserver);

    let (sources, summary) = node
        .research("r2", "What is async-std?", observer)
        .await
        .expect("research should succeed with no web");

    assert_eq!(sources.len(), 0);
    assert!(summary.contains("Researcher r2: What is async-std?"));
    assert!(summary.contains("captured 0 sources"));
}

#[tokio::test]
async fn iterative_researcher_node_includes_brief_and_model_in_summary() {
    let hit = WebSearchHit {
        url: "https://example.com/runtime".to_string(),
        title: "Async runtime".to_string(),
        snippet: "Async runtime overview".to_string(),
        matched_query: "async runtime".to_string(),
        search_tool: "fake".to_string(),
        search_engine: "fake".to_string(),
        author: None,
    };
    let web = web_gatherer_with_search_hits(vec![hit]);
    let analysis: Arc<dyn AnalysisEngine> = Arc::new(crate::analysis::NoopAnalysisEngine);
    let node = IterativeResearcherNode::new(Some(web), analysis)
        .with_brief(Some("Compare Rust async runtimes".to_string()))
        .with_research_model(Some("anthropic:claude-sonnet-4".to_string()));
    let observer: Arc<dyn SessionObserver> = Arc::new(NoopObserver);

    let (_sources, summary) = node
        .research("r3", "What is smol?", observer)
        .await
        .unwrap();

    assert!(summary.contains("Mission: Compare Rust async runtimes"));
    assert!(summary.contains("Model: anthropic:claude-sonnet-4"));
}

/// In-memory analysis engine that returns deterministic compressed output.
#[derive(Debug, Default, Clone, Copy)]
struct FakeAnalysisEngine;

#[async_trait]
impl AnalysisEngine for FakeAnalysisEngine {
    async fn analyze(
        &self,
        _topic: &str,
        _sources: &[SourceBody],
    ) -> anyhow::Result<AnalysisResult> {
        Ok(AnalysisResult {
            summary: "Tokio is the dominant Rust async runtime.".to_string(),
            findings: vec!["Tokio provides an executor and reactor.".to_string()],
            top_implications: vec!["Most projects choose Tokio.".to_string()],
            open_questions: vec!["How does Tokio compare to async-std?".to_string()],
            ..AnalysisResult::default()
        })
    }

    fn with_brief(&self, _brief: Option<String>) -> Arc<dyn AnalysisEngine> {
        Arc::new(*self)
    }
}

#[tokio::test]
async fn iterative_researcher_node_compresses_findings_with_citations() {
    let hit = WebSearchHit {
        url: "https://example.com/tokio".to_string(),
        title: "Tokio async runtime".to_string(),
        snippet: "A runtime for writing reliable network applications".to_string(),
        matched_query: "Tokio async runtime".to_string(),
        search_tool: "fake".to_string(),
        search_engine: "fake".to_string(),
        author: None,
    };
    let web = web_gatherer_with_search_hits(vec![hit]);
    let analysis: Arc<dyn AnalysisEngine> = Arc::new(FakeAnalysisEngine);
    let node = IterativeResearcherNode::new(Some(web), analysis)
        .with_brief(Some("Compare Rust async runtimes".to_string()));
    let observer: Arc<dyn SessionObserver> = Arc::new(NoopObserver);

    let (sources, summary) = node
        .research("r-compress", "What is Tokio?", observer)
        .await
        .unwrap();

    assert!(!sources.is_empty(), "should capture sources");
    assert!(
        summary.contains("Tokio is the dominant Rust async runtime."),
        "summary missing LLM-compressed summary:\n{summary}"
    );
    assert!(
        summary.contains("Tokio provides an executor and reactor."),
        "summary missing finding:\n{summary}"
    );
    assert!(
        summary.contains("Most projects choose Tokio."),
        "summary missing implication:\n{summary}"
    );
    assert!(
        summary.contains("[#1]"),
        "summary missing citation marker:\n{summary}"
    );
    assert!(
        summary.contains("https://example.com/tokio"),
        "summary missing original source URL:\n{summary}"
    );
}

#[tokio::test]
async fn iterative_researcher_node_persists_sources_to_vault() {
    let tmp = tempfile::tempdir().unwrap();
    let project_root = tmp.path();
    let vault = SourceVault::open(project_root, "vault-run").unwrap();

    let hit = WebSearchHit {
        url: "https://example.com/tokio".to_string(),
        title: "Tokio async runtime".to_string(),
        snippet: "A runtime for writing reliable network applications".to_string(),
        matched_query: "Tokio async runtime".to_string(),
        search_tool: "fake".to_string(),
        search_engine: "fake".to_string(),
        author: None,
    };
    let web = web_gatherer_with_search_hits(vec![hit]);
    let analysis: Arc<dyn AnalysisEngine> = Arc::new(crate::analysis::NoopAnalysisEngine);
    let node = IterativeResearcherNode::new(Some(web), analysis).with_vault(Some(Arc::new(vault)));
    let observer: Arc<dyn SessionObserver> = Arc::new(NoopObserver);

    let (sources, _summary) = node
        .research("r-vault", "What is Tokio?", observer)
        .await
        .unwrap();

    assert_eq!(sources.len(), 1, "expected one captured source");

    let reopened = SourceVault::open(project_root, "vault-run").unwrap();
    let stored = reopened.list(10).unwrap();
    assert_eq!(stored.len(), 1, "vault should contain one persisted source");
    assert!(stored[0].url.contains("example.com/tokio"));
    assert!(
        !stored[0].body_text.is_empty(),
        "stored body should not be empty"
    );
}

#[test]
fn supervisor_state_tracks_assignments() {
    let mut state = SupervisorState::new("Rust async runtimes");
    state.add_sub_topic("What is Tokio?");
    state.add_sub_topic("What is async-std?");
    assert_eq!(state.assignments.len(), 2);
    assert_eq!(state.pending().len(), 2);

    state.set_in_progress("researcher-1");
    state.set_completed("researcher-1", vec![], "done");
    assert_eq!(state.completed().len(), 1);
    assert!(state.pending().iter().any(|a| a.id == "researcher-2"));
}

#[tokio::test]
async fn supervisor_node_plans_sub_topics() {
    let supervisor = SupervisorNode::new(Arc::new(HeuristicPlanner::new())).with_max_sub_topics(3);
    let topics = supervisor.plan("Rust async runtimes").await.unwrap();
    assert_ne!(topics, Vec::<String>::new());
    assert!(topics.len() <= 3);
}

#[test]
fn build_competitive_sub_topics_creates_one_question_per_entity() {
    let entities = vec![
        crate::entities::CompetitiveEntity {
            name: "Fireworks AI".to_string(),
            category: Some("inference provider".to_string()),
        },
        crate::entities::CompetitiveEntity {
            name: "Groq".to_string(),
            category: Some("inference provider".to_string()),
        },
    ];
    let criteria = vec!["LLM inference".to_string()];
    let topics = build_competitive_sub_topics(&entities, &criteria);
    assert_eq!(topics.len(), 2);
    assert!(topics[0].starts_with("Research Fireworks AI (inference provider)"));
    assert!(topics[0].contains("LLM inference"));
    assert!(topics[1].starts_with("Research Groq (inference provider)"));
    // The overall topic must not be embedded so each researcher's
    // searches stay scoped to its own entity.
    assert!(!topics[0].contains("Compare inference providers"));
    assert!(!topics[1].contains("Compare inference providers"));
}

#[test]
fn sub_topic_matches_entity_anchors_on_prefix() {
    // The criteria clause may name other entities, so a substring match
    // would misattribute; the prefix anchor must select only the exact
    // entity, with a token boundary so prefix-colliding names are safe.
    let topic = "Research Together.ai (inference provider) across \
         dimensions: LLM inference, pricing";
    assert!(sub_topic_matches_entity(topic, "Together.ai"));
    assert!(!sub_topic_matches_entity(topic, "Fireworks AI"));
    assert!(!sub_topic_matches_entity(topic, "Groq"));
    // Token boundary: entity `Groq` must not match a sub-topic for
    // `Groq Cloud` and vice versa.
    assert!(!sub_topic_matches_entity(
        "Research Groq Cloud across dimensions: latency",
        "Groq"
    ));
    assert!(!sub_topic_matches_entity(
        "Research Groq across dimensions: latency",
        "Groq Cloud"
    ));
}

#[test]
fn competitive_entity_name_extracts_entity_from_sub_topic() {
    assert_eq!(
        competitive_entity_name("Research Groq (inference provider) across dimensions: x"),
        Some("Groq".to_string())
    );
    assert_eq!(
        competitive_entity_name("Research GitHub Copilot across dimensions: x"),
        Some("GitHub Copilot".to_string())
    );
    assert_eq!(competitive_entity_name("What is the state of Rust?"), None);
    assert_eq!(competitive_entity_name("Research "), None);
}

#[test]
fn build_summary_fallback_filters_findings_by_entity() {
    let mut state = ResearchState::new("compare AlphaAgent and BetaAgent");
    state.add_source(sample_web_source(
        "Beta review",
        "BetaAgent is a cloud platform with many tools.",
    ));
    state.add_source(sample_web_source(
        "Alpha review",
        "AlphaAgent is a fast local terminal agent.",
    ));
    let summary = build_summary("researcher-1", "Research AlphaAgent", &state, None, None);
    let findings_idx = summary.find("## Findings").expect("findings block");
    let findings = &summary[findings_idx..];
    assert!(findings.contains("Alpha review"));
    assert!(!findings.contains("Beta review"));
}

/// Deterministic web source with a given title and body text.
fn sample_web_source(title: &str, body: &str) -> Source {
    Source::Web {
        url: format!("https://example.com/{title}"),
        title: title.to_string(),
        captured_at: chrono::Utc::now(),
        published_at: None,
        body_path: std::path::PathBuf::from("sources/web-01.md"),
        body: body.to_string(),
        relevance: String::new(),
        search_tool: String::new(),
        search_engine: String::new(),
        content_type: None,
        page_type: None,
        media_type: "page".to_string(),
        language: None,
        author: None,
        oa_recovery: None,
    }
}

#[test]
fn build_competitive_sub_topics_returns_empty_for_no_entities() {
    let topics = build_competitive_sub_topics(&[], &[]);
    assert_eq!(topics, Vec::<String>::new());
}
