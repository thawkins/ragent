//! Live stage progress for a `/research deck` run (spec `diffdeck` FR-031).
//!
//! A deck run is quick on the deterministic path but slow whenever it does real
//! work: the enrichment stage issues one model call per finding plus one grouping
//! call, and the optional presentation export is one round trip per slide. Before
//! this module the TUI published nothing until the run finished, so the message
//! window sat silent for the whole of the slowest part.
//!
//! Three pieces make the progress visible:
//!
//! - [`DeckTracker`] is the run's state: a fixed stage list with each stage's
//!   status and detail, plus the completed/total counts of a countable stage.
//!   It renders as the same `[research] Research Progress - \`{name}\`` message
//!   the `/research create` tracker uses, so a deck run replaces its own message
//!   in place instead of appending a new one per stage.
//! - [`encode_deck_progress_event`] carries one stage transition to the message
//!   window as a sentinel-prefixed [`Event::AgentNotice`], which is how every
//!   other research progress line already travels; the sentinel keeps the change
//!   local to the TUI instead of adding an event variant to `ragent-types`.
//! - [`DeckProgressAdapter`] implements
//!   [`ragent_research::deck_progress::DeckProgress`] over an [`EventBus`], so the
//!   research crate can report stages without knowing anything about events.

use std::sync::Arc;

use ragent_research::deck_progress::{DeckEvent, DeckProgress, DeckStage, DeckStepStatus};
use ragent_types::event::{Event, EventBus};

/// A sentinel marking an [`Event::AgentNotice`] message as a deck progress
/// update. The remainder of the message is a JSON payload produced by
/// [`encode_deck_progress_event`].
///
/// It is distinct from [`crate::research_progress::PROGRESS_SENTINEL`] because a
/// deck report is a fixed stage list, not an append-only session log: routing it
/// through the session tracker would append a duplicate step per stage
/// transition.
pub const DECK_PROGRESS_SENTINEL: &str = "__research_deck_progress__";

/// One row of a deck run's stage list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckStageRow {
    /// Which stage this row reports.
    pub stage: DeckStage,
    /// The stage's state.
    pub status: DeckStepStatus,
    /// What the stage is doing or did.
    pub detail: String,
    /// Units completed, when the stage is countable.
    pub completed: usize,
    /// Units in total, when the stage is countable.
    pub total: Option<usize>,
}

impl DeckStageRow {
    /// The `N of M` marker this row prints, or an empty string when the stage is
    /// not countable.
    #[must_use]
    pub fn counter(&self) -> String {
        match self.total {
            Some(total) if total > 0 => {
                format!("[{}/{}] ", self.completed.min(total), total)
            }
            _ => String::new(),
        }
    }
}

/// The stage progress of one deck run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DeckTracker {
    /// One row per stage the run has reported, in the order first reported.
    ///
    /// The orchestration reports its stages in run order, so "order first
    /// reported" and "run order" agree; keeping the row list in arrival order
    /// rather than in [`ragent_research::deck_progress::DeckStage::ALL`] order
    /// means a stage whose row was never reported (because the run stopped
    /// before reaching it) is not rendered as a phantom pending line.
    pub rows: Vec<DeckStageRow>,
    /// `true` once the run has finished, so the renderer can close the list.
    pub finished: bool,
    /// Why the run failed, once it has; rendered instead of `[ok] Complete`.
    pub failure: Option<String>,
}

impl DeckTracker {
    /// A tracker with no report yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Apply one report, keyed by its stage.
    ///
    /// A report for a stage that already has a row replaces that row in place,
    /// so "started" becomes "done" (or "skipped") without a second line. A report
    /// carrying a unit count also sets `started` when the stage was previously
    /// marked done, which cannot happen in run order but keeps the row's
    /// `completed` count authoritative if a stage ever reports twice.
    pub fn apply(&mut self, event: &DeckEvent) {
        let row = DeckStageRow {
            stage: event.stage,
            status: event.status,
            detail: event.detail.clone(),
            completed: event.completed,
            total: event.total,
        };
        match self.rows.iter_mut().find(|r| r.stage == event.stage) {
            Some(existing) => {
                // A per-unit report during a stage keeps the stage's live state;
                // a terminal report replaces it.
                *existing = row;
            }
            None => self.rows.push(row),
        }
    }

    /// Mark the whole run finished.
    pub fn finish(&mut self) {
        self.finished = true;
    }

    /// Mark the whole run as ended by a failure.
    pub fn fail(&mut self, reason: impl Into<String>) {
        self.finished = true;
        self.failure = Some(reason.into());
    }

    /// The stage the run is currently working on, if any.
    #[must_use]
    pub fn current(&self) -> Option<&DeckStageRow> {
        self.rows
            .iter()
            .rev()
            .find(|row| row.status == DeckStepStatus::Started)
    }

    /// Render the stage list as the body of the run's message-window entry.
    ///
    /// The layout mirrors [`crate::research_progress::ResearchProgress::render`]:
    /// a `[research] Research Progress - \`{name}\`` header that the TUI matches on
    /// to replace this run's message in place, then one `icon stage - detail`
    /// line per stage.
    #[must_use]
    pub fn render(&self, name: &str) -> String {
        let mut out = String::new();
        out.push_str(&format!("[research] Research Progress - `{name}`\n"));
        out.push_str("Command: /research deck\n");
        out.push('\n');
        for row in &self.rows {
            out.push_str(&format!(
                "  {} {:<16} - {}{}\n",
                status_icon(row.status),
                row.stage.label(),
                row.counter(),
                row.detail
            ));
        }
        if let Some(reason) = &self.failure {
            out.push('\n');
            out.push_str(&format!("[err] Failed: {reason}\n"));
        } else if self.finished {
            out.push('\n');
            out.push_str("[ok] Complete\n");
        }
        out
    }
}

/// The icon for a stage state, matching the session tracker's vocabulary.
fn status_icon(status: DeckStepStatus) -> &'static str {
    match status {
        DeckStepStatus::Started => ">",
        DeckStepStatus::Done => "[ok]",
        DeckStepStatus::Skipped => "o",
    }
}

/// The JSON payload carried in a deck progress notice.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct DeckProgressPayload {
    name: String,
    stage: String,
    status: String,
    detail: String,
    completed: usize,
    total: Option<usize>,
}

/// Encode one deck stage report as a sentinel-prefixed `AgentNotice` message.
///
/// `name` is the research item, so the TUI can match the report to the run's
/// message and replace it in place.
#[must_use]
pub fn encode_deck_progress_event(name: &str, event: &DeckEvent) -> String {
    let payload = DeckProgressPayload {
        name: name.to_string(),
        stage: event.stage.label().to_string(),
        status: event.status.as_str().to_string(),
        detail: event.detail.clone(),
        completed: event.completed,
        total: event.total,
    };
    let json = serde_json::to_string(&payload).unwrap_or_else(|_| "{}".to_string());
    format!("{DECK_PROGRESS_SENTINEL}{json}")
}

/// A decoded deck progress notice.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedDeckProgress {
    /// The research item the run belongs to.
    pub name: String,
    /// The stage this report is about.
    pub stage: DeckStage,
    /// The stage's state.
    pub status: DeckStepStatus,
    /// What the stage is doing or did.
    pub detail: String,
    /// Units completed, when the stage is countable.
    pub completed: usize,
    /// Units in total, when the stage is countable.
    pub total: Option<usize>,
}

impl DecodedDeckProgress {
    /// The report this notice carries, ready to apply to a [`DeckTracker`].
    #[must_use]
    pub fn event(&self) -> DeckEvent {
        DeckEvent {
            stage: self.stage,
            status: self.status,
            detail: self.detail.clone(),
            total: self.total,
            completed: self.completed,
        }
    }
}

/// Decode a deck progress notice, returning `None` for any other message.
#[must_use]
pub fn decode_deck_progress_event(message: &str) -> Option<DecodedDeckProgress> {
    let rest = message.strip_prefix(DECK_PROGRESS_SENTINEL)?;
    let payload: DeckProgressPayload = serde_json::from_str(rest).ok()?;
    let stage = parse_stage(&payload.stage)?;
    let status = match payload.status.as_str() {
        "started" => DeckStepStatus::Started,
        "done" => DeckStepStatus::Done,
        "skipped" => DeckStepStatus::Skipped,
        _ => return None,
    };
    Some(DecodedDeckProgress {
        name: payload.name,
        stage,
        status,
        detail: payload.detail,
        completed: payload.completed,
        total: payload.total,
    })
}

/// Resolve a stage label back to its enum value.
fn parse_stage(label: &str) -> Option<DeckStage> {
    DeckStage::ALL
        .into_iter()
        .find(|stage| stage.label() == label)
}

/// A [`DeckProgress`] sink that publishes each report to the TUI event bus.
///
/// The TUI's `/research deck` handler builds one per run, so a report always
/// carries the right session and item name without the research crate having to
/// know either.
pub struct DeckProgressAdapter {
    event_bus: Arc<EventBus>,
    session_id: String,
    name: String,
}

impl DeckProgressAdapter {
    /// Build an adapter that publishes for `name` in `session_id`.
    #[must_use]
    pub fn new(event_bus: Arc<EventBus>, session_id: String, name: String) -> Self {
        Self {
            event_bus,
            session_id,
            name,
        }
    }
}

impl std::fmt::Debug for DeckProgressAdapter {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DeckProgressAdapter")
            .field("session_id", &self.session_id)
            .field("name", &self.name)
            .finish_non_exhaustive()
    }
}

impl DeckProgress for DeckProgressAdapter {
    fn report(&self, event: &DeckEvent) {
        self.event_bus.publish(Event::AgentNotice {
            session_id: self.session_id.clone(),
            message: encode_deck_progress_event(&self.name, event),
        });
    }
}

#[cfg(test)]
#[path = "../tests/inline/deck_progress_tests.rs"]
mod tests;
