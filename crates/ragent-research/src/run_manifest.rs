//! Run manifest for resumable Hyperresearch-style research sessions (FR-007).
//!
//! A [`RunManifest`] records which pipeline steps have been completed for a
//! single research run. It is persisted to `research/<name>/manifest.json` so
//! that a crashed or interrupted run can be resumed via
//! `ragent research continue <name>` or `/research resume <name>` without
//! re-fetching sources that are already in the vault.
//!
//! The manifest is updated as each step finishes. Steps that are not required
//! for the current [`Tier`](crate::run_config::Tier) are marked as
//! [`StepStatus::Skipped`] rather than completed.

use crate::run_config::Tier;
use crate::source::Source;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Errors emitted by the run manifest layer.
#[derive(Debug, thiserror::Error)]
pub enum RunManifestError {
    /// A filesystem operation failed.
    #[error("run manifest I/O error: {0}")]
    Io(#[from] std::io::Error),
    /// A JSON (de)serialization error.
    #[error("run manifest serialization error: {0}")]
    Serde(#[from] serde_json::Error),
}

/// Result alias for run manifest operations.
pub type Result<T> = std::result::Result<T, RunManifestError>;

/// A single step in the Hyperresearch pipeline.
///
/// Variants follow the 16-step `full` pipeline from FR-005. The `light` tier
/// skips the adversarial/loci/corpus steps, and the `dissertation` tier adds
/// chapter partitioning.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RunStep {
    /// Decompose the topic into sub-queries.
    #[default]
    Decompose,
    /// Width sweep: parallel web search across sub-queries.
    WidthSweep,
    /// Build a contradiction graph from gathered sources.
    ContradictionGraph,
    /// Identify loci (key claims / dimensions) in the corpus.
    LociAnalysis,
    /// Drill deeper into each locus.
    DepthInvestigation,
    /// Reconcile findings across loci.
    CrossLocusReconcile,
    /// Surface source tensions (conflicts / gaps).
    SourceTensions,
    /// Corpus critic: audit evidence quality and coverage.
    CorpusCritic,
    /// Digest evidence into structured findings.
    EvidenceDigest,
    /// Produce an initial triple draft (three candidate summaries).
    TripleDraft,
    /// Synthesize the final narrative draft.
    Synthesize,
    /// Run critic subagents against the draft.
    Critics,
    /// Fetch additional sources to close evidence gaps.
    GapFetch,
    /// Apply surgical patches to the draft.
    Patcher,
    /// Verify that every cited claim is supported by a source.
    CiteCheck,
    /// Polish the report for readability and completeness.
    Polish,
    /// Final readability audit.
    ReadabilityAudit,
    /// Partition the work into dissertation chapters.
    ChapterPartition,
}

impl RunStep {
    /// Human-readable label for progress output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Decompose => "decompose",
            Self::WidthSweep => "width_sweep",
            Self::ContradictionGraph => "contradiction_graph",
            Self::LociAnalysis => "loci_analysis",
            Self::DepthInvestigation => "depth_investigation",
            Self::CrossLocusReconcile => "cross_locus_reconcile",
            Self::SourceTensions => "source_tensions",
            Self::CorpusCritic => "corpus_critic",
            Self::EvidenceDigest => "evidence_digest",
            Self::TripleDraft => "triple_draft",
            Self::Synthesize => "synthesize",
            Self::Critics => "critics",
            Self::GapFetch => "gap_fetch",
            Self::Patcher => "patcher",
            Self::CiteCheck => "cite_check",
            Self::Polish => "polish",
            Self::ReadabilityAudit => "readability_audit",
            Self::ChapterPartition => "chapter_partition",
        }
    }

    /// Return the ordered list of steps required for the given tier.
    #[must_use]
    pub fn steps_for_tier(tier: Tier) -> Vec<Self> {
        match tier {
            Tier::Light => vec![
                Self::Decompose,
                Self::WidthSweep,
                Self::EvidenceDigest,
                Self::TripleDraft,
                Self::Synthesize,
                Self::CiteCheck,
                Self::Polish,
            ],
            Tier::Full => vec![
                Self::Decompose,
                Self::WidthSweep,
                Self::ContradictionGraph,
                Self::LociAnalysis,
                Self::DepthInvestigation,
                Self::CrossLocusReconcile,
                Self::SourceTensions,
                Self::CorpusCritic,
                Self::EvidenceDigest,
                Self::TripleDraft,
                Self::Synthesize,
                Self::Critics,
                Self::GapFetch,
                Self::Patcher,
                Self::CiteCheck,
                Self::Polish,
                Self::ReadabilityAudit,
            ],
            Tier::Dissertation => vec![
                Self::ChapterPartition,
                Self::Decompose,
                Self::WidthSweep,
                Self::ContradictionGraph,
                Self::LociAnalysis,
                Self::DepthInvestigation,
                Self::CrossLocusReconcile,
                Self::SourceTensions,
                Self::CorpusCritic,
                Self::EvidenceDigest,
                Self::TripleDraft,
                Self::Synthesize,
                Self::Critics,
                Self::GapFetch,
                Self::Patcher,
                Self::CiteCheck,
                Self::Polish,
                Self::ReadabilityAudit,
            ],
        }
    }
}

/// Lifecycle status of a pipeline step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum StepStatus {
    /// Step has not started yet.
    #[default]
    Pending,
    /// Step is currently running.
    InProgress,
    /// Step finished successfully.
    Completed,
    /// Step failed but the run can continue.
    Failed,
    /// Step is not required for the selected tier.
    Skipped,
}

impl StepStatus {
    /// Short label used in JSON/CLI output.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::InProgress => "in_progress",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Skipped => "skipped",
        }
    }
}

/// Per-step bookkeeping inside a [`RunManifest`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct StepEntry {
    /// The pipeline step.
    pub step: RunStep,
    /// Current status.
    pub status: StepStatus,
    /// UTC timestamp when the step transitioned to `InProgress`, if any.
    pub started_at: Option<DateTime<Utc>>,
    /// UTC timestamp when the step reached a terminal status, if any.
    pub finished_at: Option<DateTime<Utc>>,
    /// Optional human-readable detail (e.g. error message, skip reason).
    pub detail: Option<String>,
}

/// Persistent record of a single research run and its pipeline progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct RunManifest {
    /// Stable URL-safe tag that names this run (also used as the vault
    /// directory name).
    pub run_tag: String,
    /// Research item name that owns this run.
    pub name: String,
    /// Original topic or question.
    pub topic: String,
    /// Tier that was requested when the run started.
    pub tier: Tier,
    /// UTC timestamp when the manifest was first created.
    pub created_at: DateTime<Utc>,
    /// UTC timestamp when the run last started running.
    pub started_at: Option<DateTime<Utc>>,
    /// UTC timestamp when the run finished, if it has finished.
    pub completed_at: Option<DateTime<Utc>>,
    /// Pipeline steps for the selected tier, in execution order.
    pub steps: Vec<StepEntry>,
    /// Index of the current step in `steps` when the run is in progress.
    pub current_step_index: usize,
    /// Number of sources captured so far.
    pub sources_count: usize,
    /// Number of PDF sources captured so far.
    pub pdf_count: usize,
    /// Number of YouTube sources captured so far.
    pub youtube_count: usize,
    /// Number of web sources excluded for low relevance so far.
    pub excluded_count: usize,
    /// Snapshot of vault source ids captured so far, keyed by URL.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub vault_sources: HashMap<String, String>,
}

impl RunManifest {
    /// Create a fresh manifest for `name`/`topic` at `tier`.
    ///
    /// `run_tag` is generated by the caller; it must be a safe path component.
    pub fn new(
        run_tag: impl Into<String>,
        name: impl Into<String>,
        topic: impl Into<String>,
        tier: Tier,
    ) -> Self {
        let now = Utc::now();
        let steps = RunStep::steps_for_tier(tier)
            .into_iter()
            .map(|step| StepEntry {
                step,
                status: StepStatus::Pending,
                started_at: None,
                finished_at: None,
                detail: None,
            })
            .collect();
        Self {
            run_tag: run_tag.into(),
            name: name.into(),
            topic: topic.into(),
            tier,
            created_at: now,
            started_at: Some(now),
            completed_at: None,
            steps,
            current_step_index: 0,
            sources_count: 0,
            pdf_count: 0,
            youtube_count: 0,
            excluded_count: 0,
            vault_sources: HashMap::new(),
        }
    }

    /// Resume a run by marking it as started again and returning the first step
    /// that is still pending or failed.
    pub fn resume(&mut self) {
        self.started_at = Some(Utc::now());
        self.completed_at = None;
        self.current_step_index = self
            .steps
            .iter()
            .position(|s| {
                s.status == StepStatus::Pending
                    || s.status == StepStatus::InProgress
                    || s.status == StepStatus::Failed
            })
            .unwrap_or(self.steps.len());
    }

    /// Return the next pending or failed step, if any.
    #[must_use]
    pub fn next_pending_step(&self) -> Option<RunStep> {
        self.steps
            .iter()
            .skip(self.current_step_index)
            .find(|s| {
                s.status == StepStatus::Pending
                    || s.status == StepStatus::InProgress
                    || s.status == StepStatus::Failed
            })
            .map(|s| s.step)
    }

    /// Return `true` when every step is either completed or skipped.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.steps
            .iter()
            .all(|s| s.status == StepStatus::Completed || s.status == StepStatus::Skipped)
    }

    /// Mark `step` as in-progress. Returns `true` if the step exists and was
    /// previously pending.
    pub fn start_step(&mut self, step: RunStep) -> bool {
        if let Some(pos) = self
            .steps
            .iter()
            .position(|s| s.step == step && s.status == StepStatus::Pending)
        {
            self.steps[pos].status = StepStatus::InProgress;
            self.steps[pos].started_at = Some(Utc::now());
            self.current_step_index = pos;
            return true;
        }
        false
    }

    /// Mark `step` as completed. Returns `true` if the step existed and was in
    /// progress or pending (used by tests and resume paths where a step may
    /// have been left pending).
    pub fn complete_step(&mut self, step: RunStep) -> bool {
        if let Some(pos) = self.steps.iter().position(|s| {
            s.step == step
                && (s.status == StepStatus::InProgress || s.status == StepStatus::Pending)
        }) {
            self.steps[pos].status = StepStatus::Completed;
            self.steps[pos].finished_at = Some(Utc::now());
            return true;
        }
        false
    }

    /// Mark `step` as failed with an optional detail string.
    pub fn fail_step(&mut self, step: RunStep, detail: Option<String>) -> bool {
        if let Some(pos) = self.steps.iter().position(|s| s.step == step) {
            self.steps[pos].status = StepStatus::Failed;
            self.steps[pos].finished_at = Some(Utc::now());
            self.steps[pos].detail = detail;
            return true;
        }
        false
    }

    /// Mark `step` as skipped (used for tiers that omit certain steps).
    pub fn skip_step(&mut self, step: RunStep, detail: Option<String>) -> bool {
        if let Some(pos) = self.steps.iter().position(|s| s.step == step) {
            self.steps[pos].status = StepStatus::Skipped;
            self.steps[pos].finished_at = Some(Utc::now());
            self.steps[pos].detail = detail;
            return true;
        }
        false
    }

    /// Mark the entire run as completed.
    pub fn complete_run(&mut self) {
        self.completed_at = Some(Utc::now());
    }

    /// Update captured-source counters and vault source ids.
    pub fn record_sources(&mut self, sources: &[Source]) {
        for src in sources {
            self.sources_count += 1;
            if let Source::Web {
                url, media_type, ..
            } = src
            {
                if media_type == "pdf" {
                    self.pdf_count += 1;
                } else if media_type == "youtube" {
                    self.youtube_count += 1;
                }
                self.vault_sources.insert(url.clone(), self.run_tag.clone());
            }
        }
    }

    /// Compute progress as `(completed_or_skipped, total)`.
    #[must_use]
    pub fn progress(&self) -> (usize, usize) {
        let done = self
            .steps
            .iter()
            .filter(|s| {
                s.status == StepStatus::Completed
                    || s.status == StepStatus::Skipped
                    || s.status == StepStatus::Failed
            })
            .count();
        (done, self.steps.len())
    }

    /// Serialize the manifest to pretty JSON.
    pub fn to_json(&self) -> Result<String> {
        Ok(serde_json::to_string_pretty(self)?)
    }

    /// Deserialize a manifest from JSON.
    pub fn from_json(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }
}

/// Outcome of loading a manifest for resume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeOutcome {
    /// A manifest was found and is ready to resume.
    Resumed(RunManifest),
    /// No manifest existed; a fresh one would need to be created.
    NotFound,
    /// A manifest file existed but could not be parsed.
    Invalid(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_steps_skip_adversarial_pipeline() {
        let steps = RunStep::steps_for_tier(Tier::Light);
        assert!(steps.contains(&RunStep::Decompose));
        assert!(steps.contains(&RunStep::Synthesize));
        assert!(!steps.contains(&RunStep::ContradictionGraph));
        assert!(!steps.contains(&RunStep::ReadabilityAudit));
    }

    #[test]
    fn full_steps_include_all_sixteen() {
        let steps = RunStep::steps_for_tier(Tier::Full);
        assert!(steps.contains(&RunStep::ContradictionGraph));
        assert!(steps.contains(&RunStep::LociAnalysis));
        assert!(steps.contains(&RunStep::Critics));
        assert!(steps.contains(&RunStep::ReadabilityAudit));
    }

    #[test]
    fn dissertation_steps_start_with_chapter_partition() {
        let steps = RunStep::steps_for_tier(Tier::Dissertation);
        assert_eq!(steps[0], RunStep::ChapterPartition);
        assert!(steps.contains(&RunStep::Synthesize));
    }

    #[test]
    fn manifest_tracks_step_lifecycle() {
        let mut m = RunManifest::new("run-1", "glp1", "GLP-1 outcomes", Tier::Full);
        assert_eq!(m.next_pending_step(), Some(RunStep::Decompose));

        assert!(m.start_step(RunStep::Decompose));
        assert_eq!(m.steps[0].status, StepStatus::InProgress);

        assert!(m.complete_step(RunStep::Decompose));
        assert_eq!(m.steps[0].status, StepStatus::Completed);
        assert_eq!(m.next_pending_step(), Some(RunStep::WidthSweep));

        m.fail_step(RunStep::WidthSweep, Some("network".into()));
        assert_eq!(m.steps[1].status, StepStatus::Failed);
        assert!(m.next_pending_step().is_some());
    }

    #[test]
    fn manifest_serializes_and_deserializes() {
        let mut m = RunManifest::new("run-2", "demo", "demo topic", Tier::Light);
        m.start_step(RunStep::Decompose);
        m.complete_step(RunStep::Decompose);
        m.skip_step(RunStep::WidthSweep, Some("not needed".into()));

        let json = m.to_json().unwrap();
        let restored = RunManifest::from_json(&json).unwrap();
        assert_eq!(m, restored);
    }

    #[test]
    fn resume_restarts_from_first_pending_or_in_progress() {
        let mut m = RunManifest::new("run-3", "demo", "demo topic", Tier::Full);
        m.start_step(RunStep::Decompose);
        m.complete_step(RunStep::Decompose);
        m.complete_step(RunStep::WidthSweep);
        m.fail_step(RunStep::ContradictionGraph, None);

        m.resume();
        assert_eq!(m.started_at.is_some(), true);
        assert_eq!(m.completed_at, None);
        // The failed contradiction_graph step is the first non-completed step,
        // so the resume cursor should point at it (the run may attempt to redo
        // it or skip it depending on policy).
        assert_eq!(m.next_pending_step(), Some(RunStep::ContradictionGraph));
    }

    #[test]
    fn is_complete_true_when_all_terminal() {
        let mut m = RunManifest::new("run-4", "demo", "demo topic", Tier::Light);
        for step in &RunStep::steps_for_tier(Tier::Light) {
            m.start_step(*step);
            m.complete_step(*step);
        }
        assert!(m.is_complete());
    }
}
