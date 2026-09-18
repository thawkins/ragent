//! Unit tests for `archdoc::local_source` - bounded local file/folder
//! acquisition with supported-format filtering for `/spec govcreate` (spec
//! `govdoc` T-007, FR-005, NFR-002, NFR-003, NFR-004).
//!
//! The budget exhaustion check and the supported-format predicate are pure and
//! tested without any I/O. The acquisition path runs against real fixture
//! directories built in `target/temp`-style tempdirs; no test touches the
//! network.

use std::path::{Path, PathBuf};

use ragent_tools_extended::archdoc::url_source::{BudgetReason, ExclusionReason};
use ragent_tools_extended::archdoc::{
    DEFAULT_LOCAL_MAX_TOTAL_CHARS, DEFAULT_MAX_FILES, LocalAcquisitionBudget,
    LocalAcquisitionStats, LocalBudgetReason, acquire_local, is_supported_document,
    local_budget_exhausted,
};

// ===========================================================================
// Fixture helpers
// ===========================================================================

/// Build a fresh fixture directory under the system temp dir, unique per test.
fn fixture_dir(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("govdoc-t007-{}-{}", name, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("fixture dir should be created");
    dir
}

/// Write a file under a fixture root, creating parents.
fn write_file(root: &Path, rel: &str, content: &str) -> PathBuf {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("fixture parent should be created");
    }
    std::fs::write(&path, content).expect("fixture file should be written");
    path
}

fn tear_down(root: &Path) {
    let _ = std::fs::remove_dir_all(root);
}

// ===========================================================================
// Pure budget and predicate tests (NFR-002)
// ===========================================================================

#[test]
fn test_local_budget_defaults_use_named_constants() {
    let budget = LocalAcquisitionBudget::default();
    assert_eq!(budget.max_files, DEFAULT_MAX_FILES);
    assert_eq!(DEFAULT_MAX_FILES, 100);
    assert_eq!(budget.max_total_chars, DEFAULT_LOCAL_MAX_TOTAL_CHARS);
    // The deadline is shared with the URL budget rather than re-literalised.
    assert_eq!(
        budget.deadline_ms,
        ragent_tools_extended::archdoc::AcquisitionBudget::default().deadline_ms
    );
}

#[test]
fn test_local_budget_exhausted_within_limits_is_none() {
    let stats = LocalAcquisitionStats {
        files_extracted: 3,
        total_chars: 100,
        elapsed_ms: 500,
    };
    assert_eq!(
        local_budget_exhausted(&stats, &LocalAcquisitionBudget::default()),
        None
    );
}

#[test]
fn test_local_budget_exhausted_file_cap() {
    let stats = LocalAcquisitionStats {
        files_extracted: 5,
        total_chars: 10,
        elapsed_ms: 1,
    };
    let budget = LocalAcquisitionBudget::new(5, 1_000_000, 120_000);
    assert_eq!(
        local_budget_exhausted(&stats, &budget),
        Some(LocalBudgetReason::MaxFiles)
    );
    assert_eq!(
        LocalBudgetReason::MaxFiles.as_corpus_reason(),
        BudgetReason::MaxPages
    );
}

#[test]
fn test_local_budget_exhausted_char_budget() {
    let stats = LocalAcquisitionStats {
        files_extracted: 1,
        total_chars: 2_000,
        elapsed_ms: 1,
    };
    let budget = LocalAcquisitionBudget::new(100, 2_000, 120_000);
    assert_eq!(
        local_budget_exhausted(&stats, &budget),
        Some(LocalBudgetReason::MaxTotalChars)
    );
}

#[test]
fn test_local_budget_exhausted_deadline_takes_precedence() {
    let stats = LocalAcquisitionStats {
        files_extracted: 100,
        total_chars: 200_000,
        elapsed_ms: 30_000,
    };
    let budget = LocalAcquisitionBudget::new(100, 200_000, 30_000);
    assert_eq!(
        local_budget_exhausted(&stats, &budget),
        Some(LocalBudgetReason::Deadline)
    );
}

#[test]
fn test_is_supported_document_by_extension() {
    assert!(is_supported_document(Path::new("a/sad.md")));
    assert!(is_supported_document(Path::new("a/design.pdf")));
    assert!(is_supported_document(Path::new("a/pack.docx")));
    assert!(is_supported_document(Path::new("a/notes.txt")));
    assert!(is_supported_document(Path::new("no-extension")));
    assert!(!is_supported_document(Path::new("a/diagram.png")));
    assert!(!is_supported_document(Path::new("a/legacy.doc")));
    assert!(!is_supported_document(Path::new("a/binary.exe")));
}

// ===========================================================================
// Acquisition against real fixtures (FR-005)
// ===========================================================================

#[test]
fn test_acquire_single_supported_file() {
    let root = fixture_dir("single");
    let file = write_file(
        &root,
        "arch.md",
        "# Payments architecture\n\nComponents: gateway, ledger.\n",
    );

    let corpus = acquire_local(&file, &LocalAcquisitionBudget::default());

    assert!(!corpus.is_empty());
    assert_eq!(corpus.sources.len(), 1);
    assert!(corpus.excluded.is_empty());
    assert!(corpus.text.contains("# Payments architecture"));
    assert!(corpus.text.contains("Components: gateway, ledger."));
    assert_eq!(corpus.stats.pages_fetched, 1);
    assert!(corpus.budget_reached.is_none());
    // The reference label is the file path.
    assert_eq!(corpus.reference, file.display().to_string());

    tear_down(&root);
}

#[test]
fn test_acquire_directory_walks_nested_subdirectories() {
    let root = fixture_dir("nested");
    write_file(&root, "01-overview.md", "# Overview\n");
    write_file(&root, "components/02-gateway.md", "# Gateway component\n");
    write_file(
        &root,
        "components/deep/03-ledger.txt",
        "Ledger responsibilities\n",
    );

    let corpus = acquire_local(&root, &LocalAcquisitionBudget::default());

    assert_eq!(corpus.sources.len(), 3);
    assert!(corpus.excluded.is_empty());
    let text = corpus.text.clone();
    assert!(text.contains("# Overview"));
    assert!(text.contains("# Gateway component"));
    assert!(text.contains("Ledger responsibilities"));
    assert_eq!(corpus.stats.pages_fetched, 3);
    // Deterministic order: depth-first, lexical per level.
    assert_eq!(
        corpus.sources[0].summary,
        root.join("01-overview.md").display().to_string()
    );

    tear_down(&root);
}

#[test]
fn test_unsupported_files_are_excluded_not_fatal() {
    let root = fixture_dir("mixed");
    write_file(&root, "arch.md", "# Architecture\n");
    let png = write_file(&root, "diagram.png", "fake-binary");
    let exe = write_file(&root, "tool.exe", "fake-binary");

    let corpus = acquire_local(&root, &LocalAcquisitionBudget::default());

    assert_eq!(corpus.sources.len(), 1);
    assert_eq!(corpus.excluded.len(), 2);
    assert!(corpus.text.contains("# Architecture"));
    // Unsupported files map to the corpus NoContent reason (they contribute no
    // text), preserving the "excluded, with reason" record FR-005 needs.
    assert!(
        corpus
            .excluded
            .iter()
            .any(|e| e.url == png.display().to_string() && e.reason == ExclusionReason::NoContent)
    );
    assert!(
        corpus
            .excluded
            .iter()
            .any(|e| e.url == exe.display().to_string() && e.reason == ExclusionReason::NoContent)
    );

    tear_down(&root);
}

#[test]
fn test_empty_supported_file_yields_empty_corpus() {
    let root = fixture_dir("empty");
    write_file(&root, "arch.md", "   \n  \n");

    let corpus = acquire_local(&root, &LocalAcquisitionBudget::default());

    assert!(corpus.is_empty());
    assert_eq!(corpus.excluded.len(), 1);
    assert_eq!(corpus.excluded[0].reason, ExclusionReason::NoContent);
    assert_eq!(corpus.stats.pages_fetched, 1);

    tear_down(&root);
}

#[test]
fn test_file_cap_truncates_and_reports_budget() {
    let root = fixture_dir("filecap");
    for index in 0..5 {
        write_file(
            &root,
            &format!("doc-{index:02}.md"),
            &format!("# Doc {index}\n"),
        );
    }
    let budget = LocalAcquisitionBudget::new(3, 1_000_000, 120_000);

    let corpus = acquire_local(&root, &budget);

    assert_eq!(corpus.sources.len(), 3);
    assert_eq!(corpus.budget_reached, Some(BudgetReason::MaxPages));
    assert_eq!(corpus.stats.pages_fetched, 3);

    tear_down(&root);
}

#[test]
fn test_char_budget_truncates_content_and_reports_budget() {
    let root = fixture_dir("charcap");
    let body = "x".repeat(500);
    write_file(&root, "a.md", &format!("# A\n{body}"));
    write_file(&root, "b.md", &format!("# B\n{body}"));
    // 600 chars: a.md fits fully (~502), b.md is truncated to the remainder.
    let budget = LocalAcquisitionBudget::new(100, 600, 120_000);

    let corpus = acquire_local(&root, &budget);

    assert_eq!(corpus.budget_reached, Some(BudgetReason::MaxTotalChars));
    assert!(corpus.stats.total_chars <= 600);
    assert!(corpus.stats.total_chars > 600 - 510);

    tear_down(&root);
}

#[test]
fn test_zero_deadline_reports_deadline() {
    let root = fixture_dir("deadline");
    write_file(&root, "a.md", "# A\n");
    let budget = LocalAcquisitionBudget::new(100, 200_000, 0);

    let corpus = acquire_local(&root, &budget);

    assert_eq!(corpus.budget_reached, Some(BudgetReason::Deadline));

    tear_down(&root);
}

#[test]
fn test_non_utf8_supported_file_is_read_failed_exclusion() {
    let root = fixture_dir("utf8");
    let path = root.join("bad.md");
    std::fs::write(&path, [0xff, 0xfe, 0xfd, 0x00]).expect("fixture should be written");

    let corpus = acquire_local(&path, &LocalAcquisitionBudget::default());

    assert!(corpus.is_empty());
    assert_eq!(corpus.excluded.len(), 1);
    assert_eq!(corpus.excluded[0].reason, ExclusionReason::FetchFailed);

    tear_down(&root);
}

#[cfg(unix)]
#[test]
fn test_symlinked_entries_are_never_followed() {
    let root = fixture_dir("symlink");
    write_file(&root, "real/arch.md", "# Inside\n");
    // A symlink pointing outside the fixture must not be followed (NFR-003).
    let outside = fixture_dir("symlink-outside");
    write_file(&outside, "outside.md", "# Outside\n");
    std::os::unix::fs::symlink(&outside, root.join("linked")).expect("symlink should be created");

    let corpus = acquire_local(&root, &LocalAcquisitionBudget::default());

    assert_eq!(corpus.sources.len(), 1);
    assert!(corpus.text.contains("# Inside"));
    assert!(!corpus.text.contains("# Outside"));

    tear_down(&root);
    tear_down(&outside);
}
