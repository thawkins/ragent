//! Iterative research engine (T-004, T-005, T-016).
//!
//! [`IterativeEngine`] drives the multi-pass research loop required by the
//! `researchext` spec. It:
//!
//! 1. Plans sub-questions from a topic (FR-001).
//! 2. Runs iterations until the plan is answered, the score plateaus, or the
//!    iteration budget is exhausted (FR-007, FR-008).
//! 3. Gathers sources for pending sub-questions — in parallel when a
//!    [`ConcurrencyConfig`] is supplied (T-016, FR-014).
//! 4. Detects missing links and emits follow-up bridge queries (FR-005).
//! 5. Emits structured progress events for every transition (FR-003).
//!
//! The engine operates on a mutable [`ResearchState`]; callers can persist and
//! resume state across turns (T-012, T-013).

use crate::analysis::{AnalysisEngine, AnalysisResult, build_source_bodies};
use crate::session::{SessionEvent, SessionObserver, SynthesisEvent};
use crate::source::Source;
use crate::state::{EvidenceGap, ResearchState, SubQuestionStatus};
use crate::verify::Verifier;
use crate::web_gatherer::{GatherEvent, GatherObserver, WebGatherError, WebGatherer};
use crate::{AdaptiveStopper, Planner};
use async_trait::async_trait;
use futures::StreamExt;
use std::sync::Arc;

/// Configuration for one run of the iterative engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EngineConfig {
    /// Maximum number of research iterations.
    pub max_iterations: u32,
    /// Maximum web sources to capture per sub-question per iteration.
    pub max_sources_per_question: usize,
    /// Maximum concurrent sub-question gathering tasks (T-016).
    pub max_concurrency: usize,
    /// Set to `true` to keep iterating even when the score plateaus.
    pub force_deeper: bool,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            max_iterations: 3,
            max_sources_per_question: 3,
            max_concurrency: 4,
            force_deeper: false,
        }
    }
}

/// Result of one engine loop iteration.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IterationResult {
    /// Sub-questions answered this iteration.
    pub answered: Vec<String>,
    /// New sources captured this iteration.
    pub sources_added: usize,
    /// New gaps detected this iteration.
    pub gaps_added: Vec<String>,
}

/// Critic/evaluator abstraction. Implementations inspect the current
/// [`ResearchState`] and any LLM analysis output and produce a quality score
/// plus any new evidence gaps.
#[async_trait]
pub trait Critic: Send + Sync {
    /// Evaluate the current state and produce a score + gaps.
    async fn evaluate(
        &self,
        state: &ResearchState,
        analysis: Option<&AnalysisResult>,
    ) -> CriticResult;
}

/// Output of a [`Critic`] evaluation.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CriticResult {
    /// Quality score (higher is better). `None` when the critic cannot score.
    pub score: Option<u32>,
    /// New evidence gaps detected this iteration.
    pub gaps: Vec<EvidenceGap>,
}

/// Deterministic critic used when no LLM is available. Scores based on coverage
/// and creates gaps for sub-questions that have no sources.
#[derive(Debug, Default, Clone, Copy)]
pub struct SimpleCritic;

#[async_trait]
impl Critic for SimpleCritic {
    async fn evaluate(
        &self,
        state: &ResearchState,
        _analysis: Option<&AnalysisResult>,
    ) -> CriticResult {
        let answered = state
            .plan
            .sub_questions
            .iter()
            .filter(|sq| sq.status == SubQuestionStatus::Answered)
            .count();
        let total = state.plan.sub_questions.len().max(1);
        let source_bonus = state.sources.len() * 5;
        let score = ((answered * 50) / total + source_bonus).min(100) as u32;

        let mut gaps = Vec::new();
        for sq in &state.plan.sub_questions {
            let has_source = state.sources.iter().any(|s| {
                s.has_body()
                    && sq.question.to_lowercase().split_whitespace().any(|w| {
                        s.title().to_lowercase().contains(w)
                            || s.path_or_url().to_lowercase().contains(w)
                    })
            });
            if !has_source {
                gaps.push(EvidenceGap {
                    id: format!("gap-{}", sq.id),
                    description: format!(
                        "No direct source found for sub-question: {}",
                        sq.question
                    ),
                    sub_question_ids: vec![sq.id.clone()],
                    resolved: false,
                });
            }
        }

        CriticResult {
            score: Some(score),
            gaps,
        }
    }
}

/// Forwards [`GatherEvent`]s from the web gatherer into the session observer as
/// [`SessionEvent`]s, and records failures in the shared state.
#[derive(Clone)]
struct StateGatherForwarder {
    observer: Arc<dyn SessionObserver>,
}

impl GatherObserver for StateGatherForwarder {
    fn on_event(&self, event: GatherEvent) {
        match event {
            GatherEvent::SearchFailed { error } => {
                self.observer.on_event(SessionEvent::SourceFailed {
                    source: None,
                    error,
                });
            }
            GatherEvent::FetchFailed { url, error } => {
                self.observer.on_event(SessionEvent::SourceFailed {
                    source: Some(url.clone()),
                    error,
                });
                self.observer.on_event(SessionEvent::WebFetchFailed {
                    url,
                    error: String::new(),
                });
            }
            GatherEvent::QueriesDecomposed { queries } => {
                self.observer
                    .on_event(SessionEvent::QueriesDecomposed { queries });
            }
            GatherEvent::SourceCaptured {
                url,
                title,
                search_tool,
                search_engine,
                body_preview,
                language,
                oa_recovery,
            } => {
                self.observer.on_event(SessionEvent::WebCaptured {
                    url,
                    title,
                    search_tool,
                    search_engine,
                    body_preview,
                    language,
                    oa_recovery,
                });
            }
            _ => {}
        }
    }
}

/// The iterative research engine.
#[derive(Clone)]
pub struct IterativeEngine {
    planner: Arc<dyn Planner>,
    web: Option<WebGatherer>,
    analysis: Arc<dyn AnalysisEngine>,
    critic: Arc<dyn Critic>,
    config: EngineConfig,
}

impl std::fmt::Debug for IterativeEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("IterativeEngine")
            .field("has_web", &self.web.is_some())
            .field("max_iterations", &self.config.max_iterations)
            .finish_non_exhaustive()
    }
}

impl IterativeEngine {
    /// Build an engine with the given planner, optional web gatherer, analysis
    /// engine, critic, and configuration.
    pub fn new(
        planner: Arc<dyn Planner>,
        web: Option<WebGatherer>,
        analysis: Arc<dyn AnalysisEngine>,
        critic: Arc<dyn Critic>,
        config: EngineConfig,
    ) -> Self {
        Self {
            planner,
            web,
            analysis,
            critic,
            config,
        }
    }

    /// Run the iterative loop from scratch for `topic`, returning the final
    /// state. The `observer` receives all lifecycle events.
    pub async fn run(
        &self,
        topic: &str,
        observer: Arc<dyn SessionObserver>,
    ) -> anyhow::Result<ResearchState> {
        let plan = self.planner.plan(topic).await?;
        let mut state = ResearchState::new(topic);
        state.plan = plan;
        observer.on_event(SessionEvent::PlanUpdated {
            sub_questions: state
                .plan
                .sub_questions
                .iter()
                .map(|sq| sq.question.clone())
                .collect(),
        });

        let mut stopper = AdaptiveStopper::new(self.config.max_iterations)
            .with_force_deeper(self.config.force_deeper);

        loop {
            let iteration = state.iteration_count + 1;
            let decision = stopper.decide(iteration, state.evaluation_score, state.is_complete());
            if decision.should_stop() {
                observer.on_event(SessionEvent::IterationCompleted {
                    iteration: state.iteration_count,
                    score: state.evaluation_score,
                });
                break;
            }

            let result = self.run_iteration(&mut state, observer.clone()).await?;
            state.increment_iteration();

            // Evaluate and update gaps.
            let analysis = self.synthesize(&state).await.ok();
            let critic_result = self.critic.evaluate(&state, analysis.as_ref()).await;
            state.set_evaluation_score(critic_result.score.unwrap_or(0));
            for gap in critic_result.gaps {
                if !state.gaps.iter().any(|g| g.id == gap.id) {
                    state.gaps.push(gap);
                }
            }

            observer.on_event(SessionEvent::Synthesis(SynthesisEvent::CriticResult {
                score: state.evaluation_score,
                gaps: state
                    .active_gaps()
                    .iter()
                    .map(|g| g.description.clone())
                    .collect(),
            }));

            // Verify claims from the synthesis step (T-010).
            let verification = self.verify(&state, analysis.as_ref()).await;
            observer.on_event(SessionEvent::VerificationResult {
                passed: verification.passed,
                issues: verification.issues.clone(),
            });

            // Follow-up bridge queries from active gaps (FR-005).
            let queries: Vec<String> = state
                .active_gaps()
                .iter()
                .map(|g| format!("{} evidence for {}", g.description, topic))
                .collect();
            if !queries.is_empty() {
                for q in &queries {
                    state.add_follow_up_query(q.clone());
                }
                observer.on_event(SessionEvent::FollowUpQueries { queries });
            }

            observer.on_event(SessionEvent::IterationCompleted {
                iteration,
                score: state.evaluation_score,
            });

            if result.answered.is_empty() && result.sources_added == 0 && !self.config.force_deeper
            {
                // No progress and not forced deeper: stop early.
                break;
            }
        }

        Ok(state)
    }

    /// Run a single iteration over pending sub-questions. Pending questions
    /// are processed concurrently up to `max_concurrency` (T-016).
    async fn run_iteration(
        &self,
        state: &mut ResearchState,
        observer: Arc<dyn SessionObserver>,
    ) -> anyhow::Result<IterationResult> {
        let pending: Vec<String> = state
            .pending_sub_questions()
            .iter()
            .map(|sq| sq.id.clone())
            .collect();
        if pending.is_empty() {
            return Ok(IterationResult::default());
        }

        let mut result = IterationResult::default();
        let max_sources = self.config.max_sources_per_question;
        let forwarder = StateGatherForwarder {
            observer: observer.clone(),
        };

        let questions: std::collections::HashMap<String, String> = pending
            .iter()
            .filter_map(|id| {
                state
                    .plan
                    .sub_questions
                    .iter()
                    .find(|sq| sq.id == *id)
                    .map(|sq| (id.clone(), sq.question.clone()))
            })
            .collect();

        let engine = self.clone();
        let tasks: Vec<_> = pending
            .into_iter()
            .map(|id| {
                let engine = engine.clone();
                let forwarder = forwarder.clone();
                let question = questions.get(&id).cloned().unwrap_or_default();
                async move {
                    (
                        id.clone(),
                        question.clone(),
                        engine
                            .gather_for_question(&question, max_sources, &forwarder)
                            .await,
                    )
                }
            })
            .collect();

        let concurrency = self.config.max_concurrency.max(1);
        let mut stream = futures::stream::iter(tasks).buffer_unordered(concurrency);
        while let Some((id, question, gather_result)) = stream.next().await {
            state.set_sub_question_status(&id, SubQuestionStatus::InProgress);
            observer.on_event(SessionEvent::SubQuestionStatusChanged {
                id: id.clone(),
                status: SubQuestionStatus::InProgress.as_str().to_string(),
            });

            match gather_result {
                Ok(sources) => {
                    if sources.is_empty() {
                        state.record_failed_source(Some(&question), "no web sources captured");
                        observer.on_event(SessionEvent::SourceFailed {
                            source: Some(question.clone()),
                            error: "no web sources captured".into(),
                        });
                    } else {
                        for src in sources {
                            if let Source::Web {
                                url,
                                title,
                                search_tool,
                                search_engine,
                                body,
                                language,
                                oa_recovery,
                                ..
                            } = &src
                            {
                                let body_preview: String = body
                                    .lines()
                                    .filter(|l| !l.trim_start().starts_with("```"))
                                    .collect::<Vec<_>>()
                                    .join("\n")
                                    .chars()
                                    .take(crate::web_gatherer::MIN_EXTRACTABLE_CONTENT_CHARS)
                                    .collect();
                                let lang = language
                                    .as_deref()
                                    .map(str::to_uppercase)
                                    .unwrap_or_else(|| "UNKNOWN".to_string());
                                observer.on_event(SessionEvent::WebCaptured {
                                    url: url.clone(),
                                    title: title.clone(),
                                    search_tool: search_tool.clone(),
                                    search_engine: search_engine.clone(),
                                    body_preview,
                                    language: lang,
                                    oa_recovery: oa_recovery.clone(),
                                });
                            }
                            state.add_source(src);
                            result.sources_added += 1;
                        }
                    }
                }
                Err(e) => {
                    state.record_failed_source(Some(&question), e.to_string());
                    observer.on_event(SessionEvent::SourceFailed {
                        source: Some(question.clone()),
                        error: e.to_string(),
                    });
                }
            }

            state.set_sub_question_status(&id, SubQuestionStatus::Answered);
            observer.on_event(SessionEvent::SubQuestionStatusChanged {
                id: id.clone(),
                status: SubQuestionStatus::Answered.as_str().to_string(),
            });
            result.answered.push(id);
        }

        Ok(result)
    }

    async fn gather_for_question(
        &self,
        question: &str,
        max_sources: usize,
        forwarder: &StateGatherForwarder,
    ) -> Result<Vec<Source>, WebGatherError> {
        if let Some(web) = &self.web {
            web.gather_with_observer(question, max_sources, Some(forwarder))
                .await
                .map(|r| r.sources)
        } else {
            Ok(Vec::new())
        }
    }

    /// Attach a JSONL gather log to the web gatherer so every candidate URL
    /// and its capture/rejection outcome is recorded under `<log_dir>/`.
    /// No-op when web gathering is disabled.
    #[must_use]
    pub fn with_gather_log(mut self, log: crate::gather_log::GatherLog) -> Self {
        if let Some(web) = self.web.take() {
            self.web = Some(web.with_gather_log(log));
        }
        self
    }

    /// Run the analysis engine over the current sources, if any.
    async fn synthesize(&self, state: &ResearchState) -> anyhow::Result<AnalysisResult> {
        let bodies = build_source_bodies(&state.sources, |src| {
            src.body().and_then(|b| {
                if b.is_empty() {
                    None
                } else {
                    Some(b.to_string())
                }
            })
        });
        self.analysis.analyze(&state.plan.topic, &bodies).await
    }

    /// Verify claims in the analysis against the current sources.
    async fn verify(
        &self,
        state: &ResearchState,
        analysis: Option<&AnalysisResult>,
    ) -> crate::verify::VerificationResult {
        let verifier = crate::verify::KeywordVerifier::new();
        verifier.verify(state, analysis).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::analysis::NoopAnalysisEngine;
    use crate::planner::HeuristicPlanner;
    use crate::session::{NoopObserver, SessionEvent};
    use crate::web_gatherer::{
        WebFetchTool, WebFetchedPage, WebGatherer, WebSearchHit, WebSearchTool,
    };
    use std::collections::VecDeque;
    use std::sync::Mutex;

    #[derive(Debug, Default)]
    struct FakeSearch {
        hits: Mutex<VecDeque<Vec<WebSearchHit>>>,
    }

    #[async_trait]
    impl WebSearchTool for FakeSearch {
        async fn search(
            &self,
            query: &str,
            _max_results: usize,
        ) -> anyhow::Result<Vec<WebSearchHit>> {
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
                body: "body text ".repeat(30),
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
}
