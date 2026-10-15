//! String truncation utilities that respect UTF-8 character boundaries.
//!
//! Rust string slicing with byte indices (`&s[..n]`) panics when the index
//! falls inside a multi-byte UTF-8 character (e.g. an em dash `-` or en dash
//! `-`). This module provides safe, char-boundary-aware truncation helpers.

/// Returns `true` when `s` starts with an `http://` or `https://` scheme.
///
/// The single shared HTTP-scheme predicate; the config, telemetry, tool, and
/// UI layers all route their URL validation through it so a scheme check is
/// never re-spelled by hand (audit Medium 7). The check is case-sensitive to
/// match the way these URLs are actually built and persisted.
#[must_use]
pub fn is_http_url(s: &str) -> bool {
    s.starts_with("http://") || s.starts_with("https://")
}

/// Text width (columns) used when rendering HTML to plain text via the
/// `html2text` crate.
///
/// The single shared value for the masterfetch extractor, the TUI markdown
/// worker, and the agent reference resolver, so the rendered width cannot
/// drift between them (audit Low, html2text width magic number).
pub const HTML2TEXT_WIDTH: usize = 120;

/// The final path component of `path` as a `&str`, or `""` when it has no file
/// name or that name is not valid UTF-8.
///
/// The single shared form of the `path.file_name().and_then(|n| n.to_str())`
/// idiom (audit Complex 7); callers that need an owned value can `.to_owned()`
/// the result.
#[must_use]
pub fn path_file_name(path: &std::path::Path) -> &str {
    path.file_name().and_then(|n| n.to_str()).unwrap_or("")
}

/// Build an HTTP `Authorization: Bearer <key>` header value.
///
/// The single shared builder for the bearer header the LLM providers, the
/// masterfetch search backends, and the GitHub client all send, so the exact
/// header shape is defined once (audit Medium 6). Returns a [`String`] rather
/// than a `reqwest::header::HeaderValue` so this leaf crate need not depend on
/// `reqwest`; every call site feeds it to `.header("Authorization", ...)`,
/// which accepts a `String`.
#[must_use]
pub fn bearer_auth_header(key: &str) -> String {
    format!("Bearer {key}")
}

/// Truncate a string to at most `max_chars` Unicode scalar values, appending
/// an ellipsis (`...`) when the string was shortened.
///
/// # Examples
///
/// ```
/// use ragent_types::strutil::truncate_chars;
///
/// assert_eq!(truncate_chars("hello world", 20), "hello world");
/// assert_eq!(truncate_chars("hello world", 5), "he..."); // budget includes "..."
/// // Works with multi-byte characters (budget includes the 3-char ellipsis):
/// assert_eq!(truncate_chars("caf\u{e9} r\u{e9}sum\u{e9}", 5), "ca...");
/// ```
#[must_use]
pub fn truncate_chars(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        return s.to_string();
    }
    // Keep the result within `max_chars` including the 3-char ASCII ellipsis.
    let head = max_chars.saturating_sub(3);
    let truncated: String = s.chars().take(head).collect();
    format!("{truncated}...")
}

/// Format a byte count as a compact human-readable string.
///
/// Uses binary units (`KB`/`MB`) and one decimal place once the value exceeds
/// 1 KiB. Byte counts below 1 KiB are rendered verbatim as `"<n> B"`.
///
/// # Examples
///
/// ```
/// use ragent_types::strutil::format_size;
///
/// assert_eq!(format_size(0), "0 B");
/// assert_eq!(format_size(512), "512 B");
/// assert_eq!(format_size(1024), "1.0 KB");
/// assert_eq!(format_size(1536), "1.5 KB");
/// assert_eq!(format_size(1024 * 1024), "1.0 MB");
/// ```
#[must_use]
pub fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{bytes} B")
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    }
}

/// Truncate a string to at most `max_bytes` bytes, stepping back from the
/// cut point until it lands on a valid UTF-8 char boundary, then appending
/// an ellipsis (`...`) when the string was shortened.
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
/// assert_eq!(truncate_bytes("hello", 3), "hel...");
/// // "e" is 2 bytes; byte index 3 lands on a boundary:
/// assert_eq!(truncate_bytes("cafe", 3), "caf...");
/// ```
#[must_use]
pub fn truncate_bytes(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    format!("{}...", &s[..floor_char_boundary(s, max_bytes)])
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
/// // "e" is 2 bytes; byte index 3 lands on a boundary:
/// assert_eq!(truncate_bytes_no_ellipsis("cafe", 3), "caf");
/// ```
#[must_use]
pub fn truncate_bytes_no_ellipsis(s: &str, max_bytes: usize) -> String {
    if s.len() <= max_bytes {
        return s.to_string();
    }
    s[..floor_char_boundary(s, max_bytes)].to_string()
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
/// // "e" is 2 bytes (offsets 3-4); byte index 4 lands inside it and steps back.
/// assert_eq!(floor_char_boundary("caf\u{e9}", 4), 3);
/// // "em dash" is 3 bytes; byte index 3 lands inside it and steps to 1.
/// assert_eq!(floor_char_boundary("a\u{2014}b", 3), 1);
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

/// The first `max` characters of `s` (the whole string when it is shorter).
///
/// This is the shared "short id prefix" helper: display sites that want the
/// leading 8 characters of a task/session id use it instead of hand-rolling
/// `id.chars().take(8).collect()`.
///
/// # Examples
///
/// ```
/// use ragent_types::strutil::short_prefix;
///
/// assert_eq!(short_prefix("abcdefghij", 8), "abcdefgh");
/// assert_eq!(short_prefix("abc", 8), "abc");
/// ```
#[must_use]
pub fn short_prefix(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

/// The last `max` characters of `s` (the whole string when it is shorter).
///
/// This is the shared "short id suffix" helper: the TUI renders a session/task
/// id by its unique trailing suffix. Falls back to the whole string when the
/// cut would land mid-UTF-8-codepoint, so it never panics.
///
/// # Examples
///
/// ```
/// use ragent_types::strutil::short_suffix;
///
/// assert_eq!(short_suffix("abcdefghij", 8), "cdefghij");
/// assert_eq!(short_suffix("abc", 8), "abc");
/// ```
#[must_use]
pub fn short_suffix(s: &str, max: usize) -> String {
    let count = s.chars().count();
    if count <= max {
        return s.to_string();
    }
    s.chars().skip(count - max).collect()
}

/// Format a duration in seconds as a compact `"2m34s"` / `"1h5m"` string.
///
/// Under a minute renders as `"<n>s"`, under an hour as `"<m>m<s>s"`, and above
/// an hour as `"<h>h<m>m"` (seconds dropped). Shared by the TUI status surfaces
/// that previously each had their own copy.
///
/// # Examples
///
/// ```
/// use ragent_types::strutil::format_duration_compact;
///
/// assert_eq!(format_duration_compact(34), "34s");
/// assert_eq!(format_duration_compact(154), "2m34s");
/// assert_eq!(format_duration_compact(3900), "1h5m");
/// ```
#[must_use]
pub fn format_duration_compact(secs: i64) -> String {
    let secs = secs.max(0);
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Format a duration in seconds as `"1h 2m 3s"` (units with spaces, zero-value
/// leading units dropped).
///
/// # Examples
///
/// ```
/// use ragent_types::strutil::format_duration_hms;
///
/// assert_eq!(format_duration_hms(5), "5s");
/// assert_eq!(format_duration_hms(154), "2m 34s");
/// assert_eq!(format_duration_hms(3900), "1h 5m 0s");
/// ```
#[must_use]
pub fn format_duration_hms(secs: i64) -> String {
    let secs = secs.max(0);
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    if hours > 0 {
        format!("{hours}h {minutes}m {seconds}s")
    } else if minutes > 0 {
        format!("{minutes}m {seconds}s")
    } else {
        format!("{seconds}s")
    }
}

/// Format a duration in seconds as `"M:SS"`.
///
/// # Examples
///
/// ```
/// use ragent_types::strutil::format_mmss;
///
/// assert_eq!(format_mmss(5), "0:05");
/// assert_eq!(format_mmss(154), "2:34");
/// ```
#[must_use]
pub fn format_mmss(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}
