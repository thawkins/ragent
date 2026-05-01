//! Content formatting utilities for consistent tool output presentation.
//!
//! This module provides helpers for formatting tool output content according
//! to the standardized patterns defined in the tool output consistency plan.
//!
//! # Format Patterns
//!
//! - **Pattern A (Summary + Content)**: Summary line + blank line + raw content
//! - **Pattern B (Summary Only)**: Just a summary line with operation result
//! - **Pattern C (Structured)**: Structured header with exit code, duration, etc.
//!
//! # Examples
//!
//! ```
//! use ragent_core::tool::format::{format_summary_content, format_status_output};
//!
//! // Pattern A: Summary + Content
//! let output = format_summary_content("5 matches found", "line1\nline2");
//!
//! // Pattern C: Structured output
//! let output = format_status_output(0, "Hello", "", 150, false);
//! ```

/// Format content with a summary line followed by the raw content.
///
/// This follows **Pattern A** from the content format standard.
///
/// # Arguments
///
/// * `summary` - A brief summary of the result (e.g., "42 lines read")
/// * `content` - The raw content to display
///
/// # Returns
///
/// A formatted string with the summary, a blank line, and the content.
pub fn format_summary_content(summary: impl AsRef<str>, content: impl AsRef<str>) -> String {
    let summary = summary.as_ref();
    let content = content.as_ref();

    if content.is_empty() {
        summary.to_string()
    } else {
        format!("{}\n\n{}", summary, content)
    }
}

/// Format a structured output with exit code, stdout, stderr, and timing info.
///
/// This follows **Pattern C** from the content format standard,
/// used by execution tools like bash.
///
/// # Arguments
///
/// * `exit_code` - The process exit code (0 for success)
/// * `stdout` - Standard output content
/// * `stderr` - Standard error content
/// * `duration_ms` - Execution duration in milliseconds
/// * `timed_out` - Whether the execution timed out
///
/// # Returns
///
/// A formatted string with structured header and output sections.
pub fn format_status_output(
    exit_code: i32,
    stdout: impl AsRef<str>,
    stderr: impl AsRef<str>,
    duration_ms: u64,
    timed_out: bool,
) -> String {
    let stdout = stdout.as_ref();
    let stderr = stderr.as_ref();

    let mut result = String::new();

    // Header with exit code and timing
    result.push_str(&format!(
        "Exit code: {}\nDuration: {}ms",
        exit_code, duration_ms
    ));
    if timed_out {
        result.push_str(" (timed out)");
    }
    result.push('\n');

    // Stdout section
    if !stdout.is_empty() {
        result.push_str("\nSTDOUT:\n");
        result.push_str(stdout);
    }

    // Stderr section
    if !stderr.is_empty() {
        result.push_str("\nSTDERR:\n");
        result.push_str(stderr);
    }

    result
}

/// Format a simple summary-only result.
///
/// This follows **Pattern B** from the content format standard,
/// used by file modification tools like write, edit, create.
///
/// # Arguments
///
/// * `description` - A description of what was done (e.g., "Wrote 42 lines to file.txt")
///
/// # Returns
///
/// The description string unchanged.
pub fn format_simple_summary(description: impl AsRef<str>) -> String {
    description.as_ref().to_string()
}

/// Format bytes into a human-readable string.
///
/// Converts byte counts into KB, MB, GB representations for display.
///
/// # Arguments
///
/// * `bytes` - The number of bytes
///
/// # Returns
///
/// A human-readable string like "1.5 KB" or "2.3 MB".
///
/// # Examples
///
/// ```
/// use ragent_core::tool::format::format_bytes;
///
/// assert_eq!(format_bytes(512), "512 B");
/// assert_eq!(format_bytes(1536), "1.5 KB");
/// assert_eq!(format_bytes(1572864), "1.5 MB");
/// ```
pub fn format_bytes(bytes: usize) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = 1024.0 * KB;
    const GB: f64 = 1024.0 * MB;

    let bytes_f = bytes as f64;

    if bytes_f >= GB {
        format!("{:.1} GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.1} MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.1} KB", bytes_f / KB)
    } else {
        format!("{} B", bytes)
    }
}

/// Format a line count with proper pluralization.
///
/// # Arguments
///
/// * `count` - The number of lines
///
/// # Returns
///
/// A string like "1 line" or "42 lines".
///
/// # Examples
///
/// ```
/// use ragent_core::tool::format::format_line_count;
///
/// assert_eq!(format_line_count(1), "1 line");
/// assert_eq!(format_line_count(42), "42 lines");
/// ```
pub fn format_line_count(count: usize) -> String {
    if count == 1 {
        "1 line".to_string()
    } else {
        format!("{} lines", count)
    }
}

/// Format a file count with proper pluralization.
///
/// # Arguments
///
/// * `count` - The number of files
///
/// # Returns
///
/// A string like "1 file" or "42 files".
pub fn format_file_count(count: usize) -> String {
    if count == 1 {
        "1 file".to_string()
    } else {
        format!("{} files", count)
    }
}

/// Format a match count with proper pluralization.
///
/// # Arguments
///
/// * `count` - The number of matches
///
/// # Returns
///
/// A string like "1 match" or "42 matches".
pub fn format_match_count(count: usize) -> String {
    if count == 1 {
        "1 match".to_string()
    } else {
        format!("{} matches", count)
    }
}

/// Format an entry count with proper pluralization.
///
/// # Arguments
///
/// * `count` - The number of entries
///
/// # Returns
///
/// A string like "1 entry" or "42 entries".
pub fn format_entry_count(count: usize) -> String {
    if count == 1 {
        "1 entry".to_string()
    } else {
        format!("{} entries", count)
    }
}

/// Format an edit summary showing old and new line counts.
///
/// # Arguments
///
/// * `old_lines` - Number of lines before the edit
/// * `new_lines` - Number of lines after the edit
///
/// # Returns
///
/// A descriptive string like "replaced 5 lines with 3 lines".
pub fn format_edit_summary(old_lines: usize, new_lines: usize) -> String {
    if old_lines == 1 && new_lines == 1 {
        "replaced 1 line".to_string()
    } else if old_lines == 1 {
        format!("replaced 1 line with {} lines", new_lines)
    } else if new_lines == 1 {
        format!("replaced {} lines with 1 line", old_lines)
    } else {
        format!("replaced {} lines with {} lines", old_lines, new_lines)
    }
}

/// Format a path for display, making it relative if under the working directory.
///
/// # Arguments
///
/// * `path` - The absolute or relative path
/// * `working_dir` - The working directory to make the path relative to
///
/// # Returns
///
/// A string representation of the path, relative if possible.
pub fn format_display_path(
    path: impl AsRef<std::path::Path>,
    working_dir: impl AsRef<std::path::Path>,
) -> String {
    let path = path.as_ref();
    let working_dir = working_dir.as_ref();

    path.strip_prefix(working_dir)
        .unwrap_or(path)
        .to_string_lossy()
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_summary_content() {
        let result = format_summary_content("5 matches found", "line1\nline2");
        assert_eq!(result, "5 matches found\n\nline1\nline2");

        // Empty content returns just the summary
        let result = format_summary_content("No results", "");
        assert_eq!(result, "No results");
    }

    #[test]
    fn test_format_status_output() {
        let result = format_status_output(0, "Hello\nWorld", "", 150, false);
        assert!(result.contains("Exit code: 0"));
        assert!(result.contains("Duration: 150ms"));
        assert!(result.contains("STDOUT:"));
        assert!(result.contains("Hello\nWorld"));

        // With stderr
        let result = format_status_output(1, "", "Error occurred", 200, false);
        assert!(result.contains("Exit code: 1"));
        assert!(result.contains("STDERR:"));
        assert!(result.contains("Error occurred"));

        // With timeout
        let result = format_status_output(124, "", "", 30000, true);
        assert!(result.contains("(timed out)"));
    }

    #[test]
    fn test_format_simple_summary() {
        let result = format_simple_summary("Wrote 42 lines to file.txt");
        assert_eq!(result, "Wrote 42 lines to file.txt");
    }

    #[test]
    fn test_format_bytes() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(1024), "1.0 KB");
        assert_eq!(format_bytes(1536), "1.5 KB");
        assert_eq!(format_bytes(1024 * 1024), "1.0 MB");
        assert_eq!(format_bytes((1024 * 1024 * 3) / 2), "1.5 MB");
        assert_eq!(format_bytes(1024 * 1024 * 1024), "1.0 GB");
    }

    #[test]
    fn test_format_line_count() {
        assert_eq!(format_line_count(1), "1 line");
        assert_eq!(format_line_count(0), "0 lines");
        assert_eq!(format_line_count(42), "42 lines");
    }

    #[test]
    fn test_format_file_count() {
        assert_eq!(format_file_count(1), "1 file");
        assert_eq!(format_file_count(0), "0 files");
        assert_eq!(format_file_count(5), "5 files");
    }

    #[test]
    fn test_format_match_count() {
        assert_eq!(format_match_count(1), "1 match");
        assert_eq!(format_match_count(0), "0 matches");
        assert_eq!(format_match_count(10), "10 matches");
    }

    #[test]
    fn test_format_entry_count() {
        assert_eq!(format_entry_count(1), "1 entry");
        assert_eq!(format_entry_count(0), "0 entries");
        assert_eq!(format_entry_count(3), "3 entries");
    }

    #[test]
    fn test_format_edit_summary() {
        assert_eq!(format_edit_summary(1, 1), "replaced 1 line");
        assert_eq!(format_edit_summary(1, 5), "replaced 1 line with 5 lines");
        assert_eq!(format_edit_summary(5, 1), "replaced 5 lines with 1 line");
        assert_eq!(format_edit_summary(5, 3), "replaced 5 lines with 3 lines");
    }

    #[test]
    fn test_format_display_path() {
        use std::path::Path;

        // Path under working dir becomes relative
        let result = format_display_path(
            Path::new("/home/user/project/src/main.rs"),
            Path::new("/home/user/project"),
        );
        assert_eq!(result, "src/main.rs");

        // Path outside working dir stays absolute
        let result = format_display_path(Path::new("/etc/config"), Path::new("/home/user/project"));
        assert_eq!(result, "/etc/config");

        // Already relative path
        let result = format_display_path(Path::new("src/main.rs"), Path::new("/home/user/project"));
        assert_eq!(result, "src/main.rs");
    }
}
