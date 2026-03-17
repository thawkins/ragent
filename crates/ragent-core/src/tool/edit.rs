//! Surgical text replacement tool for file editing.
//!
//! Provides [`EditTool`], which replaces exactly one occurrence of a search
//! string with a replacement string in a file, ensuring precise edits.
//!
//! Matching is attempted in three passes to handle common LLM output quirks:
//!
//! 1. **Exact match** – the fastest and most precise path.
//! 2. **CRLF-normalised match** – handles files with `\r\n` line endings when
//!    the LLM generates `\n`-only search strings (because the `read` tool
//!    strips `\r` via `.lines()`).
//! 3. **Trailing-whitespace-stripped match** – handles lines where the file has
//!    trailing spaces/tabs that the LLM silently omitted in `old_str`.

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

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let path_str = input["path"].as_str().context("Missing required 'path' parameter")?;
        let old_str = input["old_str"]
            .as_str()
            .context("Missing required 'old_str' parameter")?;
        let new_str = input["new_str"]
            .as_str()
            .context("Missing required 'new_str' parameter")?;

        let path = resolve_path(&ctx.working_dir, path_str);

        let content = tokio::fs::read_to_string(&path)
            .await
            .with_context(|| format!("Cannot read file '{}': file may not exist or is not accessible", path.display()))?;

        let (start, end) = match find_replacement_range(&content, old_str) {
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

        let new_content = format!("{}{}{}", &content[..start], new_str, &content[end..]);
        tokio::fs::write(&path, &new_content)
            .await
            .with_context(|| format!("Failed to write file: {}", path.display()))?;

        let old_lines = old_str.lines().count();
        let new_lines = new_str.lines().count();
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
/// should be replaced.  Three passes are attempted in order:
///
/// 1. **Exact** – raw substring search.
/// 2. **CRLF-normalised** – strip `\r` from both sides, then map the match
///    position back to the original bytes via character-level tracking.
/// 3. **Trailing-whitespace-stripped** – strip trailing spaces/tabs from every
///    line in both sides, find the match by line number, then return the
///    corresponding span in the original (including original trailing whitespace
///    so it is replaced cleanly).
pub(crate) fn find_replacement_range(content: &str, needle: &str) -> Result<(usize, usize), FindError> {
    // ── Pass 1: exact ────────────────────────────────────────────────────────
    let exact_count = content.matches(needle).count();
    if exact_count == 1 {
        let start = content.find(needle).unwrap();
        return Ok((start, start + needle.len()));
    }
    if exact_count > 1 {
        return Err(FindError::MultipleMatches(exact_count));
    }

    // ── Pass 2: CRLF normalisation ───────────────────────────────────────────
    let norm_content = strip_cr(content);
    let norm_needle  = strip_cr(needle);
    let crlf_count   = norm_content.matches(norm_needle.as_str()).count();
    if crlf_count == 1 {
        let norm_start = norm_content.find(norm_needle.as_str()).unwrap();
        let norm_end   = norm_start + norm_needle.len();
        // Map normalised byte offsets back to positions in the original string.
        let start = norm_to_orig_byte(content, norm_start);
        let end   = norm_to_orig_byte(content, norm_end);
        return Ok((start, end));
    }
    if crlf_count > 1 {
        return Err(FindError::MultipleMatches(crlf_count));
    }

    // ── Pass 3: trailing-whitespace stripping ────────────────────────────────
    // Normalise both sides: strip_cr first so .lines() sees clean \n endings.
    let ws_content = strip_trailing_ws(&norm_content);
    let ws_needle  = strip_trailing_ws(&norm_needle);
    if ws_needle.is_empty() {
        return Err(FindError::NotFound);
    }
    let ws_count = ws_content.matches(ws_needle.as_str()).count();
    if ws_count == 1 {
        let ws_start = ws_content.find(ws_needle.as_str()).unwrap();
        // Determine which lines of the normalised (CRLF-stripped) content are covered.
        let start_line = ws_content[..ws_start].chars().filter(|&c| c == '\n').count();
        let needle_line_count = ws_needle.lines().count();
        let end_line = start_line + needle_line_count; // first line NOT in the match

        let orig_start = byte_offset_of_line(content, start_line);
        let orig_end   = byte_offset_of_line(content, end_line);
        return Ok((orig_start, orig_end));
    }
    if ws_count > 1 {
        return Err(FindError::MultipleMatches(ws_count));
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
        find_replacement_range(content, needle).expect("should find match")
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
            find_replacement_range(c, "goodbye"),
            Err(FindError::NotFound)
        ));
    }

    #[test]
    fn multiple_matches_returns_err() {
        let c = "foo\nfoo\n";
        assert!(matches!(
            find_replacement_range(c, "foo"),
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
}

