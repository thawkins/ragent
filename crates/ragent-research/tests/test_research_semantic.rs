//! Integration test for T-012: semantic-retrieval augmentation of `/research`.
//!
//! Exercises the full `ResearchSession::run` pipeline with a mock
//! [`SemanticResearchAugmentor`] that injects a prior research finding which a
//! pure keyword (FTS5/`search`) search would **not** return — satisfying
//! NFR-006's "semantically relevant finding that a pure keyword search would
//! miss" requirement.
//!
//! The mock augmentor:
//! - `retrieve_for_topic` returns a hit about "vector databases" for a topic
//!   that contains the word "embeddings" but NOT "vector" or "database" — so
//!   the hit is semantically relevant but lexically disjoint.
//! - `index_sources` and `index_document` are no-ops (recorded) so the test
//!   asserts the session calls them without needing a real vector store.
//!
//! No network and no real LLM are required: web gathering is faked and
//! local gathering is disabled.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ragent_research::{
    AnalysisEngine, AnalysisOutcome, AnalysisResult, NoopAnalysisEngine, OutputFormat,
    ResearchManager, ResearchSession, SemanticHit, SemanticHitKind, SemanticResearchAugmentor,
    SessionConfig, SessionEvent, SessionObserver, Source, SynthesizeOutcome, WebFetchTool,
    WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool,
};
use tempfile::TempDir;

// ── Mock semantic augmentor ──────────────────────────────────────────────

/// A mock [`SemanticResearchAugmentor`] that records every call and returns a
/// single semantically-relevant (but lexically disjoint) hit on retrieve.
#[derive(Clone, Default)]
struct MockSemanticAugmentor {
    index_sources_calls: Arc<Mutex<Vec<usize>>>,
    index_document_calls: Arc<Mutex<Vec<(String, String, String, usize)>>>,
    retrieve_calls: Arc<Mutex<Vec<(String, usize)>>>,
}

#[async_trait]
impl SemanticResearchAugmentor for MockSemanticAugmentor {
    async fn index_sources(&self, sources: &[Source]) -> anyhow::Result<usize> {
        self.index_sources_calls.lock().unwrap().push(sources.len());
        Ok(sources.len())
    }

    async fn index_document(
        &self,
        name: &str,
        title: &str,
        topic: &str,
        sources: &[Source],
    ) -> anyhow::Result<usize> {
        self.index_document_calls.lock().unwrap().push((
            name.to_string(),
            title.to_string(),
            topic.to_string(),
            sources.len(),
        ));
        Ok(1)
    }

    async fn retrieve_for_topic(&self, topic: &str, n: usize) -> anyhow::Result<Vec<SemanticHit>> {
        self.retrieve_calls
            .lock()
            .unwrap()
            .push((topic.to_string(), n));
        // Return one hit that is SEMANTICALLY related to embeddings but uses
        // none of the topic's keywords — so a keyword/FTS search over the
        // prior-research corpus would miss it.
        Ok(vec![SemanticHit {
            id: "prior:db-engines:topic".to_string(),
            score: 0.91,
            kind: SemanticHitKind::PriorTopic,
            title: "Vector databases for similarity search".to_string(),
            snippet: "Specialised vector databases (Qdrant, LanceDB) store embeddings and retrieve by cosine similarity.".to_string(),
            payload: serde_json::json!({"name": "db-engines"}),
        }])
    }
}

// ── Mock web gatherer (no network) ───────────────────────────────────────

struct NoSearch;

#[async_trait]
impl WebSearchTool for NoSearch {
    async fn search(&self, _: &str, _: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        Ok(Vec::new())
    }
}

struct PageFetch;

#[async_trait]
impl WebFetchTool for PageFetch {
    async fn fetch(&self, _: &str) -> anyhow::Result<WebFetchedPage> {
        Ok(WebFetchedPage {
            url: String::new(),
            title: String::new(),
            body: String::new(),
            published_at: None,
        })
    }
}

// ── Recording observer ──────────────────────────────��───────────────────

#[derive(Clone, Default)]
struct RecordingObserver {
    events: Arc<Mutex<Vec<SessionEvent>>>,
}

impl SessionObserver for RecordingObserver {
    fn on_event(&self, event: SessionEvent) {
        self.events.lock().unwrap().push(event);
    }
}

// ── Test ─────────────────────────────────────────────────────────────────

/// T-012 / NFR-006: a research run with a semantic augmentor injects a
/// semantically-relevant prior source that a keyword search would miss, emits
/// a `SemanticSourceRetrieved` event, and calls `index_sources` +
/// `index_document`.
#[tokio::test]
async fn semantic_augmentor_injects_lexically_disjoint_finding() {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(&root).unwrap();
    let manager = ResearchManager::new(root.clone());

    let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(PageFetch));
    let analysis: Arc<dyn AnalysisEngine> = Arc::new(NoopAnalysisEngine);
    let augmentor = Arc::new(MockSemanticAugmentor::default());

    let session = ResearchSession::new(manager, Some(web), None, analysis)
        .with_semantic_augmentor(augmentor.clone());

    let observer = RecordingObserver::default();
    // Topic mentions "embeddings" but NOT "vector" or "database".
    let config = SessionConfig {
        topic: "choosing an embeddings model for semantic search".to_string(),
        max_web_results: 0,
        disable_local: true,
        disable_specs: true,
        ..Default::default()
    };

    let outcome = session
        .run(
            "embeddings-choice",
            "Embeddings Choice",
            &config,
            Arc::new(observer.clone()),
        )
        .await
        .expect("research run");

    // The injected source must appear in the outcome's source list.
    let injected = outcome
        .sources
        .iter()
        .find(|s| s.title().contains("Vector databases"));
    assert!(
        injected.is_some(),
        "semantic augmentor should inject a lexically-disjoint prior source; got sources: {:?}",
        outcome
            .sources
            .iter()
            .map(|s| s.title().to_string())
            .collect::<Vec<_>>()
    );

    // A `SemanticSourceRetrieved` event must have been emitted.
    let events = observer.events.lock().unwrap();
    let semantic_event = events.iter().any(|e| matches!(
        e,
        SessionEvent::SemanticSourceRetrieved { title, .. } if title.contains("Vector databases")
    ));
    assert!(
        semantic_event,
        "expected a SemanticSourceRetrieved event for the injected hit"
    );

    // The augmentor must have been asked to index captured sources and the
    // final document (FR-017 / FR-016).
    assert!(
        !augmentor.index_sources_calls.lock().unwrap().is_empty(),
        "index_sources should be called after the gather phase"
    );
    let docs = augmentor.index_document_calls.lock().unwrap().clone();
    assert!(
        !docs.is_empty(),
        "index_document should be called after the session completes"
    );
    assert_eq!(docs[0].0, "embeddings-choice");
    assert!(docs[0].2.contains("embeddings"));
}

/// T-012: when no augmentor is attached, the session behaves exactly as before
/// (no semantic events, no injected sources).
#[tokio::test]
async fn no_augmentor_means_no_semantic_sources_or_events() {
    let tmp = TempDir::new().expect("temp dir");
    let root = tmp.path().to_path_buf();
    std::fs::create_dir_all(&root).unwrap();
    let manager = ResearchManager::new(root.clone());

    let web = WebGatherer::new(Arc::new(NoSearch), Arc::new(PageFetch));
    let analysis: Arc<dyn AnalysisEngine> = Arc::new(NoopAnalysisEngine);
    // No with_semantic_augmentor.
    let session = ResearchSession::new(manager, Some(web), None, analysis);

    let observer = RecordingObserver::default();
    let config = SessionConfig {
        topic: "embeddings model selection".to_string(),
        max_web_results: 0,
        disable_local: true,
        disable_specs: true,
        ..Default::default()
    };

    let outcome = session
        .run("no-aug", "No Aug", &config, Arc::new(observer.clone()))
        .await
        .expect("research run");

    assert!(
        outcome
            .sources
            .iter()
            .all(|s| !s.title().contains("Vector databases")),
        "no semantic source should be injected without an augmentor"
    );
    let events = observer.events.lock().unwrap();
    assert!(
        events
            .iter()
            .all(|e| !matches!(e, SessionEvent::SemanticSourceRetrieved { .. })),
        "no SemanticSourceRetrieved event without an augmentor"
    );
}

// Silence unused-import warnings for items pulled in via the prelude above
// that the no-augmentor path doesn't reference.
#[allow(dead_code)]
fn _unused() {
    let _ = (
        SynthesizeOutcome::FallbackEmpty,
        AnalysisResult::default(),
        AnalysisOutcome::FallbackEmpty,
        OutputFormat::Report,
        PathBuf::new(),
    );
}
