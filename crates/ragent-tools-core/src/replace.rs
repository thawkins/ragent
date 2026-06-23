//! Shared whitespace-tolerant replacement matcher.
//!
//! [`find_replacement_range`] is the canonical implementation of the
//! multi-pass matcher used by every replace-style tool in ragent (`edit`,
//! `multiedit`, `memory_replace`). It locates a unique byte range `[start, end)`
//! in `content` where `needle` should be replaced, and computes the effective
//! replacement text (which may differ from `new_str` when leading indentation
//! has to be re-applied).
//!
//! # Passes
//!
//! Matching is attempted in seven passes, in order. The first pass that yields
//! a unique match wins; passes that yield multiple matches fall through to
//! later (looser) passes so a looser pass can still disambiguate.
//!
//! 1. **Exact** – raw substring search.
//! 2. **CRLF-normalised** – strip `\r` from both sides, then map back to
//!    original bytes.
//! 3. **Trailing-whitespace-stripped** – strip trailing spaces/tabs from every
//!    line.
//! 4. **Leading-whitespace-stripped** – strip leading spaces/tabs from every
//!    line. The common indentation of the matched file lines is re-applied to
//!    `new_str` while preserving relative indentation.
//! 5. **Collapsed-whitespace** – collapse ALL whitespace runs to single spaces
//!    for comparison, then replace whole lines. On multiple candidates the
//!    one whose per-line leading whitespace is closest to the needle's is
//!    preferred.
//! 6. **Blank-line-normalised** – tolerate at most one leading and one trailing
//!    blank-line difference between `needle` and `content`.
//! 7. **Final-newline-normalised** – tolerate trailing `\n` presence
//!    disagreements in either direction.

/// Error returned by [`find_replacement_range`] when no unique match is found.
#[derive(Debug)]
pub enum FindError {
    /// The needle does not match anywhere in the content under any pass.
    NotFound,
    /// The needle matches multiple locations; no pass could disambiguate.
    /// Carries the match count from the most informative pass.
    MultipleMatches(usize),
}

/// Diagnostic kind carried by [`FindDiag`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FindDiagKind {
    /// The needle does not match anywhere in the content under any pass.
    NotFound,
    /// The needle matches multiple locations; no pass could disambiguate.
    /// Carries the match count from the most informative pass.
    MultipleMatches(usize),
}

/// A richer replacement-failure diagnostic returned by
/// [`find_replacement_range_diag`].
///
/// Carries the [`FindDiagKind`], the name of the last matching pass attempted
/// (`pass`), and — when discoverable — the 0-based line number of the closest
/// near-match attempt (`closest_line`). This lets callers like `multiedit`
/// produce actionable error messages (WSPLAN M3-T4).
#[derive(Debug, Clone)]
pub struct FindDiag {
    /// What kind of failure occurred.
    pub kind: FindDiagKind,
    /// Name of the last matching pass attempted (e.g. `"collapsed"`).
    pub pass: &'static str,
    /// 0-based line number of the closest near-match attempt, when known.
    pub closest_line: Option<usize>,
}

impl FindDiag {
    /// Build a `NotFound` diagnostic.
    pub(crate) fn not_found(pass: &'static str, closest_line: Option<usize>) -> Self {
        Self {
            kind: FindDiagKind::NotFound,
            pass,
            closest_line,
        }
    }

    /// Build a `MultipleMatches` diagnostic.
    pub(crate) fn multiple(pass: &'static str, count: usize, closest_line: Option<usize>) -> Self {
        Self {
            kind: FindDiagKind::MultipleMatches(count),
            pass,
            closest_line,
        }
    }
}

impl From<FindDiag> for FindError {
    fn from(d: FindDiag) -> FindError {
        match d.kind {
            FindDiagKind::NotFound => FindError::NotFound,
            FindDiagKind::MultipleMatches(n) => FindError::MultipleMatches(n),
        }
    }
}

/// Try to find the unique byte range `[start, end)` in `content` where `needle`
/// should be replaced, and compute the effective replacement text.
///
/// Returns `(start, end, effective_new_str)` on success, where
/// `effective_new_str` equals `new_str` for passes 1–3 and 6–7, but may have
/// leading indentation re-applied for passes 4–5 when the LLM stripped the
/// code's leading whitespace.
///
/// See the [crate docs](self) for the full list of passes.
pub fn find_replacement_range(
    content: &str,
    needle: &str,
    new_str: &str,
) -> Result<(usize, usize, String), FindError> {
    find_replacement_range_diag(content, needle, new_str).map_err(FindError::from)
}

/// Diagnostic variant of [`find_replacement_range`] that returns a [`FindDiag`]
/// on failure, carrying the last matching pass attempted and — when
/// discoverable — the 0-based line number of the closest near-match attempt.
///
/// This is the canonical implementation; [`find_replacement_range`] is a thin
/// wrapper that discards the extra diagnostic detail.
pub fn find_replacement_range_diag(
    content: &str,
    needle: &str,
    new_str: &str,
) -> Result<(usize, usize, String), FindDiag> {
    // Track the last pass attempted and the closest near-match line for
    // actionable error reporting (WSPLAN M3-T4). Intermediate assignments are
    // intentionally overwritten by later passes; suppress the unused-assignment
    // lint for the whole function.
    #![allow(unused_assignments)]
    let mut last_pass: &'static str = "exact";
    let mut closest_line: Option<usize> = None;

    // When a pass sees multiple matches we do NOT return immediately — a later,
    // looser pass may still disambiguate (e.g. collapsed-whitespace proximity).
    // We record the most informative "multiple" result and surface it only if
    // no later pass succeeds.
    let mut best_multiple: Option<FindDiag> = None;

    // ── Pass 1: exact ────────────────────────────────────────────────────────
    let exact_count = content.matches(needle).count();
    if exact_count == 1 {
        let start = content.find(needle).unwrap();
        return Ok((start, start + needle.len(), new_str.to_string()));
    }
    if exact_count > 1 {
        best_multiple = Some(FindDiag::multiple("exact", exact_count, None));
    }

    // ── Pass 2: CRLF normalisation ───────────────────────────────────────────
    last_pass = "crlf";
    let norm_content = strip_cr(content);
    let norm_needle = strip_cr(needle);
    let crlf_count = norm_content.matches(norm_needle.as_str()).count();
    if crlf_count == 1 {
        let norm_start = norm_content.find(norm_needle.as_str()).unwrap();
        let norm_end = norm_start + norm_needle.len();
        let start = norm_to_orig_byte(content, norm_start);
        let end = norm_to_orig_byte(content, norm_end);
        return Ok((start, end, new_str.to_string()));
    }
    if crlf_count > 1 && best_multiple.is_none() {
        closest_line = Some(byte_offset_of_line(
            content,
            line_of_nth_match(&norm_content, norm_needle.as_str(), 0),
        ));
        best_multiple = Some(FindDiag::multiple("crlf", crlf_count, closest_line));
    }

    // ── Pass 3: trailing-whitespace stripping ────────────────────────────────
    last_pass = "trailing-ws";
    let ws_content = strip_trailing_ws(&norm_content);
    let ws_needle = strip_trailing_ws(&norm_needle);
    if ws_needle.is_empty() {
        // Needle is whitespace-only after stripping; later passes can't help.
        return Err(best_multiple.unwrap_or_else(|| FindDiag::not_found(last_pass, closest_line)));
    }
    let ws_count = ws_content.matches(ws_needle.as_str()).count();
    if ws_count == 1 {
        let ws_start = ws_content.find(ws_needle.as_str()).unwrap();
        let start_line = ws_content[..ws_start]
            .chars()
            .filter(|&c| c == '\n')
            .count();
        let needle_line_count = ws_needle.lines().count();
        let end_line = start_line + needle_line_count;
        let orig_start = byte_offset_of_line(content, start_line);
        let orig_end = byte_offset_of_line(content, end_line);
        return Ok((orig_start, orig_end, new_str.to_string()));
    }
    if ws_count > 1 && best_multiple.is_none() {
        closest_line = Some(line_of_nth_match(&ws_content, ws_needle.as_str(), 0));
        best_multiple = Some(FindDiag::multiple("trailing-ws", ws_count, closest_line));
    }
    // ── Pass 4: leading-whitespace stripping ─────────────────────────────────    // Handles LLMs that read line-numbered output (e.g. " 281  registry.register(...)")
    // and accidentally strip the code's leading indentation from old_str/new_str.
    // We compare trimmed lines; on a unique match we re-apply the **common**
    // leading indentation of the matched file lines to `new_str`, preserving
    // any relative indentation already present in `new_str`.
    last_pass = "leading-ws";
    let content_lines: Vec<&str> = content.lines().collect();
    let needle_lines_trimmed: Vec<&str> = {
        let mut v: Vec<&str> = needle.lines().map(str::trim_start).collect();
        // Drop trailing empty lines produced by a trailing `\n` so the window
        // length matches the needle's logical line count.
        while v.last().is_some_and(|l| l.is_empty()) {
            v.pop();
        }
        v
    };
    let n = needle_lines_trimmed.len();

    if n > 0 && !needle_lines_trimmed.iter().all(|l| l.is_empty()) {
        let mut lws_matches: Vec<usize> = Vec::new(); // start line indices
        'outer: for start_idx in 0..=content_lines.len().saturating_sub(n) {
            for (i, needle_line) in needle_lines_trimmed.iter().enumerate() {
                let file_line = content_lines.get(start_idx + i).copied().unwrap_or("");
                if file_line.trim_start() != *needle_line {
                    continue 'outer;
                }
            }
            lws_matches.push(start_idx);
        }
        match lws_matches.len() {
            0 => {}
            1 => {
                let start_idx = lws_matches[0];
                let orig_start = byte_offset_of_line(content, start_idx);
                let orig_end = byte_offset_of_line(content, start_idx + n);
                // Re-apply the common leading whitespace of the matched file
                // lines so that relative indentation already present in
                // `new_str` is preserved rather than doubled.
                let indent = common_leading_ws(&content_lines[start_idx..start_idx + n]);
                let effective_new = if indent.is_empty() {
                    new_str.to_string()
                } else {
                    reindent_with(new_str, indent)
                };
                return Ok((orig_start, orig_end, effective_new));
            }
            // Fall through to pass 5 instead of hard-erroring here. Pass 5's
            // whitespace-proximity disambiguation may still be able to pick a
            // unique candidate from the leading-whitespace matches.
            _ => {}
        }
    }

    // ── Pass 5: collapse-all-whitespace ──────────────────────────────────────
    // Normalise every line by trimming and collapsing internal whitespace runs
    // to a single space, then compare line-by-line. Because we always do whole-
    // line replacements, partial-match ambiguity cannot arise. On a unique match
    // the common leading indent from the matched file lines is re-applied to
    // `new_str`.
    //
    // If collapsed matching yields multiple candidates, prefer the candidate
    // whose per-line leading whitespace is closest (summed absolute char-length
    // distance) to the needle's per-line leading whitespace. This prevents
    // over-normalisation from turning a solvable unique edit into an ambiguous
    // error when one candidate is clearly a better whitespace match.
    last_pass = "collapsed";
    let needle_lines_collapsed: Vec<String> = {
        let mut v: Vec<String> = needle
            .lines()
            .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
            .collect();
        // Drop trailing empty collapsed lines (from a trailing `\n`) so the
        // window length matches the needle's logical line count.
        while v.last().is_some_and(String::is_empty) {
            v.pop();
        }
        v
    };
    let n5 = needle_lines_collapsed.len();

    if n5 > 0 && needle_lines_collapsed.iter().any(|l| !l.is_empty()) {
        let mut cws_matches: Vec<usize> = Vec::new();
        'cws: for start_idx in 0..=content_lines.len().saturating_sub(n5) {
            for (i, needle_line) in needle_lines_collapsed.iter().enumerate() {
                let file_collapsed = content_lines
                    .get(start_idx + i)
                    .copied()
                    .unwrap_or("")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if file_collapsed != *needle_line {
                    continue 'cws;
                }
            }
            cws_matches.push(start_idx);
        }
        match cws_matches.len() {
            0 => {}
            1 => {
                let start_idx = cws_matches[0];
                let orig_start = byte_offset_of_line(content, start_idx);
                let orig_end = byte_offset_of_line(content, start_idx + n5);
                let indent = common_leading_ws(&content_lines[start_idx..start_idx + n5]);
                let effective_new = if indent.is_empty() {
                    new_str.to_string()
                } else {
                    reindent_with(new_str, indent)
                };
                return Ok((orig_start, orig_end, effective_new));
            }
            count => {
                // Disambiguate by whitespace proximity: pick the candidate whose
                // per-line leading whitespace is closest (smallest total char
                // distance) to the needle's per-line leading whitespace. This
                // prevents over-normalisation from turning a solvable unique
                // edit into an ambiguous error when one candidate is clearly a
                // better whitespace match.
                if let Some((orig_start, orig_end, effective_new)) =
                    disambiguate_by_whitespace_proximity(content, needle, &cws_matches, n5, new_str)
                {
                    return Ok((orig_start, orig_end, effective_new));
                }
                closest_line = Some(cws_matches.first().copied().unwrap_or(0));
                if best_multiple.is_none() {
                    best_multiple = Some(FindDiag::multiple("collapsed", count, closest_line));
                }
            }
        }
    }

    // ── Pass 6: blank-line normalisation ─────────────────────────────────────
    // `str::lines()` drops a trailing empty segment, so a needle that includes
    // one leading or trailing blank line may not byte-match the file even when
    // the non-blank content is identical. Strip at most one leading and one
    // trailing blank line from the needle's line array, then search for the
    // resulting window in the content's line array using exact (CRLF-tolerant)
    // line equality. Map the matched window back to original byte offsets.
    last_pass = "blank-line";
    if let Some((start, end)) = try_blank_line_normalised(content, needle) {
        return Ok((start, end, new_str.to_string()));
    }

    // ── Pass 7: final-newline normalisation ──────────────────────────────────
    // Tolerate trailing-`\n` presence disagreements in either direction. This
    // covers the four cases: (file has \n, needle lacks), (needle has \n, file
    // lacks), and the symmetric CRLF variants.
    last_pass = "final-newline";
    if let Some((start, end)) = try_final_newline_normalised(content, needle) {
        return Ok((start, end, new_str.to_string()));
    }

    // Best-effort closest-line hint for actionable diagnostics: find the line
    // in `content` whose collapsed form most closely resembles the needle's
    // first collapsed line. This is only a heuristic for error messages.
    if closest_line.is_none() {
        closest_line = closest_collapsed_line(content, needle);
    }

    // If any earlier pass saw multiple matches, surface that — it is more
    // actionable than a bare `NotFound`. Prefer the earliest (strictest) pass.
    if let Some(mult) = best_multiple {
        return Err(mult);
    }

    Err(FindDiag::not_found(last_pass, closest_line))
}

/// Remove all `\r` characters (handles both `\r\n` and lone `\r`).
fn strip_cr(s: &str) -> String {
    s.chars().filter(|&c| c != '\r').collect()
}

/// Return the 0-based line index of the `nth` (0-based) occurrence of `needle`
/// in `haystack`, counting `\n` characters before the match start.
fn line_of_nth_match(haystack: &str, needle: &str, nth: usize) -> usize {
    let mut from = 0usize;
    let mut found = 0usize;
    while let Some(pos) = haystack[from..].find(needle) {
        let abs = from + pos;
        if found == nth {
            return haystack[..abs].chars().filter(|&c| c == '\n').count();
        }
        found += 1;
        from = abs + needle.len().max(1);
    }
    0
}

/// Best-effort: find the 0-based line index in `content` whose collapsed form
/// shares the most leading tokens with the first collapsed line of `needle`.
/// Used only to produce an actionable "closest line" hint in error messages.
fn closest_collapsed_line(content: &str, needle: &str) -> Option<usize> {
    let needle_first = needle
        .lines()
        .next()
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>();
    if needle_first.is_empty() {
        return None;
    }
    let mut best_line: Option<usize> = None;
    let mut best_score: usize = 0;
    for (i, line) in content.lines().enumerate() {
        let tokens = line.split_whitespace().collect::<Vec<_>>();
        if tokens.is_empty() {
            continue;
        }
        // Score = longest common prefix token count with the needle's first line.
        let score = tokens
            .iter()
            .zip(needle_first.iter())
            .take_while(|(a, b)| a == b)
            .count();
        if score > best_score {
            best_score = score;
            best_line = Some(i);
        }
    }
    best_line
}

/// Strip trailing whitespace from every line and re-join with `\n`.
fn strip_trailing_ws(s: &str) -> String {
    s.lines().map(str::trim_end).collect::<Vec<_>>().join("\n")
}

/// Map a byte offset in the CRLF-normalised string (all `\r` removed) back to
/// the corresponding byte offset in the original string.
fn norm_to_orig_byte(original: &str, norm_offset: usize) -> usize {
    let mut norm_pos = 0usize;
    let mut orig_pos = 0usize;
    for c in original.chars() {
        if norm_pos == norm_offset {
            return orig_pos;
        }
        if c != '\r' {
            norm_pos += c.len_utf8();
        }
        orig_pos += c.len_utf8();
    }
    orig_pos // reached end
}

/// Return the byte offset of the start of line `line_idx` (0-based) in `s`.
/// Lines are counted by `\n` occurrences (covers `\r\n` and bare `\n`).
/// Returns `s.len()` when `line_idx` is beyond the last line.
pub fn byte_offset_of_line(s: &str, line_idx: usize) -> usize {
    if line_idx == 0 {
        return 0;
    }
    let mut n = 0usize;
    for (i, c) in s.char_indices() {
        if c == '\n' {
            n += 1;
            if n == line_idx {
                return i + 1;
            }
        }
    }
    s.len()
}

/// Extract leading whitespace (spaces/tabs) from a line.
fn leading_ws(line: &str) -> &str {
    let trimmed_len = line.trim_start().len();
    &line[..line.len() - trimmed_len]
}

/// Compute the common leading whitespace shared by every non-blank line in
/// `lines`. Blank lines are ignored (they contribute no indentation). Returns
/// the longest prefix of spaces/tabs shared by all non-blank lines; if there
/// are no non-blank lines, returns an empty slice.
fn common_leading_ws<'a>(lines: &[&'a str]) -> &'a str {
    let mut common: Option<&'a str> = None;
    for line in lines {
        if line.trim().is_empty() {
            continue;
        }
        let lws = leading_ws(line);
        common = Some(match common {
            None => lws,
            Some(prev) => {
                // Longest common prefix of prev and lws.
                let n = prev
                    .chars()
                    .zip(lws.chars())
                    .take_while(|(a, b)| a == b)
                    .count();
                &prev[..n]
            }
        });
        if common.is_none_or(str::is_empty) {
            break;
        }
    }
    common.unwrap_or("")
}

/// Prepend `indent` to every non-blank line of `s`, preserving the trailing
/// newline if present.
///
/// Blank lines are left untouched so that no trailing whitespace is introduced
/// into otherwise-empty lines. Relative indentation already present in `new_str`
/// is preserved because the common leading whitespace of the matched file lines
/// (not the first line's full indentation) is what gets prepended — lines that
/// already carry deeper relative indentation keep that depth on top of the
/// re-applied common indent.
fn reindent_with(s: &str, indent: &str) -> String {
    if indent.is_empty() {
        return s.to_string();
    }
    let mut result = s
        .lines()
        .map(|l| {
            if l.trim().is_empty() {
                l.to_string()
            } else {
                format!("{indent}{l}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n");
    if s.ends_with('\n') {
        result.push('\n');
    }
    result
}

// ── Blank-line & final-newline helpers (passes 6 & 7) ─────────────────────────

/// Strip at most one leading and one trailing blank line from `s` and return
/// the line array of the trimmed result. "Blank" means empty or whitespace-only.
///
/// This is used to normalise the inconsistency where `str::lines()` drops a
/// trailing empty segment and the LLM may include or omit a blank line that the
/// file does not.
fn strip_one_blank_edge_lines(s: &str) -> (Vec<&str>, bool, bool) {
    let mut lines: Vec<&str> = s.lines().collect();
    let mut stripped_leading = false;
    let mut stripped_trailing = false;
    if let Some(first) = lines.first()
        && first.trim().is_empty()
    {
        lines.remove(0);
        stripped_leading = true;
    }
    if let Some(last) = lines.last()
        && last.trim().is_empty()
    {
        lines.pop();
        stripped_trailing = true;
    }
    (lines, stripped_leading, stripped_trailing)
}

/// Pass 6: blank-line normalisation. Strip at most one leading and one trailing
/// blank line from the needle's line array, then search for the resulting
/// window in the content's line array using exact (CRLF-tolerant) line
/// equality. Map the matched window back to the original `content` byte range.
///
/// This handles all four edge-difference cases (leading/trailing blank in
/// needle-only, file-only, or both) because the window search runs against the
/// full content line array.
fn try_blank_line_normalised(content: &str, needle: &str) -> Option<(usize, usize)> {
    let content_lines: Vec<&str> = content.lines().collect();
    let (needle_lines, needle_stripped_leading, needle_stripped_trailing) =
        strip_one_blank_edge_lines(needle);
    // Only engage this pass if the needle actually had a blank edge to strip.
    if !needle_stripped_leading && !needle_stripped_trailing {
        return None;
    }
    let n = needle_lines.len();
    if n == 0 || needle_lines.iter().all(|l| l.trim().is_empty()) {
        return None;
    }

    // Find unique window in content_lines where each line CRLF-normalised
    // equals the corresponding needle line.
    let norm_needle_lines: Vec<String> = needle_lines.iter().map(|l| strip_cr(l)).collect();
    let mut matches: Vec<usize> = Vec::new();
    'outer: for start_idx in 0..=content_lines.len().saturating_sub(n) {
        for (i, nl) in norm_needle_lines.iter().enumerate() {
            let file_line = content_lines.get(start_idx + i).copied().unwrap_or("");
            if strip_cr(file_line) != *nl {
                continue 'outer;
            }
        }
        matches.push(start_idx);
    }
    if matches.len() == 1 {
        let start_idx = matches[0];
        let orig_start = byte_offset_of_line(content, start_idx);
        let orig_end = byte_offset_of_line(content, start_idx + n);
        return Some((orig_start, orig_end));
    }
    None
}

/// Pass 7: final-newline normalisation. Tolerate trailing-`\n` presence
/// disagreements in either direction. Returns the byte range in `content` for
/// the matched substring.
fn try_final_newline_normalised(content: &str, needle: &str) -> Option<(usize, usize)> {
    let content_has_trailing = content.ends_with('\n');
    let needle_has_trailing = needle.ends_with('\n');
    if content_has_trailing == needle_has_trailing {
        return None; // no final-newline disagreement to fix
    }

    // Normalise both to no trailing newline for comparison.
    let content_core = content.strip_suffix('\n').unwrap_or(content);
    let needle_core = needle.strip_suffix('\n').unwrap_or(needle);
    if needle_core.is_empty() {
        return None;
    }

    let count = content_core.matches(needle_core).count();
    if count == 1 {
        let start = content_core.find(needle_core).unwrap();
        let end = start + needle_core.len();
        // `content_core` is `content` without its trailing `\n` (if any), so
        // byte offsets are identical to `content` offsets for the non-trailing
        // portion.
        return Some((start, end));
    }

    // Also try CRLF-normalised comparison on the core strings.
    let norm_content = strip_cr(content_core);
    let norm_needle = strip_cr(needle_core);
    let crlf_count = norm_content.matches(norm_needle.as_str()).count();
    if crlf_count == 1 {
        let norm_start = norm_content.find(norm_needle.as_str()).unwrap();
        let norm_end = norm_start + norm_needle.len();
        let start = norm_to_orig_byte(content_core, norm_start);
        let end = norm_to_orig_byte(content_core, norm_end);
        return Some((start, end));
    }
    None
}

/// Disambiguate multiple collapsed-whitespace candidates by picking the one
/// whose per-line leading whitespace is closest (smallest total character
/// distance) to the needle's per-line leading whitespace. Returns that
/// candidate's byte range with the effective new string, or `None` if no
/// candidate is strictly closer than the others (a tie).
fn disambiguate_by_whitespace_proximity(
    content: &str,
    needle: &str,
    cws_matches: &[usize],
    n5: usize,
    new_str: &str,
) -> Option<(usize, usize, String)> {
    let content_lines: Vec<&str> = content.lines().collect();
    let needle_lines: Vec<&str> = needle.lines().collect();

    // Sum of absolute differences in leading-whitespace character length across
    // aligned lines. This is a cheap proxy for "how different is the
    // whitespace"; ties are broken by being strict-less.
    let score = |start_idx: usize| -> usize {
        let mut total = 0usize;
        for i in 0..n5 {
            let file_lws =
                leading_ws(content_lines.get(start_idx + i).copied().unwrap_or("")).len();
            let needle_lws = leading_ws(needle_lines.get(i).copied().unwrap_or("")).len();
            total += file_lws.abs_diff(needle_lws);
        }
        total
    };

    let mut best_idx: Option<usize> = None;
    let mut best_score: usize = usize::MAX;
    let mut tie = false;
    for &start_idx in cws_matches {
        let s = score(start_idx);
        match s.cmp(&best_score) {
            std::cmp::Ordering::Less => {
                best_score = s;
                best_idx = Some(start_idx);
                tie = false;
            }
            std::cmp::Ordering::Equal => {
                tie = true;
            }
            std::cmp::Ordering::Greater => {}
        }
    }

    if tie {
        return None;
    }
    let start_idx = best_idx?;
    let orig_start = byte_offset_of_line(content, start_idx);
    let orig_end = byte_offset_of_line(content, start_idx + n5);
    let indent = common_leading_ws(&content_lines[start_idx..start_idx + n5]);
    let effective_new = if indent.is_empty() {
        new_str.to_string()
    } else {
        reindent_with(new_str, indent)
    };
    Some((orig_start, orig_end, effective_new))
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn check(content: &str, needle: &str) -> (usize, usize) {
        let (s, e, _) = find_replacement_range(content, needle, "").expect("should find match");
        (s, e)
    }

    fn check_with_new(content: &str, needle: &str, new_str: &str) -> (usize, usize, String) {
        find_replacement_range(content, needle, new_str).expect("should find match")
    }

    #[test]
    fn exact_match() {
        let c = "fn foo() {\n    bar\n}\n";
        let (s, e) = check(c, "    bar\n");
        assert_eq!(&c[s..e], "    bar\n");
    }

    #[test]
    fn crlf_normalised_match() {
        let c = "fn foo() {\r\n    bar\r\n}\r\n";
        let needle = "fn foo() {\n    bar\n}\n";
        let (s, e) = check(c, needle);
        assert_eq!(&c[s..e], "fn foo() {\r\n    bar\r\n}\r\n");
    }

    #[test]
    fn trailing_whitespace_match() {
        let c = "fn foo() {  \n    bar  \n}\n";
        let needle = "fn foo() {\n    bar\n}\n";
        let (s, e) = check(c, needle);
        assert_eq!(&c[s..e], "fn foo() {  \n    bar  \n}\n");
    }

    #[test]
    fn trailing_whitespace_and_crlf() {
        let c = "fn foo() {  \r\n    bar  \r\n}\r\n";
        let needle = "fn foo() {\n    bar\n}\n";
        let (s, e) = check(c, needle);
        assert_eq!(&c[s..e], c);
    }

    #[test]
    fn not_found_returns_err() {
        let c = "hello world\n";
        assert!(matches!(
            find_replacement_range(c, "goodbye", ""),
            Err(FindError::NotFound)
        ));
    }

    #[test]
    fn multiple_matches_returns_err() {
        let c = "foo\nfoo\n";
        assert!(matches!(
            find_replacement_range(c, "foo", ""),
            Err(FindError::MultipleMatches(2))
        ));
    }

    #[test]
    fn byte_offset_of_line_basic() {
        let s = "a\nb\nc\n";
        assert_eq!(byte_offset_of_line(s, 0), 0);
        assert_eq!(byte_offset_of_line(s, 1), 2);
        assert_eq!(byte_offset_of_line(s, 2), 4);
        assert_eq!(byte_offset_of_line(s, 3), 6);
        assert_eq!(byte_offset_of_line(s, 99), 6); // beyond end → len
    }

    #[test]
    fn leading_whitespace_stripped_match() {
        let c = "fn setup() {\n    registry.register(A);\n    registry.register(B);\n}\n";
        let needle = "registry.register(A);\nregistry.register(B);\n";
        let new_str = "registry.register(A);\nregistry.register(C);\n";
        let (s, e, effective) = check_with_new(c, needle, new_str);
        assert_eq!(
            &c[s..e],
            "    registry.register(A);\n    registry.register(B);\n"
        );
        assert_eq!(
            effective,
            "    registry.register(A);\n    registry.register(C);\n"
        );
    }

    #[test]
    fn leading_whitespace_match_preserves_relative_indent() {
        let c = "    fn foo() {\n        let x = 1;\n    }\n";
        let needle = "fn foo() {\n    let x = 1;\n}\n";
        let new_str = "fn foo() {\n    let x = 2;\n}\n";
        let (s, e, effective) = check_with_new(c, needle, new_str);
        assert_eq!(&c[s..e], "    fn foo() {\n        let x = 1;\n    }\n");
        assert_eq!(effective, "    fn foo() {\n        let x = 2;\n    }\n");
    }

    #[test]
    fn collapsed_whitespace_match() {
        let c = "\tlet  x  =  1;\n\tlet  y  =  2;\n";
        let needle = "let x = 1;\nlet y = 2;\n";
        let new_str = "let x = 1;\nlet y = 99;\n";
        let (s, e, effective) = check_with_new(c, needle, new_str);
        assert_eq!(&c[s..e], "\tlet  x  =  1;\n\tlet  y  =  2;\n");
        assert_eq!(effective, "\tlet x = 1;\n\tlet y = 99;\n");
    }

    // ── M1-T1: blank-line normalisation ───────────────────────────────────────

    #[test]
    fn blank_line_leading_in_needle() {
        let c = "fn foo() {\n    bar\n}\n";
        let needle = "\nfn foo() {\n    bar\n}\n";
        let new_str = "fn foo() {\n    baz\n}\n";
        let (s, e, effective) = check_with_new(c, needle, new_str);
        assert_eq!(&c[s..e], "fn foo() {\n    bar\n}\n");
        assert_eq!(effective, new_str);
    }

    #[test]
    fn blank_line_trailing_in_needle() {
        let c = "fn foo() {\n    bar\n}\n";
        let needle = "fn foo() {\n    bar\n}\n\n";
        let (s, e) = check(c, needle);
        assert_eq!(&c[s..e], "fn foo() {\n    bar\n}\n");
    }

    #[test]
    fn blank_line_leading_in_file() {
        let c = "\nfn foo() {\n    bar\n}\n";
        let needle = "fn foo() {\n    bar\n}\n";
        let (s, e) = check(c, needle);
        assert_eq!(&c[s..e], "fn foo() {\n    bar\n}\n");
    }

    #[test]
    fn blank_line_trailing_in_file() {
        let c = "fn foo() {\n    bar\n}\n\n";
        let needle = "fn foo() {\n    bar\n}\n";
        let (s, e) = check(c, needle);
        assert_eq!(&c[s..e], "fn foo() {\n    bar\n}\n");
    }

    #[test]
    fn blank_line_both_edges_differ() {
        let c = "\nfn foo() {\n    bar\n}\n\n";
        let needle = "fn foo() {\n    bar\n}\n";
        let (s, e) = check(c, needle);
        assert_eq!(&c[s..e], "fn foo() {\n    bar\n}\n");
    }

    // ── M1-T2: final-newline normalisation ────────────────────────────────────

    #[test]
    fn final_newline_file_has_needle_lacks() {
        let c = "fn foo() {\n    bar\n}\n";
        let needle = "fn foo() {\n    bar\n}";
        let (s, e) = check(c, needle);
        assert_eq!(&c[s..e], "fn foo() {\n    bar\n}");
    }

    #[test]
    fn final_newline_needle_has_file_lacks() {
        let c = "fn foo() {\n    bar\n}";
        let needle = "fn foo() {\n    bar\n}\n";
        let (s, e) = check(c, needle);
        assert_eq!(&c[s..e], "fn foo() {\n    bar\n}");
    }

    #[test]
    fn final_newline_crlf_disagreement() {
        let c = "fn foo() {\r\n    bar\r\n}\r\n";
        let needle = "fn foo() {\n    bar\n}";
        let (s, e) = check(c, needle);
        assert_eq!(&c[s..e], "fn foo() {\r\n    bar\r\n}");
    }

    // ── M1-T3: collapsed-whitespace false-positive reduction ───────────────────

    #[test]
    fn collapsed_disambiguates_by_whitespace_proximity() {
        // Two lines collapse to the same signature, and earlier passes all see
        // multiple matches (or none), so we reach pass 5 collapsed with >1
        // candidate. The matcher should prefer the candidate whose leading
        // whitespace is closest to the needle's, rather than erroring.
        //
        // Needle has no leading indent (0 chars). Line A has an 8-space indent,
        // line B has a 4-space indent. Both lines contain the needle's exact
        // core (so exact / trailing-WS see multiple matches), and both collapse
        // to the same signature. B (4 chars from 0) is closer than A (8 chars),
        // so B wins.
        let c = "        let  x = 1;\n    let  x = 1;\n";
        let needle = "let  x = 1;\n";
        let (s, e) = check(c, needle);
        assert_eq!(&c[s..e], "    let  x = 1;\n");
    }

    #[test]
    fn collapsed_tie_still_errors() {
        let c = "    let  x = 1;\n    let  x = 1;\n";
        let needle = "let  x = 1;\n";
        assert!(matches!(
            find_replacement_range(c, needle, ""),
            Err(FindError::MultipleMatches(_))
        ));
    }

    // ── M1-T4: relative indentation preservation in reindent_with ─────────────

    #[test]
    fn reindent_preserves_relative_indentation_nested() {
        let c = "    fn foo() {\n        let x = 1;\n    }\n";
        let needle = "fn foo() {\n    let x = 1;\n}\n";
        let new_str = "fn foo() {\n    let x = 2;\n}\n";
        let (s, e, effective) = check_with_new(c, needle, new_str);
        assert_eq!(&c[s..e], "    fn foo() {\n        let x = 1;\n    }\n");
        assert_eq!(effective, "    fn foo() {\n        let x = 2;\n    }\n");
    }

    #[test]
    fn reindent_tab_vs_space_preserves_relative() {
        let c = "\tfn foo() {\n\t\tlet x = 1;\n\t}\n";
        let needle = "fn foo() {\n    let x = 1;\n}\n";
        let new_str = "fn foo() {\n    let x = 2;\n}\n";
        let (s, e, effective) = check_with_new(c, needle, new_str);
        assert_eq!(&c[s..e], "\tfn foo() {\n\t\tlet x = 1;\n\t}\n");
        assert_eq!(effective, "\tfn foo() {\n\t    let x = 2;\n\t}\n");
    }

    #[test]
    fn reindent_blank_lines_left_untouched() {
        let c = "    fn foo() {\n\n    }\n";
        let needle = "fn foo() {\n\n}\n";
        let new_str = "fn foo() {\n\n}\n";
        let (s, e, effective) = check_with_new(c, needle, new_str);
        assert_eq!(&c[s..e], "    fn foo() {\n\n    }\n");
        assert_eq!(effective, "    fn foo() {\n\n    }\n");
    }

    #[test]
    fn common_leading_ws_handles_mixed_indent() {
        let lines = vec!["    foo", "  bar"];
        assert_eq!(common_leading_ws(&lines), "  ");
        let lines = vec!["    foo", "", "    bar"];
        assert_eq!(common_leading_ws(&lines), "    ");
        let lines = vec!["", "  ", ""];
        assert_eq!(common_leading_ws(&lines), "");
    }
}
