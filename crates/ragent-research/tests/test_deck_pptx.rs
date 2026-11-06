//! Tests for the optional PowerPoint twin of a diff deck (spec `diffdeck`
//! FR-030).
//!
//! The contract under test is that the markdown deck is the canonical artifact
//! and the presentation is an addition to it:
//!
//! - a sink receives a plan that describes the same slides as the markdown deck
//!   that was just written, and that deck is on disk before the sink runs;
//! - a sink that fails, or cannot reach a file, is recorded as
//!   [`DeckPptxOutcome::Failed`] while the run still returns `Ok` and the
//!   markdown deck keeps its bytes;
//! - no sink at all is [`DeckPptxOutcome::Unavailable`], and the run is complete
//!   rather than degraded.
//!
//! Every case drives [`ResearchManager::deck_with_sink`] against a real item
//! directory inside a `tempfile::TempDir`, so the ordering the contract rests on
//! (markdown published, then sink invoked) is observed rather than assumed.

#![allow(clippy::assert_is_empty)]

use async_trait::async_trait;
use ragent_research::deck::DeckRunOptions;
use ragent_research::deck_pptx::{
    DeckPptxOutcome, DeckSlide, DeckSlidePlan, DeckSlideSink, SlideKind, deck_pptx_path,
};
use ragent_research::{ResearchManager, ResearchName};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
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

/// The deck fixture pair: the newer report adds a concept and rewords a
/// finding's headline, so the diff is non-empty.
const OLDER: &str = "# Deck Fixture\n\n## Executive Summary\n\nRetrieval reduces hallucination.\n\n## Concepts\n\n### 1. Vector Retrieval\n\n**Definition:** Approximate nearest-neighbour indexes for retrieval.\n\n## Findings\n\n### **Finding 1** - Vector indexes accelerate search\n\nApproximate nearest-neighbour indexes keep latency flat. [#1]\n";

const NEWER: &str = "# Deck Fixture\n\n## Executive Summary\n\nRetrieval reduces hallucination.\n\n## Concepts\n\n### 1. Vector Retrieval\n\n**Definition:** Approximate nearest-neighbour indexes for retrieval.\n\n### 2. Reranking Value\n\n**Definition:** When a reranker earns its cost.\n\n## Findings\n\n### **Finding 1** - Vector indexes cut query latency\n\nApproximate nearest-neighbour indexes keep latency flat and recall high. [#1]\n";

/// A manager over a temp root holding `deckspec` with the two fixture reports.
fn fixture() -> (TempDir, ResearchManager) {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_report(&research_root, "deckspec", "2025-01-10", OLDER);
    write_report(&research_root, "deckspec", "2025-02-14", NEWER);
    let manager = ResearchManager::new(&research_root);
    (dir, manager)
}

/// What one sink call observed, for assertions after the run.
#[derive(Debug, Clone)]
struct Observed {
    /// Whether the markdown deck existed on disk when the sink was called.
    markdown_present: bool,
    /// The markdown deck's bytes at that moment.
    markdown_bytes: Option<String>,
    /// The plan the sink was handed.
    plan: DeckSlidePlan,
}

/// A sink that records its call and reports `outcome`, optionally writing the
/// file the plan names so the happy path can be asserted end to end.
struct RecordingSink {
    /// Receives whatever the run did.
    calls: Mutex<Vec<Observed>>,
    /// What the sink reports back.
    outcome: DeckPptxOutcome,
    /// When set, the sink writes a stand-in file at `plan.path` first.
    write_file: bool,
}

impl RecordingSink {
    fn new(outcome: DeckPptxOutcome, write_file: bool) -> Self {
        Self {
            calls: Mutex::new(Vec::new()),
            outcome,
            write_file,
        }
    }

    fn calls(&self) -> Vec<Observed> {
        self.calls
            .lock()
            .expect("the call log is not poisoned")
            .clone()
    }
}

#[async_trait]
impl DeckSlideSink for RecordingSink {
    async fn write_deck(&self, plan: &DeckSlidePlan) -> DeckPptxOutcome {
        let markdown = plan.path.with_extension("md");
        let bytes = std::fs::read_to_string(&markdown).ok();
        if self.write_file {
            std::fs::write(&plan.path, b"PK\x03\x04stand-in presentation")
                .expect("the sink can write its stand-in file");
        }
        self.calls
            .lock()
            .expect("the call log is not poisoned")
            .push(Observed {
                markdown_present: bytes.is_some(),
                markdown_bytes: bytes,
                plan: plan.clone(),
            });
        // A `Written` outcome carries the file the sink actually wrote, so a
        // fixture that writes its stand-in file reports `plan.path`.
        match &self.outcome {
            DeckPptxOutcome::Written {
                slides, provider, ..
            } if self.write_file => DeckPptxOutcome::Written {
                path: plan.path.clone(),
                slides: *slides,
                provider: provider.clone(),
            },
            other => other.clone(),
        }
    }
}

// -- the no-sink case ------------------------------------------------------

#[tokio::test]
async fn test_no_sink_reports_unavailable_and_writes_only_markdown() {
    let (_dir, manager) = fixture();
    let outcome = manager
        .deck_with_sink("deckspec", &DeckRunOptions::default(), None, None)
        .await
        .expect("the fixture pair produces a deck without a sink");

    assert_eq!(outcome.pptx, DeckPptxOutcome::Unavailable);
    assert!(
        outcome.path.exists(),
        "the markdown deck is written with no sink at all"
    );
    assert!(
        !deck_pptx_path(&outcome.path).exists(),
        "no sink means no presentation file"
    );
    assert_eq!(outcome.pptx.written_path(), None);
}

// -- the happy path --------------------------------------------------------

#[tokio::test]
async fn test_sink_writes_beside_the_markdown_deck_and_reports_its_path() {
    let (_dir, manager) = fixture();
    let sink = RecordingSink::new(
        DeckPptxOutcome::Written {
            path: PathBuf::new(),
            slides: 0,
            provider: "test-sink".to_string(),
        },
        true,
    );
    let outcome = manager
        .deck_with_sink("deckspec", &DeckRunOptions::default(), None, Some(&sink))
        .await
        .expect("a sink that reports success keeps the run successful");

    let expected = deck_pptx_path(&outcome.path);
    // The outcome is the sink's own report, passed through unchanged: the
    // orchestration stage has no business rewriting what the sink said it
    // wrote. The fixture sink therefore reports the path it was told to use.
    assert_eq!(
        outcome.pptx,
        DeckPptxOutcome::Written {
            path: expected.clone(),
            slides: 0,
            provider: "test-sink".to_string(),
        }
    );
    assert_eq!(outcome.pptx.written_path(), Some(expected.as_path()));
    assert!(
        expected.exists(),
        "the presentation sits beside the markdown deck"
    );
    assert_eq!(
        expected.parent(),
        outcome.path.parent(),
        "both artifacts of one run share a directory"
    );
    assert_eq!(
        expected.file_stem(),
        outcome.path.file_stem(),
        "both artifacts of one run share a stem"
    );
}

#[tokio::test]
async fn test_sink_is_handed_the_slides_of_the_markdown_deck_it_sits_beside() {
    let (_dir, manager) = fixture();
    let sink = RecordingSink::new(DeckPptxOutcome::Unavailable, false);
    let outcome = manager
        .deck_with_sink("deckspec", &DeckRunOptions::default(), None, Some(&sink))
        .await
        .expect("the fixture pair produces a deck");

    let calls = sink.calls();
    assert_eq!(calls.len(), 1, "the sink runs exactly once per deck run");

    let Observed {
        markdown_present,
        markdown_bytes,
        plan,
    } = &calls[0];
    let written = std::fs::read_to_string(&outcome.path).expect("the deck is readable");
    assert!(
        *markdown_present,
        "the markdown deck is published before the sink runs"
    );
    assert_eq!(
        markdown_bytes.as_deref(),
        Some(written.as_str()),
        "the sink observes the published markdown deck"
    );

    assert_eq!(plan.path, deck_pptx_path(&outcome.path));
    assert_eq!(plan.item, "deckspec");
    assert_eq!(plan.from, "2025-01-10");
    assert_eq!(plan.to, "2025-02-14");
    assert_eq!(plan.title.as_deref(), Some("Deck Fixture"));
    assert!(
        plan.force,
        "the presentation is refreshed whenever its markdown twin is"
    );

    // The plan describes the run, not a transcription of the file: the slides
    // are rendered from the same diff the markdown deck was, so no slide carries
    // a raw markdown heading or a backticked status tag.
    assert!(
        !plan.slides.is_empty(),
        "the plan carries the cover and the changed elements"
    );
    assert_eq!(
        plan.slides.first().map(|slide| slide.kind),
        Some(SlideKind::Cover),
        "the deck opens with its title slide"
    );
    for slide in &plan.slides {
        assert!(
            !slide.title.contains("##") && !slide.title.contains('`'),
            "a slide title is rendered text, not markdown: {:?}",
            slide.title
        );
        assert!(
            !slide.lead.contains('`') && !slide.lead.starts_with('#'),
            "a slide lead is rendered text, not markdown: {:?}",
            slide.lead
        );
    }
}

// -- the failing sink ------------------------------------------------------

#[tokio::test]
async fn test_failing_sink_leaves_the_markdown_deck_intact_and_is_reported() {
    let (_dir, manager) = fixture();
    let sink = RecordingSink::new(
        DeckPptxOutcome::Failed {
            reason: "no presentation server answered".to_string(),
        },
        false,
    );
    let outcome = manager
        .deck_with_sink("deckspec", &DeckRunOptions::default(), None, Some(&sink))
        .await
        .expect("a failing presentation export cannot fail a complete deck run");

    assert_eq!(
        outcome.pptx,
        DeckPptxOutcome::Failed {
            reason: "no presentation server answered".to_string(),
        }
    );
    assert_eq!(outcome.pptx.written_path(), None);
    let markdown = std::fs::read_to_string(&outcome.path).expect("the deck is readable");
    assert_eq!(
        markdown, outcome.document.markdown,
        "the published deck keeps the text the run rendered"
    );
    assert!(
        !deck_pptx_path(&outcome.path).exists(),
        "a sink that did not write its file leaves no presentation behind"
    );
}

#[tokio::test]
async fn test_a_rerun_replaces_the_presentation_beside_a_replaced_deck() {
    let (_dir, manager) = fixture();
    let first = RecordingSink::new(
        DeckPptxOutcome::Failed {
            reason: "first run".to_string(),
        },
        true,
    );
    let outcome = manager
        .deck_with_sink("deckspec", &DeckRunOptions::default(), None, Some(&first))
        .await
        .expect("the first run writes the deck");
    let pptx = deck_pptx_path(&outcome.path);
    assert!(pptx.exists(), "the first run wrote its presentation");

    // A second run over the same destination is refused by the markdown deck's
    // own FR-017 guard; under `--force` it rewrites both artifacts.
    let blocked = manager
        .deck_with_sink("deckspec", &DeckRunOptions::default(), None, Some(&first))
        .await;
    assert!(
        blocked.is_err(),
        "the markdown deck's existing-file guard still applies"
    );

    let second = RecordingSink::new(
        DeckPptxOutcome::Failed {
            reason: "second run".to_string(),
        },
        true,
    );
    let options = DeckRunOptions {
        force: true,
        ..DeckRunOptions::default()
    };
    let outcome = manager
        .deck_with_sink("deckspec", &options, None, Some(&second))
        .await
        .expect("--force rewrites the deck and its presentation");
    assert_eq!(second.calls().len(), 1);
    assert!(deck_pptx_path(&outcome.path).exists());
}

// -- the shared slide plan -------------------------------------------------

#[test]
fn test_outcome_describe_treats_a_skipped_export_as_a_warning() {
    let unavailable = DeckPptxOutcome::Unavailable.describe();
    assert!(
        unavailable.contains("no PowerPoint"),
        "an absent export names the missing capability: {unavailable}"
    );

    let failed = DeckPptxOutcome::Failed {
        reason: "server timed out".to_string(),
    }
    .describe();
    assert!(
        failed.contains("server timed out"),
        "a failed export carries the sink's reason: {failed}"
    );

    let written = DeckPptxOutcome::Written {
        path: PathBuf::from("/tmp/DECK-2026-01-01.pptx"),
        slides: 12,
        provider: "deckcraft".to_string(),
    }
    .describe();
    assert!(
        written.contains("12 slides"),
        "the count is reported: {written}"
    );
    assert!(
        written.contains("via deckcraft"),
        "the provider is reported: {written}"
    );
}

#[test]
fn test_markdown_and_presentation_paths_are_siblings() {
    let markdown = PathBuf::from("/root/research/aisurvey/DECK-2026-11-04.md");
    assert_eq!(
        deck_pptx_path(&markdown),
        PathBuf::from("/root/research/aisurvey/DECK-2026-11-04.pptx")
    );
}

#[test]
fn test_deck_slide_new_carries_no_status() {
    let slide = DeckSlide::new(SlideKind::Cover, "Cover");
    assert_eq!(slide.title, "Cover");
    assert_eq!(slide.kind, SlideKind::Cover);
    assert!(slide.lead.is_empty());
    assert!(slide.bullets.is_empty());
    assert!(slide.table.is_none());
    assert_eq!(slide.status, None);
    assert_eq!(slide.counts, None);
}
