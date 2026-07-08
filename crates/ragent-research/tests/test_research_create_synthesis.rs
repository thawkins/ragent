//! FR-005 / FR-006 / T-012: integration test for a full `/research create`
//! synthesis pass.
//!
//! Exercises [`ragent_research::ResearchSession`] end-to-end with a mock
//! [`AnalysisEngine`] that returns (a) a malformed response and (b) a
//! well-formed response, asserting that:
//!
//! - A malformed model response surfaces
//!   [`SynthesizeOutcome::FallbackEmpty`] and the written `RESEARCH.md` still
//!   contains mechanically-derived findings (FR-005, FR-006).
//! - A well-formed model response surfaces [`SynthesizeOutcome::Llm`] and the
//!   written `RESEARCH.md` contains the LLM findings verbatim.
//!
//! The mock engines implement [`ragent_research::AnalysisEngine`] directly so
//! no real LLM provider is required. Web gathering is faked so no network
//! access is needed. Local gathering is disabled (`disable_local: true`).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use ragent_research::{
    AnalysisEngine, AnalysisOutcome, AnalysisResult, NoopAnalysisEngine, OutputFormat,
    ResearchManager, ResearchSession, SessionConfig, SessionEvent, SessionObserver, SourceBody,
    SynthesizeOutcome, WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool,
};
use tempfile::TempDir;

// ── Mock analysis engines ────────────────────────────────────────────────

/// A mock [`AnalysisEngine`] that simulates the real `LlmAnalysisEngine`'s
/// malformed-output path: its `analyze_with_outcome` override returns
/// [`AnalysisOutcome::FallbackEmpty`] with a mechanically-extracted finding,
/// exactly as `parse_analysis_response_with_outcome` would when the model
/// response cannot be parsed into the required structure (FR-005).
struct MalformedMockEngine;

#[async_trait]
impl AnalysisEngine for MalformedMockEngine {
    async fn analyze(
        &self,
        _topic: &str,
        _sources: &[SourceBody],
    ) -> anyhow::Result<AnalysisResult> {
        Ok(AnalysisResult::default())
    }

    async fn analyze_with_outcome(
        &self,
        _topic: &str,
        _sources: &[SourceBody],
    ) -> anyhow::Result<(AnalysisResult, AnalysisOutcome)> {
        // Mimic the real engine: the model response was malformed, so the
        // mechanical fallback supplies a placeholder finding and the outcome
        // is FallbackEmpty (FR-005 / FR-006).
        let result = AnalysisResult {
            summary: "(the model response was malformed; the following findings \
                 were extracted mechanically and may be incomplete)"
                .to_string(),
            findings: vec![
                "**Headline:** Findings could not be structured\n\n\
                 **Observation:** (findings could not be structured — see below)\n\n\
                 The raw model response was unparseable.\n\n\
                 **Analysis:** (extracted mechanically)\n\n\
                 **Cross-reference / Dependencies:** No direct dependencies.\n\n\
                 **Implication:** Re-run `/research create` or refine the topic."
                    .to_string(),
            ],
            cross_references: Vec::new(),
            open_questions: Vec::new(),
        };
        Ok((result, AnalysisOutcome::FallbackEmpty))
    }
}

/// A mock [`AnalysisEngine`] that overrides `analyze_with_outcome` to return
/// a well-formed [`AnalysisResult`] tagged [`AnalysisOutcome::Llm`]. The
/// finding carries the four required labels and a `[#1]` citation so it
/// passes the malformed check and the citation/date validation.
struct WellFormedMockEngine;

#[async_trait]
impl AnalysisEngine for WellFormedMockEngine {
    async fn analyze(
        &self,
        _topic: &str,
        _sources: &[SourceBody],
    ) -> anyhow::Result<AnalysisResult> {
        Ok(AnalysisResult::default())
    }

    async fn analyze_with_outcome(
        &self,
        _topic: &str,
        _sources: &[SourceBody],
    ) -> anyhow::Result<(AnalysisResult, AnalysisOutcome)> {
        let finding = "**Observation:** The source describes async/await idioms [#1].\n\n\
             **Analysis:** This is directly relevant to the topic.\n\n\
             **Cross-reference / Dependencies:** No direct dependencies.\n\n\
             **Implication:** Adopt the idioms described in the source."
            .to_string();
        let result = AnalysisResult {
            summary: "LLM-synthesized summary of Rust async.".into(),
            findings: vec![finding],
            cross_references: Vec::new(),
            open_questions: Vec::new(),
        };
        Ok((result, AnalysisOutcome::Llm))
    }
}

// ── Fakes for the web gathering phase (no network) ───────────────────────

#[derive(Debug, Default)]
struct FakeSearch {
    hits: Vec<WebSearchHit>,
}

#[async_trait]
impl WebSearchTool for FakeSearch {
    async fn search(&self, _query: &str, _max: usize) -> anyhow::Result<Vec<WebSearchHit>> {
        Ok(self.hits.clone())
    }
}

#[derive(Debug, Default)]
struct FakeFetch {
    pages: HashMap<String, WebFetchedPage>,
}

#[async_trait]
impl WebFetchTool for FakeFetch {
    async fn fetch(&self, url: &str) -> anyhow::Result<WebFetchedPage> {
        self.pages
            .get(url)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("no fake page for {url}"))
    }
}

/// Collect every [`SessionEvent::SynthesizeResult`] so the test can assert on
/// the surfaced [`SynthesizeOutcome`].
#[derive(Debug, Default)]
struct CaptureSynthesize {
    outcomes: Mutex<Vec<SynthesizeOutcome>>,
}

impl SessionObserver for CaptureSynthesize {
    fn on_event(&self, event: SessionEvent) {
        if let SessionEvent::SynthesizeResult { outcome, .. } = event {
            self.outcomes.lock().unwrap().push(outcome);
        }
    }
}

/// Build a minimal session config with local + spec gathering disabled so
/// only the faked web source is captured.
fn cfg_with_topic(topic: &str) -> SessionConfig {
    SessionConfig {
        topic: topic.into(),
        max_web_results: 5,
        max_local_sources: 5,
        disable_local: true,
        disable_specs: true,
        ..SessionConfig::default()
    }
}

/// Wire a [`ResearchSession`] with one fake web source and no local gatherer.
fn session_with_engine(
    research_root: &std::path::Path,
    engine: Arc<dyn AnalysisEngine>,
) -> ResearchSession {
    let manager = ResearchManager::new(research_root);
    let web = WebGatherer::new(
        Arc::new(FakeSearch {
            hits: vec![WebSearchHit {
                url: "https://example.com/async".into(),
                title: "Rust Async Guide".into(),
                snippet: "async/await idioms".into(),
                matched_query: "Rust async".into(),
            }],
        }),
        Arc::new(FakeFetch {
            pages: HashMap::from([(
                "https://example.com/async".into(),
                WebFetchedPage {
                    published_at: None,
                    url: "https://example.com/async".into(),
                    title: "Rust Async Guide".into(),
                    body: "Use async/await for concurrent Rust.".into(),
                },
            )]),
        }),
    );
    ResearchSession::new(manager, Some(web), None, engine)
}

#[tokio::test]
async fn malformed_llm_response_surfaces_fallback_empty_and_writes_findings() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    let session = session_with_engine(&research_root, Arc::new(MalformedMockEngine));
    let observer = Arc::new(CaptureSynthesize::default());

    let _outcome = session
        .run(
            "malformed-test",
            "Malformed",
            &cfg_with_topic("Rust async"),
            observer.clone(),
        )
        .await
        .unwrap();

    // FR-005: the SynthesizeResult event must surface FallbackEmpty (the
    // mock returned Ok with empty findings; session.rs attributes it to the
    // fallback path because there is no LLM content).
    let has_fallback;
    {
        let outcomes = observer.outcomes.lock().unwrap();
        has_fallback = outcomes.contains(&SynthesizeOutcome::FallbackEmpty);
    }
    assert!(
        has_fallback,
        "expected at least one FallbackEmpty outcome, got {:?}",
        observer.outcomes.lock().unwrap()
    );

    // FR-006: the final RESEARCH.md still contains findings (from the
    // engine's fallback finding).
    let body = tokio::fs::read_to_string(research_root.join("malformed-test/RESEARCH.md"))
        .await
        .unwrap();
    assert!(
        body.contains("## Findings"),
        "RESEARCH.md must still contain a Findings section, got:\n{body}"
    );
    assert!(
        body.contains("### Finding 1 — Findings could not be structured"),
        "RESEARCH.md findings must have a headline heading, got:\n{body}"
    );
    assert!(
        body.contains("**Implication:**"),
        "RESEARCH.md findings must carry the Implication label, got:\n{body}"
    );
    // A source was captured, so the References Index should list it.
    assert!(
        body.contains("https://example.com/async"),
        "RESEARCH.md References Index should list the captured web source"
    );
}

#[tokio::test]
async fn well_formed_llm_response_surfaces_llm_and_writes_llm_findings() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    let session = session_with_engine(&research_root, Arc::new(WellFormedMockEngine));
    let observer = Arc::new(CaptureSynthesize::default());

    let _outcome = session
        .run(
            "wellformed-test",
            "WellFormed",
            &cfg_with_topic("Rust async"),
            observer.clone(),
        )
        .await
        .unwrap();

    // The well-formed mock returns AnalysisOutcome::Llm, so session.rs must
    // surface SynthesizeOutcome::Llm.
    let has_llm;
    {
        let outcomes = observer.outcomes.lock().unwrap();
        has_llm = outcomes.contains(&SynthesizeOutcome::Llm);
    }
    assert!(
        has_llm,
        "expected at least one Llm outcome, got {:?}",
        observer.outcomes.lock().unwrap()
    );

    // RESEARCH.md must contain the LLM finding verbatim, including the
    // [#1] citation.
    let body = tokio::fs::read_to_string(research_root.join("wellformed-test/RESEARCH.md"))
        .await
        .unwrap();
    assert!(
        body.contains("The source describes async/await idioms [#1]"),
        "RESEARCH.md must contain the LLM finding verbatim, got:\n{body}"
    );
    assert!(
        body.contains("LLM-synthesized summary of Rust async."),
        "RESEARCH.md must contain the LLM summary verbatim, got:\n{body}"
    );
    assert!(body.contains("## Findings"));
    assert!(
        body.contains("### Finding 1 — The source describes async/await idioms"),
        "RESEARCH.md findings must have a headline heading, got:\n{body}"
    );
    assert!(body.contains("**Observation:**"));
    assert!(body.contains("**Analysis:**"));
    assert!(body.contains("**Cross-reference / Dependencies:**"));
    assert!(body.contains("**Implication:**"));
}

#[tokio::test]
async fn executive_summary_format_writes_shorter_summary_instruction() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    let session = session_with_engine(&research_root, Arc::new(WellFormedMockEngine));

    let cfg = SessionConfig {
        topic: "Rust async".into(),
        output_format: OutputFormat::ExecutiveSummary,
        max_web_results: 5,
        max_local_sources: 5,
        disable_local: true,
        disable_specs: true,
        ..SessionConfig::default()
    };
    let _outcome = session
        .run(
            "exec-summary-test",
            "Exec Summary",
            &cfg,
            Arc::new(CaptureSynthesize::default()),
        )
        .await
        .unwrap();

    let body = tokio::fs::read_to_string(research_root.join("exec-summary-test/RESEARCH.md"))
        .await
        .unwrap();
    // Non-default formats are persisted in the frontmatter.
    assert!(
        body.contains("requested_format: executive-summary"),
        "frontmatter should record requested format, got:\n{body}"
    );
    assert!(body.contains("## Summary"));
    assert!(body.contains("## Findings"));
}

#[tokio::test]
async fn no_llm_engine_surfaces_no_llm_outcome_and_writes_mechanical_findings() {
    let tmp = TempDir::new().unwrap();
    let research_root = tmp.path().join("research");
    tokio::fs::create_dir_all(&research_root).await.unwrap();

    // NoopAnalysisEngine -> session.rs surfaces NoLlm and uses the
    // session-level mechanical fallback.
    let session = session_with_engine(&research_root, Arc::new(NoopAnalysisEngine));
    let observer = Arc::new(CaptureSynthesize::default());

    let _outcome = session
        .run(
            "no-llm-test",
            "NoLlm",
            &cfg_with_topic("Rust async"),
            observer.clone(),
        )
        .await
        .unwrap();

    let has_no_llm;
    {
        let outcomes = observer.outcomes.lock().unwrap();
        has_no_llm = outcomes.contains(&SynthesizeOutcome::NoLlm);
    }
    assert!(
        has_no_llm,
        "expected a NoLlm outcome when no LLM engine is wired in, got {:?}",
        observer.outcomes.lock().unwrap()
    );

    // RESEARCH.md still gets mechanical findings derived from the captured
    // web source.
    let body = tokio::fs::read_to_string(research_root.join("no-llm-test/RESEARCH.md"))
        .await
        .unwrap();
    assert!(body.contains("## Findings"));
    assert!(
        body.contains("### Finding 1 —"),
        "RESEARCH.md findings must have a headline heading, got:\n{body}"
    );
    assert!(body.contains("**Observation:**"));
    assert!(body.contains("**Implication:**"));
    assert!(body.contains("https://example.com/async"));
}
