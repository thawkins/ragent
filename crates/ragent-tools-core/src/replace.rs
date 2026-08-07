//! Shared replacement matchers.
//!
//! [`find_exact_replacement_range`] is the canonical matcher used by every
//! replace-style tool in ragent (`edit`, `multi_edit`, `apply_patch`;
//! historically also the legacy `memory_replace`). It locates a unique byte
//! range `[start, end)` in `content` where `needle` should be replaced.
//!
//! # Matching semantics
//!
//! Matching is **strict exact-byte** by default: the needle must occur exactly
//! once, byte-for-byte. There is no CRLF tolerance, no trailing/leading
//! whitespace tolerance, no indentation re-application, and no blank-line or
//! final-newline normalisation. What you read (bytes) is what you match.
//!
//! # Opt-in flexible matching
//!
//! [`find_flexible_replacement_range`] provides the opt-in whitespace-collapse
//! mode used when the caller passes `"collapse_whitespace": true` to `edit` /
//! `multi_edit`. In that mode backslash escapes (`\t`, `\n`, `\r`, `\\`) in
//! the needle are decoded, and every run of whitespace in the needle matches a
//! non-empty run of whitespace in the content, so collapsed whitespace
//! differences (indentation depth, alignment spaces, blank lines) do not cause
//! spurious match failures.

/// Error returned by [`find_exact_replacement_range`] when no unique match is
/// found.
#[derive(Debug)]
pub enum FindError {
    /// The needle does not occur anywhere in the content.
    NotFound,
    /// The needle occurs at more than one location; carries the match count.
    MultipleMatches(usize),
}

/// Find the unique byte range `[start, end)` in `content` where `needle` should
/// be replaced using **only** exact substring matching.
///
/// Whitespace, indentation, and line endings must match exactly.
///
/// Returns `(start, end, new_str)` on success. `new_str` is returned unchanged
/// because exact matching never needs indentation re-application.
///
/// # Errors
///
/// - [`FindError::NotFound`] if `needle` does not occur in `content`.
/// - [`FindError::MultipleMatches(n)`] if `needle` occurs more than once.
pub fn find_exact_replacement_range(
    content: &str,
    needle: &str,
    new_str: &str,
) -> Result<(usize, usize, String), FindError> {
    let count = content.matches(needle).count();
    if count == 0 {
        return Err(FindError::NotFound);
    }
    if count > 1 {
        return Err(FindError::MultipleMatches(count));
    }
    let start = content.find(needle).unwrap();
    Ok((start, start + needle.len(), new_str.to_string()))
}

/// Decode common backslash escape sequences in a needle.
///
/// Supported escapes: `\t` (tab), `\n` (line feed), `\r` (carriage return),
/// and `\\` (literal backslash). A backslash followed by any other character
/// is kept verbatim (both the backslash and the character) so needles such as
/// Windows paths (`C:\new`) or regex fragments are not mangled.
#[must_use]
pub fn decode_escapes(raw: &str) -> String {
    if !raw.contains('\\') {
        return raw.to_string();
    }
    let mut out = String::with_capacity(raw.len());
    let mut chars = raw.chars();
    while let Some(c) = chars.next() {
        if c == '\\' {
            match chars.next() {
                Some('t') => out.push('\t'),
                Some('n') => out.push('\n'),
                Some('r') => out.push('\r'),
                Some('\\') => out.push('\\'),
                Some(other) => {
                    out.push('\\');
                    out.push(other);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}

/// Find the unique byte range `[start, end)` in `content` where `needle`
/// should be replaced using **whitespace-tolerant** matching.
///
/// Matching runs in two lanes:
///
/// 1. **Exact lane** — if the (escape-decoded) needle occurs exactly once
///    byte-for-byte, that match wins and behaviour is identical to
///    [`find_exact_replacement_range`]. If it occurs zero times, the general
///    lane runs; if it occurs more than once, the general lane must agree on
///    the same occurrence or the edit is rejected as ambiguous.
/// 2. **General lane** — whitespace-flexible scan. Every run of whitespace in
///    the decoded needle matches a non-empty run of whitespace in the content
///    (spaces, tabs, newlines, CRs, form feeds, vertical tabs collapsed), and
///    non-whitespace characters must match exactly.
///
/// Returns `(start, end, new_str)` on success, where `[start, end)` is the
/// byte range **in the original content** that matched — its length may differ
/// from the needle's length.
///
/// # Errors
///
/// - [`FindError::NotFound`] if the needle cannot be found under either lane.
/// - [`FindError::MultipleMatches(n)`] if the match is ambiguous.
pub fn find_flexible_replacement_range(
    content: &str,
    needle: &str,
    new_str: &str,
) -> Result<(usize, usize, String), FindError> {
    let decoded = decode_escapes(needle);

    // ── Lane 1: exact substring count ────────────────────────────────────
    let exact_count = content.matches(decoded.as_str()).count();
    if exact_count == 1 {
        let start = content.find(decoded.as_str()).unwrap();
        return Ok((start, start + decoded.len(), new_str.to_string()));
    }

    // ── Lane 2: whitespace-flexible general scan ──────────────────────────
    let hay: Vec<char> = content.chars().collect();
    let pat: Vec<char> = decoded.chars().collect();
    let byte_offsets: Vec<usize> = content.char_indices().map(|(i, _)| i).collect();

    // Fold consecutive whitespace runs in the pattern down to a single space
    // marker and record the folded positions that represent whitespace runs.
    let mut pat_folded: Vec<char> = Vec::with_capacity(pat.len());
    let mut run_positions: Vec<usize> = Vec::new();
    {
        let mut idx = 0;
        while idx < pat.len() {
            if pat[idx].is_whitespace() {
                run_positions.push(pat_folded.len());
                pat_folded.push(' ');
                while idx < pat.len() && pat[idx].is_whitespace() {
                    idx += 1;
                }
            } else {
                pat_folded.push(pat[idx]);
                idx += 1;
            }
        }
    }

    let mut matches: Vec<(usize, usize)> = Vec::new();
    if !pat_folded.is_empty() {
        'anchors: for si in 0..hay.len() {
            if !hay[si].is_whitespace() && hay[si] != pat_folded[0] {
                continue; // anchored literals cannot start on whitespace
            }
            let mut h = si;
            let mut p = 0;
            let mut ok = true;
            while p < pat_folded.len() {
                if pat_folded[p] == ' ' && run_positions.contains(&p) {
                    // A folded whitespace run must consume ≥1 whitespace chars.
                    let start_h = h;
                    while h < hay.len() && hay[h].is_whitespace() {
                        h += 1;
                    }
                    if h == start_h {
                        ok = false;
                        break;
                    }
                    p += 1;
                } else {
                    if h >= hay.len() || hay[h] != pat_folded[p] {
                        ok = false;
                        break;
                    }
                    h += 1;
                    p += 1;
                }
            }
            if ok {
                matches.push((si, h));
                // Deterministic fail-fast: 3 distinct hits can never collapse
                // into a unique match.
                if matches.len() > 2 {
                    break 'anchors;
                }
            }
        }
    }

    match matches.len() {
        0 => Err(FindError::NotFound),
        1 => {
            let (si, hi) = matches[0];
            if exact_count > 1 {
                // The flexible hit must agree with one of the exact hits,
                // otherwise the needle is genuinely ambiguous.
                let s = byte_offsets[si];
                let e = if hi >= hay.len() {
                    content.len()
                } else {
                    byte_offsets[hi]
                };
                if !content
                    .match_indices(decoded.as_str())
                    .any(|(pos, _)| pos == s && pos + decoded.len() == e)
                {
                    return Err(FindError::MultipleMatches(exact_count));
                }
            }
            let start = byte_offsets[si];
            let end = if hi >= hay.len() {
                content.len()
            } else {
                byte_offsets[hi]
            };
            Ok((start, end, new_str.to_string()))
        }
        _ => Err(FindError::MultipleMatches(matches.len())),
    }
}

/// Diagnostic kind carried by [`FindDiag`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindDiagKind {
    /// The needle does not occur anywhere in the content.
    NotFound,
    /// The needle occurs at more than one location; carries the match count.
    MultipleMatches(usize),
}

/// A richer replacement-failure diagnostic used by `edit` and `multi_edit` to
/// produce actionable error messages.
#[derive(Debug, Clone)]
pub struct FindDiag {
    /// What kind of failure occurred.
    pub kind: FindDiagKind,
}

impl FindDiag {
    /// Build a `NotFound` diagnostic.
    #[must_use]
    pub const fn not_found() -> Self {
        Self {
            kind: FindDiagKind::NotFound,
        }
    }

    /// Build a `MultipleMatches` diagnostic.
    #[must_use]
    pub const fn multiple(count: usize) -> Self {
        Self {
            kind: FindDiagKind::MultipleMatches(count),
        }
    }
}

impl From<FindDiag> for FindError {
    fn from(d: FindDiag) -> Self {
        match d.kind {
            FindDiagKind::NotFound => Self::NotFound,
            FindDiagKind::MultipleMatches(n) => Self::MultipleMatches(n),
        }
    }
}

/// Format a [`FindDiag`] into an actionable error message.
///
/// The message names the file path, explains whether the needle was not found
/// or matched multiple times, and reminds the caller that a byte-for-byte
/// match is required (re-read the file to obtain exact bytes).
#[must_use]
pub fn format_match_failure(diag: &FindDiag, path: &std::path::Path) -> String {
    match diag.kind {
        FindDiagKind::NotFound => format!(
            "old_string not found in {}. Matching is byte-for-byte exact: \
             indentation, whitespace, and line endings must match precisely. \
             Re-read the file and include 3–5 lines of context around the \
             change point.",
            path.display(),
        ),
        FindDiagKind::MultipleMatches(n) => format!(
            "old_string found {} times in {}. It must match exactly once. \
             Add more surrounding context to make the match unique.",
            n,
            path.display(),
        ),
    }
}
