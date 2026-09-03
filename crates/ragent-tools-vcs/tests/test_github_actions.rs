//! Tests for the GitHub Actions tool's pure helper `extract_context_ranges`.

use ragent_tools_vcs::github::github_actions::extract_context_ranges;

#[test]
fn test_extract_context_ranges_no_matches() {
    let lines = vec!["all good", "nothing here", "just info"];
    let ranges = extract_context_ranges(&lines);
    assert_eq!(
        ranges,
        Vec::<(usize, usize)>::new(),
        "expected no ranges for clean log"
    );
}

#[test]
fn test_extract_context_ranges_single_match_centred() {
    let total = 30;
    let lines: Vec<&str> = (0..total)
        .map(|i| if i == 15 { "ERROR boom" } else { "line" })
        .collect();
    let ranges = extract_context_ranges(&lines);
    assert_eq!(ranges.len(), 1, "expected a single merged range");
    let (start, end) = ranges[0];
    // ±10 context around line 15 → [5, 26)
    assert_eq!(start, 5);
    assert_eq!(end, 26);
}

#[test]
fn test_extract_context_ranges_match_near_start_clamped() {
    let lines: Vec<&str> = (0..40)
        .map(|i| if i == 2 { "failed early" } else { "line" })
        .collect();
    let ranges = extract_context_ranges(&lines);
    assert_eq!(ranges.len(), 1);
    let (start, end) = ranges[0];
    assert_eq!(start, 0, "start should clamp to 0");
    assert_eq!(end, 13, "end should be match + 10 + 1");
}

#[test]
fn test_extract_context_ranges_match_near_end_clamped() {
    let lines: Vec<&str> = (0..20)
        .map(|i| if i == 18 { "ERROR at end" } else { "line" })
        .collect();
    let ranges = extract_context_ranges(&lines);
    assert_eq!(ranges.len(), 1);
    let (start, end) = ranges[0];
    assert_eq!(start, 8);
    assert_eq!(end, 20, "end should clamp to lines.len()");
}

#[test]
fn test_extract_context_ranges_overlapping_matches_merge() {
    // Two matches within 2*CONTEXT_RADIUS of each other must merge into one range.
    let lines: Vec<&str> = (0..40)
        .map(|i| match i {
            10 | 25 => "error here",
            _ => "line",
        })
        .collect();
    let ranges = extract_context_ranges(&lines);
    assert_eq!(ranges.len(), 1, "adjacent windows should merge");
    let (start, end) = ranges[0];
    assert_eq!(start, 0);
    assert_eq!(end, 36);
}

#[test]
fn test_extract_context_ranges_case_insensitive() {
    let lines = vec!["ERROR", "Error", "error", "FAILED", "failed"];
    let ranges = extract_context_ranges(&lines);
    // Every line matches, so one merged range covers all of them.
    assert_eq!(ranges.len(), 1);
    assert_eq!(ranges[0], (0, 5));
}
