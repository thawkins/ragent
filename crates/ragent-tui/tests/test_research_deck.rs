//! `/research deck <name>` wiring in the TUI (spec `diffdeck`, FR-001, FR-010).
//!
//! The slash arm is a thin shell around [`ResearchManager::deck`]: it spawns
//! the orchestration on the tokio runtime and reports the outcome through the
//! event bus. Every test therefore drives the public
//! `execute_slash_command` entry point against a real item directory in a
//! tempdir, drains the bus, and asserts on the `From: /research deck`
//! `TextDelta` and the `AgentNotice` that follow.
//!
//! cwd isolation: `App::handle_research_command` roots the manager at
//! `cwd_path.join("research")`, so each test points `app.cwd_path` at its own
//! tempdir instead of mutating the process cwd.

use ragent_agent::event::Event;
use ragent_tui::App;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[path = "support/mod.rs"]
mod support;

/// The older fixture report: one concept, one finding.
const OLDER: &str = "# Deck Fixture\n\n## Executive Summary\n\nRetrieval reduces hallucination.\n\n## Concepts\n\n### 1. Vector Retrieval\n\n**Definition:** Approximate nearest-neighbour indexes for retrieval.\n\n## Findings\n\n### **Finding 1** - Vector indexes accelerate search\n\nApproximate nearest-neighbour indexes keep latency flat. [#1]\n";

/// The newer fixture report: a second concept is added and the finding's
/// headline is reworded, so the pair changes two elements.
const NEWER: &str = "# Deck Fixture\n\n## Executive Summary\n\nRetrieval reduces hallucination.\n\n## Concepts\n\n### 1. Vector Retrieval\n\n**Definition:** Approximate nearest-neighbour indexes for retrieval.\n\n### 2. Reranking Value\n\n**Definition:** When a reranker earns its cost.\n\n## Findings\n\n### **Finding 1** - Vector indexes cut query latency\n\nApproximate nearest-neighbour indexes keep latency flat and recall high. [#1]\n";

/// A `research/` root of a fresh temp directory.
fn root(dir: &tempfile::TempDir) -> PathBuf {
    dir.path().join("research")
}

/// Write `body` as a dated report of `name` under `research_root`.
fn write_report(research_root: &Path, name: &str, stamp: &str, body: &str) {
    let dir = research_root.join(name);
    std::fs::create_dir_all(&dir).expect("the item directory is creatable");
    std::fs::write(dir.join(format!("RESEARCH-{stamp}.md")), body).expect("the report is writable");
}

/// A temp project holding the two fixture reports of `deckspec`, plus an app
/// rooted at it with a live session.
fn fixture(tag: &str) -> (tempfile::TempDir, App) {
    let dir = tempfile::tempdir().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_report(&research_root, "deckspec", "2025-01-10", OLDER);
    write_report(&research_root, "deckspec", "2025-02-14", NEWER);
    let mut app = support::make_app();
    // A unique session id keeps events from a previous test out of this one.
    app.session_id = Some(format!("deck-{tag}"));
    app.cwd_path = dir.path().to_path_buf();
    (dir, app)
}

/// Drain the bus for a short window, returning every event published.
async fn drain(app: &App) -> Vec<Event> {
    let mut rx = app.event_bus.subscribe();
    let mut events = Vec::new();
    for _ in 0..200 {
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    events
}

/// Every `TextDelta` text published for this session, joined.
fn text_deltas(events: &[Event]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            Event::TextDelta { text, .. } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Every `AgentNotice` message published for this session, joined.
fn agent_notices(events: &[Event]) -> String {
    events
        .iter()
        .filter_map(|event| match event {
            Event::AgentNotice { message, .. } => Some(message.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// -- FR-001: the arm runs the orchestration --------------------------------

#[tokio::test]
async fn test_deck_command_sets_a_waiting_status_line() {
    let (_dir, mut app) = fixture("status");
    app.execute_slash_command("/research deck deckspec").await;
    assert_eq!(app.status, "[wait] research: deck 'deckspec'...");
}

#[tokio::test]
async fn test_deck_command_writes_the_deck_and_reports_its_path() {
    let (dir, mut app) = fixture("write");
    app.execute_slash_command("/research deck deckspec").await;
    let events = drain(&app).await;
    let text = text_deltas(&events);

    let item_dir = root(&dir).join("deckspec");
    let decks: Vec<PathBuf> = std::fs::read_dir(&item_dir)
        .expect("the item directory is readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("DECK-"))
        })
        .collect();
    assert_eq!(decks.len(), 1, "exactly one deck must be written");
    let written = &decks[0];

    assert!(text.starts_with("From: /research deck"), "got: {text}");
    assert!(
        text.contains(&format!("`{}`", written.display())),
        "the report must name the written path: {text}"
    );
}

// -- FR-010: the reported counts are the deck's own ------------------------

#[tokio::test]
async fn test_deck_command_reports_per_status_counts() {
    let (_dir, mut app) = fixture("counts");
    app.execute_slash_command("/research deck deckspec").await;
    let text = text_deltas(&drain(&app).await);

    // The fixture adds one Open Question and rewords one finding headline. A
    // finding's identity IS its headline (FR-009), so a reworded headline is a
    // different item and reads `NEW`; `UPDATE` needs a matched identity whose
    // normalised text differs. The unchanged Executive Summary's item is `OLD`.
    assert!(text.contains("NEW 2"), "got: {text}");
    assert!(text.contains("ADDED 0"), "got: {text}");
    assert!(text.contains("UPDATE 0"), "got: {text}");
    assert!(text.contains("OLD 2"), "got: {text}");
    assert!(text.contains("2 change(s)"), "got: {text}");
    // Both changed elements get a slide.
    assert!(text.contains("2 element slide(s)"), "got: {text}");
}

#[tokio::test]
async fn test_deck_command_reports_the_covered_stamp_range() {
    let (_dir, mut app) = fixture("range");
    app.execute_slash_command("/research deck deckspec").await;
    let text = text_deltas(&drain(&app).await);
    assert!(
        text.contains("2025-01-10 to 2025-02-14"),
        "the range must name both reports: {text}"
    );
}

#[tokio::test]
async fn test_deck_command_publishes_one_success_block() {
    let (_dir, mut app) = fixture("notice");
    app.execute_slash_command("/research deck deckspec").await;
    let events = drain(&app).await;
    let text = text_deltas(&events);
    assert!(text.contains("[ok] Wrote"), "got: {text}");
    assert!(text.contains("2 change(s)"), "got: {text}");
    // The result is reported once, not echoed as a second notice bubble.
    assert!(
        !agent_notices(&events).contains("written"),
        "got: {}",
        agent_notices(&events)
    );
}

#[tokio::test]
async fn test_deck_command_reports_no_changes_for_identical_reports() {
    let dir = tempfile::tempdir().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_report(&research_root, "same", "2025-01-10", OLDER);
    write_report(&research_root, "same", "2025-02-14", OLDER);
    let mut app = support::make_app();
    app.session_id = Some("deck-nochange".to_string());
    app.cwd_path = dir.path().to_path_buf();

    app.execute_slash_command("/research deck same").await;
    let events = drain(&app).await;
    let text = text_deltas(&events);
    assert!(text.contains("no changes detected"), "got: {text}");
    assert!(
        !text.contains("NEW 0"),
        "the no-change line omits counts: {text}"
    );
}

// -- FR-014 / FR-015 / FR-016: the flags reach the orchestration -----------

#[tokio::test]
async fn test_deck_command_elements_filter_narrows_the_counts() {
    let (dir, mut app) = fixture("elements");
    app.execute_slash_command("/research deck deckspec --elements concepts")
        .await;
    let text = text_deltas(&drain(&app).await);

    // Only the Concepts element is left, so the findings change and the
    // Executive Summary's `OLD` item are gone from the counts.
    assert!(text.contains("NEW 1"), "got: {text}");
    assert!(text.contains("OLD 1"), "got: {text}");
    assert!(text.contains("1 change(s)"), "got: {text}");
    assert!(text.contains("1 element slide(s)"), "got: {text}");
    // The deck really was restricted: only one element slide is written.
    let written = std::fs::read_dir(root(&dir).join("deckspec"))
        .expect("the item directory is readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("DECK-"))
        })
        .expect("the deck is written");
    let body = std::fs::read_to_string(written).expect("the deck is readable");
    assert!(body.contains("## Reranking Value"), "{body}");
    assert!(!body.contains("## Findings"), "{body}");
}

#[tokio::test]
async fn test_deck_command_include_old_adds_unchanged_element_slides() {
    let (dir, mut app) = fixture("include-old");
    app.execute_slash_command("/research deck deckspec --include-old")
        .await;
    let text = text_deltas(&drain(&app).await);
    // Two changed elements plus the unchanged Executive Summary.
    assert!(text.contains("3 element slide(s)"), "got: {text}");
    let written = std::fs::read_dir(root(&dir).join("deckspec"))
        .expect("the item directory is readable")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .find(|path| {
            path.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("DECK-"))
        })
        .expect("the deck is written");
    let body = std::fs::read_to_string(written).expect("the deck is readable");
    assert!(body.contains("## Executive Summary"), "{body}");
}

#[tokio::test]
async fn test_deck_command_out_flag_writes_to_the_named_path() {
    let (dir, mut app) = fixture("out");
    let out = dir.path().join("named-deck.md");
    app.execute_slash_command(&format!("/research deck deckspec --out {}", out.display()))
        .await;
    let text = text_deltas(&drain(&app).await);
    assert!(out.is_file(), "the named path must receive the deck");
    assert!(
        text.contains(&format!("`{}`", out.display())),
        "the report must name the named path: {text}"
    );
}

// -- FR-017 / FR-018 / FR-016: refusals are reported, never silent ---------

#[tokio::test]
async fn test_deck_command_existing_deck_without_force_is_refused() {
    let (dir, mut app) = fixture("exists");
    let out = dir.path().join("named-deck.md");
    std::fs::write(&out, "previous deck").expect("the existing deck is writable");

    app.execute_slash_command(&format!("/research deck deckspec --out {}", out.display()))
        .await;
    let events = drain(&app).await;
    let text = text_deltas(&events);
    assert!(text.starts_with("From: /research deck"), "got: {text}");
    assert!(text.contains("**Error:**"), "got: {text}");
    assert!(text.contains("--force"), "got: {text}");
    // The refusal ends the run: a terminal failure report is published so
    // the progress tracker and `[wait]` status do not stay live.
    assert!(
        agent_notices(&events).contains("deckspec failed:"),
        "got: {}",
        agent_notices(&events)
    );
    // The refusal wrote nothing.
    assert_eq!(
        std::fs::read_to_string(&out).expect("the existing deck is readable"),
        "previous deck"
    );
}

#[tokio::test]
async fn test_deck_command_missing_item_is_refused() {
    let (_dir, mut app) = fixture("missing");
    app.execute_slash_command("/research deck nosuchitem").await;
    let events = drain(&app).await;
    let text = text_deltas(&events);
    assert!(text.contains("**Error:**"), "got: {text}");
    assert!(text.contains("nosuchitem"), "got: {text}");
}

#[tokio::test]
async fn test_deck_command_single_report_is_refused() {
    let dir = tempfile::tempdir().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_report(&research_root, "lonely", "2025-01-10", OLDER);
    let mut app = support::make_app();
    app.session_id = Some("deck-lonely".to_string());
    app.cwd_path = dir.path().to_path_buf();

    app.execute_slash_command("/research deck lonely").await;
    let text = text_deltas(&drain(&app).await);
    assert!(text.contains("**Error:**"), "got: {text}");
    // Nothing was written into the item directory.
    let names: Vec<String> = std::fs::read_dir(research_root.join("lonely"))
        .expect("the item directory is readable")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .collect();
    assert_eq!(names.len(), 1, "only the report may remain: {names:?}");
}
