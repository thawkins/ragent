//! Surgical text replacement tool for file editing.
//!
//! Provides [`EditTool`], which replaces exactly one occurrence of a search
//! string with a replacement string in a file, ensuring precise edits.
//!
//! Matching is attempted in four passes to handle common LLM output quirks:
//!
//! 1. **Exact match** – the fastest and most precise path.
//! 2. **CRLF-normalised match** – handles files with `\r\n` line endings when
//!    the LLM generates `\n`-only search strings (because the `read` tool
//!    strips `\r` via `.lines()`).
//! 3. **Trailing-whitespace-stripped match** – handles lines where the file has
//!    trailing spaces/tabs that the LLM silently omitted in `old_str`.
//! 4. **Leading-whitespace-stripped match** – handles LLMs that read
//!    line-numbered output (e.g. `" 281  registry.register(...)"`) and
//!    accidentally strip the code's leading indentation when writing `old_str`.
//!    When matched this way, the detected indentation is automatically
//!    re-applied to every line of `new_str`.
//! 5. **Collapsed-whitespace match** – collapses ALL whitespace (leading,
//!    trailing, and internal runs) to single spaces before comparing.
//!    Since matches are always whole-line replacements there is no partial-
//!    match ambiguity. Handles tabs-vs-spaces, double spaces, mixed indentation.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

/// Replaces an exact, unique occurrence of `old_str` with `new_str` in a file.
///
/// The search string must match exactly once; zero or multiple matches are
/// treated as errors to prevent ambiguous edits.
pub struct EditTool;

#[async_trait::async_trait]
impl Tool for EditTool {
    fn name(&self) -> &str {
        "edit"
    }

        /// Returns a human-readable description of what the tool does.
        fn description(&self) -> &str {
            "Replace an exact occurrence of old_str with new_str in a file. \
               The old_str must match exactly one location in the file."
        }
    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the file to edit"
                },
                "old_str": {
                    "type": "string",
                    "description": "Exact string to find and replace"
                },
                "new_str": {
                    "type": "string",
                    "description": "Replacement string"
                }
            },
            "required": ["path", "old_str", "new_str"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:write"
    }

    /// Performs a surgical text replacement in a file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `path`, `old_str`, or `new_str` parameter is missing or invalid
    /// - The file cannot be read (file not found, permission denied, not UTF-8)
    /// - The `old_str` is not found in the file
    /// - The `old_str` matches multiple locations (ambiguous edit)
    /// - The file cannot be written after the edit
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"]
            .as_str()
            .context("Missing required 'path' parameter")?;
        let old_str = input["old_str"]
            .as_str()
            .context("Missing required 'old_str' parameter")?;
        let new_str = input["new_str"]
            .as_str()
            .context("Missing required 'new_str' parameter")?;

        let path = resolve_path(&ctx.working_dir, path_str);

        // Acquire file lock to serialize concurrent edits to the same file
        let _lock = super::file_lock::lock_file(&path).await;

        let content = tokio::fs::read_to_string(&path).await.with_context(|| {
            format!(
                "Cannot read file '{}': file may not exist or is not accessible",
                path.display()
            )
        })?;

        let (start, end, effective_new_str) =
            match find_replacement_range(&content, old_str, new_str) {
                Ok(range) => range,
                Err(FindError::NotFound) => bail!(
                    "old_str not found in {}. Make sure it matches exactly.",
                    path.display()
                ),
                Err(FindError::MultipleMatches(n)) => bail!(
                    "old_str found {} times in {}. It must match exactly once. \
                 Add more context to make it unique.",
                    n,
                    path.display()
                ),
            };

        let new_content = format!(
            "{}{}{}",
            &content[..start],
            effective_new_str,
            &content[end..]
        );
        tokio::fs::write(&path, &new_content)
            .await
            .with_context(|| format!("Failed to write file: {}", path.display()))?;

        let old_lines = old_str.lines().count();
        let new_lines = effective_new_str.lines().count();
        let lines_changed = old_lines.max(new_lines);

        Ok(ToolOutput {
            content: format!(
                "Edited {}: replaced {} line{} with {} line{}",
                path.display(),
                old_lines,
                if old_lines == 1 { "" } else { "s" },
                new_lines,
                if new_lines == 1 { "" } else { "s" },
            ),
            metadata: Some(json!({
                "path": path.display().to_string(),
                "old_lines": old_lines,
                "new_lines": new_lines,
                "lines": lines_changed,
            })),
        })
    }
}

// ── Matching helpers ──────────────────────────────────────────────────────────

#[derive(Debug)]
pub(crate) enum FindError {
    NotFound,
    MultipleMatches(usize),
}

/// Try to find the unique byte range `[start, end)` in `content` where `needle`
/// should be replaced, and compute the effective replacement text.
///
/// Returns `(start, end, effective_new_str)` on success, where `effective_new_str`
/// equals `new_str` for passes 1–3, but may have leading indentation re-applied
/// for passes 4–5 when the LLM stripped the code's leading whitespace.
///
/// Five passes are attempted in order:
///
/// 1. **Exact** – raw substring search.
/// 2. **CRLF-normalised** – strip `\r` from both sides, then map back to original bytes.
/// 3. **Trailing-whitespace-stripped** – strip trailing spaces/tabs from every line.
/// 4. **Leading-whitespace-stripped** – strip leading spaces/tabs from every line.
///    The indentation of the first matched line is re-applied to every line of `new_str`.
/// 5. **Collapsed-whitespace** – collapse ALL whitespace runs (leading, trailing,
///    internal) to single spaces for comparison, then replace whole lines.
///    Handles tabs-vs-spaces, multiple spaces, and mixed indentation differences.
pub(crate) fn find_replacement_range(
    content: &str,
    needle: &str,
    new_str: &str,
) -> Result<(usize, usize, String), FindError> {
    // ── Pass 1: exact ────────────────────────────────────────────────────────
    let exact_count = content.matches(needle).count();
    if exact_count == 1 {
        let start = content.find(needle).unwrap();
        return Ok((start, start + needle.len(), new_str.to_string()));
    }
    if exact_count > 1 {
        return Err(FindError::MultipleMatches(exact_count));
    }

    // ── Pass 2: CRLF normalisation ───────────────────────────────────────────
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
    if crlf_count > 1 {
        return Err(FindError::MultipleMatches(crlf_count));
    }

    // ── Pass 3: trailing-whitespace stripping ────────────────────────────────
    let ws_content = strip_trailing_ws(&norm_content);
    let ws_needle = strip_trailing_ws(&norm_needle);
    if ws_needle.is_empty() {
        return Err(FindError::NotFound);
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
    if ws_count > 1 {
        return Err(FindError::MultipleMatches(ws_count));
    }

    // ── Pass 4: leading-whitespace stripping ─────────────────────────────────
    // Handles LLMs that read line-numbered output (e.g. " 281  registry.register(...)")
    // and accidentally strip the code's leading indentation from old_str/new_str.
    // We compare trimmed lines; on a unique match we re-apply the original
    // indentation of the first matched line to every line of new_str.
    let content_lines: Vec<&str> = content.lines().collect();
    let needle_lines_trimmed: Vec<&str> = needle.lines().map(str::trim_start).collect();
    let n = needle_lines_trimmed.len();

    if n > 0 && !needle_lines_trimmed.iter().all(|l| l.is_empty()) {
        let mut lws_matches: Vec<usize> = Vec::new(); // start line indices
        'outer: for start_idx in 0..=content_lines.len().saturating_sub(n) {
            for i in 0..n {
                let file_line = content_lines.get(start_idx + i).copied().unwrap_or("");
                if file_line.trim_start() != needle_lines_trimmed[i] {
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
                // Detect indentation from the first matched line in the original file.
                let indent = leading_ws(content_lines[start_idx]);
                let effective_new = if indent.is_empty() {
                    new_str.to_string()
                } else {
                    reindent_with(new_str, indent)
                };
                return Ok((orig_start, orig_end, effective_new));
            }
            count => return Err(FindError::MultipleMatches(count)),
        }
    }

    // ── Pass 5: collapse-all-whitespace ──────────────────────────────────────
    // Normalise every line by trimming and collapsing internal whitespace runs
    // to a single space, then compare line-by-line. Because we always do whole-
    // line replacements, partial-match ambiguity cannot arise. On a unique match
    // the leading indent from the first matched file line is re-applied to new_str.
    let needle_lines_collapsed: Vec<String> = needle
        .lines()
        .map(|l| l.split_whitespace().collect::<Vec<_>>().join(" "))
        .collect();
    let n5 = needle_lines_collapsed.len();

    if n5 > 0 && needle_lines_collapsed.iter().any(|l| !l.is_empty()) {
        let mut cws_matches: Vec<usize> = Vec::new();
        'cws: for start_idx in 0..=content_lines.len().saturating_sub(n5) {
            for i in 0..n5 {
                let file_collapsed = content_lines
                    .get(start_idx + i)
                    .copied()
                    .unwrap_or("")
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                if file_collapsed != needle_lines_collapsed[i] {
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
                let indent = leading_ws(content_lines[start_idx]);
                let effective_new = if indent.is_empty() {
                    new_str.to_string()
                } else {
                    reindent_with(new_str, indent)
                };
                return Ok((orig_start, orig_end, effective_new));
            }
            count => return Err(FindError::MultipleMatches(count)),
        }
    }

    Err(FindError::NotFound)
}

/// Remove all `\r` characters (handles both `\r\n` and lone `\r`).
fn strip_cr(s: &str) -> String {
    s.chars().filter(|&c| c != '\r').collect()
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
pub(crate) fn byte_offset_of_line(s: &str, line_idx: usize) -> usize {
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

/// Prepend `indent` to every line of `s`, preserving the trailing newline if present.
fn reindent_with(s: &str, indent: &str) -> String {
    let mut result = s
        .lines()
        .map(|l| format!("{}{}", indent, l))
        .collect::<Vec<_>>()
        .join("\n");
    if s.ends_with('\n') {
        result.push('\n');
    }
    result
}

fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
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
        // File has CRLF; LLM generates LF needle.
        let c = "fn foo() {\r\n    bar\r\n}\r\n";
        let needle = "fn foo() {\n    bar\n}\n";
        let (s, e) = check(c, needle);
        assert_eq!(&c[s..e], "fn foo() {\r\n    bar\r\n}\r\n");
    }

    #[test]
    fn trailing_whitespace_match() {
        // File has trailing spaces; LLM omits them.
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
        // Should span the full original content
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
        // Simulates LLM reading "281  registry.register(A);\n282  registry.register(B);"
        // and writing old_str without the 4-space code indentation.
        let c = "fn setup() {\n    registry.register(A);\n    registry.register(B);\n}\n";
        let needle = "registry.register(A);\nregistry.register(B);\n"; // no leading spaces
        let new_str = "registry.register(A);\nregistry.register(C);\n"; // no leading spaces
        let (s, e, effective) = check_with_new(c, needle, new_str);
        // Should span both register lines including their indentation
        assert_eq!(
            &c[s..e],
            "    registry.register(A);\n    registry.register(B);\n"
        );
        // new_str should have the 4-space indent re-applied to each line
        assert_eq!(
            effective,
            "    registry.register(A);\n    registry.register(C);\n"
        );
    }

    #[test]
    fn leading_whitespace_match_preserves_relative_indent() {
        // The LLM drops the common 4-space indent but keeps relative indent within the block.
        let c = "    fn foo() {\n        let x = 1;\n    }\n";
        let needle = "fn foo() {\n    let x = 1;\n}\n"; // 4-space common indent dropped
        let new_str = "fn foo() {\n    let x = 2;\n}\n";
        let (s, e, effective) = check_with_new(c, needle, new_str);
        assert_eq!(&c[s..e], "    fn foo() {\n        let x = 1;\n    }\n");
        assert_eq!(effective, "    fn foo() {\n        let x = 2;\n    }\n");
    }

    #[test]
    fn collapsed_whitespace_match() {
        // File uses tab indentation AND has extra internal spaces (e.g. formatted alignment).
        // LLM wrote spaces for indent and single spaces internally — both differ from file.
        let c = "\tlet  x  =  1;\n\tlet  y  =  2;\n";
        let needle = "let x = 1;\nlet y = 2;\n"; // spaces for indent, single internal spaces
        let new_str = "let x = 1;\nlet y = 99;\n";
        let (s, e, effective) = check_with_new(c, needle, new_str);
        // Should span both original lines (with tabs and extra spaces)
        assert_eq!(&c[s..e], "\tlet  x  =  1;\n\tlet  y  =  2;\n");
        // new_str should get the tab indent re-applied from the first matched line
        assert_eq!(effective, "\tlet x = 1;\n\tlet y = 99;\n");
    }
}
