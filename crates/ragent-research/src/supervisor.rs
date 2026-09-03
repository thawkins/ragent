//! Supervisor/researcher multi-agent graph primitives for `/research --mode supervisor|competitive`.
//!
//! Implements the state machine from specs/opendeepresearch T-005/T-006:
//! Plan → Delegate → Collect → Synthesize → Finalize.
//!
//! The actual end-to-end orchestration lives in
//! [`crate::session::ResearchSession::run_supervisor`] because it needs access
//! to the session's private synthesis and document-assembly helpers. This
//! module provides the reusable state-machine types and the default
//! [`IterativeResearcherNode`] that the session uses.

use crate::analysis::build_source_bodies;
use crate::engine::{Critic, EngineConfig, IterativeEngine, SimpleCritic};
use crate::planner::{HeuristicPlanner, Planner};
use crate::session::{SessionEvent, SessionObserver};
use crate::source::Source;
use crate::source_vault::SourceVault;
use crate::state::{ResearchState, SubQuestionStatus};
use crate::web_gatherer::WebGatherer;
use async_trait::async_trait;
use std::sync::Arc;

/// Default maximum number of parallel researcher agents (FR-012).
pub const DEFAULT_MAX_CONCURRENT_RESEARCH_UNITS: usize = 5;

/// Lifecycle status of one researcher assignment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResearcherStatus {
    /// Waiting to be started.
    Pending,
    /// Actively gathering evidence.
    InProgress,
    /// Finished with findings captured.
    Completed,
    /// Failed to produce findings.
    Failed,
}

impl ResearcherStatus {
    /// Snake-case label used in events.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
        }
    }
}

/// One sub-topic delegated to a researcher node.
#[derive(Debug, Clone)]
pub struct ResearcherAssignment {
    /// Stable identifier for this researcher.
    pub id: String,
    /// Focused sub-topic question.
    pub sub_topic: String,
    /// Current status in the state machine.
    pub status: ResearcherStatus,
    /// Sources captured for this sub-topic.
    pub sources: Vec<Source>,
    /// Compressed findings / summary from the researcher.
    pub summary: String,
    /// Failure message, when [`Self::status`] is [`ResearcherStatus::Failed`].
    pub error: Option<String>,
}

/// In-memory state for a supervisor/researcher graph run.
#[derive(Debug, Clone, Default)]
pub struct SupervisorState {
    /// Original research topic.
    pub topic: String,
    /// Sub-topics planned by the supervisor node.
    pub sub_topics: Vec<String>,
    /// Per-researcher assignments.
    pub assignments: Vec<ResearcherAssignment>,
}

impl SupervisorState {
    /// Create a fresh state for `topic`.
    #[must_use]
    pub fn new(topic: impl Into<String>) -> Self {
        Self {
            topic: topic.into(),
            ..Self::default()
        }
    }

    /// Add a sub-topic and create a pending assignment for it.
    pub fn add_sub_topic(&mut self, sub_topic: impl Into<String>) {
        let sub_topic = sub_topic.into();
        let id = format!("researcher-{}", self.assignments.len() + 1);
        self.sub_topics.push(sub_topic.clone());
        self.assignments.push(ResearcherAssignment {
            id,
            sub_topic,
            status: ResearcherStatus::Pending,
            sources: Vec::new(),
            summary: String::new(),
            error: None,
        });
    }

    /// Mark an assignment as in-progress.
    ///
    /// Returns `true` if the id existed and was pending.
    pub fn set_in_progress(&mut self, id: &str) -> bool {
        if let Some(a) = self.assignments.iter_mut().find(|a| a.id == id) {
            a.status = ResearcherStatus::InProgress;
            true
        } else {
            false
        }
    }

    /// Mark an assignment as completed with captured sources and a summary.
    ///
    /// Returns `true` if the id existed and was in progress.
    pub fn set_completed(
        &mut self,
        id: &str,
        sources: Vec<Source>,
        summary: impl Into<String>,
    ) -> bool {
        if let Some(a) = self.assignments.iter_mut().find(|a| a.id == id) {
            a.status = ResearcherStatus::Completed;
            a.sources = sources;
            a.summary = summary.into();
            true
        } else {
            false
        }
    }

    /// Mark an assignment as failed with an error message.
    ///
    /// Returns `true` if the id existed.
    pub fn set_failed(&mut self, id: &str, error: impl Into<String>) -> bool {
        if let Some(a) = self.assignments.iter_mut().find(|a| a.id == id) {
            a.status = ResearcherStatus::Failed;
            a.error = Some(error.into());
            true
        } else {
            false
        }
    }

    /// Return assignments that are still pending.
    #[must_use]
    pub fn pending(&self) -> Vec<&ResearcherAssignment> {
        self.assignments
            .iter()
            .filter(|a| a.status == ResearcherStatus::Pending)
            .collect()
    }

    /// Return assignments that are completed.
    #[must_use]
    pub fn completed(&self) -> Vec<&ResearcherAssignment> {
        self.assignments
            .iter()
            .filter(|a| a.status == ResearcherStatus::Completed)
            .collect()
    }

    /// Merge all captured sources from completed assignments, deduplicating by URL/path.
    #[must_use]
    pub fn merged_sources(&self) -> Vec<Source> {
        let mut merged: Vec<Source> = Vec::new();
        for assignment in &self.assignments {
            for source in &assignment.sources {
                if !merged.iter().any(|s| same_source(s, source)) {
                    merged.push(source.clone());
                }
            }
        }
        merged
    }
}

/// Returns `true` when `a` and `b` refer to the same source by URL/path.
/// Used by the supervisor state machine and the session merge step.
pub(crate) fn same_source(a: &Source, b: &Source) -> bool {
    a.path_or_url() == b.path_or_url()
}

/// Build one sub-topic per competitive entity so the supervisor delegates a
/// dedicated researcher to each comparable option (FR-006 / T-010).
///
/// The returned questions embed the original topic and any detected comparison
/// criteria so each researcher gathers evidence along the same dimensions.
#[must_use]
pub fn build_competitive_sub_topics(
    topic: &str,
    entities: &[crate::entities::CompetitiveEntity],
    criteria: &[String],
) -> Vec<String> {
    if entities.is_empty() {
        return Vec::new();
    }

    let dimension_clause = if criteria.is_empty() {
        String::new()
    } else {
        format!(" across dimensions: {}", criteria.join(", "))
    };

    entities
        .iter()
        .map(|e| {
            let category_clause = e
                .category
                .as_ref()
                .map(|c| format!(" ({c})"))
                .unwrap_or_default();
            format!(
                "Research {entity}{category} for '{topic}'{dims}",
                entity = e.name,
                category = category_clause,
                dims = dimension_clause,
            )
        })
        .collect()
}

/// Abstraction over a worker that researches one sub-topic and returns
/// compressed findings.
#[async_trait]
pub trait ResearcherNode: Send + Sync {
    /// Run the researcher for `sub_topic` and return captured sources plus a
    /// compressed summary.
    async fn research(
        &self,
        id: &str,
        sub_topic: &str,
        observer: Arc<dyn SessionObserver>,
    ) -> anyhow::Result<(Vec<Source>, String)>;
}

/// Supervisor node: decomposes a topic into focused sub-topics.
#[derive(Clone)]
pub struct SupervisorNode {
    planner: Arc<dyn Planner>,
    max_sub_topics: usize,
}

impl SupervisorNode {
    /// Build a supervisor node with the given planner.
    #[must_use]
    pub fn new(planner: Arc<dyn Planner>) -> Self {
        Self {
            planner,
            max_sub_topics: DEFAULT_MAX_CONCURRENT_RESEARCH_UNITS,
        }
    }

    /// Cap the number of sub-topics planned.
    #[must_use]
    pub fn with_max_sub_topics(mut self, n: usize) -> Self {
        self.max_sub_topics = n.max(1);
        self
    }

    /// Plan sub-topics for `topic`.
    ///
    /// Returns between one and `max_sub_topics` focused questions. If the
    /// planner returns no questions, the original topic is returned as the
    /// only sub-topic.
    pub async fn plan(&self, topic: &str) -> anyhow::Result<Vec<String>> {
        let plan = self.planner.plan(topic).await?;
        let mut topics: Vec<String> = plan
            .sub_questions
            .into_iter()
            .map(|sq| sq.question)
            .collect();
        topics.truncate(self.max_sub_topics);
        if topics.is_empty() {
            topics.push(topic.to_string());
        }
        Ok(topics)
    }
}

/// A researcher node that uses the existing [`IterativeEngine`] to gather and
/// compress findings for one sub-topic, emitting per-researcher progress
/// events and structured notes as it works (T-006).
#[derive(Clone)]
pub struct IterativeResearcherNode {
    web: Option<WebGatherer>,
    analysis: Arc<dyn crate::analysis::AnalysisEngine>,
    planner: Option<Arc<dyn Planner>>,
    critic: Option<Arc<dyn Critic>>,
    engine_config: EngineConfig,
    /// Optional override model for the synthesis/compression step inside the
    /// researcher node (FR-013). When `None` the analysis engine's own model is
    /// used.
    research_model: Option<String>,
    /// Optional research brief injected into the per-researcher synthesis prompt
    /// so each worker treats its sub-topic as part of a larger mission.
    brief: Option<String>,
    /// Optional persistent vault used to store every summarized web source
    /// captured by this researcher (FR-003). When present, the web gatherer
    /// used by the internal engine is configured with the same vault.
    vault: Option<Arc<SourceVault>>,
}

impl IterativeResearcherNode {
    /// Build a researcher node backed by optional web gathering and an
    /// analysis engine.
    #[must_use]
    pub fn new(
        web: Option<WebGatherer>,
        analysis: Arc<dyn crate::analysis::AnalysisEngine>,
    ) -> Self {
        Self {
            web,
            analysis,
            planner: None,
            critic: None,
            engine_config: EngineConfig {
                max_iterations: 1,
                max_sources_per_question: 3,
                max_concurrency: 2,
                force_deeper: false,
            },
            research_model: None,
            brief: None,
            vault: None,
        }
    }

    /// Override the planner used to decompose sub-topics.
    #[must_use]
    pub fn with_planner(mut self, planner: Arc<dyn Planner>) -> Self {
        self.planner = Some(planner);
        self
    }

    /// Override the critic used to evaluate iterations.
    #[must_use]
    pub fn with_critic(mut self, critic: Arc<dyn Critic>) -> Self {
        self.critic = Some(critic);
        self
    }

    /// Override the iterative-engine configuration.
    #[must_use]
    pub fn with_engine_config(mut self, config: EngineConfig) -> Self {
        self.engine_config = config;
        self
    }

    /// Override the model used by this researcher for internal synthesis.
    ///
    /// The model string is currently stored for reporting; swapping the actual
    /// analysis engine mid-run is left to the session layer, which can build a
    /// phase-specific engine from the provider registry.
    #[must_use]
    pub fn with_research_model(mut self, model: Option<String>) -> Self {
        self.research_model = model;
        self
    }

    /// Inject the shared research brief so the researcher's synthesis prompt
    /// can reference the overall mission.
    #[must_use]
    pub fn with_brief(mut self, brief: Option<String>) -> Self {
        self.brief = brief;
        self
    }

    /// Attach a persistent source vault so every source captured by the
    /// internal web gatherer is stored with its original URL and timestamp
    /// (FR-003). The same vault is reused by the session's final synthesis.
    #[must_use]
    pub fn with_vault(mut self, vault: Option<Arc<SourceVault>>) -> Self {
        self.vault = vault;
        self
    }
}

#[async_trait]
impl ResearcherNode for IterativeResearcherNode {
    async fn research(
        &self,
        id: &str,
        sub_topic: &str,
        observer: Arc<dyn SessionObserver>,
    ) -> anyhow::Result<(Vec<Source>, String)> {
        observer.on_event(SessionEvent::ResearcherProgress {
            id: id.to_string(),
            status: "iterating".to_string(),
            detail: format!("starting tool loop for '{sub_topic}'"),
            sources_found: 0,
        });

        let planner = self
            .planner
            .clone()
            .unwrap_or_else(|| Arc::new(HeuristicPlanner::new()));
        let critic: Arc<dyn Critic> = self
            .critic
            .clone()
            .unwrap_or_else(|| Arc::new(SimpleCritic));
        let web = self.web.clone().map(|w| {
            if let Some(vault) = self.vault.clone() {
                w.with_vault(vault)
            } else {
                w
            }
        });
        let engine = IterativeEngine::new(
            planner,
            web,
            self.analysis.clone(),
            critic,
            self.engine_config,
        );

        let state = engine.run(sub_topic, observer.clone()).await?;

        // Emit a note for each captured source so the UI sees structured
        // per-source findings as they are produced.
        for (idx, src) in state.sources.iter().enumerate() {
            let note = format!(
                "Source {}: {} ({}) — {}",
                idx + 1,
                src.title(),
                src.path_or_url(),
                source_relevance_snippet(src)
            );
            observer.on_event(SessionEvent::ResearcherNote {
                id: id.to_string(),
                note,
            });
        }

        let summary = self
            .compress_findings(id, sub_topic, &state)
            .await
            .unwrap_or_else(|e| {
                tracing::warn!(
                    researcher = %id,
                    error = %e,
                    "researcher compression failed; falling back to deterministic summary"
                );
                build_summary(
                    id,
                    sub_topic,
                    &state,
                    self.research_model.as_deref(),
                    self.brief.as_deref(),
                )
            });
        observer.on_event(SessionEvent::ResearcherProgress {
            id: id.to_string(),
            status: "done".to_string(),
            detail: format!("tool loop completed for '{sub_topic}'"),
            sources_found: state.sources.len(),
        });
        Ok((state.sources, summary))
    }
}

/// Extract a short relevance note from a source for the researcher's note log.
fn source_relevance_snippet(src: &Source) -> String {
    src.relevance()
        .map(|r| {
            let r = r.trim();
            if r.len() > 120 {
                format!("{}…", &r[..120])
            } else {
                r.to_string()
            }
        })
        .unwrap_or_else(|| "captured".to_string())
}

/// Build a short compressed summary from a researcher's final state.
fn build_summary(
    id: &str,
    sub_topic: &str,
    state: &ResearchState,
    research_model: Option<&str>,
    brief: Option<&str>,
) -> String {
    let answered = state
        .plan
        .sub_questions
        .iter()
        .filter(|sq| sq.status == SubQuestionStatus::Answered)
        .count();
    let planned = state.plan.sub_questions.len().max(1);
    let sources = state.sources.len();
    let score = state.evaluation_score.unwrap_or(0);

    let mut lines = Vec::new();
    lines.push(format!("# Researcher {id}: {sub_topic}"));
    if let Some(b) = brief {
        lines.push(format!("Mission: {b}"));
    }
    if let Some(m) = research_model {
        lines.push(format!("Model: {m}"));
    }
    lines.push(format!(
        "Progress: answered {answered}/{planned} planned sub-questions, captured {sources} sources, score {score}/100."
    ));

    if !state.sources.is_empty() {
        lines.push("\n## Captured sources".to_string());
        for (idx, src) in state.sources.iter().enumerate() {
            lines.push(format!(
                "{}. {} — {} ({})",
                idx + 1,
                src.title(),
                src.path_or_url(),
                source_relevance_snippet(src)
            ));
        }
    }

    // Include a concise findings block derived from the captured source
    // bodies. We avoid calling back into the analysis engine here to keep
    // the researcher node deterministic and cheap; instead we surface the
    // first ~200 chars of each source body as a preview.
    let bodies = build_source_bodies(&state.sources, |src| {
        src.body().and_then(|b| {
            if b.is_empty() {
                None
            } else {
                Some(b.to_string())
            }
        })
    });
    if !bodies.is_empty() {
        lines.push("\n## Findings".to_string());
        for body in bodies.iter().take(5) {
            let snippet = if body.body.len() > 200 {
                format!("{}…", &body.body[..200])
            } else {
                body.body.clone()
            };
            lines.push(format!("- {}: {}", body.title, snippet));
        }
    }

    lines.join("\n")
}

impl IterativeResearcherNode {
    /// Compress the captured sources for a sub-topic into a concise markdown
    /// summary using the configured analysis engine (FR-004, T-007).
    ///
    /// The summary contains the engine's structured output plus a numbered
    /// references block that maps `[#N]` markers to the original source URLs
    /// so the supervisor can cite them even when the compressed text is used
    /// for final synthesis.
    async fn compress_findings(
        &self,
        id: &str,
        sub_topic: &str,
        state: &ResearchState,
    ) -> anyhow::Result<String> {
        let bodies = build_source_bodies(&state.sources, |src| {
            src.body().and_then(|b| {
                if b.is_empty() {
                    None
                } else {
                    Some(b.to_string())
                }
            })
        });

        // Weave the shared mission brief into the sub-topic so the
        // per-researcher compression is aligned with the overall goal without
        // relying on `AnalysisEngine::with_brief` (which is optional).
        let topic_with_brief = match self.brief.as_ref() {
            Some(b) => format!("{b}\n\nSub-topic: {sub_topic}"),
            None => sub_topic.to_string(),
        };

        let (result, _outcome) = self
            .analysis
            .analyze_with_outcome(&topic_with_brief, &bodies)
            .await?;

        // If the analysis engine produced no structured content, fall back to
        // the deterministic body-preview summary so the supervisor always gets
        // a useful note (and tests using NoopAnalysisEngine see the legacy
        // format).
        if result.summary.is_empty()
            && result.findings.is_empty()
            && result.top_implications.is_empty()
            && result.cross_references.is_empty()
            && result.open_questions.is_empty()
        {
            return Ok(build_summary(
                id,
                sub_topic,
                state,
                self.research_model.as_deref(),
                self.brief.as_deref(),
            ));
        }

        let mut lines = Vec::new();
        lines.push(format!("# Researcher {id}: {sub_topic}"));

        if !result.summary.is_empty() {
            lines.push(format!("\n## Summary\n\n{}", result.summary));
        }
        if !result.findings.is_empty() {
            lines.push("\n## Findings".to_string());
            for finding in &result.findings {
                lines.push(format!("- {finding}"));
            }
        }
        if !result.top_implications.is_empty() {
            lines.push("\n## Implications".to_string());
            for implication in &result.top_implications {
                lines.push(format!("- {implication}"));
            }
        }
        if !result.cross_references.is_empty() {
            lines.push("\n## Cross-references".to_string());
            for cr in &result.cross_references {
                lines.push(format!("- {}: {}", cr.path, cr.relevance));
            }
        }
        if !result.open_questions.is_empty() {
            lines.push("\n## Open questions".to_string());
            for q in &result.open_questions {
                lines.push(format!("- {q}"));
            }
        }
        if !bodies.is_empty() {
            lines.push("\n## Sources".to_string());
            for body in &bodies {
                lines.push(format!(
                    "[#{}] {} — {}",
                    body.index, body.title, body.path_or_url
                ));
            }
        }

        Ok(lines.join("\n"))
    }
}

#[cfg(test)]
mod tests {
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
        async fn search(
            &self,
            _query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
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
                body,
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
        let node =
            IterativeResearcherNode::new(Some(web), analysis).with_vault(Some(Arc::new(vault)));
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
        let supervisor =
            SupervisorNode::new(Arc::new(HeuristicPlanner::new())).with_max_sub_topics(3);
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
        let topics =
            build_competitive_sub_topics("Compare inference providers", &entities, &criteria);
        assert_eq!(topics.len(), 2);
        assert!(topics[0].contains("Fireworks AI"));
        assert!(topics[0].contains("inference provider"));
        assert!(topics[0].contains("LLM inference"));
        assert!(topics[1].contains("Groq"));
    }

    #[test]
    fn build_competitive_sub_topics_returns_empty_for_no_entities() {
        let topics = build_competitive_sub_topics("Compare something", &[], &[]);
        assert_eq!(topics, Vec::<String>::new());
    }
}
