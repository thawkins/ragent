//! String truncation utilities that respect UTF-8 character boundaries.
//!
//! Rust string slicing with byte indices (`&s[..n]`) panics when the index
//! falls inside a multi-byte UTF-8 character (e.g. an em dash `—` or en dash
//! `–`). This module provides safe, char-boundary-aware truncation helpers.

/// Truncate a string to at most `max_chars` Unicode scalar values, appending
/// an ellipsis (`…`) when the string was shortened.
///
/// # Examples
///
/// ```
/// use ragent_types::strutil::truncate_chars;
///
/// assert_eq!(truncate_chars("hello world", 20), "hello world");
/// assert_eq!(truncate_chars("hello world", 5), "hello…");
/// // Works with multi-byte characters:
/// assert_eq!(truncate_chars("café résumé", 5), "café …");
/// ```
#[must_use]
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    let truncated: String = s.chars().take(max_chars).collect();
    format!("{truncated}…")
}

/// Truncate a string to at most `max_bytes` bytes, stepping back from the
/// cut point until it lands on a valid UTF-8 char boundary, then appending
/// an ellipsis (`…`) when the string was shortened.
///
/// Prefer [`truncate_chars`] when the limit is expressed in visible characters
/// rather than bytes.
///
/// # Examples
///
/// ```
/// use ragent_types::strutil::truncate_bytes;
///
/// assert_eq!(truncate_bytes("hello", 10), "hello");
/// assert_eq!(truncate_bytes("hello", 3), "hel…");
/// // "é" is 2 bytes; byte index 3 lands on a boundary:
/// assert_eq!(truncate_bytes("café", 3), "caf…");
/// ```
#[must_use]
pub fn truncate_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    format!("{}…", &s[..end])
}
