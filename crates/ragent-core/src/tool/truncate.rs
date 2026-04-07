//! Content truncation utilities for managing large tool outputs.
//!
//! This module provides helpers for truncating content to a maximum number of lines
//! while preserving the beginning and optionally the end of the content. When
//! truncation occurs, an informative marker is inserted to indicate how many
//! lines were omitted.
//!
//! # Examples
//!
//! ```
//! use ragent_core::tool::truncate::truncate_content;
//!
//! let content = "line1\nline2\nline3\nline4\nline5";
//! let truncated = truncate_content(content, 3);
//! // Returns first 2 lines + "... (1 line omitted) ..." pattern
//! ```

/// Truncate content to a maximum number of lines.
///
/// If the content has fewer than or equal to `max_lines` lines, it is returned
/// unchanged. If it has more, the content is truncated to show the first
/// `max_lines - 1` lines, followed by a marker indicating how many lines
/// were omitted.
///
/// # Arguments
///
/// * `content` - The content to potentially truncate
/// * `max_lines` - Maximum number of lines to include in the output
///
/// # Returns
///
/// The potentially truncated content string.
///
/// # Examples
///
/// ```
/// use ragent_core::tool::truncate::truncate_content;
///
/// // Content within limit - returned unchanged
/// let result = truncate_content("line1\nline2\nline3", 5);
/// assert_eq!(result, "line1\nline2\nline3");
///
/// // Content exceeds limit - truncated with marker
/// let content = "a\nb\nc\nd\ne";
/// let result = truncate_content(content, 3);
/// assert!(result.contains("a"));
/// assert!(result.contains("... (3 lines omitted) ..."));
/// ```
pub fn truncate_content(content: impl AsRef<str>, max_lines: usize) -> String {
    let content = content.as_ref();

    if max_lines == 0 {
        return String::new();
    }

    // Count lines
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    if total_lines <= max_lines {
        // Content fits within limit, return unchanged
        content.to_string()
    } else {
        // Truncate content
        let lines_to_show = max_lines.saturating_sub(1);
        let lines_omitted = total_lines.saturating_sub(lines_to_show);

        let mut result = lines[..lines_to_show].join("\n");

        // Add omission marker
        let marker = if lines_omitted == 1 {
            "... (1 line omitted) ...".to_string()
        } else {
            format!("... ({} lines omitted) ...", lines_omitted)
        };

        result.push('\n');
        result.push_str(&marker);

        result
    }
}

/// Truncate content to show both beginning and end with an omission marker in the middle.
///
/// This is useful when you want to preserve both the start and end of content
/// while truncating the middle. Useful for showing file context around search results.
///
/// # Arguments
///
/// * `content` - The content to potentially truncate
/// * `max_lines` - Maximum total lines to include
/// * `head_lines` - Number of lines to show from the beginning
/// * `tail_lines` - Number of lines to show from the end
///
/// # Returns
///
/// The potentially truncated content with head and tail preserved.
///
/// # Examples
///
/// ```
/// use ragent_core::tool::truncate::truncate_content_head_tail;
///
/// let content = (1..=20).map(|n| format!("line{}", n)).collect::<Vec<_>>().join("\n");
/// let result = truncate_content_head_tail(&content, 10, 3, 3);
/// // Shows first 3 lines + omission marker + last 3 lines
/// ```
pub fn truncate_content_head_tail(
    content: impl AsRef<str>,
    max_lines: usize,
    head_lines: usize,
    tail_lines: usize,
) -> String {
    let content = content.as_ref();

    if max_lines == 0 || head_lines == 0 || tail_lines == 0 {
        return truncate_content(content, max_lines);
    }

    // Count lines
    let lines: Vec<&str> = content.lines().collect();
    let total_lines = lines.len();

    if total_lines <= max_lines {
        // Content fits within limit, return unchanged
        return content.to_string();
    }

    if head_lines + tail_lines >= max_lines {
        // Can't show both head and tail within max_lines, fall back to simple truncation
        return truncate_content(content, max_lines);
    }

    // Take head and tail
    let head = &lines[..head_lines];
    let tail = &lines[total_lines.saturating_sub(tail_lines)..];

    let lines_omitted = total_lines.saturating_sub(head_lines + tail_lines);

    let mut result = head.join("\n");
    result.push('\n');

    // Add omission marker
    let marker = if lines_omitted == 1 {
        "... (1 line omitted) ...".to_string()
    } else {
        format!("... ({} lines omitted) ...", lines_omitted)
    };
    result.push_str(&marker);
    result.push('\n');

    result.push_str(&tail.join("\n"));

    result
}

/// Get line count statistics for content.
///
/// Returns a tuple of (displayed_lines, total_lines, was_truncated).
///
/// # Arguments
///
/// * `content` - The content to analyze
/// * `max_lines` - The maximum lines that would be displayed
///
/// # Returns
///
/// A tuple containing:
/// - Number of lines that would be displayed
/// - Total number of lines in the content
/// - Whether truncation would occur
///
/// # Examples
///
/// ```
/// use ragent_core::tool::truncate::get_truncation_stats;
///
/// let (displayed, total, truncated) = get_truncation_stats("line1\nline2\nline3", 5);
/// assert_eq!(displayed, 3);
/// assert_eq!(total, 3);
/// assert!(!truncated);
///
/// let (displayed, total, truncated) = get_truncation_stats("a\nb\nc\nd\ne", 3);
/// assert_eq!(displayed, 3); // 2 shown + 1 marker line
/// assert_eq!(total, 5);
/// assert!(truncated);
/// ```
pub fn get_truncation_stats(content: impl AsRef<str>, max_lines: usize) -> (usize, usize, bool) {
    let content = content.as_ref();
    let total_lines = content.lines().count();

    if total_lines <= max_lines {
        (total_lines, total_lines, false)
    } else {
        let displayed = max_lines; // Account for the omission marker line
        (displayed, total_lines, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert!(result.contains("a"));
        assert!(result.contains("b"));
        // c, d, e should be omitted
        assert!(!result.contains("\nc\n"));
        assert!(!result.contains("\nd\n"));
        assert!(result.contains("... (3 lines omitted) ..."));
    }

    #[test]
    fn test_truncate_content_single_omission() {
        let content = "a\nb\nc\nd";
        let result = truncate_content(content, 3);

        assert!(result.contains("a"));
        assert!(result.contains("b"));
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
        assert!(result.contains("a"));
        assert!(result.contains("b"));
        assert!(result.contains("c"));
        assert!(result.contains("d"));
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
}
