//! Integration tests for `ragent-tools-core` content truncation helpers.
//!
//! Relocated from the inline `#[cfg(test)]` module in `src/truncate.rs`
//! (T-008 of the testconsolidate spec). All tested functions are public:
//! `truncate_content`, `truncate_content_head_tail`, `get_truncation_stats`.

use ragent_tools_core::truncate::{
    get_truncation_stats, truncate_content, truncate_content_head_tail,
};

#[test]
fn test_truncate_content_no_truncation() {
    let content = "line1\nline2\nline3";
    let result = truncate_content(content, 5);
    assert_eq!(result, content);
}

#[test]
fn test_truncate_content_single_line() {
    let content = "line1";
    let result = truncate_content(content, 5);
    assert_eq!(result, content);
}

#[test]
fn test_truncate_content_empty() {
    let result = truncate_content("", 5);
    assert_eq!(result, "");
}

#[test]
fn test_truncate_content_with_truncation() {
    let content = "a\nb\nc\nd\ne";
    let result = truncate_content(content, 3);

    assert!(result.contains('a'));
    assert!(result.contains('b'));
    // c, d, e should be omitted
    assert!(!result.contains("\nc\n"));
    assert!(!result.contains("\nd\n"));
    assert!(result.contains("... (3 lines omitted) ..."));
}

#[test]
fn test_truncate_content_single_omission() {
    let content = "a\nb\nc\nd";
    let result = truncate_content(content, 3);

    assert!(result.contains('a'));
    assert!(result.contains('b'));
    assert!(result.contains("... (2 lines omitted) ..."));
}

#[test]
fn test_truncate_content_one_line_omitted() {
    let content = "a\nb\nc\nd";
    let result = truncate_content(content, 3);
    assert!(result.contains("... (2 lines omitted) ..."));
}

#[test]
fn test_truncate_content_max_lines_zero() {
    let content = "line1\nline2";
    let result = truncate_content(content, 0);
    assert_eq!(result, "");
}

#[test]
fn test_truncate_content_head_tail() {
    let content = (1..=20)
        .map(|n| format!("line{}", n))
        .collect::<Vec<_>>()
        .join("\n");

    let result = truncate_content_head_tail(&content, 10, 3, 3);

    assert!(result.contains("line1"));
    assert!(result.contains("line2"));
    assert!(result.contains("line3"));
    assert!(result.contains("line18"));
    assert!(result.contains("line19"));
    assert!(result.contains("line20"));
    assert!(result.contains("... (14 lines omitted) ..."));

    // Middle lines should be omitted
    assert!(!result.contains("line10"));
}

#[test]
fn test_truncate_content_head_tail_no_truncation_needed() {
    let content = "line1\nline2\nline3";
    let result = truncate_content_head_tail(content, 5, 2, 2);
    assert_eq!(result, content);
}

#[test]
fn test_truncate_content_head_tail_exceeds_max() {
    let content = "a\nb\nc\nd\ne\nf\ng"; // 7 lines
    // head (3) + tail (3) = 6 > max_lines (5), should fall back to simple
    let result = truncate_content_head_tail(content, 5, 3, 3);

    // Should fall back to simple truncate with max_lines=5
    // Simple truncate shows first 4 lines + marker
    assert!(result.contains('a'));
    assert!(result.contains('b'));
    assert!(result.contains('c'));
    assert!(result.contains('d'));
    assert!(result.contains("... (3 lines omitted) ..."));
}

#[test]
fn test_get_truncation_stats() {
    let (displayed, total, truncated) = get_truncation_stats("line1\nline2", 5);
    assert_eq!(displayed, 2);
    assert_eq!(total, 2);
    assert!(!truncated);

    let (displayed, total, truncated) = get_truncation_stats("a\nb\nc\nd\ne", 3);
    assert_eq!(displayed, 3);
    assert_eq!(total, 5);
    assert!(truncated);
}

#[test]
fn test_get_truncation_stats_empty() {
    let (displayed, total, truncated) = get_truncation_stats("", 5);
    assert_eq!(displayed, 0);
    assert_eq!(total, 0);
    assert!(!truncated);
}
