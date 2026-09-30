//! Supervisor/researcher multi-agent graph primitives for `/research --mode supervisor|competitive`.
//!
//! Implements the state machine from specs/opendeepresearch T-005/T-006:
//! Plan -> Delegate -> Collect -> Synthesize -> Finalize.
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
/// Each sub-topic names only its own entity plus the detected comparison
/// criteria. The overall topic is deliberately NOT embedded: the mission brief
/// (woven into the researcher's compression prompt) already carries it, and
/// embedding the full multi-entity topic made every researcher search for all
/// entities, which polluted the per-entity findings with cross-entity content.
#[must_use]
pub fn build_competitive_sub_topics(
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
                "Research {entity}{category}{dims}",
                entity = e.name,
                category = category_clause,
                dims = dimension_clause,
            )
        })
        .collect()
}

/// Check whether `sub_topic` was generated for `entity_name`.
///
/// Competitive sub-topics have the form `Research {entity}{category}{dims}`
/// where the criteria clause may name other entities, so a plain substring
/// search would misattribute. This predicate anchors on the leading
/// `Research {entity}` prefix and requires the entity name to be followed by
/// a clause boundary - end of string, the category clause ` (`, or the
/// dimension clause ` across ` - so a name that is a prefix of another
/// (e.g. `Groq` vs `Groq Cloud`) cannot capture the other entity's
/// assignment.
#[must_use]
pub fn sub_topic_matches_entity(sub_topic: &str, entity_name: &str) -> bool {
    let after_prefix = sub_topic.trim_start_matches("Research ").trim_start();
    let Some(after_entity) = after_prefix.strip_prefix(entity_name) else {
        return false;
    };
    after_entity.is_empty()
        || after_entity.starts_with(" (")
        || after_entity.starts_with(" across ")
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
                "Source {}: {} ({}) - {}",
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
                format!("{}...", &r[..r.floor_char_boundary(120)])
            } else {
                r.to_string()
            }
        })
        .unwrap_or_else(|| "captured".to_string())
}

/// Extract the entity name from a competitive-shaped sub-topic
/// (`Research {entity}{category}{dims}`), if the sub-topic has that shape.
fn competitive_entity_name(sub_topic: &str) -> Option<String> {
    let rest = sub_topic
        .trim_start()
        .strip_prefix("Research ")?
        .trim_start();
    if rest.is_empty() {
        return None;
    }
    // Entity names may contain spaces ("GitHub Copilot"); the name ends at
    // the category clause `(` or the dimension clause ` across `.
    let end = rest
        .find(" (")
        .or(rest.find(" across "))
        .unwrap_or(rest.len());
    let name = rest[..end].trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

/// Check whether a captured source body (title plus text) mentions `entity`.
fn mentions_entity(body: &crate::analysis::SourceBody, entity: &str) -> bool {
    let lc_entity = entity.to_lowercase();
    body.title.to_lowercase().contains(&lc_entity) || body.body.to_lowercase().contains(&lc_entity)
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
                "{}. {} - {} ({})",
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
    // first ~200 chars of each source body as a preview. For competitive
    // sub-topics the previews are filtered to sources that mention the
    // entity so the fallback cannot publish other entities' content under
    // this entity's profile.
    let entity = competitive_entity_name(sub_topic);
    let bodies = build_source_bodies(&state.sources, |src| {
        src.body().and_then(|b| {
            if b.is_empty() {
                None
            } else {
                Some(b.to_string())
            }
        })
    });
    let bodies: Vec<_> = match entity.as_deref() {
        Some(name) => bodies
            .into_iter()
            .filter(|b| mentions_entity(b, name))
            .collect(),
        None => bodies,
    };
    if !bodies.is_empty() {
        lines.push("\n## Findings".to_string());
        for body in bodies.iter().take(5) {
            let snippet = if body.body.len() > 200 {
                format!("{}...", &body.body[..body.body.floor_char_boundary(200)])
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
        let mut topic_with_brief = match self.brief.as_ref() {
            Some(b) => format!("{b}\n\nSub-topic: {sub_topic}"),
            None => sub_topic.to_string(),
        };
        // Competitive sub-topics have the `Research {entity}...` shape. The
        // comparison-table compression template asks for findings that compare
        // the entities, but a per-entity profile needs entity-scoped findings;
        // countermand that here so each researcher reports on its own entity
        // and mentions others only as explicit contrast.
        if sub_topic.trim_start().starts_with("Research ") {
            topic_with_brief.push_str(
                "\n\nScope: report findings about the single entity named at the start of the \
                 sub-topic only. Mention other entities solely as explicit contrast, never as \
                 the subject of a finding.",
            );
        }

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
                    "[#{}] {} - {}",
                    body.index, body.title, body.path_or_url
                ));
            }
        }

        Ok(lines.join("\n"))
    }
}

#[cfg(test)]
#[path = "../tests/inline/supervisor_tests.rs"]
mod tests;
