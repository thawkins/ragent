//! Stage progress for a diff-deck run (spec `diffdeck` FR-031).
//!
//! A deck run is not instantaneous: the enrichment stage issues one model call
//! per finding plus one grouping call, and the optional presentation export
//! writes a slide at a time. Without a progress channel every surface has to
//! wait for the final outcome, so the run looks wedged for as long as the slowest
//! stage takes.
//!
//! [`DeckProgress`] is that channel. It is deliberately a *sink* - a trait the
//! caller implements - for the same reason [`crate::deck_pptx::DeckSlideSink`]
//! is: `ragent-research` must not depend on an event bus or a terminal, so the
//! orchestration reports "step N of M" to a transport-aware implementation
//! elsewhere (in this workspace the TUI's [`crate::deck_progress`] adapter and
//! the CLI's stderr reporter).
//!
//! Three properties keep the channel additive:
//!
//! - a run with no sink at all ([`DeckProgress::none`]) is a complete run, and
//!   every call site reports through an `Option`-free `()` so no stage has to
//!   special-case the absence;
//! - reporting never fails a run: an implementation must swallow its own
//!   delivery failure, because a progress line is not an artifact;
//! - the events describe the stages the orchestration actually runs, so a
//!   surface cannot disagree with the outcome it later prints.

use std::sync::Arc;

use crate::deck::DeckElement;

/// One stage of a deck run, in the order the orchestration performs it.
///
/// The order is the FR-001 stage list: select the pair, read both reports,
/// diff them, resolve and check the destination, enrich, render and publish,
/// then export the presentation. A stage that is skipped (no model, no sink)
/// reports [`DeckEvent::Skipped`] rather than going silent, so a reader can tell
/// "not applicable" apart from "still running".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DeckStage {
    /// FR-018: the item is looked up and the report series classified.
    Select,
    /// FR-019: both reports of the pair are read and parsed.
    Read,
    /// FR-004: the two reports are diffed element by element.
    Diff,
    /// FR-016, FR-017: the destination is resolved and checked.
    Destination,
    /// FR-023: one paragraph per finding, from the model or deterministically.
    EnrichFindings,
    /// FR-024: the findings are grouped under the concepts.
    EnrichConcepts,
    /// FR-003: the deck document is rendered and written atomically.
    Render,
    /// FR-030: the optional PowerPoint twin is written.
    Export,
}

impl DeckStage {
    /// Every stage, in run order.
    pub const ALL: [Self; 8] = [
        Self::Select,
        Self::Read,
        Self::Diff,
        Self::Destination,
        Self::EnrichFindings,
        Self::EnrichConcepts,
        Self::Render,
        Self::Export,
    ];

    /// The human-readable stage name a surface prints.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Select => "select",
            Self::Read => "read",
            Self::Diff => "diff",
            Self::Destination => "destination",
            Self::EnrichFindings => "enrich findings",
            Self::EnrichConcepts => "group concepts",
            Self::Render => "render",
            Self::Export => "export",
        }
    }

    /// This stage's 1-based position in the run order.
    #[must_use]
    pub fn ordinal(self) -> usize {
        Self::ALL
            .iter()
            .position(|stage| *stage == self)
            .map_or(1, |index| index + 1)
    }
}

/// A stage's state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeckStepStatus {
    /// The stage has begun and has not yet finished.
    Started,
    /// The stage finished successfully.
    Done,
    /// The stage was not applicable to this run (no model, no sink, no element).
    Skipped,
}

impl DeckStepStatus {
    /// The one-word status a surface prints.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Done => "done",
            Self::Skipped => "skipped",
        }
    }
}

/// One progress report from a deck run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeckEvent {
    /// Which stage this report is about.
    pub stage: DeckStage,
    /// The stage's state.
    pub status: DeckStepStatus,
    /// A human-readable detail line: what the stage is doing, or what it did.
    pub detail: String,
    /// How many units (items, findings, slides) the stage covers in total, when
    /// it is countable. `None` for the fixed stages, which have no unit count.
    pub total: Option<usize>,
    /// How many units the stage has completed so far.
    pub completed: usize,
}

impl DeckEvent {
    /// A start report for `stage`.
    #[must_use]
    pub fn started(stage: DeckStage, detail: impl Into<String>) -> Self {
        Self {
            stage,
            status: DeckStepStatus::Started,
            detail: detail.into(),
            total: None,
            completed: 0,
        }
    }

    /// A completion report for `stage`.
    #[must_use]
    pub fn done(stage: DeckStage, detail: impl Into<String>) -> Self {
        Self {
            stage,
            status: DeckStepStatus::Done,
            detail: detail.into(),
            total: None,
            completed: 0,
        }
    }

    /// A "not applicable" report for `stage`.
    #[must_use]
    pub fn skipped(stage: DeckStage, detail: impl Into<String>) -> Self {
        Self {
            stage,
            status: DeckStepStatus::Skipped,
            detail: detail.into(),
            total: None,
            completed: 0,
        }
    }

    /// A progress report for one unit of a countable stage.
    ///
    /// `completed` is the number of units finished *including* this one, so the
    /// first of five units reports `completed: 1, total: 5`.
    #[must_use]
    pub fn progress(
        stage: DeckStage,
        completed: usize,
        total: usize,
        detail: impl Into<String>,
    ) -> Self {
        Self {
            stage,
            status: DeckStepStatus::Started,
            detail: detail.into(),
            total: Some(total),
            completed,
        }
    }

    /// The `(completed, total)` pair, when this report is for a countable unit.
    #[must_use]
    pub const fn unit(&self) -> Option<(usize, usize)> {
        match self.total {
            Some(total) => Some((self.completed, total)),
            None => None,
        }
    }
}

/// A sink for a deck run's stage reports (FR-031).
///
/// Implementations must not panic and must not fail the run: a report that
/// cannot be delivered is dropped, because progress is a courtesy and the deck
/// is the artifact.
pub trait DeckProgress: Send + Sync {
    /// Report one stage transition or unit of progress.
    fn report(&self, event: &DeckEvent);
}

/// A progress sink that discards everything.
///
/// Used by [`crate::ResearchManager::deck`] and every other entry point that has
/// no surface to report to, so the orchestration has exactly one code path.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoopDeckProgress;

impl DeckProgress for NoopDeckProgress {
    fn report(&self, _event: &DeckEvent) {}
}

/// Report `event` to `sink` only when a sink was supplied.
///
/// The free function keeps every call site in the orchestration a one-liner (and
/// free of an `if let`), which is what makes the reporting cheap enough to leave
/// in the deterministic path.
pub fn report(sink: Option<&(dyn DeckProgress + '_)>, event: DeckEvent) {
    if let Some(sink) = sink {
        sink.report(&event);
    }
}

/// The label of the element a stage is working on, for a report's detail line.
///
/// `None` means "the whole deck", which is what the fixed stages report.
#[must_use]
pub fn element_label(element: Option<DeckElement>) -> &'static str {
    match element {
        Some(element) => element.label(),
        None => "deck",
    }
}

/// A shared, type-erased progress sink.
///
/// The orchestration takes `&dyn DeckProgress` for the same reason it takes
/// `&dyn DeckModel`: the caller owns the lifetime. This alias exists so a caller
/// that needs to hand ownership into a spawned task can keep one.
pub type SharedDeckProgress = Arc<dyn DeckProgress>;

#[cfg(test)]
#[path = "../tests/inline/deck_progress_tests.rs"]
mod tests;
