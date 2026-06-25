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

use ragent_research::session::{SessionEvent, SessionPhase, SynthesizeOutcome};

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
}

impl StepStatus {
    /// Icon used in the rendered log list.
    pub fn icon(self) -> &'static str {
        match self {
            Self::Started => "▶",
            Self::Done => "✓",
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
        }
    }

    /// Apply a parsed step update, appending or completing a step.
    pub fn apply(&mut self, phase: SessionPhase, status: StepStatus, detail: impl Into<String>) {
        let detail = detail.into();
        // If the last step is the same phase and now we're marking it done,
        // update it in place rather than appending a duplicate line.
        if status == StepStatus::Done
            && let Some(last) = self.steps.last_mut()
            && last.phase == phase.as_str()
            && last.status == StepStatus::Started
        {
            last.status = StepStatus::Done;
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
    pub fn finish(&mut self, total_sources: usize) {
        self.total_sources = Some(total_sources);
        self.done = true;
    }

    /// Render the tracker as a markdown log list for the message window.
    pub fn render(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("🔬 Research Progress — `{}`\n", self.name));
        out.push_str(&format!("Topic: {}\n", self.topic));
        out.push('\n');
        for step in &self.steps {
            out.push_str(&format!(
                "  {} {:<8} — {}\n",
                step.status.icon(),
                step.phase,
                step.detail
            ));
        }
        if self.done
            && let Some(total) = self.total_sources
        {
            out.push('\n');
            out.push_str(&format!(
                "✅ Complete — {total} source(s). Use `/research open {}` to view the result.",
                self.name
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
}

/// Encode a [`SessionEvent`] plus run metadata as a sentinel-prefixed
/// `AgentNotice` message string.
pub fn encode_progress_event(name: &str, topic: &str, event: &SessionEvent) -> String {
    let (phase, status, detail, total_sources) = match event {
        SessionEvent::Phase { phase } => (*phase, "started", phase_description(*phase), None),
        SessionEvent::WebCaptured { url, title } => (
            SessionPhase::Web,
            "captured",
            format!("captured {} — {}", url, title),
            None,
        ),
        SessionEvent::LocalCaptured { path, score } => (
            SessionPhase::Local,
            "captured",
            format!("captured {path} (score {score})"),
            None,
        ),
        SessionEvent::SpecCaptured { spec_id } => (
            SessionPhase::Specs,
            "captured",
            format!("referenced spec {spec_id}"),
            None,
        ),
        SessionEvent::SynthesizeResult { outcome, detail } => {
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
            (SessionPhase::Synthesize, "done", detail, None)
        }
        SessionEvent::Done { total_sources } => (
            SessionPhase::Finalize,
            "done",
            "marked complete".to_string(),
            Some(*total_sources),
        ),
    };
    let payload = ProgressPayload {
        name: name.to_string(),
        topic: topic.to_string(),
        phase: phase.as_str().to_string(),
        status: status.to_string(),
        detail,
        total_sources,
    };
    let json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    format!("{PROGRESS_SENTINEL}{json}")
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
}

/// Try to decode an [`Event::AgentNotice`] message as a research progress
/// update. Returns `None` if the message is not sentinel-prefixed or the
/// payload is malformed.
pub fn decode_progress_event(message: &str) -> Option<DecodedProgress> {
    let rest = message.strip_prefix(PROGRESS_SENTINEL)?;
    let payload: ProgressPayload = serde_json::from_str(rest).ok()?;
    let phase = parse_phase(&payload.phase)?;
    let status = match payload.status.as_str() {
        "started" => StepStatus::Started,
        "captured" | "done" => StepStatus::Done,
        _ => return None,
    };
    Some(DecodedProgress {
        name: payload.name,
        topic: payload.topic,
        phase,
        status,
        detail: payload.detail,
        total_sources: payload.total_sources,
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
        let encoded = encode_progress_event("foo", "bar", &SessionEvent::Done { total_sources: 7 });
        let decoded = decode_progress_event(&encoded).expect("decode");
        assert_eq!(decoded.phase, SessionPhase::Finalize);
        assert_eq!(decoded.status, StepStatus::Done);
        assert_eq!(decoded.total_sources, Some(7));
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
            &SessionEvent::SynthesizeResult {
                outcome: SynthesizeOutcome::Llm,
                detail: None,
            },
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
            &SessionEvent::SynthesizeResult {
                outcome: SynthesizeOutcome::FallbackError,
                detail: Some("provider returned 401".into()),
            },
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
            &SessionEvent::SynthesizeResult {
                outcome: SynthesizeOutcome::NoLlm,
                detail: None,
            },
        );
        let decoded = decode_progress_event(&encoded).expect("decode");
        assert!(decoded.detail.contains("no LLM engine configured"));
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
        p.finish(3);
        let rendered = p.render();
        assert!(rendered.contains("🔬 Research Progress"));
        assert!(rendered.contains("✓ setup"));
        assert!(rendered.contains("✓ web"));
        assert!(rendered.contains("✅ Complete — 3 source(s)"));
        assert!(rendered.contains("/research open rust-async"));
    }
}
