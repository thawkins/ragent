//! Tests for the deck event line the CLI prints (spec `diffdeck` T-011:
//! FR-001, FR-010, FR-013).
//!
//! `render_deck_event_json` is the machine-readable half of the
//! `ragent research deck` output; the human summary beside it is asserted in the
//! root crate's `test_research_deck_cli_command`. These cases drive a real
//! `ResearchManager::deck` run so the rendered line is built from a written deck,
//! not a hand-made outcome.

use ragent_research::deck::{DeckRunOptions, DeckWriteError};
use ragent_research::manager::ResearchError;
use ragent_research::{ResearchManager, ResearchName, render_deck_event_json};
use std::path::Path;
use tempfile::TempDir;

/// The directory of item `name` under `research_root`.
fn item_dir(research_root: &Path, name: &str) -> std::path::PathBuf {
    let name = ResearchName::try_new(name).expect("the fixture name is valid");
    research_root.join(name.dir_name())
}

/// Write `body` as a dated report of `name` under `research_root`.
fn write_report(research_root: &Path, name: &str, stamp: &str, body: &str) {
    let dir = item_dir(research_root, name);
    std::fs::create_dir_all(&dir).expect("the item directory is creatable");
    std::fs::write(dir.join(format!("RESEARCH-{stamp}.md")), body).expect("the report is writable");
}

/// The older fixture report: one question, one finding.
const OLDER: &str = "# Deck Fixture\n\n## Executive Summary\n\nRetrieval reduces hallucination.\n\n## Concepts\n\n### 1. Vector Retrieval\n\n**Definition:** Approximate nearest-neighbour indexes for retrieval.\n\n## Findings\n\n### **Finding 1** - Vector indexes accelerate search\n\nApproximate nearest-neighbour indexes keep latency flat. [#1]\n";

/// The newer fixture report: a second concept is added and the finding's
/// headline is reworded, so the pair changes two elements (FR-009).
const NEWER: &str = "# Deck Fixture\n\n## Executive Summary\n\nRetrieval reduces hallucination.\n\n## Concepts\n\n### 1. Vector Retrieval\n\n**Definition:** Approximate nearest-neighbour indexes for retrieval.\n\n### 2. Reranking Value\n\n**Definition:** When a reranker earns its cost.\n\n## Findings\n\n### **Finding 1** - Vector indexes cut query latency\n\nApproximate nearest-neighbour indexes keep latency flat and recall high. [#1]\n";

/// A manager over a temp root holding `edgedeck` with the two fixture reports.
fn fixture() -> (TempDir, ResearchManager) {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = dir.path().join("research");
    write_report(&research_root, "edgedeck", "2025-01-10", OLDER);
    write_report(&research_root, "edgedeck", "2025-02-14", NEWER);
    let manager = ResearchManager::new(&research_root);
    (dir, manager)
}

/// The payload of a rendered deck event line, parsed back from its JSON.
fn event_payload(line: &str) -> serde_json::Value {
    let json = line
        .strip_prefix("ragent-research: ")
        .expect("the line carries the family prefix");
    let value: serde_json::Value = serde_json::from_str(json).expect("the line is JSON");
    assert_eq!(value["kind"], "deck");
    value["payload"].clone()
}

#[tokio::test]
async fn test_deck_event_line_carries_the_written_decks_counts() {
    let (_dir, manager) = fixture();
    let outcome = manager
        .deck("edgedeck", &DeckRunOptions::default())
        .await
        .expect("the fixture pair produces a deck");

    let line = render_deck_event_json("edgedeck", &outcome);
    assert!(
        line.starts_with("ragent-research: {"),
        "the line must carry the existing family prefix: {line}"
    );
    // One line only: the CLI contract is a single machine-readable record.
    assert!(!line.contains('\n'), "the event must be one line: {line}");

    let payload = event_payload(&line);
    assert_eq!(payload["name"], "edgedeck");
    assert_eq!(
        payload["path"].as_str().map(str::to_string),
        Some(outcome.path.display().to_string())
    );
    assert_eq!(payload["from"], "2025-01-10");
    assert_eq!(payload["to"], "2025-02-14");
    assert_eq!(payload["no_changes"], false);
    // FR-010: the counts are the written deck's own.
    assert_eq!(payload["new"], outcome.new_count);
    assert_eq!(payload["added"], outcome.added_count);
    assert_eq!(payload["update"], outcome.update_count);
    assert_eq!(payload["old"], outcome.old_count);
    assert_eq!(payload["changes"], outcome.change_count());
    assert_eq!(
        payload["element_slides"],
        serde_json::json!(["Concepts", "Findings"]),
        "element slides are listed in FR-003 precedence order"
    );
}

#[tokio::test]
async fn test_deck_event_line_reports_the_no_change_case() {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = dir.path().join("research");
    let body = "# Stable Item\n\n## Executive Summary\n\nNothing changes here.\n\n## Findings\n\n### **Finding 1** - Unchanged headline\n\nUnchanged body.\n";
    write_report(&research_root, "stable", "2025-01-10", body);
    write_report(&research_root, "stable", "2025-02-14", body);
    let manager = ResearchManager::new(&research_root);

    let outcome = manager
        .deck("stable", &DeckRunOptions::default())
        .await
        .expect("identical reports still produce a deck");

    let payload = event_payload(&render_deck_event_json("stable", &outcome));
    assert_eq!(payload["no_changes"], true);
    assert_eq!(payload["changes"], 0);
    assert_eq!(
        payload["element_slides"],
        serde_json::json!([]),
        "a no-change deck renders no element slide (FR-013)"
    );
}

#[tokio::test]
async fn test_deck_event_line_carries_an_out_path_verbatim() {
    let (dir, manager) = fixture();
    let out = dir.path().join("custom-deck.md");
    let options = DeckRunOptions {
        out: Some(out.display().to_string()),
        ..DeckRunOptions::default()
    };
    let outcome = manager
        .deck("edgedeck", &options)
        .await
        .expect("the fixture pair produces a deck");

    let payload = event_payload(&render_deck_event_json("edgedeck", &outcome));
    assert_eq!(
        payload["path"].as_str().map(str::to_string),
        Some(out.display().to_string()),
        "the event names the file --out asked for"
    );
    assert_eq!(
        std::fs::read_to_string(&out).expect("the deck exists"),
        outcome.document.markdown,
        "the event describes the file that was actually written"
    );
}

#[tokio::test]
async fn test_deck_event_line_is_absent_for_a_refused_run() {
    // A refusal is an error, so no outcome exists to render a line from. This
    // pins the contract the CLI relies on: the event is built from a successful
    // outcome, never invented for a failed run.
    let (_dir, manager) = fixture();
    let options = DeckRunOptions {
        from: Some("2030-01-01".to_string()),
        ..DeckRunOptions::default()
    };
    let err = manager
        .deck("edgedeck", &options)
        .await
        .expect_err("a stamp with no report must be refused");
    assert!(
        !matches!(err, ResearchError::DeckWrite(DeckWriteError::Exists { .. })),
        "the fixture refusal is about selection, not the destination: {err}"
    );
    assert!(
        err.to_string().contains("2030-01-01"),
        "the refusal names the offending stamp: {err}"
    );
}
