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

/// Truncate a string to at most `max_bytes` bytes, stepping back from the cut
/// point until it lands on a valid UTF-8 char boundary, but do **not** append
/// an ellipsis or any other suffix.
///
/// Returns `s` unchanged when it is already within the byte budget. Use this
/// when the caller wants to attach its own truncation notice.
///
/// # Examples
///
/// ```
/// use ragent_types::strutil::truncate_bytes_no_ellipsis;
///
/// assert_eq!(truncate_bytes_no_ellipsis("hello", 10), "hello");
/// assert_eq!(truncate_bytes_no_ellipsis("hello", 3), "hel");
/// // "é" is 2 bytes; byte index 3 lands on a boundary:
/// assert_eq!(truncate_bytes_no_ellipsis("café", 3), "caf");
/// ```
#[must_use]
pub fn truncate_bytes_no_ellipsis(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    let mut end = max_bytes;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// Round `index` down to the nearest UTF-8 char boundary in `s` (clamped to
/// `s.len()`).
///
/// Byte offsets derived from arithmetic (a reserved-suffix budget) or from a
/// byte-length limit are not guaranteed to sit between characters; slicing a
/// string at such an offset panics. Use this to make `&s[..index]` safe
/// (FUNC-003).
///
/// # Examples
///
/// ```
/// use ragent_types::strutil::floor_char_boundary;
///
/// // "é" is 2 bytes (offsets 3-4); byte index 4 lands inside it and steps back.
/// assert_eq!(floor_char_boundary("café", 4), 3);
/// assert_eq!(floor_char_boundary("a—b", 3), 1);
/// assert_eq!(floor_char_boundary("abc", 99), 3);
/// ```
#[must_use]
pub fn floor_char_boundary(s: &str, index: usize) -> usize {
    let mut end = index.min(s.len());
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    end
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floor_char_boundary_snaps_to_character_start() {
        // "é" is 2 bytes (offsets 3 and 4): index 3 is the char start, index 4
        // is inside the character and steps back to 3. Index 5 is the end.
        assert_eq!(floor_char_boundary("café", 3), 3);
        assert_eq!(floor_char_boundary("café", 4), 3);
        assert_eq!(floor_char_boundary("café", 5), 5);
        // "—" (em dash) is 3 bytes starting at offset 1: any cut inside steps to 1.
        assert_eq!(floor_char_boundary("a—b", 1), 1);
        assert_eq!(floor_char_boundary("a—b", 2), 1);
        assert_eq!(floor_char_boundary("a—b", 3), 1);
        assert_eq!(floor_char_boundary("a—b", 4), 4);
        // Out-of-range clamps to the length.
        assert_eq!(floor_char_boundary("abc", 99), 3);
        assert_eq!(floor_char_boundary("", 5), 0);
    }

    #[test]
    fn truncate_bytes_no_ellipsis_keeps_short_strings() {
        assert_eq!(truncate_bytes_no_ellipsis("hello", 10), "hello");
    }

    #[test]
    fn truncate_bytes_no_ellipsis_truncates_at_char_boundary() {
        // "é" is 2 bytes; byte index 3 lands between 'f' and 'é'.
        assert_eq!(truncate_bytes_no_ellipsis("café", 3), "caf");
        // "—" (em dash) is 3 bytes; "a—" is exactly 4 bytes and is a valid cut.
        assert_eq!(truncate_bytes_no_ellipsis("a—b", 4), "a—");
        // Cutting inside the em dash should step back to the start of the dash.
        assert_eq!(truncate_bytes_no_ellipsis("a—b", 3), "a");
    }

    #[test]
    fn truncate_bytes_no_ellipsis_empty_when_zero() {
        assert_eq!(truncate_bytes_no_ellipsis("hello", 0), "");
    }
}
