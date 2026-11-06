//! Tests for the deck run's stage progress reporting (spec `diffdeck` FR-031).
//!
//! The contract under test is that a deck run narrates its own stages, so a
//! surface can show a run that is working rather than one that has gone quiet:
//!
//! - every stage the run actually executes reports a start and a terminal state,
//!   in run order, and a stage that is not applicable reports `skipped` instead of
//!   going silent;
//! - a countable stage reports one unit report per unit, with a `completed`
//!   count that never exceeds its `total`, so a surface can render `N of M`;
//! - a run with no progress sink behaves exactly as before - progress is a
//!   courtesy, and the absence of a sink changes nothing about the artifact.
//!
//! Every case drives [`ResearchManager::deck_with_progress`] against a real item
//! directory inside a `tempfile::TempDir`, so the reports describe the run that
//! actually happened rather than a rehearsal.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use async_trait::async_trait;
use ragent_research::deck::DeckRunOptions;
use ragent_research::deck_pptx::{DeckPptxOutcome, DeckSlidePlan, DeckSlideSink};
use ragent_research::deck_progress::{DeckEvent, DeckProgress, DeckStage, DeckStepStatus};
use ragent_research::{ResearchManager, ResearchName};
use tempfile::TempDir;

/// The research root of a fresh temp directory.
fn root(dir: &TempDir) -> PathBuf {
    dir.path().join("research")
}

/// The directory of item `name` under `research_root`.
fn item_dir(research_root: &Path, name: &str) -> PathBuf {
    let name = ResearchName::try_new(name).expect("the fixture name is valid");
    research_root.join(name.dir_name())
}

/// Write `body` as a dated report of `name` under `research_root`.
fn write_report(research_root: &Path, name: &str, stamp: &str, body: &str) {
    let dir = item_dir(research_root, name);
    std::fs::create_dir_all(&dir).expect("the item directory is creatable");
    std::fs::write(dir.join(format!("RESEARCH-{stamp}.md")), body).expect("the report is writable");
}

/// The older fixture report: one concept and one finding.
const OLDER: &str = "# Progress Fixture\n\n## Executive Summary\n\nRetrieval reduces hallucination.\n\n## Concepts\n\n### 1. Vector Retrieval\n\n**Definition:** Approximate nearest-neighbour indexes for retrieval.\n\n## Findings\n\n### **Finding 1** - Vector indexes accelerate search\n\nApproximate nearest-neighbour indexes keep latency flat. [#1]\n";

/// The newer fixture report: a second concept and a reworded finding.
const NEWER: &str = "# Progress Fixture\n\n## Executive Summary\n\nRetrieval reduces hallucination.\n\n## Concepts\n\n### 1. Vector Retrieval\n\n**Definition:** Approximate nearest-neighbour indexes for retrieval.\n\n### 2. Reranking Value\n\n**Definition:** When a reranker earns its cost.\n\n## Findings\n\n### **Finding 1** - Vector indexes cut query latency\n\nApproximate nearest-neighbour indexes keep latency flat and recall high. [#1]\n";

/// A manager over a temp root holding `progspec` with the two fixture reports.
fn fixture() -> (TempDir, ResearchManager) {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_report(&research_root, "progspec", "2025-01-10", OLDER);
    write_report(&research_root, "progspec", "2025-02-14", NEWER);
    let manager = ResearchManager::new(&research_root);
    (dir, manager)
}

/// A progress sink that records every report it receives.
#[derive(Default)]
struct Recorder {
    events: Mutex<Vec<DeckEvent>>,
}

impl Recorder {
    fn events(&self) -> Vec<DeckEvent> {
        self.events
            .lock()
            .expect("the event log is not poisoned")
            .clone()
    }

    /// The status reported for `stage`, most recent last.
    fn statuses(&self, stage: DeckStage) -> Vec<DeckStepStatus> {
        self.events()
            .into_iter()
            .filter(|event| event.stage == stage)
            .map(|event| event.status)
            .collect()
    }
}

impl DeckProgress for Recorder {
    fn report(&self, event: &DeckEvent) {
        self.events
            .lock()
            .expect("the event log is not poisoned")
            .push(event.clone());
    }
}

/// A sink that reports `slides` written without touching the filesystem.
struct StubSink {
    /// The slide count to report.
    slides: usize,
}

#[async_trait]
impl DeckSlideSink for StubSink {
    async fn write_deck(&self, plan: &DeckSlidePlan) -> DeckPptxOutcome {
        DeckPptxOutcome::Written {
            path: plan.path.clone(),
            slides: self.slides,
            provider: "stub".to_string(),
        }
    }
}

// -- the stage sequence ----------------------------------------------------

#[tokio::test]
async fn test_run_reports_every_executed_stage_in_order() {
    let (_dir, manager) = fixture();
    let recorder = Recorder::default();
    manager
        .deck_with_progress(
            "progspec",
            &DeckRunOptions::default(),
            None,
            None,
            Some(&recorder),
        )
        .await
        .expect("the fixture pair produces a deck");

    // Every stage of a model-free, sink-less run still reports: the enrichment
    // and export stages say `skipped` rather than dropping out of the list.
    for stage in DeckStage::ALL {
        assert!(
            !recorder.statuses(stage).is_empty(),
            "stage {:?} reported nothing",
            stage.label()
        );
    }

    // The fixed stages report exactly one start and one terminal state each, and
    // the terminal state is `done` for the stages that do real work.
    for stage in [
        DeckStage::Select,
        DeckStage::Read,
        DeckStage::Diff,
        DeckStage::Destination,
        DeckStage::Render,
    ] {
        assert_eq!(
            recorder.statuses(stage),
            vec![DeckStepStatus::Started, DeckStepStatus::Done],
            "stage {:?}",
            stage.label()
        );
    }
}

#[tokio::test]
async fn test_run_publishes_stages_in_run_order() {
    let (_dir, manager) = fixture();
    let recorder = Recorder::default();
    manager
        .deck_with_progress(
            "progspec",
            &DeckRunOptions::default(),
            None,
            None,
            Some(&recorder),
        )
        .await
        .expect("the fixture pair produces a deck");

    // The first report for each stage must arrive in the order the run performs
    // the stages, so a surface can append them as a stable list.
    let mut seen: Vec<DeckStage> = Vec::new();
    for event in recorder.events() {
        if seen.last() != Some(&event.stage) {
            seen.push(event.stage);
        }
    }
    let mut ordinals: Vec<usize> = seen.iter().map(|stage| stage.ordinal()).collect();
    let sorted = {
        let mut copy = ordinals.clone();
        copy.sort_unstable();
        copy
    };
    assert_eq!(
        ordinals, sorted,
        "stages reported out of run order: {seen:?}"
    );
    ordinals.dedup();
}

#[tokio::test]
async fn test_report_acknowledges_the_start_before_any_work() {
    let (_dir, manager) = fixture();
    let recorder = Recorder::default();
    manager
        .deck_with_progress(
            "progspec",
            &DeckRunOptions::default(),
            None,
            None,
            Some(&recorder),
        )
        .await
        .expect("the fixture pair produces a deck");

    let first = recorder
        .events()
        .into_iter()
        .next()
        .expect("at least one report");
    assert_eq!(first.stage, DeckStage::Select);
    assert_eq!(first.status, DeckStepStatus::Started);
    // The acknowledgment names the item, so the message window can show that the
    // command was accepted before the lookup has even finished.
    assert!(
        first.detail.contains("progspec"),
        "detail: {}",
        first.detail
    );
}

#[tokio::test]
async fn test_missing_item_reports_the_failing_stage_and_no_others() {
    let (_dir, manager) = fixture();
    let recorder = Recorder::default();
    let error = manager
        .deck_with_progress(
            "nothere",
            &DeckRunOptions::default(),
            None,
            None,
            Some(&recorder),
        )
        .await
        .expect_err("an unknown item is refused");
    assert!(error.to_string().contains("nothere"));
    // The run stopped inside `select`, so that is the only stage that reported
    // and the surface never shows a stage that was not reached.
    let stages: Vec<DeckStage> = recorder.events().iter().map(|e| e.stage).collect();
    assert_eq!(stages, vec![DeckStage::Select]);
}

// -- the skipped stages ----------------------------------------------------

#[tokio::test]
async fn test_no_model_skips_both_enrichment_stages_with_a_reason() {
    let (_dir, manager) = fixture();
    let recorder = Recorder::default();
    manager
        .deck_with_progress(
            "progspec",
            &DeckRunOptions::default(),
            None,
            None,
            Some(&recorder),
        )
        .await
        .expect("the fixture pair produces a deck");

    for stage in [DeckStage::EnrichFindings, DeckStage::EnrichConcepts] {
        let statuses = recorder.statuses(stage);
        assert_eq!(statuses, vec![DeckStepStatus::Skipped], "stage {stage:?}");
    }
    let skipped = recorder
        .events()
        .into_iter()
        .find(|event| event.stage == DeckStage::EnrichFindings)
        .expect("the enrichment stage reported");
    assert!(
        skipped.detail.contains("no model"),
        "a skipped stage says why: {}",
        skipped.detail
    );
}

#[tokio::test]
async fn test_no_sink_skips_the_export_stage() {
    let (_dir, manager) = fixture();
    let recorder = Recorder::default();
    manager
        .deck_with_progress(
            "progspec",
            &DeckRunOptions::default(),
            None,
            None,
            Some(&recorder),
        )
        .await
        .expect("the fixture pair produces a deck");

    assert_eq!(
        recorder.statuses(DeckStage::Export),
        vec![DeckStepStatus::Skipped]
    );
}

#[tokio::test]
async fn test_present_sink_reports_start_then_done_with_the_slide_count() {
    let (_dir, manager) = fixture();
    let recorder = Recorder::default();
    let sink = StubSink { slides: 7 };
    manager
        .deck_with_progress(
            "progspec",
            &DeckRunOptions::default(),
            None,
            Some(&sink),
            Some(&recorder),
        )
        .await
        .expect("the fixture pair produces a deck");

    assert_eq!(
        recorder.statuses(DeckStage::Export),
        vec![DeckStepStatus::Started, DeckStepStatus::Done]
    );
    let events = recorder.events();
    let done = events
        .iter()
        .rev()
        .find(|event| event.stage == DeckStage::Export)
        .expect("the export stage reported");
    assert!(
        done.detail.contains('7'),
        "the closing report names the slide count: {}",
        done.detail
    );
}

// -- unit counts -----------------------------------------------------------

#[tokio::test]
async fn test_unit_reports_never_exceed_their_total() {
    let (_dir, manager) = fixture();
    let recorder = Recorder::default();
    manager
        .deck_with_progress(
            "progspec",
            &DeckRunOptions::default(),
            None,
            None,
            Some(&recorder),
        )
        .await
        .expect("the fixture pair produces a deck");

    for event in recorder.events() {
        if let Some((completed, total)) = event.unit() {
            assert!(
                completed <= total,
                "stage {:?} reported {completed} of {total}",
                event.stage.label()
            );
        }
    }
}

// -- the no-sink case is unchanged -----------------------------------------

#[tokio::test]
async fn test_run_without_a_progress_sink_still_writes_the_deck() {
    let (_dir, manager) = fixture();
    let outcome = manager
        .deck_with_sink("progspec", &DeckRunOptions::default(), None, None)
        .await
        .expect("the fixture pair produces a deck");
    assert!(outcome.path.exists(), "the deck is written");
    // Progress is a courtesy: no sink means no reports and no change to the run.
    assert!(!outcome.no_changes, "the fixture pair differs");
}
