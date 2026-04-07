//! Visual regression tests for tool output rendering
//!
//! This test suite captures expected TUI output patterns for each tool category
//! and verifies consistent formatting across tool output patterns.

use ragent_core::tool::format::*;
use ragent_core::tool::truncate::truncate_content;

// =============================================================================
// Content Pattern Visual Tests
// =============================================================================

#[test]
fn test_pattern_a_visual_format() {
    // Pattern A: Summary + Content
    // Expected format: "Summary line\n\nContent lines"
    let content =
        format_summary_content("5 matches found", "match1\nmatch2\nmatch3\nmatch4\nmatch5");

    // Should have separator
    assert!(
        content.contains("\n\n"),
        "Pattern A should have blank line separator"
    );

    // First line should be summary
    let lines: Vec<&str> = content.lines().collect();
    assert!(
        lines[0].contains("matches found"),
        "First line should be summary"
    );

    // After separator, should have content
    assert!(
        content.contains("match1"),
        "Should contain content after separator"
    );
}

#[test]
fn test_pattern_b_visual_format() {
    // Pattern B: Summary Only
    // Expected format: "Summary line"
    let content = format_simple_summary("Wrote 42 bytes to file.txt");

    // Should be a single line
    let lines: Vec<&str> = content.lines().collect();
    assert_eq!(lines.len(), 1, "Pattern B should be single line");

    // Should not have separator
    assert!(
        !content.contains("\n\n"),
        "Pattern B should not have separator"
    );
}

#[test]
fn test_pattern_c_visual_format() {
    // Pattern C: Structured
    // Expected format: "Exit code: X\nDuration: Yms\n\nSTDOUT:\n..."
    let content = format_status_output(0, "Hello output", "", 150, false);

    // Should have structured header
    assert!(
        content.contains("Exit code:"),
        "Should have exit code header"
    );
    assert!(content.contains("Duration:"), "Should have duration header");
    assert!(content.contains("ms"), "Should have ms unit");

    // Should have stdout section
    assert!(content.contains("STDOUT:"), "Should have STDOUT section");
}

#[test]
fn test_pattern_c_with_stderr_visual_format() {
    let content = format_status_output(1, "", "Error occurred", 200, false);

    // Should have stderr section
    assert!(
        content.contains("STDERR:"),
        "Should have STDERR section when stderr present"
    );
    assert!(
        !content.contains("STDOUT:"),
        "Should not have STDOUT section when empty"
    );
}

// =============================================================================
// Truncation Visual Tests
// =============================================================================

#[test]
fn test_truncation_visual_format() {
    // When content is truncated, should show marker
    let content = "line1\nline2\nline3\nline4\nline5";
    let result = truncate_content(content, 3);

    // Should contain marker
    assert!(result.contains("..."), "Should have ellipsis marker");
    assert!(result.contains("omitted"), "Should have 'omitted' text");
    assert!(result.contains("lines"), "Should mention lines");
}

#[test]
fn test_truncation_marker_position() {
    // Marker should be at the end
    let content = "a\nb\nc\nd\ne"; // 5 lines
    let result = truncate_content(content, 3); // Show 2 + marker

    // Should end with marker
    assert!(
        result.ends_with("lines omitted) ...") || result.ends_with("line omitted) ..."),
        "Should end with omission marker: {}",
        result
    );
}

// =============================================================================
// Byte Formatting Visual Tests
// =============================================================================

#[test]
fn test_bytes_visual_format() {
    assert_eq!(format_bytes(0), "0 B", "0 bytes should show '0 B'");
    assert_eq!(format_bytes(512), "512 B", "Small bytes should show as B");

    // KB should have decimal
    let kb = format_bytes(1536);
    assert!(kb.contains("."), "KB should have decimal: {}", kb);
    assert!(kb.contains("KB"), "KB should have unit: {}", kb);

    // MB should have decimal
    let mb = format_bytes(1024 * 1024 + 512 * 1024);
    assert!(mb.contains("."), "MB should have decimal: {}", mb);
    assert!(mb.contains("MB"), "MB should have unit: {}", mb);
}

// =============================================================================
// Count Formatting Visual Tests
// =============================================================================

#[test]
fn test_line_count_singular_plural() {
    assert_eq!(format_line_count(0), "0 lines", "0 should be plural");
    assert_eq!(format_line_count(1), "1 line", "1 should be singular");
    assert_eq!(format_line_count(2), "2 lines", "2+ should be plural");
}

#[test]
fn test_file_count_singular_plural() {
    assert_eq!(format_file_count(0), "0 files", "0 should be plural");
    assert_eq!(format_file_count(1), "1 file", "1 should be singular");
    assert_eq!(format_file_count(2), "2 files", "2+ should be plural");
}

#[test]
fn test_match_count_singular_plural() {
    assert_eq!(format_match_count(0), "0 matches", "0 should be plural");
    assert_eq!(format_match_count(1), "1 match", "1 should be singular");
    assert_eq!(format_match_count(2), "2 matches", "2+ should be plural");
}

// =============================================================================
// Edit Summary Visual Tests
// =============================================================================

#[test]
fn test_edit_summary_visual_format() {
    // Same line count
    let same = format_edit_summary(5, 5);
    assert!(
        !same.contains("→"),
        "Same count should not use arrow: {}",
        same
    );

    // Different line count
    let diff = format_edit_summary(3, 5);
    assert!(
        diff.contains("with"),
        "Different count should use 'with': {}",
        diff
    );
}

// =============================================================================
// Path Display Visual Tests
// =============================================================================

#[test]
fn test_display_path_relative_visual() {
    use std::path::Path;

    // Path under working dir should be relative
    let result = format_display_path(
        Path::new("/home/user/project/src/main.rs"),
        Path::new("/home/user/project"),
    );
    assert_eq!(result, "src/main.rs", "Should be relative path");
}

#[test]
fn test_display_path_absolute_visual() {
    use std::path::Path;

    // Path outside working dir should stay absolute
    let result = format_display_path(Path::new("/etc/passwd"), Path::new("/home/user/project"));
    assert_eq!(result, "/etc/passwd", "Should remain absolute");
}

// =============================================================================
// Tool Output Category Visual Tests
// =============================================================================

#[test]
fn test_file_operation_output_visual() {
    // Write operation
    let summary = format_summary_content("10 lines written to test.txt", "");
    assert!(summary.contains("written"), "Write should say 'written'");
    assert!(summary.contains("lines"), "Should mention lines");
}

#[test]
fn test_search_operation_output_visual() {
    // Search results
    let content = format_summary_content("5 matches found in 3 files", "match1\nmatch2");
    assert!(content.contains("matches"), "Should mention matches");
    assert!(content.contains("files"), "Should mention files");
}

#[test]
fn test_execution_output_visual() {
    // Bash output
    let content = format_status_output(0, "output", "", 100, false);
    assert!(
        content.contains("Exit code:"),
        "Should have exit code label"
    );
    assert!(content.contains("Duration:"), "Should have duration label");
}

// =============================================================================
// Consistency Visual Tests
// =============================================================================

#[test]
fn test_consistent_terminators() {
    // All content should use \n (Unix style), not \r\n
    let content1 = format_summary_content("test", "line1\nline2");
    assert!(!content1.contains("\r\n"), "Should use Unix line endings");

    let content2 = format_status_output(0, "out", "", 50, false);
    assert!(!content2.contains("\r\n"), "Should use Unix line endings");
}

#[test]
fn test_no_trailing_whitespace() {
    // Summary lines shouldn't have trailing whitespace
    let summary = format_simple_summary("Test summary");
    assert!(!summary.ends_with(' '), "Should not have trailing space");
    assert!(!summary.ends_with("\t"), "Should not have trailing tab");
}

#[test]
fn test_consistent_capitalization() {
    // Pattern A: Summary starts with capital letter typically
    let summary1 = format_summary_content("5 Lines read", "content");
    let first_char = summary1.chars().next().unwrap();
    assert!(
        first_char.is_ascii_digit() || first_char.is_ascii_uppercase(),
        "Summary should start with number or capital"
    );

    // Pattern C: Headers are capitalized
    let status = format_status_output(0, "", "", 100, false);
    assert!(
        status.contains("Exit code:"),
        "'Exit' should be capitalized"
    );
    assert!(
        status.contains("Duration:"),
        "'Duration' should be capitalized"
    );
}

// =============================================================================
// Edge Case Visual Tests
// =============================================================================

#[test]
fn test_empty_content_visual() {
    // Empty content should still produce valid output
    let pattern_a = format_summary_content("No results", "");
    assert!(
        !pattern_a.contains("\n\n"),
        "Empty Pattern A should not have separator"
    );
    assert_eq!(pattern_a, "No results");
}

#[test]
fn test_single_line_content_visual() {
    let content = format_summary_content("1 match found", "the_match");
    assert!(content.contains("\n\n"), "Should have separator");
    assert!(content.ends_with("the_match"), "Should end with content");
}

#[test]
fn test_very_long_summary_visual() {
    // Long summary should still be readable
    let long_summary = "a".repeat(200);
    let content = format_summary_content(&long_summary, "content");
    assert!(content.contains("\n\n"), "Should still have separator");
}

// =============================================================================
// Snapshot-Style Tests
// =============================================================================

/// This test documents the expected output format for each pattern type.
/// If these assertions fail, the output format has changed.
#[test]
fn test_expected_output_snapshots() {
    // Pattern A snapshot
    let pattern_a = format_summary_content("3 lines read", "Line 1\nLine 2\nLine 3");
    assert_eq!(pattern_a, "3 lines read\n\nLine 1\nLine 2\nLine 3");

    // Pattern B snapshot
    let pattern_b = format_simple_summary("Wrote 100 bytes");
    assert_eq!(pattern_b, "Wrote 100 bytes");

    // Pattern C snapshot (success)
    let pattern_c = format_status_output(0, "Hello\nWorld", "", 150, false);
    assert_eq!(
        pattern_c,
        "Exit code: 0\nDuration: 150ms\n\nSTDOUT:\nHello\nWorld"
    );

    // Pattern C snapshot (with stderr)
    let pattern_c_err = format_status_output(1, "partial", "Error!", 200, false);
    assert!(pattern_c_err.contains("STDOUT:\npartial"));
    assert!(pattern_c_err.contains("STDERR:\nError!"));
}

/// This test documents expected truncation format.
#[test]
fn test_truncation_snapshot() {
    let content = "a\nb\nc\nd\ne"; // 5 lines
    let truncated = truncate_content(content, 3);

    // Should be: "a\nb\n... (3 lines omitted) ..."
    assert!(
        truncated.starts_with("a\nb"),
        "Should start with first 2 lines"
    );
    assert!(
        truncated.contains("... (3 lines omitted) ..."),
        "Should have correct marker"
    );
}
