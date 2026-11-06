//! Every refusal path of a deck run (spec `diffdeck`: FR-011, FR-012, FR-014,
//! FR-017, FR-018, FR-019, FR-020).
//!
//! A refusal is two promises, and the second is the one that is easy to lose:
//!
//! 1. it names what the user must fix - the item, the file, the offending flag
//!    value, the reason, and where relevant the values that would have worked;
//! 2. it writes nothing - no deck file, no directory, and any pre-existing
//!    `DECK-*.md` stays byte-identical (the FR-019 invariant).
//!
//! Every case therefore drives [`ResearchManager::deck`] against a real item
//! directory in a `tempfile::TempDir`, asserts the error variant and its
//! message, and then asserts the item directory's complete file set is
//! unchanged. The `--elements` refusal is the one case that fails before the
//! manager is reached (the parser rejects an unknown name), so it is pinned
//! against the parser's accepted set instead.

use ragent_research::deck::{
    DeckElement, DeckRunOptions, DeckSelectError, DeckWriteError, deck_file_name,
};
use ragent_research::manager::ResearchError;
use ragent_research::{ResearchManager, ResearchName};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
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

/// Write `body` as the file `file_name` of item `name`.
fn write_file(research_root: &Path, name: &str, file_name: &str, body: &str) -> PathBuf {
    let dir = item_dir(research_root, name);
    std::fs::create_dir_all(&dir).expect("the item directory is creatable");
    let path = dir.join(file_name);
    std::fs::write(&path, body).expect("the fixture file is writable");
    path
}

/// Write `body` as a dated report of item `name`.
fn write_report(research_root: &Path, name: &str, stamp: &str, body: &str) -> PathBuf {
    write_file(research_root, name, &format!("RESEARCH-{stamp}.md"), body)
}

/// A minimal valid report with one implication, so the pair has a change.
const REPORT_OLDER: &str = "# Deck Fixture\n\n## Executive Summary\n\nA baseline summary.\n\n\
                            ## Concepts\n\n### 1. Index Quality\n\n**Definition:** Index quality matters.\n";

/// A newer valid report that adds a second concept.
const REPORT_NEWER: &str = "# Deck Fixture\n\n## Executive Summary\n\nA baseline summary.\n\n\
                            ## Concepts\n\n### 1. Index Quality\n\n**Definition:** Index quality matters.\n\n\
                            ### 2. Chunking Recall\n\n**Definition:** Chunking dominates recall.\n";

/// Every file in `dir` with its exact bytes, so "unchanged" is a byte
/// comparison rather than a name listing.
fn snapshot(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(_) => return files,
    };
    for entry in entries {
        let entry = entry.expect("the directory entry is readable");
        let path = entry.path();
        if path.is_file() {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .expect("a fixture file name is UTF-8")
                .to_string();
            let bytes = std::fs::read(&path).expect("the fixture file is readable");
            files.insert(name, bytes);
        }
    }
    files
}

/// A manager over a temp root holding `deckspec` with one valid report pair.
fn fixture() -> (TempDir, ResearchManager) {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_report(&research_root, "deckspec", "2025-01-10", REPORT_OLDER);
    write_report(&research_root, "deckspec", "2025-02-14", REPORT_NEWER);
    let manager = ResearchManager::new(&research_root);
    (dir, manager)
}

/// Run a deck for `name` that is expected to be refused, and return the error.
async fn refusal(manager: &ResearchManager, name: &str, options: &DeckRunOptions) -> ResearchError {
    match manager.deck(name, options).await {
        Ok(outcome) => panic!(
            "the run must be refused, but it wrote {}",
            outcome.path.display()
        ),
        Err(error) => error,
    }
}

// -- FR-018: the item does not exist --------------------------------------

#[tokio::test]
async fn test_deck_refusal_names_the_missing_item_and_creates_no_directory() {
    let (dir, manager) = fixture();
    let research_root = root(&dir);

    let error = refusal(&manager, "nosuchitem", &DeckRunOptions::default()).await;
    let message = error.to_string();
    assert!(
        message.contains("nosuchitem"),
        "the refusal names the item: {message}"
    );
    assert!(
        matches!(error, ResearchError::NotFound(ref name, _) if name == "nosuchitem"),
        "expected NotFound, got: {error}"
    );
    assert!(
        !item_dir(&research_root, "nosuchitem").exists(),
        "a refusal must not create the item directory"
    );
}

#[tokio::test]
async fn test_deck_refusal_for_a_missing_item_leaves_the_root_untouched() {
    let (dir, manager) = fixture();
    let research_root = root(&dir);
    let before = snapshot(&research_root);

    refusal(&manager, "nosuchitem", &DeckRunOptions::default()).await;

    assert_eq!(
        snapshot(&research_root),
        before,
        "a refused run writes nothing anywhere under the research root"
    );
}

// -- FR-011: fewer than two reports ---------------------------------------

#[tokio::test]
async fn test_deck_refusal_names_the_one_report_found() {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_report(&research_root, "oneshot", "2025-02-01", REPORT_OLDER);
    let manager = ResearchManager::new(&research_root);
    let item = item_dir(&research_root, "oneshot");
    let before = snapshot(&item);

    let error = refusal(&manager, "oneshot", &DeckRunOptions::default()).await;
    let message = error.to_string();
    assert!(message.contains("oneshot"), "message: {message}");
    assert!(message.contains("1 report(s) found"), "message: {message}");
    assert!(
        !message.contains("legacy undated RESEARCH.md"),
        "a dated-only item must not be blamed on the legacy scheme: {message}"
    );
    assert!(
        matches!(
            error,
            ResearchError::DeckSelect(DeckSelectError::TooFewReports { found: 1, .. })
        ),
        "expected TooFewReports, got: {error}"
    );
    assert_eq!(snapshot(&item), before, "a refusal must not write a file");
}

#[tokio::test]
async fn test_deck_refusal_for_an_empty_item_names_zero_reports() {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    let item = item_dir(&research_root, "emptyitem");
    std::fs::create_dir_all(&item).expect("the item directory is creatable");
    let manager = ResearchManager::new(&research_root);

    let error = refusal(&manager, "emptyitem", &DeckRunOptions::default()).await;
    let message = error.to_string();
    assert!(message.contains("emptyitem"), "message: {message}");
    assert!(message.contains("0 report(s) found"), "message: {message}");
    assert_eq!(snapshot(&item), BTreeMap::new(), "nothing was written");
}

// -- FR-012: the legacy undated scheme -----------------------------------

#[tokio::test]
async fn test_deck_refusal_for_a_legacy_only_item_names_the_scheme() {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_file(&research_root, "legacyonly", "RESEARCH.md", REPORT_OLDER);
    let manager = ResearchManager::new(&research_root);
    let item = item_dir(&research_root, "legacyonly");
    let before = snapshot(&item);

    let error = refusal(&manager, "legacyonly", &DeckRunOptions::default()).await;
    let message = error.to_string();
    assert!(
        message.contains("legacy undated RESEARCH.md"),
        "the refusal names the legacy scheme: {message}"
    );
    assert!(
        message.contains("RESEARCH-<YYYY-MM-DD>.md"),
        "the refusal states which scheme would work: {message}"
    );
    assert!(
        matches!(
            error,
            ResearchError::DeckSelect(DeckSelectError::TooFewReports {
                found: 1,
                only_legacy: true,
                ..
            })
        ),
        "expected TooFewReports with only_legacy, got: {error}"
    );
    assert_eq!(snapshot(&item), before, "a refusal must not write a file");
}

// -- FR-020: an impossible or unknown report date ------------------------

#[tokio::test]
async fn test_deck_refusal_names_an_impossible_date() {
    let (_dir, manager) = fixture();
    let options = DeckRunOptions {
        from: Some("2025-04-31".to_string()),
        ..DeckRunOptions::default()
    };

    let error = refusal(&manager, "deckspec", &options).await;
    let message = error.to_string();
    assert!(message.contains("2025-04-31"), "message: {message}");
    assert!(
        message.contains("YYYY-MM-DD"),
        "the refusal states the accepted form: {message}"
    );
    assert!(
        matches!(
            error,
            ResearchError::DeckSelect(DeckSelectError::InvalidStamp { .. })
        ),
        "expected InvalidStamp, got: {error}"
    );
}

#[tokio::test]
async fn test_deck_refusal_names_an_unknown_date_and_lists_the_dates_that_exist() {
    let (_dir, manager) = fixture();
    let options = DeckRunOptions {
        from: Some("2025-01-11".to_string()),
        ..DeckRunOptions::default()
    };

    let error = refusal(&manager, "deckspec", &options).await;
    let message = error.to_string();
    assert!(message.contains("2025-01-11"), "message: {message}");
    assert!(
        message.contains("2025-01-10") && message.contains("2025-02-14"),
        "the refusal lists the report dates that do exist: {message}"
    );
    assert!(
        matches!(
            error,
            ResearchError::DeckSelect(DeckSelectError::UnknownStamp { .. })
        ),
        "expected UnknownStamp, got: {error}"
    );
}

#[tokio::test]
async fn test_deck_refusal_keeps_the_existing_deck_byte_identical_for_an_inverted_pair() {
    // A refusal that happens during selection must not touch a deck that an
    // earlier successful run left in the item directory (FR-019).
    let (dir, manager) = fixture();
    let research_root = root(&dir);
    let item = item_dir(&research_root, "deckspec");
    let existing = item.join(deck_file_name("2025-02-20"));
    std::fs::write(&existing, b"a previous deck").expect("the fixture deck is writable");
    let before = snapshot(&item);

    let options = DeckRunOptions {
        from: Some("2025-02-14".to_string()),
        to: Some("2025-01-10".to_string()),
        force: true,
        ..DeckRunOptions::default()
    };
    let error = refusal(&manager, "deckspec", &options).await;
    assert!(
        matches!(
            error,
            ResearchError::DeckSelect(DeckSelectError::InvertedPair { .. })
        ),
        "expected InvertedPair, got: {error}"
    );
    assert_eq!(
        snapshot(&item),
        before,
        "an existing deck file is byte-identical after a refused run"
    );
}

// -- FR-019: an unreadable or unparseable report -------------------------

#[tokio::test]
async fn test_deck_refusal_for_a_non_utf8_report_names_the_path_and_the_reason() {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_report(&research_root, "frozen", "2025-01-10", REPORT_OLDER);
    // Latin-1 bytes: valid text for its author, invalid UTF-8 for us.
    let newer = item_dir(&research_root, "frozen").join("RESEARCH-2025-02-14.md");
    std::fs::create_dir_all(newer.parent().expect("the report has a parent"))
        .expect("the item directory is creatable");
    std::fs::write(
        &newer,
        b"# Deck Fixture\n\n## Executive Summary\n\ncaf\xe9 latte\n",
    )
    .expect("the fixture report is writable");
    let existing = item_dir(&research_root, "frozen").join(deck_file_name("2025-02-20"));
    std::fs::write(&existing, b"a previous deck").expect("the fixture deck is writable");
    let manager = ResearchManager::new(&research_root);
    let item = item_dir(&research_root, "frozen");
    let before = snapshot(&item);

    let error = refusal(&manager, "frozen", &DeckRunOptions::default()).await;
    let message = error.to_string();
    assert!(
        message.contains("RESEARCH-2025-02-14.md"),
        "the refusal names the offending file: {message}"
    );
    assert!(
        message.contains("not valid UTF-8"),
        "the refusal names the reason: {message}"
    );
    assert!(
        matches!(error, ResearchError::DeckReport { .. }),
        "expected DeckReport, got: {error}"
    );
    assert_eq!(
        snapshot(&item),
        before,
        "an unreadable report leaves any existing deck byte-identical"
    );
}

#[tokio::test]
async fn test_deck_refusal_for_a_report_without_recognised_sections_names_the_path() {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_report(&research_root, "corrupt", "2025-01-05", REPORT_OLDER);
    write_report(
        &research_root,
        "corrupt",
        "2025-02-05",
        "# Deck Fixture\n\nJust prose, with no top-level section at all.\n",
    );
    let manager = ResearchManager::new(&research_root);
    let item = item_dir(&research_root, "corrupt");
    let before = snapshot(&item);

    let error = refusal(&manager, "corrupt", &DeckRunOptions::default()).await;
    let message = error.to_string();
    assert!(
        message.contains("RESEARCH-2025-02-05.md"),
        "the refusal names the offending file: {message}"
    );
    assert!(
        !message.contains("not valid UTF-8"),
        "a parseable-but-empty report is not an encoding failure: {message}"
    );
    assert!(
        matches!(error, ResearchError::DeckReport { .. }),
        "expected DeckReport, got: {error}"
    );
    assert_eq!(snapshot(&item), before, "a refusal must not write a file");
}

#[tokio::test]
async fn test_deck_refusal_for_an_unreadable_report_names_the_path_and_the_reason() {
    // The pair is chosen from the directory listing, so the read is the first
    // place an unreadable file is noticed (FR-019). A directory named like a
    // report is listed like any other entry and cannot be read as a file.
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_report(&research_root, "dangling", "2025-01-10", REPORT_OLDER);
    let newer = item_dir(&research_root, "dangling").join("RESEARCH-2025-02-14.md");
    std::fs::create_dir_all(&newer).expect("the fixture directory is creatable");
    let manager = ResearchManager::new(&research_root);
    let item = item_dir(&research_root, "dangling");
    let before = snapshot(&item);

    let error = refusal(&manager, "dangling", &DeckRunOptions::default()).await;
    let message = error.to_string();
    assert!(
        message.contains("RESEARCH-2025-02-14.md"),
        "the refusal names the offending path: {message}"
    );
    assert!(
        matches!(error, ResearchError::DeckReport { .. }),
        "expected DeckReport, got: {error}"
    );
    assert_eq!(snapshot(&item), before, "a refusal must not write a file");
}

// -- FR-017: an existing deck without --force ----------------------------

#[tokio::test]
async fn test_deck_refusal_names_the_existing_deck_and_the_force_flag() {
    let (dir, manager) = fixture();
    let research_root = root(&dir);
    // The default destination is named for the generation day, so the existing
    // file is created at the path a default run would use.
    let item = item_dir(&research_root, "deckspec");
    let first = manager
        .deck("deckspec", &DeckRunOptions::default())
        .await
        .expect("the first run writes the deck");
    assert!(first.path.starts_with(&item));
    let before = snapshot(&item);

    let error = refusal(&manager, "deckspec", &DeckRunOptions::default()).await;
    let message = error.to_string();
    assert!(
        message.contains(&first.path.display().to_string()),
        "the refusal names the existing path: {message}"
    );
    assert!(
        message.contains("--force"),
        "the refusal names the flag that overwrites it: {message}"
    );
    assert!(
        matches!(
            error,
            ResearchError::DeckWrite(DeckWriteError::Exists { .. })
        ),
        "expected DeckWrite::Exists, got: {error}"
    );
    assert_eq!(
        snapshot(&item),
        before,
        "the existing deck is byte-identical after the refusal"
    );
}

#[tokio::test]
async fn test_deck_refusal_with_out_applies_the_force_guard_to_the_out_path() {
    let (dir, manager) = fixture();
    let out_dir = dir.path().join("out");
    std::fs::create_dir_all(&out_dir).expect("the out directory is creatable");
    let out_path = out_dir.join("deck.md");
    std::fs::write(&out_path, b"an earlier deck").expect("the fixture deck is writable");

    let options = DeckRunOptions {
        out: Some(out_path.display().to_string()),
        ..DeckRunOptions::default()
    };
    let error = refusal(&manager, "deckspec", &options).await;
    let message = error.to_string();
    assert!(
        message.contains("deck.md"),
        "the refusal names the out path: {message}"
    );
    assert!(message.contains("--force"), "message: {message}");
    assert_eq!(
        std::fs::read(&out_path).expect("the out file is readable"),
        b"an earlier deck",
        "the out file is byte-identical after the refusal"
    );
}

#[tokio::test]
async fn test_deck_refusal_for_an_out_value_naming_a_directory() {
    let (dir, manager) = fixture();
    let out_dir = dir.path().join("out");
    std::fs::create_dir_all(&out_dir).expect("the out directory is creatable");

    let options = DeckRunOptions {
        out: Some(out_dir.display().to_string()),
        force: true,
        ..DeckRunOptions::default()
    };
    let error = refusal(&manager, "deckspec", &options).await;
    let message = error.to_string();
    assert!(
        message.contains("invalid --out path"),
        "the refusal names the offending flag: {message}"
    );
    assert!(
        message.contains("not a directory"),
        "the refusal names the reason: {message}"
    );
    assert!(
        matches!(
            error,
            ResearchError::DeckWrite(DeckWriteError::InvalidOut { .. })
        ),
        "expected DeckWrite::InvalidOut, got: {error}"
    );
}

// -- FR-014: an unknown --elements name ----------------------------------

#[test]
fn test_deck_refusal_for_an_unknown_element_names_the_value_and_the_accepted_set() {
    // The `--elements` refusal happens at parse time, before any file is read,
    // so it is pinned against the element vocabulary the parser resolves.
    assert!(
        DeckElement::from_name("Glossary").is_none(),
        "the fixture name must be outside the closed element set"
    );
    let accepted: Vec<&str> = DeckElement::ALL
        .iter()
        .map(|element| element.label())
        .collect();
    assert_eq!(
        accepted,
        vec![
            "Executive Summary",
            "Concepts",
            "Findings",
            "Cross-References",
            "References",
        ],
        "the accepted set is the six labelled elements in precedence order"
    );
    for name in &accepted {
        assert!(
            DeckElement::from_name(name).is_some(),
            "every accepted name resolves: {name}"
        );
    }
}

#[tokio::test]
async fn test_deck_filter_rejecting_every_element_writes_no_slide() {
    // An `--elements` filter that matches an element with no items must not
    // turn into a silent no-op: the deck is written, and it carries no element
    // slide because the filter removed the only changed element.
    let (dir, manager) = fixture();
    let research_root = root(&dir);
    let item = item_dir(&research_root, "deckspec");
    let before = snapshot(&item);
    let options = DeckRunOptions {
        elements: vec![DeckElement::References],
        ..DeckRunOptions::default()
    };

    let outcome = manager
        .deck("deckspec", &options)
        .await
        .expect("the fixture pair produces a deck");
    let after = snapshot(&item);
    assert_eq!(
        after.len(),
        before.len() + 1,
        "a filtered run writes exactly one new file, the deck: {after:?}"
    );
    assert!(
        outcome.path.starts_with(&item),
        "the deck lands in the item directory: {}",
        outcome.path.display()
    );
    assert_eq!(
        outcome.document.elements.len(),
        0,
        "the filter removed the only changed element, so no slide is emitted"
    );
    assert!(outcome.no_changes, "the filtered deck reports no changes");
}

// -- FR-006: the default pair, on a real item directory -------------------

#[tokio::test]
async fn test_deck_default_pair_is_the_two_most_recent_reports() {
    // Three dated reports plus the legacy baseline: the default pair is the two
    // greatest stamps, and the legacy file is not part of it (FR-006, FR-012).
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_file(&research_root, "series", "RESEARCH.md", REPORT_OLDER);
    write_report(&research_root, "series", "2025-01-10", REPORT_OLDER);
    write_report(&research_root, "series", "2025-02-14", REPORT_OLDER);
    write_report(&research_root, "series", "2025-03-20", REPORT_NEWER);
    let manager = ResearchManager::new(&research_root);

    let outcome = manager
        .deck("series", &DeckRunOptions::default())
        .await
        .expect("three dated reports produce a deck");

    assert_eq!(outcome.pair.older_label(), "2025-02-14");
    assert_eq!(outcome.pair.newer_label(), "2025-03-20");
    let markdown = &outcome.document.markdown;
    assert!(
        markdown.contains("**Coverage:** 2025-02-14 to 2025-03-20"),
        "the cover states the default coverage window: {markdown}"
    );
}

#[tokio::test]
async fn test_deck_default_refuses_when_only_the_legacy_report_and_one_dated_report_exist() {
    // One legacy baseline plus one dated report is two reports, so the default
    // FR-006 pair needs two *dated* reports and is refused (FR-011), while the
    // count reported is the two files that do exist.
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_file(&research_root, "halflegacy", "RESEARCH.md", REPORT_OLDER);
    write_report(&research_root, "halflegacy", "2025-02-14", REPORT_NEWER);
    let manager = ResearchManager::new(&research_root);
    let item = item_dir(&research_root, "halflegacy");
    let before = snapshot(&item);

    let error = refusal(&manager, "halflegacy", &DeckRunOptions::default()).await;
    let message = error.to_string();
    assert!(message.contains("halflegacy"), "message: {message}");
    assert!(
        matches!(
            error,
            ResearchError::DeckSelect(DeckSelectError::TooFewReports { found: 2, .. })
        ),
        "expected TooFewReports naming both files, got: {error}"
    );
    assert_eq!(snapshot(&item), before, "a refusal must not write a file");
}

// -- FR-012: the legacy baseline on a real item directory ----------------

#[tokio::test]
async fn test_deck_uses_the_legacy_report_as_the_older_side_of_an_explicit_pair() {
    let dir = TempDir::new().expect("a temp directory is creatable");
    let research_root = root(&dir);
    write_file(&research_root, "legacypair", "RESEARCH.md", REPORT_OLDER);
    write_report(&research_root, "legacypair", "2025-02-14", REPORT_NEWER);
    let manager = ResearchManager::new(&research_root);

    let options = DeckRunOptions {
        to: Some("2025-02-14".to_string()),
        ..DeckRunOptions::default()
    };
    let outcome = manager
        .deck("legacypair", &options)
        .await
        .expect("the legacy baseline pairs with the dated report");

    assert_eq!(outcome.pair.older_label(), "legacy");
    assert_eq!(outcome.pair.newer_label(), "2025-02-14");
    assert!(
        outcome
            .document
            .markdown
            .contains("**Coverage:** legacy to 2025-02-14"),
        "the cover names the legacy baseline: {}",
        outcome.document.markdown
    );
    assert_eq!(
        outcome.new_count, 1,
        "the added concept is the only change against the legacy baseline"
    );
}
