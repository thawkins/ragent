//! Tests for the diff-deck orchestration stage (spec `diffdeck`: FR-002,
//! FR-011, FR-014, FR-015, FR-016, FR-017, FR-018, FR-019, FR-021).
//!
//! Every case drives [`ResearchManager::deck`] against a real item directory
//! inside a `tempfile::TempDir`, so the filesystem steps (report selection,
//! reading, the `--force` guard, the `--out` resolution, and the atomic write)
//! are exercised as they run in production rather than mocked.

use ragent_research::deck::{
    DeckElement, DeckRunOptions, DeckSelectError, DeckWriteError, ItemStatus, deck_file_name,
    default_deck_path,
};
use ragent_research::manager::ResearchError;
use ragent_research::{ResearchManager, ResearchName};
use std::path::Path;
use tempfile::TempDir;

/// The research root of a fresh temp directory.
fn root(dir: &TempDir) -> std::path::PathBuf {
    dir.path().join("research")
}

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

/// The older fixture report: one concept, one finding.
const OLDER: &str = "# Deck Fixture\n\n## Executive Summary\n\nRetrieval reduces hallucination.\n\n## Concepts\n\n### 1. Vector Retrieval\n\n**Definition:** Approximate nearest-neighbour indexes for retrieval.\n\n## Findings\n\n### **Finding 1** - Vector indexes accelerate search\n\nApproximate nearest-neighbour indexes keep latency flat. [#1]\n";

/// The newer fixture report: a second concept is added and the finding's
/// **headline** is reworded, so the pair changes two elements.
///
/// A finding is identified by its headline (FR-009), so a rewritten body alone
/// stays `OLD`; the change here is the headline, not the body text.
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

/// Every entry name in `dir`, for asserting that nothing new appeared.
fn entry_names(dir: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(dir)
        .expect("the directory is readable")
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().to_str().map(str::to_string))
        .collect();
    names.sort();
    names
}

// -- FR-002: the default output path ---------------------------------------

#[tokio::test]
async fn test_deck_writes_dated_file_into_the_item_directory() {
    let (dir, manager) = fixture();
    let research_root = root(&dir);
    let outcome = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect("the fixture pair produces a deck");

    let dir_path = item_dir(&research_root, "deckspec");
    let name = outcome
        .path
        .file_name()
        .and_then(|n| n.to_str())
        .expect("the deck path names a file");
    assert!(name.starts_with("DECK-"), "unexpected deck name: {name}");
    assert!(name.to_lowercase().ends_with(".md"));
    assert_eq!(outcome.path.parent(), Some(dir_path.as_path()));
    // The write really landed, with the rendered document's own text.
    let written = std::fs::read_to_string(&outcome.path).expect("the deck is readable");
    assert_eq!(written, outcome.document.markdown);
}

#[tokio::test]
async fn test_deck_default_path_is_named_for_the_generation_day() {
    let (dir, manager) = fixture();
    let outcome = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect("the fixture pair produces a deck");
    let stamp = chrono::Utc::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let expected = default_deck_path(&item_dir(&root(&dir), "deckspec"), &stamp);
    assert_eq!(outcome.path, expected);
    assert_eq!(
        outcome.path.file_name().and_then(|n| n.to_str()),
        Some(deck_file_name(&stamp).as_str())
    );
}

// -- FR-018: an unknown item ------------------------------------------------

#[tokio::test]
async fn test_deck_refuses_unknown_item_and_creates_nothing() {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    std::fs::create_dir_all(&research_root).expect("the root is creatable");
    let manager = ResearchManager::new(&research_root);

    let err = manager
        .deck("nosuchitem", &DeckRunOptions::default())
        .await
        .expect_err("an unknown item is refused");
    match err {
        ResearchError::NotFound(name, _) => assert_eq!(name, "nosuchitem"),
        other => panic!("expected NotFound, got: {other}"),
    }
    assert!(
        !item_dir(&research_root, "nosuchitem").exists(),
        "a refusal must not create the item directory"
    );
}

// -- FR-011 / FR-012: not enough reports ------------------------------------

#[tokio::test]
async fn test_deck_refuses_item_with_one_report_and_names_the_count() {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_report(&research_root, "deckspec", "2025-01-10", OLDER);
    let manager = ResearchManager::new(&research_root);
    let item = item_dir(&research_root, "deckspec");
    let before = entry_names(&item);

    let err = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect_err("one report cannot produce a deck");
    let message = err.to_string();
    assert!(message.contains("deckspec"), "message: {message}");
    assert!(message.contains("1 report(s) found"), "message: {message}");
    assert!(
        matches!(
            err,
            ResearchError::DeckSelect(DeckSelectError::TooFewReports { found: 1, .. })
        ),
        "expected TooFewReports, got: {err}"
    );
    assert_eq!(
        entry_names(&item),
        before,
        "a refusal must not write a file"
    );
}

#[tokio::test]
async fn test_deck_refuses_legacy_only_item_naming_the_scheme() {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    let item = item_dir(&research_root, "deckspec");
    std::fs::create_dir_all(&item).expect("the item directory is creatable");
    std::fs::write(item.join("RESEARCH.md"), OLDER).expect("the legacy report is writable");
    let manager = ResearchManager::new(&research_root);
    let before = entry_names(&item);

    let err = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect_err("a lone legacy report cannot produce a deck");
    let message = err.to_string();
    assert!(
        message.contains("legacy undated RESEARCH.md"),
        "message: {message}"
    );
    assert_eq!(entry_names(&item), before);
}

#[tokio::test]
async fn test_deck_uses_legacy_report_as_the_baseline() {
    // FR-012: `RESEARCH.md` is the oldest report, so `--to` the only dated
    // report pairs it with the legacy file as the baseline.
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    let item = item_dir(&research_root, "deckspec");
    std::fs::create_dir_all(&item).expect("the item directory is creatable");
    std::fs::write(item.join("RESEARCH.md"), OLDER).expect("the legacy report is writable");
    write_report(&research_root, "deckspec", "2025-02-14", NEWER);
    let manager = ResearchManager::new(&research_root);

    let options = DeckRunOptions {
        to: Some("2025-02-14".to_string()),
        ..DeckRunOptions::default()
    };
    let outcome = manager
        .deck("deckspec", &options)
        .await
        .expect("the legacy baseline pairs with the dated report");
    assert_eq!(outcome.pair.older_label(), "legacy");
    assert_eq!(outcome.pair.newer_label(), "2025-02-14");
}

// -- FR-019: unreadable or unparseable reports ------------------------------

#[tokio::test]
async fn test_deck_refuses_non_utf8_report_and_leaves_existing_deck_bytes() {
    let (dir, manager) = fixture();
    let research_root = root(&dir);
    // Publish a deck first, then make one report unreadable-UTF-8 and force a
    // second run: the refusal must leave the published deck byte-identical.
    let first = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect("the first run succeeds");
    let published = std::fs::read(&first.path).expect("the deck is readable");

    let newer_path = item_dir(&research_root, "deckspec").join("RESEARCH-2025-02-14.md");
    std::fs::write(&newer_path, [0xff_u8, 0xfe, 0x00, 0x41]).expect("the bytes are writable");

    let options = DeckRunOptions {
        force: true,
        ..DeckRunOptions::default()
    };
    let err = manager
        .deck("deckspec", &options)
        .await
        .expect_err("a non-UTF-8 report is refused");
    let message = err.to_string();
    assert!(
        message.contains("RESEARCH-2025-02-14.md"),
        "the refusal names the file: {message}"
    );
    assert!(
        message.contains("not valid UTF-8"),
        "the refusal names the reason: {message}"
    );
    assert!(
        matches!(err, ResearchError::DeckReport { .. }),
        "expected DeckReport, got: {err}"
    );
    assert_eq!(
        std::fs::read(&first.path).expect("the deck is still readable"),
        published,
        "a refusal must leave an existing deck byte-identical"
    );
}

#[tokio::test]
async fn test_deck_refuses_report_without_recognised_sections() {
    let (dir, manager) = fixture();
    let research_root = root(&dir);
    // `--force` must not bypass the parse check.
    let newer_path = item_dir(&research_root, "deckspec").join("RESEARCH-2025-02-14.md");
    std::fs::write(
        &newer_path,
        "# Title only\n\nNo recognised sections here.\n",
    )
    .expect("the report is writable");

    let options = DeckRunOptions {
        force: true,
        ..DeckRunOptions::default()
    };
    let err = manager
        .deck("deckspec", &options)
        .await
        .expect_err("an unparseable report is refused");
    let message = err.to_string();
    assert!(
        message.contains("RESEARCH-2025-02-14.md"),
        "message: {message}"
    );
    assert!(
        message.contains("no recognised top-level section"),
        "message: {message}"
    );
}

// -- FR-017: the --force guard ---------------------------------------------

#[tokio::test]
async fn test_deck_refuses_existing_deck_without_force() {
    let (_dir, manager) = fixture();
    let first = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect("the first run succeeds");
    let published = std::fs::read(&first.path).expect("the deck is readable");

    let err = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect_err("an existing deck is refused without --force");
    let message = err.to_string();
    assert!(
        message.contains(&first.path.display().to_string()),
        "the refusal names the existing path: {message}"
    );
    assert!(message.contains("--force"), "message: {message}");
    assert!(
        matches!(err, ResearchError::DeckWrite(DeckWriteError::Exists { .. })),
        "expected Exists, got: {err}"
    );
    assert_eq!(
        std::fs::read(&first.path).expect("the deck is still readable"),
        published,
        "a refusal must leave the existing deck byte-identical"
    );
}

#[tokio::test]
async fn test_deck_force_overwrites_the_existing_deck() {
    let (dir, manager) = fixture();
    // Write a stale placeholder at the default path, then overwrite it.
    let research_root = root(&dir);
    let stamp = chrono::Utc::now()
        .date_naive()
        .format("%Y-%m-%d")
        .to_string();
    let path = default_deck_path(&item_dir(&research_root, "deckspec"), &stamp);
    std::fs::write(&path, "stale").expect("the placeholder is writable");

    let options = DeckRunOptions {
        force: true,
        ..DeckRunOptions::default()
    };
    let outcome = manager
        .deck("deckspec", &options)
        .await
        .expect("--force overwrites the existing deck");
    assert_eq!(outcome.path, path);
    let written = std::fs::read_to_string(&path).expect("the deck is readable");
    assert_eq!(written, outcome.document.markdown);
    assert!(!written.contains("stale"));
}

// -- FR-016: the --out override --------------------------------------------

#[tokio::test]
async fn test_deck_out_writes_to_the_named_path_and_creates_parents() {
    let (dir, manager) = fixture();
    let nested = dir.path().join("nested").join("deeper").join("deck.md");
    let options = DeckRunOptions {
        out: Some(nested.display().to_string()),
        ..DeckRunOptions::default()
    };
    let outcome = manager
        .deck("deckspec", &options)
        .await
        .expect("--out writes outside the item directory");
    assert_eq!(outcome.path, nested);
    let written = std::fs::read_to_string(&nested).expect("the deck is readable");
    assert_eq!(written, outcome.document.markdown);
    // The default dated file was NOT also written.
    let item_entries = entry_names(&item_dir(&root(&dir), "deckspec"));
    assert!(
        !item_entries.iter().any(|name| name.starts_with("DECK-")),
        "an --out run must not also write the default deck: {item_entries:?}"
    );
}

#[tokio::test]
async fn test_deck_refuses_an_out_value_naming_a_directory() {
    let (dir, manager) = fixture();
    let options = DeckRunOptions {
        out: Some(dir.path().display().to_string()),
        ..DeckRunOptions::default()
    };
    let err = manager
        .deck("deckspec", &options)
        .await
        .expect_err("a directory is not a deck destination");
    assert!(
        matches!(
            err,
            ResearchError::DeckWrite(DeckWriteError::InvalidOut { .. })
        ),
        "expected InvalidOut, got: {err}"
    );
}

#[tokio::test]
async fn test_deck_out_is_subject_to_the_force_guard() {
    let (dir, manager) = fixture();
    let target = dir.path().join("existing.md");
    std::fs::write(&target, "keep me").expect("the target is writable");
    let options = DeckRunOptions {
        out: Some(target.display().to_string()),
        ..DeckRunOptions::default()
    };
    let err = manager
        .deck("deckspec", &options)
        .await
        .expect_err("an existing --out target is refused without --force");
    assert!(
        matches!(err, ResearchError::DeckWrite(DeckWriteError::Exists { .. })),
        "expected Exists, got: {err}"
    );
    assert_eq!(
        std::fs::read_to_string(&target).expect("the target is readable"),
        "keep me",
        "the refusal must not modify the existing file"
    );
}

// -- FR-014: the --elements filter ----------------------------------------

#[tokio::test]
async fn test_deck_elements_filter_restricts_the_slides() {
    let (dir, manager) = fixture();
    let research_root = root(&dir);
    let full = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect("the unfiltered run succeeds");

    let filtered_path = dir.path().join("filtered.md");
    let options = DeckRunOptions {
        elements: vec![DeckElement::Concepts],
        out: Some(filtered_path.display().to_string()),
        ..DeckRunOptions::default()
    };
    let filtered = manager
        .deck("deckspec", &options)
        .await
        .expect("the filtered run succeeds");

    assert_eq!(filtered.document.elements, vec![DeckElement::Concepts]);
    assert!(
        full.document.elements.contains(&DeckElement::Findings),
        "the fixture must change Findings too: {:?}",
        full.document.elements
    );
    // The filter runs before rendering, so its counts describe only the
    // elements it wrote: the filtered diff has no Findings item and no
    // Executive Summary item at all.
    assert_eq!(filtered.old_count, 1, "one concept is unchanged");
    assert_eq!(filtered.new_count + filtered.added_count, 1);
    assert_eq!(filtered.change_count(), 1);
    assert_eq!(
        full.old_count, 2,
        "the unfiltered diff also carries the unchanged summary"
    );
    let item_entries = entry_names(&item_dir(&research_root, "deckspec"));
    assert!(
        item_entries.iter().any(|name| name.starts_with("DECK-")),
        "the unfiltered run wrote the default deck: {item_entries:?}"
    );
}

#[tokio::test]
async fn test_deck_without_elements_flag_emits_every_changed_element() {
    let (_dir, manager) = fixture();
    let outcome = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect("the fixture pair produces a deck");
    assert_eq!(
        outcome.document.elements,
        vec![DeckElement::Concepts, DeckElement::Findings],
        "FR-003 order, only the changed elements"
    );
}

// -- FR-015: --include-old ------------------------------------------------

#[tokio::test]
async fn test_deck_include_old_adds_slides_for_unchanged_elements() {
    let (dir, manager) = fixture();
    let without = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect("the default run succeeds");
    assert!(
        !without
            .document
            .elements
            .contains(&DeckElement::ExecutiveSummary),
        "an unchanged element produces no slide by default: {:?}",
        without.document.elements
    );

    let options = DeckRunOptions {
        include_old: true,
        out: Some(dir.path().join("with-old.md").display().to_string()),
        ..DeckRunOptions::default()
    };
    let with = manager
        .deck("deckspec", &options)
        .await
        .expect("the --include-old run succeeds");
    assert!(
        with.document
            .elements
            .contains(&DeckElement::ExecutiveSummary),
        "--include-old emits the unchanged element: {:?}",
        with.document.elements
    );
    // The fixture has three elements with items: the two that changed, plus
    // the unchanged Executive Summary that only `--include-old` emits.
    assert_eq!(without.document.elements.len(), 2);
    assert_eq!(with.document.elements.len(), 3);
}

// -- FR-013: an unchanged pair still writes a deck -------------------------

#[tokio::test]
async fn test_deck_identical_reports_still_write_the_no_changes_deck() {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_report(&research_root, "deckspec", "2025-01-10", NEWER);
    write_report(&research_root, "deckspec", "2025-02-14", NEWER);
    let manager = ResearchManager::new(&research_root);

    let outcome = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect("an unchanged pair still writes a deck");
    assert!(outcome.no_changes);
    assert_eq!(outcome.change_count(), 0);
    assert_eq!(
        outcome.document.elements.len(),
        0,
        "an unchanged pair emits no element slide"
    );
    let written = std::fs::read_to_string(&outcome.path).expect("the deck is readable");
    assert!(
        written.contains("_No changes detected between the two reports._"),
        "the FR-013 notice is in the deck"
    );
    // Every item of both reports is unchanged: 1 summary + 2 questions +
    // 1 finding.
    assert_eq!(outcome.old_count, 4);
}

// -- FR-004 counts reported by the outcome --------------------------------

#[tokio::test]
async fn test_deck_outcome_reports_the_per_status_counts() {
    let (_dir, manager) = fixture();
    let outcome = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect("the fixture pair produces a deck");

    // Concepts: 1 unchanged + 1 new. Findings: a finding's identity IS
    // its headline (FR-009), so a reworded headline is a different item and
    // reads `NEW`. The unchanged Executive Summary adds a second `OLD`.
    assert_eq!(outcome.old_count, 2);
    assert_eq!(outcome.update_count, 0, "no matched item changed its text");
    assert_eq!(outcome.new_count + outcome.added_count, 2);
    assert_eq!(outcome.change_count(), 2);
    assert!(!outcome.no_changes);
    // The cover slide carries the same counts.
    assert!(
        outcome
            .document
            .markdown
            .contains("**Coverage:** 2025-01-10 to 2025-02-14")
    );
}

#[tokio::test]
async fn test_deck_outcome_old_count_matches_the_rendered_diff() {
    let (_dir, manager) = fixture();
    let outcome = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect("the fixture pair produces a deck");
    // Every rendered item carries exactly one of the four statuses: a finding
    // is tagged in its `### ` heading, and a concept carries one status line.
    // The count must equal the items of the elements the deck emitted - which
    // excludes an unchanged element's items unless `--include-old` asked for
    // it.
    let statuses = [
        ItemStatus::New,
        ItemStatus::Added,
        ItemStatus::Update,
        ItemStatus::Old,
    ];
    let tagged = outcome
        .document
        .markdown
        .lines()
        .filter(|line| {
            statuses.iter().any(|status| {
                line.contains(&format!("- `{}` ", status.tag()))
                    || line.contains(&format!("### `{}` ", status.tag()))
                    || line.trim() == format!("_`{}`_", status.tag())
            })
        })
        .count();
    let emitted_items: usize = outcome
        .document
        .elements
        .iter()
        .map(|element| match element {
            DeckElement::Concepts => 2,
            DeckElement::Findings => 1,
            other => panic!("the fixture changes only two elements, got {other:?}"),
        })
        .sum();
    assert_eq!(
        tagged, emitted_items,
        "every item of every emitted element is tagged exactly once"
    );
    // The outcome's counts describe the whole diff, so the unchanged Executive
    // Summary's item counts towards `old_count` even though it has no slide.
    assert_eq!(outcome.old_count, 2);
}

// -- FR-007: an explicit report pair --------------------------------------

#[tokio::test]
async fn test_deck_honours_an_explicit_report_pair() {
    let (dir, manager) = fixture();
    let research_root = root(&dir);
    // A third, newest report the default selection would pick instead.
    write_report(
        &research_root,
        "deckspec",
        "2025-03-20",
        &NEWER.replace("How much does", "What role does"),
    );
    let options = DeckRunOptions {
        from: Some("2025-01-10".to_string()),
        to: Some("2025-02-14".to_string()),
        ..DeckRunOptions::default()
    };
    let outcome = manager
        .deck("deckspec", &options)
        .await
        .expect("an explicit pair is honoured");
    assert_eq!(outcome.pair.older_label(), "2025-01-10");
    assert_eq!(outcome.pair.newer_label(), "2025-02-14");
    assert!(
        outcome
            .document
            .markdown
            .contains("**Coverage:** 2025-01-10 to 2025-02-14"),
        "the cover names the explicit window: {}",
        outcome
            .document
            .markdown
            .lines()
            .take(8)
            .collect::<Vec<_>>()
            .join(" | ")
    );
}

// -- NFR-002 / FR-021: determinism ---------------------------------------

#[tokio::test]
async fn test_deck_is_byte_identical_across_two_runs_in_one_day() {
    let (dir, manager) = fixture();
    let second = dir.path().join("second.md");
    let first = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect("the first run succeeds");
    let again = manager
        .deck(
            "deckspec",
            &DeckRunOptions {
                out: Some(second.display().to_string()),
                ..DeckRunOptions::default()
            },
        )
        .await
        .expect("the second run succeeds");
    assert_eq!(first.document.markdown, again.document.markdown);
    assert_eq!(
        std::fs::read(&first.path).expect("the first deck is readable"),
        std::fs::read(&second).expect("the second deck is readable"),
        "the same pair must render byte-identically"
    );
}
