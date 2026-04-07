//! Tests for tool output format utilities
//!
//! This test suite verifies the format module's helper functions produce
//! standardized output according to the tool output consistency plan.

use ragent_core::tool::format::*;

// =============================================================================
// Pattern A: Summary + Content Tests
// =============================================================================

#[test]
fn test_format_summary_content_basic() {
    let summary = "5 lines read";
    let content = "line1\nline2\nline3\nline4\nline5";
    let result = format_summary_content(summary, content);

    assert!(result.contains("5 lines read"), "Summary should appear");
    assert!(result.contains("line1"), "Content should appear");
    assert!(result.contains("\n\n"), "Should have blank line separator");
    assert_eq!(result, "5 lines read\n\nline1\nline2\nline3\nline4\nline5");
}

#[test]
fn test_format_summary_content_empty_content() {
    let result = format_summary_content("No matches found", "");
    assert_eq!(
        result, "No matches found",
        "Should return just summary when content is empty"
    );
    assert!(
        !result.contains("\n\n"),
        "Should not have separator for empty content"
    );
}

#[test]
fn test_format_summary_content_single_line() {
    let result = format_summary_content("1 match found", "only_line");
    assert_eq!(result, "1 match found\n\nonly_line");
}

// =============================================================================
// Pattern C: Structured Output Tests
// =============================================================================

#[test]
fn test_format_status_output_success() {
    let result = format_status_output(0, "Hello", "", 150, false);

    assert!(result.contains("Exit code: 0"), "Should show exit code");
    assert!(result.contains("Duration: 150ms"), "Should show duration");
    assert!(result.contains("STDOUT:\nHello"), "Should show stdout");
    assert!(!result.contains("timed out"), "Should not show timeout");
    assert!(
        !result.contains("STDERR"),
        "Should not show stderr section when empty"
    );
}

#[test]
fn test_format_status_output_with_stderr() {
    let result = format_status_output(1, "", "Error message", 200, false);

    assert!(result.contains("Exit code: 1"), "Should show exit code");
    assert!(
        result.contains("STDERR:\nError message"),
        "Should show stderr"
    );
    assert!(
        !result.contains("STDOUT:"),
        "Should not show stdout section when empty"
    );
}

#[test]
fn test_format_status_output_both_streams() {
    let result = format_status_output(0, "Output", "Error", 100, false);

    assert!(result.contains("STDOUT:\nOutput"), "Should show stdout");
    assert!(result.contains("STDERR:\nError"), "Should show stderr");
}

#[test]
fn test_format_status_output_timed_out() {
    let result = format_status_output(0, "partial", "", 5000, true);

    assert!(result.contains("(timed out)"), "Should indicate timeout");
    assert!(result.contains("5000ms"), "Should still show duration");
}

#[test]
fn test_format_status_output_empty_streams() {
    let result = format_status_output(0, "", "", 50, false);

    // Should only have header
    assert_eq!(result, "Exit code: 0\nDuration: 50ms\n");
}

// =============================================================================
// Pattern B: Summary Only Tests
// =============================================================================

#[test]
fn test_format_simple_summary() {
    let result = format_simple_summary("Wrote 42 bytes to file.txt");
    assert_eq!(result, "Wrote 42 bytes to file.txt");
}

// =============================================================================
// Count Formatting Tests
// =============================================================================

#[test]
fn test_format_bytes() {
    assert_eq!(format_bytes(0), "0 B");
    assert_eq!(format_bytes(512), "512 B");
    assert_eq!(format_bytes(1024), "1.0 KB");
    assert_eq!(format_bytes(1536), "1.5 KB");
    assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
    assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
}

#[test]
fn test_format_line_count() {
    assert_eq!(format_line_count(0), "0 lines");
    assert_eq!(format_line_count(1), "1 line");
    assert_eq!(format_line_count(2), "2 lines");
    assert_eq!(format_line_count(100), "100 lines");
}

#[test]
fn test_format_file_count() {
    assert_eq!(format_file_count(0), "0 files");
    assert_eq!(format_file_count(1), "1 file");
    assert_eq!(format_file_count(2), "2 files");
    assert_eq!(format_file_count(5), "5 files");
}

#[test]
fn test_format_match_count() {
    assert_eq!(format_match_count(0), "0 matches");
    assert_eq!(format_match_count(1), "1 match");
    assert_eq!(format_match_count(2), "2 matches");
    assert_eq!(format_match_count(10), "10 matches");
}

#[test]
fn test_format_entry_count() {
    assert_eq!(format_entry_count(0), "0 entries");
    assert_eq!(format_entry_count(1), "1 entry");
    assert_eq!(format_entry_count(2), "2 entries");
    assert_eq!(format_entry_count(15), "15 entries");
}

// =============================================================================
// Edit Summary Tests
// =============================================================================

#[test]
fn test_format_edit_summary_replacement() {
    // Same number of lines
    assert_eq!(format_edit_summary(5, 5), "replaced 5 lines with 5 lines");
}

#[test]
fn test_format_edit_summary_addition() {
    // More lines added
    assert_eq!(format_edit_summary(3, 5), "replaced 3 lines with 5 lines");
}

#[test]
fn test_format_edit_summary_deletion() {
    // Lines removed
    assert_eq!(format_edit_summary(10, 2), "replaced 10 lines with 2 lines");
}

#[test]
fn test_format_edit_summary_single_line() {
    assert_eq!(format_edit_summary(1, 1), "replaced 1 line");
    assert_eq!(format_edit_summary(1, 2), "replaced 1 line with 2 lines");
    assert_eq!(format_edit_summary(2, 1), "replaced 2 lines with 1 line");
}

// =============================================================================
// Path Display Tests
// =============================================================================

#[test]
fn test_format_display_path_short() {
    use std::path::Path;
    assert_eq!(
        format_display_path(Path::new("file.txt"), Path::new("/home/user")),
        "file.txt"
    );
}

#[test]
fn test_format_display_path_with_prefix() {
    use std::path::Path;
    let result = format_display_path(
        Path::new("/home/user/project/src/main.rs"),
        Path::new("/home/user/project"),
    );
    assert_eq!(result, "src/main.rs");
}

#[test]
fn test_format_display_path_no_prefix() {
    use std::path::Path;
    let path = Path::new("/other/path/file.txt");
    let result = format_display_path(path, Path::new("/home/user"));
    assert_eq!(result, "/other/path/file.txt");
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_format_summary_content_multiline() {
    let summary = "Multi\nline\nsummary";
    let content = "content";
    let result = format_summary_content(summary, content);

    assert!(
        result.contains("Multi\nline\nsummary"),
        "Should preserve multiline summary"
    );
}

#[test]
fn test_format_status_output_zero_duration() {
    let result = format_status_output(0, "out", "", 0, false);
    assert!(result.contains("Duration: 0ms"));
}

#[test]
fn test_format_status_output_negative_exit_code() {
    // Some systems use negative exit codes for signals
    let result = format_status_output(-1, "killed", "", 100, false);
    assert!(result.contains("Exit code: -1"));
}
