//! Tests for the diff-deck report selection stage (spec `diffdeck`: FR-006,
//! FR-007, FR-011, FR-012, FR-020).
//!
//! Every case works over an in-memory list of directory entry names; nothing is
//! written to disk.

use ragent_research::deck::{DeckSelectError, ReportPair, ReportSeries};

/// Classify a list of directory entry names into the item's report series.
fn series(names: &[&str]) -> ReportSeries {
    let owned: Vec<String> = names.iter().map(|n| (*n).to_string()).collect();
    ReportSeries::classify(&owned)
}

/// The default pair of `names`, which the fixture must be able to produce.
fn default_pair(names: &[&str]) -> ReportPair {
    match series(names).select_pair("deckspec", None, None) {
        Ok(pair) => pair,
        Err(err) => panic!("expected a pair, got: {err}"),
    }
}

/// A `--from` / `--to` selection over `names`, which the fixture must support.
fn select(names: &[&str], from: Option<&str>, to: Option<&str>) -> ReportPair {
    match series(names).select_pair("deckspec", from, to) {
        Ok(pair) => pair,
        Err(err) => panic!("expected a pair, got: {err}"),
    }
}

/// Three dated reports and the legacy baseline, plus the non-report siblings an
/// item directory also holds.
const ITEM_DIR: &[&str] = &[
    "RESEARCH-2025-02-14.md",
    "RESEARCH-2025-01-10.md",
    "RESEARCH-2025-03-20.md",
    "RESEARCH.md",
    "CORPA-2025-02-14.md",
    "DECK-2025-03-21.md",
    "state.json",
    "sources",
];

#[test]
fn test_report_series_classifies_only_report_files() {
    let series = series(ITEM_DIR);
    let names: Vec<&str> = series.reports().iter().map(|r| r.name.as_str()).collect();
    assert_eq!(
        names,
        vec![
            "RESEARCH-2025-03-20.md",
            "RESEARCH-2025-02-14.md",
            "RESEARCH-2025-01-10.md",
            "RESEARCH.md",
        ]
    );
    assert_eq!(series.len(), 4);
}

#[test]
fn test_report_series_orders_dated_reports_newest_first() {
    let series = series(ITEM_DIR);
    assert_eq!(
        series.dated_stamps(),
        vec!["2025-03-20", "2025-02-14", "2025-01-10"]
    );
}

#[test]
fn test_report_series_places_the_legacy_baseline_last() {
    // `RESEARCH.md` sorts before `RESEARCH-2` byte-wise, so a plain name sort
    // would put the baseline first; FR-012 makes it the oldest report.
    let series = series(&["RESEARCH.md", "RESEARCH-2025-01-10.md"]);
    let names: Vec<&str> = series.reports().iter().map(|r| r.name.as_str()).collect();
    assert_eq!(names, vec!["RESEARCH-2025-01-10.md", "RESEARCH.md"]);
    let legacy = series.reports().last().expect("the series is not empty");
    assert!(legacy.is_legacy());
    assert_eq!(legacy.stamp_label(), "legacy");
}

#[test]
fn test_report_series_is_empty_for_a_directory_with_no_report() {
    let series = series(&["CORPA-2025-02-14.md", "state.json"]);
    assert!(series.is_empty());
    assert_eq!(series.len(), 0);
    assert_eq!(series.dated_stamps(), Vec::<&str>::new());
}

#[test]
fn test_select_pair_defaults_to_the_two_most_recent_reports() {
    // FR-006: lexicographically greatest dated name first, so the baseline is the
    // second-newest report and the target is the newest.
    let pair = default_pair(ITEM_DIR);
    assert_eq!(pair.older.name, "RESEARCH-2025-02-14.md");
    assert_eq!(pair.newer.name, "RESEARCH-2025-03-20.md");
    assert_eq!(pair.older_label(), "2025-02-14");
    assert_eq!(pair.newer_label(), "2025-03-20");
}

#[test]
fn test_select_pair_default_ignores_the_legacy_baseline() {
    // `RESEARCH.md` is present but the default pair is the two dated reports.
    let pair = default_pair(ITEM_DIR);
    assert!(!pair.older.is_legacy());
    assert!(!pair.newer.is_legacy());
}

#[test]
fn test_select_pair_to_pairs_with_the_next_older_report() {
    // FR-007 / TC-006 step 1.
    let pair = select(ITEM_DIR, None, Some("2025-02-14"));
    assert_eq!(pair.older_label(), "2025-01-10");
    assert_eq!(pair.newer_label(), "2025-02-14");
}

#[test]
fn test_select_pair_from_pairs_with_the_next_newer_report() {
    // FR-007 / TC-006 step 3.
    let pair = select(ITEM_DIR, Some("2025-02-14"), None);
    assert_eq!(pair.older_label(), "2025-02-14");
    assert_eq!(pair.newer_label(), "2025-03-20");
}

#[test]
fn test_select_pair_to_the_oldest_dated_report_pairs_with_the_legacy_baseline() {
    // FR-012: `RESEARCH.md` is the oldest report of the series, so it is the
    // adjacent older report of the oldest dated one.
    let pair = select(ITEM_DIR, None, Some("2025-01-10"));
    assert_eq!(pair.older.name, "RESEARCH.md");
    assert_eq!(pair.older_label(), "legacy");
    assert_eq!(pair.newer_label(), "2025-01-10");
}

#[test]
fn test_select_pair_honours_an_explicit_pair() {
    let pair = select(ITEM_DIR, Some("2025-01-10"), Some("2025-03-20"));
    assert_eq!(pair.older_label(), "2025-01-10");
    assert_eq!(pair.newer_label(), "2025-03-20");
}

#[test]
fn test_select_pair_refuses_an_inverted_explicit_pair() {
    let err = series(ITEM_DIR)
        .select_pair("deckspec", Some("2025-03-20"), Some("2025-01-10"))
        .expect_err("an inverted pair must be refused");
    assert!(matches!(err, DeckSelectError::InvertedPair { .. }));
    assert!(err.to_string().contains("2025-03-20"));
    assert!(err.to_string().contains("not older than"));
}

#[test]
fn test_select_pair_refuses_an_item_with_one_report() {
    // FR-011 / TC-007 step 1: names the item and the count found.
    let err = series(&["RESEARCH-2025-02-01.md", "CORPA-2025-02-01.md"])
        .select_pair("oneshot", None, None)
        .expect_err("a single report cannot produce a deck");
    let message = err.to_string();
    assert!(matches!(
        err,
        DeckSelectError::TooFewReports { found: 1, .. }
    ));
    assert!(message.contains("oneshot"), "message was: {message}");
    assert!(message.contains("1 report"), "message was: {message}");
}

#[test]
fn test_select_pair_refuses_an_item_with_only_the_legacy_report() {
    // FR-012 / TC-007 step 2: the refusal names the legacy scheme.
    let err = series(&["RESEARCH.md"])
        .select_pair("legacyonly", None, None)
        .expect_err("the legacy baseline alone cannot produce a deck");
    let message = err.to_string();
    assert!(matches!(
        err,
        DeckSelectError::TooFewReports {
            only_legacy: true,
            ..
        }
    ));
    assert!(message.contains("legacyonly"), "message was: {message}");
    assert!(message.contains("RESEARCH.md"), "message was: {message}");
}

#[test]
fn test_select_pair_refuses_a_legacy_report_as_the_newer_side() {
    // Two reports, but the newer one can only be a date: pairing `RESEARCH.md`
    // as the target is meaningless.
    let err = series(&["RESEARCH.md", "RESEARCH-2025-01-10.md"])
        .select_pair("deckspec", Some("RESEARCH.md"), None)
        .expect_err("the legacy file is not a dated report");
    assert!(matches!(err, DeckSelectError::InvalidStamp { .. }));
}

#[test]
fn test_select_pair_refuses_an_impossible_date() {
    // FR-020 / TC-006 step 5: `2025-04-31` is not a real date.
    let err = series(ITEM_DIR)
        .select_pair("deckspec", Some("2025-04-31"), None)
        .expect_err("an impossible date must be refused");
    let message = err.to_string();
    assert!(matches!(err, DeckSelectError::InvalidStamp { .. }));
    assert!(message.contains("2025-04-31"), "message was: {message}");
}

#[test]
fn test_select_pair_refuses_a_date_with_no_report_and_lists_the_dates_that_exist() {
    // FR-020 / TC-006 step 6.
    let err = series(ITEM_DIR)
        .select_pair("deckspec", Some("2025-01-11"), None)
        .expect_err("a date with no report must be refused");
    let message = err.to_string();
    assert!(matches!(err, DeckSelectError::UnknownStamp { .. }));
    assert!(message.contains("2025-01-11"), "message was: {message}");
    for expected in ["2025-01-10", "2025-02-14", "2025-03-20"] {
        assert!(message.contains(expected), "message was: {message}");
    }
}

#[test]
fn test_select_pair_refuses_from_on_the_newest_report() {
    // FR-007: `--from` needs a next-newer report to pair with.
    let err = series(ITEM_DIR)
        .select_pair("deckspec", Some("2025-03-20"), None)
        .expect_err("the newest report has no newer neighbour");
    let message = err.to_string();
    assert!(matches!(
        err,
        DeckSelectError::NoAdjacentReport {
            direction: "newer",
            ..
        }
    ));
    assert!(message.contains("2025-03-20"), "message was: {message}");
}

#[test]
fn test_select_pair_refuses_to_on_the_oldest_report_without_a_legacy_baseline() {
    let names = &["RESEARCH-2025-01-10.md", "RESEARCH-2025-02-14.md"];
    let err = series(names)
        .select_pair("deckspec", None, Some("2025-01-10"))
        .expect_err("the oldest dated report has no older neighbour");
    assert!(matches!(
        err,
        DeckSelectError::NoAdjacentReport {
            direction: "older",
            ..
        }
    ));
}

#[test]
fn test_select_pair_refuses_the_same_stamp_on_both_sides() {
    let err = series(ITEM_DIR)
        .select_pair("deckspec", Some("2025-02-14"), Some("2025-02-14"))
        .expect_err("a report cannot be compared with itself");
    assert!(matches!(err, DeckSelectError::InvertedPair { .. }));
}

#[test]
fn test_select_pair_default_refuses_two_legacy_only_entries() {
    // A duplicate `RESEARCH.md` name cannot occur on one filesystem, but an empty
    // dated set with a non-empty series must still refuse rather than panic.
    let err = series(&["RESEARCH.md"])
        .select_pair("deckspec", None, None)
        .expect_err("no dated pair exists");
    assert!(matches!(err, DeckSelectError::TooFewReports { .. }));
}
