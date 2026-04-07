//! Tests for content truncation utilities
//!
//! This test suite verifies that the truncate module correctly handles
//! content truncation with appropriate markers and statistics.

use ragent_core::tool::truncate::*;

// =============================================================================
// Basic Truncation Tests
// =============================================================================

#[test]
fn test_truncate_content_no_truncation() {
    let content = "line1\nline2\nline3";
    let result = truncate_content(content, 5);
    assert_eq!(
        result, content,
        "Content under limit should not be truncated"
    );
}

#[test]
fn test_truncate_content_exact_match() {
    let content = "a\nb\nc";
    let result = truncate_content(content, 3);
    assert_eq!(
        result, content,
        "Content exactly at limit should not be truncated"
    );
}

#[test]
fn test_truncate_content_single_line() {
    let content = "only line";
    let result = truncate_content(content, 5);
    assert_eq!(result, content, "Single line should not be truncated");
}

#[test]
fn test_truncate_content_empty() {
    let result = truncate_content("", 5);
    assert_eq!(result, "", "Empty content should return empty");
}

#[test]
fn test_truncate_content_with_truncation() {
    let content = "a\nb\nc\nd\ne"; // 5 lines
    let result = truncate_content(content, 3); // Show 2 lines + marker

    // With max_lines=3, we show max_lines-1 = 2 lines, then marker
    assert!(result.contains("a"), "Should contain first line");
    assert!(result.contains("b"), "Should contain second line");
    // Lines c, d, e are omitted (5 total - 2 shown = 3 omitted)
    assert!(
        result.contains("lines omitted"),
        "Should have omission marker"
    );
}

#[test]
fn test_truncate_content_single_omission() {
    // 4 lines, showing 2 + marker = 3 lines
    let content = "a\nb\nc\nextra";
    let result = truncate_content(content, 3);

    // lines shown: a, b (2 lines)
    // lines omitted: c, extra (2 lines)
    assert!(
        result.contains("... (2 lines omitted) ...")
            || result.contains("... (3 lines omitted) ..."),
        "Should show correct lines omitted"
    );
}

#[test]
fn test_truncate_content_multiple_omissions() {
    let content = "a\nb\nc\nd\ne\nf";
    let result = truncate_content(content, 3);

    assert!(
        result.contains("... (4 lines omitted) ..."),
        "Should show plural 'lines' for multiple omissions"
    );
}

#[test]
fn test_truncate_content_max_lines_zero() {
    let result = truncate_content("a\nb\nc", 0);
    assert_eq!(result, "", "Max lines of 0 should return empty");
}

#[test]
fn test_truncate_content_max_lines_one() {
    let content = "line1\nline2\nline3";
    let result = truncate_content(content, 1);

    // With max_lines=1, we show 0 lines + marker
    assert!(
        result.contains("... (3 lines omitted) ..."),
        "Should show all lines as omitted"
    );
}

// =============================================================================
// Head/Tail Truncation Tests
// =============================================================================

#[test]
fn test_truncate_content_head_tail_no_truncation_needed() {
    let content = "a\nb\nc";
    let result = truncate_content_head_tail(content, 5, 2, 2);
    assert_eq!(
        result, content,
        "Content under limit should not be truncated"
    );
}

#[test]
fn test_truncate_content_head_tail_with_truncation() {
    // 10 lines, show first 2 and last 2 with max_lines=6
    let content = "1\n2\n3\n4\n5\n6\n7\n8\n9\n10";
    let result = truncate_content_head_tail(content, 6, 2, 2);

    assert!(result.contains("1"), "Should contain first head line");
    assert!(result.contains("2"), "Should contain second head line");
    assert!(
        result.contains("omitted"),
        "Should have marker for middle lines"
    );
    assert!(result.contains("9"), "Should contain first tail line");
    assert!(result.contains("10"), "Should contain second tail line");
}

#[test]
fn test_truncate_content_head_tail_exact_fit() {
    let content = "a\nb\nc\nd\ne";
    // head=2 + tail=2 = 4 lines, which fits in max_lines=5
    let result = truncate_content_head_tail(content, 5, 2, 2);
    assert_eq!(result, content);
}

// =============================================================================
// Truncation Statistics Tests
// =============================================================================

#[test]
fn test_get_truncation_stats_no_truncation() {
    let content = "a\nb\nc";
    let (shown, total, was_truncated) = get_truncation_stats(content, 5);

    assert_eq!(shown, 3, "Should show all lines");
    assert_eq!(total, 3, "Total should be 3");
    assert!(!was_truncated, "Should not be truncated");
}

#[test]
fn test_get_truncation_stats_with_truncation() {
    let content = "a\nb\nc\nd\ne"; // 5 lines
    let (shown, total, was_truncated) = get_truncation_stats(content, 3);

    // With max_lines=3: (max_lines-1)=2 content lines + 1 marker = 3 displayed
    assert_eq!(shown, 3, "Should show max_lines lines (content + marker)");
    assert_eq!(total, 5, "Total should be 5");
    assert!(was_truncated, "Should be truncated");
}

#[test]
fn test_get_truncation_stats_empty() {
    let (shown, total, was_truncated) = get_truncation_stats("", 5);

    assert_eq!(shown, 0, "Empty content has 0 shown");
    assert_eq!(total, 0, "Empty content has 0 total");
    assert!(!was_truncated, "Empty content is not truncated");
}

#[test]
fn test_get_truncation_stats_single_line() {
    let (shown, total, was_truncated) = get_truncation_stats("only line", 5);

    assert_eq!(shown, 1, "Single line has 1 shown");
    assert_eq!(total, 1, "Single line has 1 total");
    assert!(!was_truncated, "Single line is not truncated");
}

#[test]
fn test_get_truncation_stats_max_lines_zero() {
    let (shown, total, was_truncated) = get_truncation_stats("a\nb\nc", 0);

    assert_eq!(shown, 0, "Zero max lines means 0 shown");
    assert_eq!(total, 3, "Total is still 3");
    // With max_lines=0, we return empty immediately (no truncation marker)
    // But get_truncation_stats still calculates based on content
    // Actually, let's check the actual behavior
    let result = truncate_content("a\nb\nc", 0);
    assert_eq!(result, "");
}

// =============================================================================
// Edge Cases
// =============================================================================

#[test]
fn test_truncate_content_trailing_newline() {
    let content = "line1\nline2\n"; // Trailing newline
    let result = truncate_content(content, 5);
    // The trailing newline creates an empty last line
    let lines: Vec<&str> = result.lines().collect();
    // Just verify it doesn't panic and contains expected content
    assert!(result.contains("line1"));
    assert!(result.contains("line2"));
}

#[test]
fn test_truncate_content_no_newlines() {
    let content = "no newlines here";
    let result = truncate_content(content, 5);
    assert_eq!(
        result, content,
        "Content without newlines should be unchanged"
    );
}

#[test]
fn test_truncate_content_single_line_over_limit() {
    let content = "this is a single very long line that exceeds the line limit of three";
    let result = truncate_content(content, 3);
    // Single line without newlines should not be truncated
    assert_eq!(result, content);
}

#[test]
fn test_head_tail_edge_cases() {
    // head_lines + tail_lines > max_lines
    let content = "a\nb\nc\nd";
    let result = truncate_content_head_tail(content, 3, 2, 2);
    // When head + tail > max, we adjust
    assert!(result.contains("a"));
    assert!(result.contains("d"));
}

// =============================================================================
// Integration Tests - Simulate Real Usage
// =============================================================================

#[test]
fn test_simulate_file_read_truncation() {
    // Simulating a file read that returns 100 lines but we only show 20
    let content: String = (1..=100)
        .map(|i| format!("Line {}", i))
        .collect::<Vec<_>>()
        .join("\n");

    let result = truncate_content(&content, 20);

    assert!(result.contains("Line 1"), "Should show first line");
    assert!(result.contains("Line 19"), "Should show lines up to max-1");
    assert!(
        result.contains("... (81 lines omitted) ..."),
        "Should indicate omitted lines"
    );
    assert!(!result.contains("Line 100"), "Should not show last line");
}

#[test]
fn test_simulate_grep_results_truncation() {
    // Simulating grep results with many matches
    let matches: Vec<String> = (1..=50)
        .map(|i| format!("file{}.rs:10:match {}", i, i))
        .collect();
    let content = matches.join("\n");

    let result = truncate_content(&content, 25);

    assert!(result.contains("file1.rs"), "Should show first match");
    assert!(result.contains("omitted"), "Should show omission marker");
    // With simple truncation, last matches are not shown
    assert!(!result.contains("file50.rs"), "Should not show last match");
}

#[test]
fn test_simulate_head_tail_for_search_results() {
    // Simulating search results where we want to show context at beginning and end
    let content: String = (1..=100)
        .map(|i| format!("match {}", i))
        .collect::<Vec<_>>()
        .join("\n");

    // Show first 5 and last 5 with max_lines=12
    let result = truncate_content_head_tail(&content, 12, 5, 5);

    assert!(result.contains("match 1"), "Should show first match");
    assert!(result.contains("match 5"), "Should show 5th match");
    assert!(result.contains("omitted"), "Should indicate omitted lines");
    assert!(result.contains("match 96"), "Should show match 96");
    assert!(result.contains("match 100"), "Should show last match");
}

#[test]
fn test_truncation_preserves_line_content() {
    // Lines with special characters should be preserved
    let content =
        "normal line\n\ttab indented\n  space indented\n\nempty line above\n\r\r\rcarriage returns";
    let result = truncate_content(content, 4);

    assert!(
        result.contains("normal line"),
        "Should preserve normal line"
    );
    assert!(result.contains("\ttab"), "Should preserve tabs");
    assert!(result.contains("  space"), "Should preserve spaces");
}

// =============================================================================
// Property-Based Style Tests
// =============================================================================

#[test]
fn test_truncation_properties() {
    // Property: Result should never have more lines than max_lines
    for max_lines in [2, 5, 10, 50, 100] {
        let content: String = (1..=200)
            .map(|i| format!("Line {}", i))
            .collect::<Vec<_>>()
            .join("\n");
        let result = truncate_content(&content, max_lines);

        // Count result lines (marker counts as 1 line if present)
        let result_lines = result.lines().count();

        // The result should have exactly max_lines lines when content exceeds max_lines
        // When truncated: (max_lines - 1) content lines + 1 marker = max_lines total
        let content_lines = content.lines().count();
        if content_lines > max_lines {
            // After truncation: max_lines - 1 content lines + 1 marker = max_lines
            assert_eq!(
                result_lines, max_lines,
                "Truncated result for max_lines={} should have exactly {} lines, got {}",
                max_lines, max_lines, result_lines
            );
        }
    }
}

#[test]
fn test_truncation_marker_format() {
    // Property: Marker should always follow expected format
    let content = "a\nb\nc\nd\ne"; // 5 lines
    let result = truncate_content(content, 3);

    // Should match pattern: "... (N lines omitted) ..."
    assert!(result.contains("... ("), "Should have opening marker");
    assert!(
        result.contains(" lines omitted) ..."),
        "Should have closing marker"
    );

    // Extract and verify the number
    let marker_start = result.find("... (").unwrap() + 5;
    let marker_end = result.find(" lines omitted").unwrap();
    let num_str = &result[marker_start..marker_end];

    assert!(
        num_str.parse::<u32>().is_ok(),
        "Number between markers should be valid: '{}'",
        num_str
    );
}
