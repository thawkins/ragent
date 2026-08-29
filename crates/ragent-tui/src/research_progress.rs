//! Research progress tracking for the `/research create` slash command.
//!
//! The TUI's [`crate::app::TuiResearchObserver`] forwards
//! [`ragent_research::SessionEvent`]s from the research engine to the TUI via
//! [`Event::AgentNotice`]. Each event carries a structured payload (prefixed with
//! [`PROGRESS_SENTINEL`]) that this module parses into [`ResearchStep`] entries
//! accumulated on a [`ResearchProgress`] tracker.
//!
//! The TUI renders the tracker as a single, self-updating log list in the
//! message window so users can follow each research phase (setup, web, local,
//! specs, synthesize, assemble, finalize) and its progress instead of a stream
//! of raw JSON lines.

pub use ragent_research::session::SessionPhase;

use ragent_research::session::{AnalysisEvent, SessionEvent, SynthesisEvent, SynthesizeOutcome};

use crate::app::sanitize_for_display;
/// Sentinel prefix marking an [`Event::AgentNotice`] message as a research
/// progress update. The remainder of the message is a JSON payload produced by
/// [`encode_progress_event`].
///
/// Using a sentinel (instead of a new `Event` variant) keeps the change local
/// to the TUI and avoids touching the shared `ragent-types` event enum and its
/// SSE serializer.
pub const PROGRESS_SENTINEL: &str = "__research_progress__";

/// Status of a single research step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepStatus {
    /// The phase has started but not yet reported any captured sources.
    Started,
    /// The phase captured one or more sources (or finished for non-capturing
    /// phases like setup/assemble/finalize).
    Done,
    /// The phase reported an error but the session continues gracefully.
    Error,
    /// A candidate was deliberately excluded by a gather policy (low
    /// relevance, too-short extraction) rather than failing on the network.
    /// Tracked separately so fetch-failure counts stay meaningful.
    Excluded,
    /// The step is not required for the selected tier and was skipped.
    Skipped,
}

impl StepStatus {
    /// Icon used in the rendered log list.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Started => "▶",
            Self::Done => "✓",
            Self::Error => "⚠",
            Self::Excluded => "−",
            Self::Skipped => "○",
        }
    }
}

/// A single line in the research progress log.
#[derive(Debug, Clone)]
pub struct ResearchStep {
    /// Phase label (e.g. "setup", "web").
    pub phase: &'static str,
    /// Human-readable description (e.g. "3 source(s) captured").
    pub detail: String,
    /// Whether the step is in-progress or complete.
    pub status: StepStatus,
}

/// Accumulated progress for a single `/research create` run.
#[derive(Debug, Clone)]
pub struct ResearchProgress {
    /// Research item name (e.g. "rust-async").
    pub name: String,
    /// Research topic string.
    pub topic: String,
    /// Ordered log of steps emitted so far.
    pub steps: Vec<ResearchStep>,
    /// Total source count once the run completes.
    pub total_sources: Option<usize>,
    /// Whether the run has finished (final `Done` event received).
    pub done: bool,
    /// Number of URLs/pages successfully fetched in the web phase.
    pub fetched_count: usize,
    /// Number of URLs/pages that failed to fetch in the web phase (network
    /// errors and timeouts only; policy exclusions are counted in
    /// [`Self::excluded_live`]).
    pub failed_count: usize,
    /// Number of live (mid-run) URLs excluded by gather policy (low
    /// relevance, too-short extraction). Superseded by the final
    /// `excluded_count` once the run completes.
    pub excluded_live: usize,
    /// Number of recovered PDF documents.
    pub pdf_count: usize,
    /// Number of recovered YouTube transcripts / video URLs.
    pub youtube_count: usize,
    /// Number of web sources fetched but excluded for low relevance.
    pub excluded_count: usize,
}

impl ResearchProgress {
    /// Build a fresh tracker for a new research run.
    pub fn new(name: impl Into<String>, topic: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            topic: topic.into(),
            steps: Vec::new(),
            total_sources: None,
            done: false,
            fetched_count: 0,
            failed_count: 0,
            excluded_live: 0,
            pdf_count: 0,
            youtube_count: 0,
            excluded_count: 0,
        }
    }

    /// Apply a parsed step update, appending or completing a step.
    ///
    /// Web-capture and web-fetch-failure events are accumulated as counts so
    /// the rendered log stays compact; a totals line is shown at the end of the
    /// web phase instead of one line per failed URL.
    pub fn apply(&mut self, phase: SessionPhase, status: StepStatus, detail: impl Into<String>) {
        let detail = detail.into();

        // Accumulate per-URL web results as counts rather than log lines.
        if phase == SessionPhase::Web && status == StepStatus::Done && detail.contains("captured ")
        {
            self.fetched_count += 1;
        } else if phase == SessionPhase::Web
            && status == StepStatus::Error
            && detail.starts_with("fetch failed for ")
        {
            self.failed_count += 1;
            return;
        } else if phase == SessionPhase::Web && status == StepStatus::Excluded {
            self.excluded_live += 1;
            return;
        }

        // If the last step is the same phase and now we're marking it done
        // or skipped, update it in place rather than appending a duplicate
        // line.
        if matches!(status, StepStatus::Done | StepStatus::Skipped)
            && let Some(last) = self.steps.last_mut()
            && last.phase == phase.as_str()
            && last.status == StepStatus::Started
        {
            last.status = status;
            last.detail = detail;
            return;
        }
        self.steps.push(ResearchStep {
            phase: phase.as_str(),
            detail,
            status,
        });
    }

    /// Mark the run complete with the final source count.
    pub fn finish(
        &mut self,
        total_sources: usize,
        pdf_count: usize,
        youtube_count: usize,
        excluded_count: usize,
    ) {
        self.total_sources = Some(total_sources);
        self.pdf_count = pdf_count;
        self.youtube_count = youtube_count;
        self.excluded_count = excluded_count;
        self.done = true;
    }

    /// Render the tracker as a markdown log list for the message window.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("🔬 Research Progress — `{}`\n", self.name));
        out.push_str(&format!("Topic: {}\n", self.topic));
        out.push('\n');
        for step in &self.steps {
            let prefix = format!("  {} {:<8} — ", step.status.icon(), step.phase);
            let lines: Vec<&str> = step.detail.lines().collect();
            if lines.is_empty() {
                out.push_str(&prefix);
                out.push('\n');
                continue;
            }
            out.push_str(&prefix);
            out.push_str(lines[0]);
            out.push('\n');
            let continuation = " ".repeat(prefix.chars().count());
            for line in lines.iter().skip(1) {
                out.push_str(&continuation);
                out.push_str(line);
                out.push('\n');
            }
        }
        if self.done
            && let Some(total) = self.total_sources
        {
            out.push('\n');
            let mut line = format!("✅ Complete — {total} source(s)");
            let mut extras = Vec::new();
            if self.pdf_count > 0 {
                extras.push(format!(
                    "{} PDF{}",
                    self.pdf_count,
                    if self.pdf_count == 1 { "" } else { "s" }
                ));
            }
            if self.youtube_count > 0 {
                extras.push(format!(
                    "{} YouTube video{}",
                    self.youtube_count,
                    if self.youtube_count == 1 { "" } else { "s" }
                ));
            }
            if self.excluded_count > 0 {
                extras.push(format!("{} excluded", self.excluded_count));
            }
            if !extras.is_empty() {
                line.push_str(&format!(", including {}", extras.join(" and ")));
            }
            line.push_str(&format!(
                ". Use `/research open {}` to view the result.",
                self.name
            ));
            out.push_str(&line);
        } else if self.fetched_count > 0 || self.failed_count > 0 || self.excluded_live > 0 {
            out.push('\n');
            out.push_str(&format!(
                "📊 Web fetch totals: {} fetched, {} failed{}",
                self.fetched_count,
                self.failed_count,
                if self.excluded_live > 0 {
                    format!(", {} excluded", self.excluded_live)
                } else {
                    String::new()
                }
            ));
        }
        out
    }
}

/// JSON payload carried inside an [`Event::AgentNotice`] message.
///
/// Fields are owned `String`s so the payload can be deserialised from a borrowed
/// message buffer without lifetime issues.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct ProgressPayload {
    name: String,
    topic: String,
    phase: String,
    status: String,
    detail: String,
    total_sources: Option<usize>,
    pdf_count: usize,
    youtube_count: usize,
    excluded_count: usize,
}

/// Encode a [`SessionEvent`] plus run metadata as a sentinel-prefixed
/// `AgentNotice` message string.
pub fn encode_progress_event(name: &str, topic: &str, event: &SessionEvent) -> String {
    let (phase, status, detail, total_sources, pdf_count, youtube_count, excluded_count) =
        match event {
            SessionEvent::Phase { phase } => {
                (*phase, "started", phase_description(*phase), None, 0, 0, 0)
            }
            SessionEvent::QueriesDecomposed { queries } => {
                let detail = if queries.is_empty() {
                    "no decomposition".to_string()
                } else {
                    format!(
                        "decomposed into {} quer{}:\n  - {}",
                        queries.len(),
                        if queries.len() == 1 { "y" } else { "ies" },
                        queries.join("\n  - ")
                    )
                };
                (SessionPhase::Web, "queries", detail, None, 0, 0, 0)
            }
            SessionEvent::WebSearchFailed { error } => (
                SessionPhase::Web,
                "error",
                format!("web search failed: {}", sanitize_for_display(error)),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::WebFetchFailed { url, error } => (
                SessionPhase::Web,
                "failed_url",
                format!(
                    "fetch failed for {}: {}",
                    sanitize_for_display(url),
                    sanitize_for_display(error)
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::WebSourceExcluded { url, reason } => (
                SessionPhase::Web,
                "excluded_url",
                format!(
                    "excluded {}: {}",
                    sanitize_for_display(url),
                    sanitize_for_display(reason)
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::WebCaptured {
                url,
                title,
                search_tool,
                search_engine,
                body_preview,
                language,
                oa_recovery: _,
            } => {
                let provenance = match (search_tool.is_empty(), search_engine.is_empty()) {
                    (true, true) => String::new(),
                    (false, true) => format!(" via {search_tool}"),
                    (true, false) => format!(" via {search_engine}"),
                    (false, false) => format!(" via {search_tool} ({search_engine})"),
                };
                let lang_tag = format!("[{language}]");
                let detail = if body_preview.is_empty() {
                    format!(
                        "{lang_tag} captured {}{} — {}",
                        sanitize_for_display(url),
                        provenance,
                        sanitize_for_display(title)
                    )
                } else {
                    format!(
                        "{lang_tag} captured {}{} — {}\n  {}",
                        sanitize_for_display(url),
                        provenance,
                        sanitize_for_display(title),
                        sanitize_for_display(body_preview)
                    )
                };
                (SessionPhase::Web, "captured", detail, None, 0, 0, 0)
            }
            SessionEvent::FromUrlBodyPreview { url, body_preview } => (
                SessionPhase::Setup,
                "preview",
                format!(
                    "--from-url body preview for {}:\n{}",
                    sanitize_for_display(url),
                    sanitize_for_display(body_preview)
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::FromFileBodyPreview { path, body_preview } => (
                SessionPhase::Setup,
                "preview",
                format!(
                    "--from-file body preview for {}:\n{}",
                    sanitize_for_display(path),
                    sanitize_for_display(body_preview)
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::LocalCaptured { path, score } => (
                SessionPhase::Local,
                "captured",
                format!("captured {} (score {})", sanitize_for_display(path), score),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::SpecCaptured { spec_id } => (
                SessionPhase::Specs,
                "captured",
                format!("referenced spec {}", sanitize_for_display(spec_id)),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult { outcome, detail }) => {
                let detail = match (outcome, detail) {
                    (SynthesizeOutcome::Llm, _) => "LLM analysis applied".to_string(),
                    (SynthesizeOutcome::FallbackEmpty, _) => {
                        "LLM returned empty content — using mechanical fallback".to_string()
                    }
                    (SynthesizeOutcome::FallbackError, Some(msg)) => {
                        format!("LLM synthesis failed: {msg} — using mechanical fallback")
                    }
                    (SynthesizeOutcome::FallbackError, None) => {
                        "LLM synthesis failed — using mechanical fallback".to_string()
                    }
                    (SynthesizeOutcome::NoLlm, _) => {
                        "no LLM engine configured — using mechanical fallback".to_string()
                    }
                };
                (SessionPhase::Synthesize, "done", detail, None, 0, 0, 0)
            }
            SessionEvent::Done {
                total_sources,
                pdf_count,
                youtube_count,
                excluded_count,
            } => (
                SessionPhase::Finalize,
                "done",
                "marked complete".to_string(),
                Some(*total_sources),
                *pdf_count,
                *youtube_count,
                *excluded_count,
            ),
            SessionEvent::RunStep {
                step,
                status,
                detail,
            } => (
                run_step_phase(step),
                run_step_status(status),
                format!(
                    "pipeline step: {step}{}",
                    detail
                        .as_ref()
                        .map(|d| format!(" ({d})"))
                        .unwrap_or_default()
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::TierDone {
                completed,
                skipped,
                failed,
            } => (
                SessionPhase::Finalize,
                "tier done",
                format!(
                    "pipeline complete: {completed} completed, {skipped} skipped, {failed} failed"
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::ConfigSnapshot {
                output_format,
                depth,
                iterations,
                tier,
                from_urls,
                from_files,
            } => {
                let mut parts = vec![format!("output format: {output_format}")];
                if let Some(t) = tier {
                    parts.push(format!("tier: {t}"));
                }
                if let Some(d) = depth {
                    parts.push(format!("depth: {d}"));
                }
                if let Some(i) = iterations {
                    parts.push(format!("iterations: {i}"));
                }
                if !from_urls.is_empty() {
                    let urls_display = from_urls
                        .iter()
                        .map(|u| sanitize_for_display(u))
                        .collect::<Vec<_>>()
                        .join(", ");
                    parts.push(format!("from-url: {urls_display}"));
                }
                if !from_files.is_empty() {
                    let files_display = from_files
                        .iter()
                        .map(|p| sanitize_for_display(p))
                        .collect::<Vec<_>>()
                        .join(", ");
                    parts.push(format!("from-file: {files_display}"));
                }
                (
                    SessionPhase::Setup,
                    "config",
                    format!("options in use: {}", parts.join(", ")),
                    None,
                    0,
                    0,
                    0,
                )
            }
            SessionEvent::Synthesis(SynthesisEvent::SynthesisAudit { audit }) => (
                SessionPhase::Synthesize,
                "done",
                format!(
                    "synthesis audit: {}/100 — {}",
                    audit.overall_score, audit.recommendation
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Analysis(AnalysisEvent::CorpusCritic { report }) => (
                SessionPhase::Synthesize,
                "done",
                format!(
                    "corpus critic: {}/100 ({}) — {} issue(s), {} gap(s)",
                    report.score,
                    if report.passed { "pass" } else { "review" },
                    report.issues.len(),
                    report.gaps.len()
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Analysis(AnalysisEvent::GapFetch { result }) => (
                SessionPhase::Synthesize,
                "done",
                format!(
                    "gap-fill fetch: {} new source(s) from {} query(s) {}",
                    result.new_sources,
                    result.queries.len(),
                    if result.attempted { "" } else { "(skipped)" }
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Synthesis(SynthesisEvent::SurgicalPatch { result }) => (
                SessionPhase::Synthesize,
                "done",
                format!(
                    "surgical patch: {} → {} ({} patch(es), {} applied)",
                    result.score_before,
                    result.score_after,
                    result.patches.len(),
                    result.patches.iter().filter(|p| p.applied).count()
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Synthesis(SynthesisEvent::CiteCheck { result }) => (
                SessionPhase::Synthesize,
                if result.passed { "done" } else { "error" },
                format!(
                    "cite check: {} citation(s) checked — {} (gate {})",
                    result.checked,
                    if result.passed {
                        "pass"
                    } else {
                        "CITATION_VERIFICATION_FAILED"
                    },
                    if result.gate_open { "open" } else { "closed" }
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Synthesis(SynthesisEvent::Polish { result }) => (
                SessionPhase::Synthesize,
                "done",
                format!(
                    "polish: {} control char(s), {} whitespace run(s), {} empty paragraph(s) changed",
                    result.control_chars_removed,
                    result.whitespace_normalized,
                    result.empty_paragraphs_removed
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Synthesis(SynthesisEvent::ReadabilityAudit { result }) => (
                SessionPhase::Synthesize,
                if result.passed { "done" } else { "error" },
                format!(
                    "readability audit: {}/100 — {} issue(s), {} recommendation(s)",
                    result.score,
                    result.issues.len(),
                    result.recommendations.len()
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::PlanUpdated { sub_questions } => (
                SessionPhase::Setup,
                "done",
                format!("plan updated: {} sub-question(s)", sub_questions.len()),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::SubQuestionStatusChanged { id, status } => (
                SessionPhase::Web,
                "done",
                format!(
                    "sub-question {}: {}",
                    sanitize_for_display(id),
                    sanitize_for_display(status)
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::SourceFailed { source, error } => (
                SessionPhase::Web,
                "error",
                match source {
                    Some(s) => format!(
                        "source failed: {} — {}",
                        sanitize_for_display(s),
                        sanitize_for_display(error)
                    ),
                    None => format!("source failed: {}", sanitize_for_display(error)),
                },
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Synthesis(SynthesisEvent::CriticResult { score, gaps }) => (
                SessionPhase::Synthesize,
                "done",
                match score {
                    Some(s) => format!("critic: {s}/100 — {} gap(s)", gaps.len()),
                    None => format!("critic: {} gap(s)", gaps.len()),
                },
                None,
                0,
                0,
                0,
            ),
            SessionEvent::VerificationResult { passed, issues } => (
                SessionPhase::Synthesize,
                if *passed { "done" } else { "error" },
                format!(
                    "verification: {} — {} issue(s)",
                    if *passed { "pass" } else { "fail" },
                    issues.len()
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::IterationCompleted { iteration, score } => (
                SessionPhase::Synthesize,
                "done",
                match score {
                    Some(s) => format!("iteration {iteration} complete (score {s})"),
                    None => format!("iteration {iteration} complete"),
                },
                None,
                0,
                0,
                0,
            ),
            SessionEvent::FollowUpQueries { queries } => (
                SessionPhase::Web,
                "queries",
                format!("{} follow-up querie(s) generated", queries.len()),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Analysis(AnalysisEvent::ContradictionGraph {
                edges,
                sources_scanned,
            }) => (
                SessionPhase::Synthesize,
                "done",
                format!(
                    "contradiction graph: {} edge(s) from {sources_scanned} source(s)",
                    edges.len()
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Analysis(AnalysisEvent::LociAnalysis {
                loci,
                sources_scanned,
            }) => (
                SessionPhase::Synthesize,
                "done",
                format!(
                    "loci analysis: {} dimension(s) from {sources_scanned} source(s)",
                    loci.loci.len()
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Analysis(AnalysisEvent::DepthInvestigation { investigations }) => (
                SessionPhase::Synthesize,
                "done",
                format!(
                    "depth investigation: {} loci classified",
                    investigations.len()
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Analysis(AnalysisEvent::CrossLocusReconcile { reconcile }) => (
                SessionPhase::Synthesize,
                "done",
                format!("cross-locus reconcile: {} pair(s)", reconcile.pairs.len()),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Analysis(AnalysisEvent::SourceTensions { tensions }) => (
                SessionPhase::Synthesize,
                "done",
                format!("source tensions: {} recorded", tensions.tensions.len()),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Analysis(AnalysisEvent::EvidenceDigest { digest }) => (
                SessionPhase::Synthesize,
                "done",
                format!(
                    "evidence digest: {} claim(s) from {} source(s)",
                    digest.claims.len(),
                    digest.sources_scanned
                ),
                None,
                0,
                0,
                0,
            ),
            SessionEvent::Analysis(AnalysisEvent::TripleDraft { draft }) => (
                SessionPhase::Synthesize,
                "done",
                format!(
                    "triple draft: {} candidate(s) produced",
                    draft.candidates.len()
                ),
                None,
                0,
                0,
                0,
            ),
        };
    let payload = ProgressPayload {
        name: name.to_string(),
        topic: topic.to_string(),
        phase: phase.as_str().to_string(),
        status: status.to_string(),
        detail,
        total_sources,
        pdf_count,
        youtube_count,
        excluded_count,
    };
    let json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    format!("{PROGRESS_SENTINEL}{json}")
}

/// Map a pipeline-step name to the session phase that owns it.
///
/// Decompose and width-sweep run during web gathering; every other
/// manifest step belongs to the synthesize pipeline. Ad-hoc steps
/// (e.g. `vault_sufficient`) also surface during web gathering.
fn run_step_phase(step: &str) -> SessionPhase {
    match step {
        "decompose" | "width_sweep" => SessionPhase::Web,
        "vault_sufficient" => SessionPhase::Setup,
        _ => SessionPhase::Synthesize,
    }
}

/// Map a run-manifest pipeline status string to a TUI step status.
///
/// Unknown statuses fall through to `'event'`, which decodes as
/// [`StepStatus::Done`] while keeping the detail visible in the log.
fn run_step_status(status: &str) -> &str {
    match status {
        "pending" | "in_progress" => "started",
        "completed" => "done",
        "failed" => "error",
        "skipped" => "skipped",
        "event" | "tier done" => "event",
        _ => "event",
    }
}

/// Human-readable description for a phase-start event.
fn phase_description(phase: SessionPhase) -> String {
    match phase {
        SessionPhase::Setup => "creating research item".to_string(),
        SessionPhase::Web => "searching the web".to_string(),
        SessionPhase::Local => "scanning project files".to_string(),
        SessionPhase::Specs => "cross-referencing specs".to_string(),
        SessionPhase::Synthesize => "synthesizing analysis with LLM".to_string(),
        SessionPhase::Assemble => "assembling RESEARCH.md".to_string(),
        SessionPhase::Finalize => "finalizing".to_string(),
    }
}

/// Parsed result of decoding a sentinel-prefixed message.
#[derive(Debug, Clone)]
pub struct DecodedProgress {
    /// Research item name.
    pub name: String,
    /// Research topic.
    pub topic: String,
    /// Phase the event belongs to.
    pub phase: SessionPhase,
    /// Step status (started or done).
    pub status: StepStatus,
    /// Human-readable detail line.
    pub detail: String,
    /// Total source count, present only on the final `Done` event.
    pub total_sources: Option<usize>,
    /// Number of recovered PDF documents.
    pub pdf_count: usize,
    /// Number of recovered YouTube transcripts / video URLs.
    pub youtube_count: usize,
    /// Number of web sources fetched but excluded for low relevance.
    pub excluded_count: usize,
}

/// Try to decode an [`Event::AgentNotice`] message as a research progress
/// update. Returns `None` if the message is not sentinel-prefixed or the
/// payload is malformed.
pub fn decode_progress_event(message: &str) -> Option<DecodedProgress> {
    let rest = message.strip_prefix(PROGRESS_SENTINEL)?;
    let payload: ProgressPayload = serde_json::from_str(rest).ok()?;
    let phase = parse_phase(&payload.phase)?;
    let status = match payload.status.as_str() {
        "started" | "in_progress" | "pending" => StepStatus::Started,
        "completed" | "captured" | "done" | "queries" | "preview" | "config" | "event"
        | "tier done" => StepStatus::Done,
        "failed" | "failed_url" | "error" => StepStatus::Error,
        "excluded_url" => StepStatus::Excluded,
        "skipped" => StepStatus::Skipped,
        _ => return None,
    };
    Some(DecodedProgress {
        name: payload.name,
        topic: payload.topic,
        phase,
        status,
        detail: payload.detail,
        total_sources: payload.total_sources,
        pdf_count: payload.pdf_count,
        youtube_count: payload.youtube_count,
        excluded_count: payload.excluded_count,
    })
}

/// Map a phase string back to a [`SessionPhase`].
fn parse_phase(s: &str) -> Option<SessionPhase> {
    Some(match s {
        "setup" => SessionPhase::Setup,
        "web" => SessionPhase::Web,
        "local" => SessionPhase::Local,
        "specs" => SessionPhase::Specs,
        "synthesize" => SessionPhase::Synthesize,
        "assemble" => SessionPhase::Assemble,
        "finalize" => SessionPhase::Finalize,
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_decode_roundtrip_queries_decomposed() {
        let queries = vec![
            "async rust ecosystem overview".into(),
            "tokio vs async-std comparison".into(),
        ];
        let encoded = encode_progress_event(
            "foo",
            "bar",
            &SessionEvent::QueriesDecomposed {
                queries: queries.clone(),
            },
        );
        let decoded = decode_progress_event(&encoded).expect("decode");
        assert_eq!(decoded.phase, SessionPhase::Web);
        assert_eq!(decoded.status, StepStatus::Done);
        assert!(
            decoded.detail.contains("decomposed into 2 queries"),
            "detail should report actual count: {}",
            decoded.detail
        );
        for q in &queries {
            assert!(
                decoded.detail.contains(q),
                "detail should list each query: {}",
                decoded.detail
            );
        }
    }

    #[test]
    fn test_encode_decode_roundtrip_from_url_body_preview() {
        let encoded = encode_progress_event(
            "rust-async",
            "async rust",
            &SessionEvent::FromUrlBodyPreview {
                url: "https://example.com/guide".into(),
                body_preview: "Long-form article about Rust async/await idioms.".into(),
            },
        );
        let decoded = decode_progress_event(&encoded).expect("decode");
        assert_eq!(decoded.phase, SessionPhase::Setup);
        assert_eq!(decoded.status, StepStatus::Done);
        assert!(
            decoded.detail.contains("https://example.com/guide"),
            "detail should mention the URL: {}",
            decoded.detail
        );
        assert!(
            decoded
                .detail
                .contains("Long-form article about Rust async/await"),
            "detail should include the body preview: {}",
            decoded.detail
        );
    }

    #[test]
    fn test_encode_decode_roundtrip_phase() {
        let encoded = encode_progress_event(
            "rust-async",
            "async rust",
            &SessionEvent::Phase {
                phase: SessionPhase::Web,
            },
        );
        assert!(encoded.starts_with(PROGRESS_SENTINEL));
        let decoded = decode_progress_event(&encoded).expect("decode");
        assert_eq!(decoded.name, "rust-async");
        assert_eq!(decoded.topic, "async rust");
        assert_eq!(decoded.phase, SessionPhase::Web);
        assert_eq!(decoded.status, StepStatus::Started);
        assert_eq!(decoded.detail, "searching the web");
    }

    #[test]
    fn test_encode_decode_roundtrip_done() {
        let encoded = encode_progress_event(
            "foo",
            "bar",
            &SessionEvent::Done {
                total_sources: 7,
                pdf_count: 2,
                youtube_count: 1,
                excluded_count: 0,
            },
        );
        let decoded = decode_progress_event(&encoded).expect("decode");
        assert_eq!(decoded.phase, SessionPhase::Finalize);
        assert_eq!(decoded.status, StepStatus::Done);
        assert_eq!(decoded.total_sources, Some(7));
        assert_eq!(decoded.pdf_count, 2);
        assert_eq!(decoded.youtube_count, 1);
    }

    #[test]
    fn test_encode_decode_roundtrip_synthesize() {
        let encoded = encode_progress_event(
            "foo",
            "bar",
            &SessionEvent::Phase {
                phase: SessionPhase::Synthesize,
            },
        );
        let decoded = decode_progress_event(&encoded).expect("decode");
        assert_eq!(decoded.phase, SessionPhase::Synthesize);
    }

    #[test]
    fn test_synthesize_result_llm_outcome_renders_cleanly() {
        let encoded = encode_progress_event(
            "foo",
            "bar",
            &SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult {
                outcome: SynthesizeOutcome::Llm,
                detail: None,
            }),
        );
        let decoded = decode_progress_event(&encoded).expect("decode");
        assert_eq!(decoded.phase, SessionPhase::Synthesize);
        assert_eq!(decoded.status, StepStatus::Done);
        assert!(decoded.detail.contains("LLM analysis applied"));
    }

    #[test]
    fn test_synthesize_result_fallback_error_includes_detail() {
        let encoded = encode_progress_event(
            "foo",
            "bar",
            &SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult {
                outcome: SynthesizeOutcome::FallbackError,
                detail: Some("provider returned 401".into()),
            }),
        );
        let decoded = decode_progress_event(&encoded).expect("decode");
        assert!(decoded.detail.contains("provider returned 401"));
        assert!(decoded.detail.contains("mechanical fallback"));
    }

    #[test]
    fn test_synthesize_result_no_llm_renders_cleanly() {
        let encoded = encode_progress_event(
            "foo",
            "bar",
            &SessionEvent::Synthesis(SynthesisEvent::SynthesizeResult {
                outcome: SynthesizeOutcome::NoLlm,
                detail: None,
            }),
        );
        let decoded = decode_progress_event(&encoded).expect("decode");
        assert!(decoded.detail.contains("no LLM engine configured"));
    }

    #[test]
    fn test_encode_decode_roundtrip_polish_and_readability_audit() {
        let encoded = encode_progress_event(
            "foo",
            "bar",
            &SessionEvent::Synthesis(SynthesisEvent::Polish {
                result: ragent_research::PolishResult {
                    changes: vec![ragent_research::PolishChange {
                        field: "summary".into(),
                        description: "normalized whitespace".into(),
                    }],
                    control_chars_removed: 1,
                    whitespace_normalized: 2,
                    empty_paragraphs_removed: 3,
                    note: "Polished".into(),
                },
            }),
        );
        let decoded = decode_progress_event(&encoded).expect("decode");
        assert_eq!(decoded.phase, SessionPhase::Synthesize);
        assert_eq!(decoded.status, StepStatus::Done);
        assert!(decoded.detail.contains("polish"));
        assert!(decoded.detail.contains("1 control char"));

        let encoded = encode_progress_event(
            "foo",
            "bar",
            &SessionEvent::Synthesis(SynthesisEvent::ReadabilityAudit {
                result: ragent_research::ReadabilityAudit {
                    score: 85,
                    passed: true,
                    issues: vec!["issue".into()],
                    recommendations: vec!["rec".into()],
                    avg_finding_length: 400,
                    missing_label_count: 0,
                    long_paragraph_count: 0,
                },
            }),
        );
        let decoded = decode_progress_event(&encoded).expect("decode");
        assert_eq!(decoded.phase, SessionPhase::Synthesize);
        assert_eq!(decoded.status, StepStatus::Done);
        assert!(decoded.detail.contains("readability audit"));
        assert!(decoded.detail.contains("85/100"));
    }

    #[test]
    fn test_decode_rejects_non_sentinel() {
        assert!(decode_progress_event("ragent-research: {...}").is_none());
        assert!(decode_progress_event("plain text").is_none());
    }

    #[test]
    fn test_decode_rejects_malformed_payload() {
        assert!(decode_progress_event(&format!("{PROGRESS_SENTINEL}not json")).is_none());
        assert!(decode_progress_event(&format!("{PROGRESS_SENTINEL}{{}}")).is_none());
    }

    #[test]
    fn test_progress_apply_appends_then_completes() {
        let mut p = ResearchProgress::new("n", "t");
        p.apply(SessionPhase::Web, StepStatus::Started, "searching the web");
        assert_eq!(p.steps.len(), 1);
        assert_eq!(p.steps[0].status, StepStatus::Started);
        p.apply(SessionPhase::Web, StepStatus::Done, "3 source(s) captured");
        assert_eq!(p.steps.len(), 1, "done updates in place");
        assert_eq!(p.steps[0].status, StepStatus::Done);
        assert_eq!(p.steps[0].detail, "3 source(s) captured");
    }

    #[test]
    fn test_progress_render_shows_log_list() {
        let mut p = ResearchProgress::new("rust-async", "async rust");
        p.apply(
            SessionPhase::Setup,
            StepStatus::Started,
            "creating research item",
        );
        p.apply(
            SessionPhase::Setup,
            StepStatus::Done,
            "creating research item",
        );
        p.apply(SessionPhase::Web, StepStatus::Started, "searching the web");
        p.apply(SessionPhase::Web, StepStatus::Done, "3 source(s) captured");
        p.finish(3, 0, 0, 0);
        let rendered = p.render();
        assert!(rendered.contains("🔬 Research Progress"));
        assert!(rendered.contains("✓ setup"));
        assert!(rendered.contains("✓ web"));
        assert!(rendered.contains("✅ Complete — 3 source(s)"));
        assert!(rendered.contains("/research open rust-async"));
    }

    #[test]
    fn test_progress_render_indents_multiline_detail() {
        let mut p = ResearchProgress::new("rust-async", "async rust");
        p.apply(
            SessionPhase::Web,
            StepStatus::Done,
            "decomposed into 3 queries:\n  - query one\n  - query two\n  - query three",
        );
        let rendered = p.render();
        assert!(rendered.contains("decomposed into 3 queries:"));
        assert!(rendered.contains("  - query one"));
        assert!(rendered.contains("  - query two"));
        assert!(rendered.contains("  - query three"));
        // Continuation lines should be indented to align with the first detail column.
        let lines: Vec<&str> = rendered.lines().collect();
        let first_idx = lines
            .iter()
            .position(|l| l.contains("decomposed into 3 queries"))
            .expect("first detail line");
        assert!(
            lines[first_idx + 1].starts_with("              "),
            "continuation line should be indented: {}",
            lines[first_idx + 1]
        );
    }
}

#[test]
fn test_failed_urls_rolled_into_totals_line() {
    let mut p = ResearchProgress::new("rust-async", "async rust");
    p.apply(SessionPhase::Web, StepStatus::Started, "searching the web");
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "[ENGLISH] captured https://a.com — A",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Error,
        "fetch failed for https://b.com: 403",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "[FRENCH] captured https://c.com — C",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Error,
        "fetch failed for https://d.com: timeout",
    );

    let rendered = p.render();
    assert!(
        !rendered.contains("fetch failed for"),
        "rendered output should not contain per-URL failure lines:\n{rendered}"
    );
    assert!(
        rendered.contains("📊 Web fetch totals: 2 fetched, 2 failed"),
        "rendered output should show the totals line:\n{rendered}"
    );
    assert_eq!(
        p.steps.len(),
        2,
        "start step is folded into the first captured URL; only captured lines logged"
    );
}

#[test]
fn test_complete_message_replaces_totals_line() {
    let mut p = ResearchProgress::new("rust-async", "async rust");
    p.apply(SessionPhase::Web, StepStatus::Started, "searching the web");
    p.apply(
        SessionPhase::Web,
        StepStatus::Done,
        "[ENGLISH] captured https://a.com — A",
    );
    p.apply(
        SessionPhase::Web,
        StepStatus::Error,
        "fetch failed for https://b.com: 403",
    );
    p.finish(1, 0, 0, 0);

    let rendered = p.render();
    assert!(
        !rendered.contains("fetch failed for"),
        "rendered output should not contain per-URL failure lines:\n{rendered}"
    );
    assert!(
        !rendered.contains("Web fetch totals"),
        "totals line should be replaced by the complete line:\n{rendered}"
    );
    assert!(
        rendered.contains("✅ Complete — 1 source(s)"),
        "rendered output should show the final complete line:\n{rendered}"
    );
}

#[test]
fn test_failed_url_status_decoded_as_error() {
    let encoded = encode_progress_event(
        "foo",
        "bar",
        &SessionEvent::WebFetchFailed {
            url: "https://x.com".into(),
            error: "403".into(),
        },
    );
    let decoded = decode_progress_event(&encoded).expect("decode");
    assert_eq!(decoded.phase, SessionPhase::Web);
    assert_eq!(decoded.status, StepStatus::Error);
    assert!(decoded.detail.contains("fetch failed for https://x.com"));
}
