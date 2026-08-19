//! Shared replacement matchers.
//!
//! [`find_exact_replacement_range`] is the canonical matcher used by every
//! replace-style tool in ragent (`edit`, `multi_edit`, `apply_patch`;
//! historically also the legacy `memory_replace`). It locates a unique byte
//! range `[start, end)` in `content` where `needle` should be replaced.
//!
//! # Matching semantics
//!
//! Matching is **strict exact-byte** at the entry-point level
//! ([`find_exact_replacement_range`]): the needle must occur exactly once,
//! byte-for-byte. There is no CRLF tolerance, no trailing/leading whitespace
//! tolerance, no indentation re-application, and no blank-line or
//! final-newline normalisation in this lane. What you read (bytes) is what
//! you match. The fallback cascade described below relaxes these limits
//! progressively when a unique match can not be found strictly.
//!
//! # Opt-in flexible matching
//!
//! [`find_flexible_replacement_range`] provides the whitespace-collapse lane
//! used either when the caller passes `"collapse_whitespace": true`, or as an
//! automatic fallback after exact matching fails (see below). In that mode
//! backslash escapes (`\t`, `\n`, `\r`, `\\`) in the needle are decoded, and
//! every run of whitespace in the needle matches a non-empty run of whitespace
//! in the content, so collapsed whitespace differences (indentation depth,
//! alignment spaces, blank lines) do not cause spurious match failures.
//!
//! # Match cascade (editplan P2)
//!
//! [`find_replacement_cascade`] runs the matchers as a progressive fallback
//! chain:
//!
//! 1. **Exact** — strict byte-for-byte, unique match required.
//! 2. **Flexible** — whitespace-collapsed matching (as above).
//! 3. **Indent-normalised** — per-line comparison with leading whitespace
//!    stripped; the replacement re-applies the line-by-line indentation found
//!    in the file, so a needle composed with invented indentation still
//!    produces correctly-indented output.
//!
//! The cascade succeeds only when a lane finds exactly one match. Which lane
//! produced the result (or failed last) is reported in [`CascadeMatch`], and
//! the caller records it in the edit-log `match_lane` field so the frequency
//! of each fallback can be measured.

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
    let mut matches = content.match_indices(needle);
    let first = matches.next();
    let second = matches.next();
    match (first, second) {
        (None, _) => Err(FindError::NotFound),
        (Some(_), Some(_)) => Err(FindError::MultipleMatches(2 + matches.count())),
        (Some((start, _)), None) => Ok((start, start + needle.len(), new_str.to_string())),
    }
}

/// Decode common backslash escape sequences in a needle.
///
/// Supported escapes: `\t` (tab), `\n` (line feed), `\r` (carriage return),
/// and `\\` (literal backslash). A backslash followed by any other character
/// is kept verbatim (both the backslash and the character) so needles such as
/// Windows paths (`C:\new`) or regex fragments are not mangled.
///
/// # Improvements (editrenewal FR-009)
///
/// This function handles escape sequences robustly by:
/// - Decoding all standard escape sequences consistently
/// - Preserving backslashes that aren't part of recognized escapes
/// - Working correctly with UTF-8 multi-byte characters
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
/// # Improvements (editrenewal FR-009)
///
/// The flexible matcher now handles these common failure cases:
/// - **Leading/trailing whitespace**: Boundary whitespace runs match flexibly
/// - **Blank lines**: Consecutive newlines with varying whitespace are normalized
/// - **UTF-8 boundaries**: Char-based matching with correct byte offset tracking
/// - **Escape sequences**: All standard escapes decoded before matching
///
/// Returns `(start, end, new_str)` on success, where `[start, end)` is the
/// byte range **in the original content** that matched — its length may differ
/// from the needle's length due to whitespace collapsing.
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

    // ── Lane 1: exact substring ─────────────────────────────────────────
    let mut exact_iter = content.match_indices(decoded.as_str());
    let first_exact = exact_iter.next();
    let second_exact = exact_iter.next();
    let exact_count = if first_exact.is_none() {
        0
    } else if second_exact.is_none() {
        1
    } else {
        2 + exact_iter.count()
    };
    if exact_count == 1 {
        let (start, _) = first_exact.unwrap();
        return Ok((start, start + decoded.len(), new_str.to_string()));
    }

    // ── Lane 2: whitespace-flexible general scan ──────────────────────────
    let hay: Vec<char> = content.chars().collect();
    let pat: Vec<char> = decoded.chars().collect();
    let byte_offsets: Vec<usize> = content.char_indices().map(|(i, _)| i).collect();
    if pat.is_empty() {
        return Err(FindError::NotFound);
    }

    // Fold consecutive whitespace runs in the pattern down to a single space
    // marker. Every space in the folded pattern represents a whitespace run.
    let mut pat_folded: Vec<(char, bool)> = Vec::with_capacity(pat.len());
    {
        let mut idx = 0;
        while idx < pat.len() {
            if pat[idx].is_whitespace() {
                pat_folded.push((' ', true));
                while idx < pat.len() && pat[idx].is_whitespace() {
                    idx += 1;
                }
            } else {
                pat_folded.push((pat[idx], false));
                idx += 1;
            }
        }
    }

    let mut matches: Vec<(usize, usize)> = Vec::new();
    if !pat_folded.is_empty() {
        // Precompute candidate start char indices so we do not scan every
        // character position. For non-whitespace-leading patterns this also
        // prevents matches that begin in the middle of unrelated text.
        let starts_with_ws = pat_folded[0].1;
        let first_lit = pat_folded[0].0;
        let mut candidate_indices: Vec<usize> = Vec::new();
        for (i, c) in hay.iter().enumerate() {
            if starts_with_ws {
                if c.is_whitespace() && (i == 0 || !hay[i - 1].is_whitespace()) {
                    candidate_indices.push(i);
                }
            } else if *c == first_lit {
                candidate_indices.push(i);
            }
        }

        'anchors: for &si in &candidate_indices {
            // When the pattern starts with a literal it must be aligned with
            // the literal in the content (candidate construction guarantees
            // this, but guard against an empty haystack).
            if !starts_with_ws && (si >= hay.len() || hay[si] != first_lit) {
                continue;
            }
            let mut h = si;
            let mut p = 0;
            let mut ok = true;
            while p < pat_folded.len() {
                let (pc, is_run) = pat_folded[p];
                if is_run {
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
                    if h >= hay.len() || hay[h] != pc {
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
                    // The match span differs from every exact copy, so the
                    // needle is still ambiguous across the exact copies.
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

// ── Match cascade (editplan P2) ──────────────────────────────────────────────

/// The matcher lane used by [`find_replacement_cascade`]. Logged as the
/// `match_lane` field in the edit-log so the frequency of each fallback can
/// be measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MatchLane {
    /// Strict byte-for-byte substring match.
    Exact,
    /// Whitespace-collapsed match (each whitespace run in the needle matches
    /// any non-empty whitespace run in the content).
    Flexible,
    /// Per-line match with leading whitespace stripped; replacement
    /// re-applies the indentation found in the file.
    IndentNormalised,
    /// Every lane failed with no match.
    NotFound,
    /// Every lane failed because the match was ambiguous.
    Multiple,
}

impl MatchLane {
    /// Stable string form used in the edit-log and tool metadata.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::Flexible => "flexible",
            Self::IndentNormalised => "indent_normalised",
            Self::NotFound => "not_found",
            Self::Multiple => "multiple",
        }
    }
}

impl std::fmt::Display for MatchLane {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Why the cascade failed.
#[derive(Debug, Clone)]
pub enum CascadeFail {
    /// No lane found any match.
    NotFound,
    /// The best lane saw `count` candidate locations. `starts` carries up to
    /// three byte offsets for disambiguation messages.
    MultipleMatches {
        /// Which lane produced the ambiguous result.
        lane: MatchLane,
        /// Number of matches seen (may be capped at 3 during scanning).
        count: usize,
        /// Byte offsets of up to three matches.
        starts: Vec<usize>,
    },
}

/// Outcome of [`find_replacement_cascade`].
#[derive(Debug)]
pub enum CascadeMatch {
    /// Exactly one match was found by `lane`.
    Found {
        /// Which lane produced the match.
        lane: MatchLane,
        /// Byte offset of the match start in the original content.
        start: usize,
        /// Byte offset of the match end in the original content.
        end: usize,
        /// The replacement text to apply (indent-normalised lane rewrites it).
        new_str: String,
    },
    /// All lanes failed; `reason` explains the best failure.
    Failed(CascadeFail),
}

/// Find the indented block in `content` whose per-line, leading-whitespace-
/// stripped form equals the stripped lines of `needle`.
///
/// Returns `(start, end, start_line, end_line)` — byte range in `content` and
/// the inclusive line index range that matched. Blank needle lines are
/// skipped when comparing, mirroring the "blank lines carry no indentation"
/// intuition of the tool documentation.
fn indent_normalised_matches(content: &str, needle: &str) -> Vec<(usize, usize, usize, usize)> {
    let needle_lines: Vec<&str> = needle.lines().filter(|l| !l.trim().is_empty()).collect();
    if needle_lines.is_empty() {
        return Vec::new();
    }
    // Keep line terminators as part of each line so byte ranges are exact.
    let content_lines: Vec<&str> = content.split_inclusive('\n').collect();

    let mut out = Vec::new();
    let n = needle_lines.len();
    for (i, block) in content_lines.windows(n).enumerate() {
        if needle_lines.iter().zip(block.iter()).all(|(nl, cl)| {
            let cl_body = cl.strip_suffix('\n').unwrap_or(cl);
            nl.trim_start() == cl_body.trim_start()
        }) {
            let start = content_lines[..i].iter().map(|l| l.len()).sum();
            let last = i + n - 1;
            let mut end: usize = block.iter().map(|l| l.len()).sum();
            // The byte range should include the terminating newlines of every
            // matched line *except* the final line, mirroring the original
            // `.lines()`-based behaviour: we replace the line content but leave
            // the file's own newline separators in place.
            if block[last].ends_with('\n') {
                end -= 1;
            }
            out.push((start, start + end, i, last));
        }
    }
    out
}

/// Re-apply the file's own indentation onto `new_str` line by line.
///
/// The needle's non-blank lines are zipped with the matched file block (using
/// `split_inclusive('\n')` so newlines are preserved): the leading whitespace of
/// each matched file line replaces whatever leading whitespace the caller put on
/// the corresponding `new_str` line. Blank `new_str` lines are left untouched.
///
/// Returns `None` when the shape is incompatible (blank/non-blank mismatch
/// between a pair of lines), in which case the lane match is discarded.
fn indent_reapply(
    content: &str,
    start_line: usize,
    end_line: usize,
    new_str: &str,
) -> Option<String> {
    let content_lines: Vec<&str> = content.split_inclusive('\n').collect();
    let block = &content_lines[start_line..=end_line];
    let new_lines: Vec<&str> = new_str.lines().collect();
    if new_lines.len() != block.len() {
        return None;
    }
    let mut out = String::new();
    for (cl, nl) in block.iter().zip(new_lines.iter()) {
        if nl.trim().is_empty() {
            let cl_body = cl.strip_suffix('\n').unwrap_or(cl);
            let cl_trimmed = cl_body.trim();
            if !cl_trimmed.is_empty() {
                // The needle claimed this line is blank but the file line is
                // not — indentation transfer would be misleading.
                return None;
            }
            out.push_str(nl);
        } else {
            let cl_body = cl.strip_suffix('\n').unwrap_or(cl);
            let cl_trimmed = cl_body.trim();
            if cl_trimmed.is_empty() {
                return None;
            }
            let indent = &cl[..cl.len() - cl.trim_start().len()];
            out.push_str(indent);
            out.push_str(nl.trim_start());
        }
        if cl.ends_with('\n') {
            out.push('\n');
        }
    }
    // Preserve the caller's new_str trailing-newline shape. If the caller
    // supplied a trailing newline we keep one; otherwise trim it. This mirrors
    // the old `.lines()`-based behaviour for files ending in a newline while
    // avoiding spurious newline insertion when the matched block is at EOF
    // without one.
    if new_str.ends_with('\n') && !out.ends_with('\n') {
        out.push('\n');
    } else if !new_str.ends_with('\n') && out.ends_with('\n') {
        out.pop();
    }
    Some(out)
}

/// Run the exact → flexible → indent-normalised cascade.
///
/// The first lane producing exactly one match wins. If every lane fails, the
/// *best* failure is reported: a multiple-match beat (lower lane wins over
/// higher lanes) beats not-found.
pub fn find_replacement_cascade(content: &str, needle: &str, new_str: &str) -> CascadeMatch {
    // ── Lane 1: exact ─────────────────────────────────────────────────────
    match find_exact_replacement_range(content, needle, new_str) {
        Ok((start, end, effective)) => {
            return CascadeMatch::Found {
                lane: MatchLane::Exact,
                start,
                end,
                new_str: effective,
            };
        }
        Err(FindError::NotFound) => {}
        Err(FindError::MultipleMatches(n)) => {
            return CascadeMatch::Failed(CascadeFail::MultipleMatches {
                lane: MatchLane::Exact,
                count: n,
                starts: content
                    .match_indices(needle)
                    .take(3)
                    .map(|(i, _)| i)
                    .collect(),
            });
        }
    }

    // ── Lane 2: flexible (whitespace-collapsed) ───────────────────────────
    // Run with an empty replacement so the returned byte range is the raw
    // matched span; substitute the caller's `new_str` on success.
    match find_flexible_replacement_range(content, needle, "") {
        Ok((start, end, _)) => {
            return CascadeMatch::Found {
                lane: MatchLane::Flexible,
                start,
                end,
                new_str: new_str.to_string(),
            };
        }
        Err(FindError::NotFound) => {}
        Err(FindError::MultipleMatches(n)) => {
            return CascadeMatch::Failed(CascadeFail::MultipleMatches {
                lane: MatchLane::Flexible,
                count: n,
                starts: Vec::new(),
            });
        }
    }

    // ── Lane 3: indent-normalised ─────────────────────────────────────────
    // Skip when the needle contains no newline: a single-line needle with
    // wrong indentation is still better served by the flexible lane above.
    if needle.contains('\n') {
        let matches = indent_normalised_matches(content, needle);
        match matches.len() {
            0 => {}
            1 => {
                let (start, end, start_line, end_line) = matches[0];
                if let Some(adjusted) = indent_reapply(content, start_line, end_line, new_str) {
                    return CascadeMatch::Found {
                        lane: MatchLane::IndentNormalised,
                        start,
                        end,
                        new_str: adjusted,
                    };
                }
            }
            _ => {
                return CascadeMatch::Failed(CascadeFail::MultipleMatches {
                    lane: MatchLane::IndentNormalised,
                    count: matches.len(),
                    starts: matches.iter().take(3).map(|m| m.0).collect(),
                });
            }
        }
    }

    CascadeMatch::Failed(CascadeFail::NotFound)
}

/// Find the contiguous block of lines in `content` most similar to the
/// (non-blank, trim_start-normalised) lines of `needle`.
///
/// Returns `Some((first_line_1based, line_count, matched, total, snippet))`
/// when at least `matched / total ≥ 0.75`; otherwise `None`. The snippet is
/// the raw text of that window, capped at 8 lines for display.
#[must_use]
pub fn nearest_window(content: &str, needle: &str) -> Option<(usize, usize, usize, usize, String)> {
    let needle_lines: Vec<&str> = needle.lines().filter(|l| !l.trim().is_empty()).collect();
    let total = needle_lines.len();
    if total == 0 {
        return None;
    }
    let content_lines: Vec<&str> = content.lines().collect();
    if content_lines.is_empty() {
        return None;
    }
    let window = total.min(content_lines.len());
    let mut best: Option<(usize, usize)> = None; // (start_idx, matched)
    for (i, block) in content_lines.windows(window).enumerate() {
        let matched = needle_lines
            .iter()
            .zip(block.iter().take(total))
            .filter(|(nl, cl)| nl.trim_start() == cl.trim_start())
            .count();
        if best.is_none_or(|(_, m)| matched > m) {
            best = Some((i, matched));
        }
    }
    let Some((start_idx, matched)) = best else {
        // No windows could be scored (content shorter than one window) or the
        // near-miss quality is below the 75% threshold — do not hint at
        // unrelated file text the caller cannot act on.
        return None;
    };
    if matched * 4 < total * 3 {
        return None; // below 75 % — not a near-miss, do not hint
    }
    let snippet: String = content_lines
        .iter()
        .skip(start_idx)
        .take(8)
        .copied()
        .collect::<Vec<_>>()
        .join("\n");
    Some((start_idx + 1, window, matched, total, snippet))
}

/// Format the "old_string not found … near-miss at line N" hint for a failed
/// match, using [`nearest_window`] to locate the most similar block.
///
/// `prefix` is the leading message: the path (and edit index for
/// `multi_edit`) plus, when `collapse_mode` is asserted, the
/// collapse-whitespace mode explanation. When no near-miss meets the 75%
/// similarity threshold, the plain [`format_match_failure`] not-found
/// diagnostic for `path` is returned instead (with the same suffix).
#[must_use]
pub fn not_found_hint(
    content: &str,
    needle: &str,
    path: &std::path::Path,
    edit_index: Option<usize>,
    collapse_mode: bool,
) -> String {
    let mode = if collapse_mode {
        " (collapse_whitespace mode: escapes decoded, whitespace runs collapsed)"
    } else {
        ""
    };
    let lead = edit_index.map_or_else(String::new, |i| format!("Edit {i}: "));
    match nearest_window(content, needle) {
        Some((line, n, matched, total, _snippet)) => {
            let content_lines: Vec<&str> = content.lines().collect();
            let start_idx = line.saturating_sub(1);
            let end_idx = (start_idx + n + 2).min(content_lines.len());
            let context_start = start_idx.saturating_sub(2);
            let mut numbered = String::new();
            for (idx, line) in content_lines
                .iter()
                .enumerate()
                .take(end_idx)
                .skip(context_start)
            {
                numbered.push_str(&format!("{:4} | {}\n", idx + 1, line));
            }
            format!(
                "{lead}old_string not found in {}{mode}. It almost matches a block \
                 starting at line {line} ({matched} / {total} needle lines match). \
                 Re-read the file and rebuild old_string from the snippet below:\n---\n{numbered}---",
                path.display()
            )
        }
        None => format!(
            "{lead}{}{mode}",
            format_match_failure(&FindDiag::not_found(), path)
        ),
    }
}

/// Maximum recommended `old_string` line count for editplan P3.10. Strings
/// longer than this are more likely to drift from the file on disk.
const MAX_RECOMMENDED_OLD_STRING_LINES: usize = 20;

/// Compute the diagnostic tag for the note field of an edit-log entry.
///
/// Returns a comma-separated tag string, or `None` when no tag applies.
#[must_use]
pub fn length_note(old_str: &str) -> Option<&'static str> {
    if old_str.lines().count() > MAX_RECOMMENDED_OLD_STRING_LINES {
        Some("long old_str")
    } else {
        None
    }
}

/// Return the 1-based line number of the byte `offset` inside `content`.
fn byte_offset_to_line(content: &str, offset: usize) -> usize {
    content[..offset].bytes().filter(|b| *b == b'\n').count() + 1
}

/// Build a disambiguation hint listing the byte offsets and surrounding
/// context of up to three match candidates.
///
/// The hint is appended to a `MultipleMatches` error so the model can extend
/// the needle on the next attempt rather than guess.
#[must_use]
pub fn disambiguation_hint(content: &str, needle: &str, starts: &[usize]) -> String {
    if starts.is_empty() {
        return String::new();
    }
    let content_lines: Vec<&str> = content.lines().collect();
    let needle_line_count = needle.lines().count().max(1);
    let mut out = String::from("Match candidates (add unique context from one of these blocks):\n");
    for (i, &s) in starts.iter().enumerate() {
        let start_line = byte_offset_to_line(content, s).saturating_sub(1);
        let end_line = (start_line + needle_line_count + 1).min(content_lines.len());
        let context_start = start_line.saturating_sub(3);
        let mut snippet = String::new();
        for (line_no, line) in content_lines
            .iter()
            .enumerate()
            .take(end_line)
            .skip(context_start)
        {
            snippet.push_str(&format!("{:4} | {}\n", line_no + 1, line));
        }
        out.push_str(&format!(
            "  {}. offset {} (line {}):\n{}",
            i + 1,
            s,
            start_line + 1,
            snippet
        ));
    }
    out.push_str("Extend old_string with more surrounding context so it matches exactly once.");
    out
}
