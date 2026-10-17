//! Branch-coverage tests for the GitHub Actions pure helpers
//! (`github/github_actions.rs`): `run_status`, `extract_context_ranges`, and
//! `filter_log_text`.
//!
//! These functions carry all the status-label and log-filtering logic the
//! `github_get_actions` tool relies on; the tool's network paths are exercised
//! elsewhere, so this file pins the deterministic logic without any I/O.

use ragent_tools_vcs::github::github_actions::{
    extract_context_ranges, filter_log_text, run_status,
};

#[test]
fn test_run_status_in_progress_and_queued_are_running() {
    assert_eq!(run_status(None, "in_progress"), "Running");
    assert_eq!(run_status(Some("success"), "in_progress"), "Running");
    assert_eq!(run_status(None, "queued"), "Running");
    assert_eq!(run_status(Some("failure"), "queued"), "Running");
}

#[test]
fn test_run_status_success_neutral_and_skipped_are_ok() {
    assert_eq!(run_status(Some("success"), "completed"), "OK");
    assert_eq!(run_status(Some("neutral"), "completed"), "OK");
    assert_eq!(run_status(Some("skipped"), "completed"), "OK");
}

#[test]
fn test_run_status_everything_else_is_failed() {
    assert_eq!(run_status(Some("failure"), "completed"), "Failed");
    assert_eq!(run_status(Some("cancelled"), "completed"), "Failed");
    assert_eq!(run_status(Some("timed_out"), "completed"), "Failed");
    assert_eq!(run_status(Some("action_required"), "completed"), "Failed");
    assert_eq!(run_status(Some("stale"), "completed"), "Failed");
    // A completed run with no conclusion (unknown state) is reported failed.
    assert_eq!(run_status(None, "completed"), "Failed");
}

#[test]
fn test_extract_context_ranges_empty_input_is_empty() {
    assert_eq!(extract_context_ranges(&[]).len(), 0);
}

#[test]
fn test_extract_context_ranges_adjacent_windows_merge() {
    // Two matches 2*CONTEXT_RADIUS apart produce windows that are adjacent, so
    // they must merge into a single contiguous range.
    let mut lines: Vec<String> = (0..40).map(|i| format!("line {i}")).collect();
    lines[10] = "an error here".to_string();
    lines[30] = "also failed".to_string();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let ranges = extract_context_ranges(&refs);
    assert_eq!(ranges.len(), 1, "adjacent windows must merge: {ranges:?}");
    // 10-10=0 .. 30+10+1=41 clamped to 40.
    assert_eq!(ranges[0], (0, 40));
}

#[test]
fn test_extract_context_ranges_distinct_windows_stay_separate() {
    let mut lines: Vec<String> = (0..80).map(|i| format!("line {i}")).collect();
    lines[5] = "failed".to_string();
    lines[60] = "error".to_string();
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let ranges = extract_context_ranges(&refs);
    assert_eq!(ranges.len(), 2, "distant windows stay separate: {ranges:?}");
    assert_eq!(ranges[0], (0, 16));
    assert_eq!(ranges[1], (50, 71));
}

#[test]
fn test_filter_log_text_empty_when_no_match() {
    let text = "all good\nnothing to see\n";
    assert_eq!(filter_log_text(text), "");
}

#[test]
fn test_filter_log_text_renders_header_and_context() {
    let mut lines: Vec<String> = (0..30).map(|i| format!("log line {i}")).collect();
    lines[15] = "ERROR: boom".to_string();
    let text = lines.join("\n");
    let out = filter_log_text(&text);
    assert!(out.contains("--- lines 5..26 ---"), "got: {out}");
    assert!(out.contains("ERROR: boom"), "match line must be present");
    assert!(out.contains("log line 5"), "context above must be present");
    assert!(out.contains("log line 25"), "context below must be present");
    assert!(!out.contains("log line 0"), "far context must be omitted");
}

#[test]
fn test_filter_log_text_multiline_snippet_crlf_tolerated() {
    // CRLF input: `lines()` strips the trailing '\r' on modern Rust, so the
    // keyword still matches and the context window is emitted.
    let text = "setup ok\r\nfailed step\r\ncleanup\r\n";
    let out = filter_log_text(text);
    assert!(out.contains("failed step"), "got: {out}");
    assert!(out.contains("--- lines 0..3 ---"), "got: {out}");
}
