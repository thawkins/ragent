//! Unified diff patch tool for applying patches to files.
//!
//! Provides [`PatchTool`], which accepts a unified diff string (as produced by
//! `diff -u` or `git diff`) and applies it to the target file(s). All hunks
//! are validated before any files are written — if any hunk fails to match,
//! no files are modified.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::{Tool, ToolContext, ToolOutput};

pub struct PatchTool;

#[async_trait::async_trait]
impl Tool for PatchTool {
    fn name(&self) -> &str {
        "patch"
    }

    fn description(&self) -> &str {
        "Apply a unified diff patch to one or more files. The patch must be in \
         unified diff format (as produced by `diff -u` or `git diff`). All hunks \
         are validated before any files are written."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "patch": {
                    "type": "string",
                    "description": "Unified diff content to apply"
                },
                "path": {
                    "type": "string",
                    "description": "Optional: override the target file path (for single-file patches)"
                },
                "fuzz": {
                    "type": "integer",
                    "description": "Number of context lines that may be dropped from \
                                    the top/bottom of each hunk when matching (default: 0)"
                }
            },
            "required": ["patch"]
        })
    }

    fn permission_category(&self) -> &str {
        "file:write"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let patch_str = input["patch"]
            .as_str()
            .context("Missing 'patch' parameter")?;
        let path_override = input["path"].as_str();
        let fuzz = input["fuzz"].as_u64().unwrap_or(0) as usize;

        let file_patches = parse_unified_diff(patch_str)?;

        if file_patches.is_empty() {
            bail!("No hunks found in the patch");
        }

        // Phase 1: Read all files and validate all hunks
        let mut file_results: HashMap<PathBuf, String> = HashMap::new();
        let mut total_hunks = 0usize;
        let mut total_lines_changed = 0usize;

        for fp in &file_patches {
            let target = if let Some(ov) = path_override {
                resolve_path(&ctx.working_dir, ov)
            } else {
                resolve_path(&ctx.working_dir, &fp.path)
            };

            let content = tokio::fs::read_to_string(&target)
                .await
                .with_context(|| format!("Failed to read file: {}", target.display()))?;

            let mut lines: Vec<String> = content.lines().map(String::from).collect();

            // Apply hunks in reverse order so earlier hunks don't shift line numbers
            // for later ones.
            let mut sorted_hunks: Vec<&Hunk> = fp.hunks.iter().collect();
            sorted_hunks.sort_by(|a, b| b.old_start.cmp(&a.old_start));

            for (i, hunk) in sorted_hunks.iter().enumerate() {
                lines = apply_hunk(&lines, hunk, fuzz).with_context(|| {
                    format!(
                        "Hunk {} failed to apply in {} at line {}",
                        fp.hunks.len() - i,
                        target.display(),
                        hunk.old_start,
                    )
                })?;
            }

            let new_content = if lines.is_empty() {
                String::new()
            } else {
                let mut s = lines.join("\n");
                // Preserve trailing newline if original had one
                if content.ends_with('\n') {
                    s.push('\n');
                }
                s
            };

            total_hunks += fp.hunks.len();
            total_lines_changed += new_content.lines().count();
            file_results.insert(target, new_content);
        }

        // Phase 2: Write all files
        for (path, content) in &file_results {
            tokio::fs::write(path, content)
                .await
                .with_context(|| format!("Failed to write file: {}", path.display()))?;
        }

        let file_count = file_results.len();
        let summary = format!(
            "Applied {} hunk{} across {} file{}",
            total_hunks,
            if total_hunks == 1 { "" } else { "s" },
            file_count,
            if file_count == 1 { "" } else { "s" },
        );

        Ok(ToolOutput {
            content: summary,
            metadata: Some(json!({
                "files": file_count,
                "hunks": total_hunks,
                "lines": total_lines_changed,
            })),
        })
    }
}

// ── Unified diff parser ──────────────────────────────────────────

/// A parsed file-level patch containing one or more hunks.
#[derive(Debug)]
struct FilePatch {
    path: String,
    hunks: Vec<Hunk>,
}

/// A single hunk from a unified diff.
#[derive(Debug)]
struct Hunk {
    old_start: usize,
    old_lines: Vec<HunkLine>,
    new_lines: Vec<HunkLine>,
}

#[derive(Debug, Clone)]
enum HunkLine {
    Context(String),
    Remove(String),
    Add(String),
}

/// Parse a unified diff string into a list of per-file patches.
fn parse_unified_diff(patch: &str) -> Result<Vec<FilePatch>> {
    let lines: Vec<&str> = patch.lines().collect();
    let mut result: Vec<FilePatch> = Vec::new();
    let mut i = 0;

    while i < lines.len() {
        // Look for --- / +++ header pair
        if lines[i].starts_with("--- ") && i + 1 < lines.len() && lines[i + 1].starts_with("+++ ") {
            let target_line = lines[i + 1];
            let path = parse_file_path(target_line);
            i += 2;

            let mut hunks = Vec::new();

            // Parse all hunks for this file
            while i < lines.len() && lines[i].starts_with("@@ ") {
                let (hunk, consumed) = parse_hunk(&lines[i..])?;
                hunks.push(hunk);
                i += consumed;
            }

            if !hunks.is_empty() {
                result.push(FilePatch { path, hunks });
            }
        } else {
            i += 1;
        }
    }

    Ok(result)
}

/// Extract file path from a `+++ b/path` or `+++ path` line.
fn parse_file_path(line: &str) -> String {
    let raw = line.strip_prefix("+++ ").unwrap_or(line);
    // Strip git-style a/ or b/ prefix
    let path = raw
        .strip_prefix("b/")
        .or_else(|| raw.strip_prefix("a/"))
        .unwrap_or(raw);
    // Strip optional tab + timestamp suffix (e.g. from `diff -u`)
    path.split('\t').next().unwrap_or(path).to_string()
}

/// Parse a single hunk starting at the `@@` line.
/// Returns the parsed hunk and the number of lines consumed.
fn parse_hunk(lines: &[&str]) -> Result<(Hunk, usize)> {
    let header = lines[0];
    let (old_start, _old_count) = parse_hunk_header(header)?;

    let mut old_lines = Vec::new();
    let mut new_lines = Vec::new();
    let mut consumed = 1; // the @@ line

    for line in &lines[1..] {
        if line.starts_with("@@ ") || line.starts_with("--- ") || line.starts_with("+++ ") {
            break;
        }
        consumed += 1;

        if let Some(text) = line.strip_prefix('-') {
            let hl = HunkLine::Remove(text.to_string());
            old_lines.push(hl.clone());
            new_lines.push(hl);
        } else if let Some(text) = line.strip_prefix('+') {
            let hl = HunkLine::Add(text.to_string());
            old_lines.push(hl.clone());
            new_lines.push(hl);
        } else if let Some(text) = line.strip_prefix(' ') {
            let hl = HunkLine::Context(text.to_string());
            old_lines.push(hl.clone());
            new_lines.push(hl);
        } else if line.starts_with('\\') {
            // "\ No newline at end of file" — skip
        } else {
            // Treat bare line as context (some diffs omit the leading space)
            let hl = HunkLine::Context(line.to_string());
            old_lines.push(hl.clone());
            new_lines.push(hl);
        }
    }

    Ok((
        Hunk {
            old_start,
            old_lines,
            new_lines,
        },
        consumed,
    ))
}

/// Parse the `@@ -old_start,old_count +new_start,new_count @@` header.
fn parse_hunk_header(header: &str) -> Result<(usize, usize)> {
    // Format: @@ -start,count +start,count @@ optional text
    let inner = header
        .strip_prefix("@@ ")
        .and_then(|s| s.split(" @@").next())
        .context("Invalid hunk header")?;

    let parts: Vec<&str> = inner.split_whitespace().collect();
    if parts.len() < 2 {
        bail!("Invalid hunk header: {}", header);
    }

    let old_part = parts[0]
        .strip_prefix('-')
        .context("Invalid old range in hunk header")?;
    let (old_start, old_count) = parse_range(old_part)?;

    Ok((old_start, old_count))
}

/// Parse `start,count` or just `start` (implied count = 1).
fn parse_range(s: &str) -> Result<(usize, usize)> {
    if let Some((start_s, count_s)) = s.split_once(',') {
        let start: usize = start_s.parse().context("Invalid range start")?;
        let count: usize = count_s.parse().context("Invalid range count")?;
        Ok((start, count))
    } else {
        let start: usize = s.parse().context("Invalid range")?;
        Ok((start, 1))
    }
}

// ── Hunk application ─────────────────────────────────────────────

/// Apply a single hunk to a set of file lines.
///
/// Uses the hunk's `old_start` as a hint, then searches nearby (within
/// `fuzz` context lines) for an exact match of the expected old content.
fn apply_hunk(file_lines: &[String], hunk: &Hunk, fuzz: usize) -> Result<Vec<String>> {
    // Build the expected "old" content and the "new" replacement content
    let old_content: Vec<&str> = hunk
        .old_lines
        .iter()
        .filter_map(|l| match l {
            HunkLine::Context(s) => Some(s.as_str()),
            HunkLine::Remove(s) => Some(s.as_str()),
            HunkLine::Add(_) => None,
        })
        .collect();

    let new_content: Vec<&str> = hunk
        .new_lines
        .iter()
        .filter_map(|l| match l {
            HunkLine::Context(s) => Some(s.as_str()),
            HunkLine::Add(s) => Some(s.as_str()),
            HunkLine::Remove(_) => None,
        })
        .collect();

    // Try exact position first (1-indexed), then search nearby
    let target_line = if hunk.old_start > 0 {
        hunk.old_start - 1
    } else {
        0
    };

    // Try with decreasing context (fuzz)
    for fuzz_level in 0..=fuzz {
        let trimmed_old = trim_context(&old_content, fuzz_level);
        let trimmed_new = trim_context(&new_content, fuzz_level);
        let context_offset = fuzz_level; // lines trimmed from top

        if let Some(pos) = find_match(file_lines, &trimmed_old, target_line) {
            let actual_pos = if fuzz_level > 0 {
                pos
            } else {
                pos
            };
            // Build result: lines before + new content + lines after
            let mut result = Vec::with_capacity(file_lines.len());
            result.extend(file_lines[..actual_pos].iter().cloned());
            for line in &trimmed_new {
                result.push(line.to_string());
            }
            let after_start = actual_pos + trimmed_old.len();
            if after_start <= file_lines.len() {
                result.extend(file_lines[after_start..].iter().cloned());
            }
            return Ok(result);
        }
    }

    bail!(
        "Could not find matching context at line {} ({} lines of old content)",
        hunk.old_start,
        old_content.len()
    );
}

/// Trim `n` context lines from the top and bottom of a slice.
fn trim_context<'a>(lines: &'a [&str], n: usize) -> Vec<&'a str> {
    if n == 0 || lines.len() <= 2 * n {
        return lines.to_vec();
    }
    lines[n..lines.len() - n].to_vec()
}

/// Search for an exact match of `needle` lines in `haystack`.
/// Start at `hint` and search outward.
fn find_match(haystack: &[String], needle: &[&str], hint: usize) -> Option<usize> {
    if needle.is_empty() {
        return Some(hint.min(haystack.len()));
    }
    if haystack.len() < needle.len() {
        return None;
    }

    let max_pos = haystack.len() - needle.len();
    let hint = hint.min(max_pos);

    // Try exact position first
    if matches_at(haystack, needle, hint) {
        return Some(hint);
    }

    // Search outward from hint
    for offset in 1..=max_pos {
        if hint + offset <= max_pos && matches_at(haystack, needle, hint + offset) {
            return Some(hint + offset);
        }
        if offset <= hint && matches_at(haystack, needle, hint - offset) {
            return Some(hint - offset);
        }
    }

    None
}

/// Check if `needle` matches `haystack` starting at position `pos`.
fn matches_at(haystack: &[String], needle: &[&str], pos: usize) -> bool {
    if pos + needle.len() > haystack.len() {
        return false;
    }
    needle
        .iter()
        .zip(&haystack[pos..])
        .all(|(n, h)| *n == h.as_str())
}

fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}
