//! Batch text replacement tool for editing multiple files.
//!
//! Provides [`MultiEditTool`], which applies multiple search-and-replace
//! operations across one or more files atomically. All edits are validated
//! before any files are written — if any match fails, no files are modified.
//!
//! # Ordering & overlap safety (WSPLAN Milestone 3)
//!
//! Edits targeting the same file are resolved against the **original** file
//! content to produce absolute byte ranges, checked for overlap, and then
//! applied from the highest end-offset to the lowest so that earlier edits'
//! offsets remain stable regardless of the JSON input order. Overlapping
//! edits on the same file produce a clear error naming the edit indices and
//! file path.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

use super::replace::{FindDiag, FindDiagKind, find_replacement_range_diag};
use super::{Tool, ToolContext, ToolOutput};

/// Applies multiple search-and-replace edits across one or more files atomically.
///
/// Each edit specifies a file path, an exact search string, and its replacement.
/// All edits are validated first (each `old_str` must match exactly once in its
/// target file). Only after all validations pass are the files written. If any
/// edit fails validation, no files are modified.
pub struct MultiEditTool;

/// A single edit operation parsed from the input JSON.
struct EditOp {
    path: PathBuf,
    old_str: String,
    new_str: String,
}

/// A resolved edit: the original input index, the byte range against the
/// original file content, and the effective replacement text (which may have
/// indentation re-applied by the shared matcher).
struct ResolvedEdit {
    /// Index of this edit in the original JSON `edits` array (for diagnostics).
    input_index: usize,
    /// Inclusive start byte offset against the original file content.
    start: usize,
    /// Exclusive end byte offset against the original file content.
    end: usize,
    /// Effective replacement text (may differ from `new_str` when indentation
    /// was re-applied by a leading-whitespace or collapsed-whitespace match).
    effective_new: String,
    /// Original `old_str` line count (for stats).
    old_lines: usize,
    /// Effective new line count (for stats).
    new_lines: usize,
}

#[async_trait::async_trait]
impl Tool for MultiEditTool {
    fn name(&self) -> &'static str {
        "multiedit"
    }

    /// # Errors
    ///
    /// Returns an error if the `edits` array is missing, malformed, or empty.
    fn description(&self) -> &'static str {
        "Apply multiple edits to one or more files atomically. Each edit replaces \
         exactly one occurrence of old_str with new_str. All edits are validated \
         before any files are written — if any match fails, no files are modified. \
         Edits to the same file are overlap-checked and applied highest-offset-first \
         so input order does not matter."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "edits": {
                    "type": "array",
                    "description": "Array of edit operations to apply",
                    "items": {
                        "type": "object",
                        "properties": {
                            "path": {
                                "type": "string",
                                "description": "Path to the file to edit"
                            },
                            "old_str": {
                                "type": "string",
                                "description": "Exact string to find (must match exactly once)"
                            },
                            "new_str": {
                                "type": "string",
                                "description": "Replacement string"
                            }
                        },
                        "required": ["path", "old_str", "new_str"]
                    }
                }
            },
            "required": ["edits"]
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    /// Executes all edits atomically.
    ///
    /// # Errors
    ///
    /// Returns an error if the `edits` array is missing or empty, if any file
    /// cannot be read, if any `old_str` does not match exactly once in its
    /// target file, or if two edits on the same file overlap.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let edits_arr = input["edits"]
            .as_array()
            .context("Missing required 'edits' array parameter")?;

        if edits_arr.is_empty() {
            bail!("The 'edits' array is empty. Provide at least one edit operation.");
        }

        // Parse all edit operations.
        let mut ops: Vec<EditOp> = Vec::with_capacity(edits_arr.len());
        for (i, edit) in edits_arr.iter().enumerate() {
            let path_str = edit["path"]
                .as_str()
                .with_context(|| format!("Edit {i}: missing 'path'"))?;
            let old_str = edit["old_str"]
                .as_str()
                .with_context(|| format!("Edit {i}: missing 'old_str'"))?;
            let new_str = edit["new_str"]
                .as_str()
                .with_context(|| format!("Edit {i}: missing 'new_str'"))?;

            ops.push(EditOp {
                path: resolve_path(&ctx.working_dir, path_str),
                old_str: old_str.to_string(),
                new_str: new_str.to_string(),
            });
        }

        // Collect unique paths and acquire locks in sorted order to prevent deadlocks.
        let mut unique_paths: Vec<PathBuf> = ops.iter().map(|op| op.path.clone()).collect();
        unique_paths.sort();
        unique_paths.dedup();

        // Acquire all file locks before reading/writing.
        let mut _locks = Vec::new();
        for path in &unique_paths {
            _locks.push(super::file_lock::lock_file(path).await);
        }

        // Phase 1: Read all target files once. Each edit is resolved against
        // this ORIGINAL content so byte ranges are stable and comparable.
        let mut file_contents: HashMap<PathBuf, String> = HashMap::new();
        for path in &unique_paths {
            let content = tokio::fs::read_to_string(path)
                .await
                .with_context(|| format!("Failed to read file: {}", path.display()))?;
            file_contents.insert(path.clone(), content);
        }

        // Phase 2: Resolve every edit against the original file content and
        // group resolved edits by file path.
        let mut resolved_by_file: HashMap<PathBuf, Vec<ResolvedEdit>> = HashMap::new();
        for (i, op) in ops.iter().enumerate() {
            let original = file_contents
                .get(&op.path)
                .expect("file content must exist for every op path");

            let (start, end, effective_new) =
                match find_replacement_range_diag(original, &op.old_str, &op.new_str) {
                    Ok(range) => range,
                    Err(diag) => bail!(format_diag_error(&diag, i, &op.path)),
                };

            let old_lines = op.old_str.lines().count();
            let new_lines = effective_new.lines().count();

            resolved_by_file
                .entry(op.path.clone())
                .or_default()
                .push(ResolvedEdit {
                    input_index: i,
                    start,
                    end,
                    effective_new,
                    old_lines,
                    new_lines,
                });
        }

        // Phase 3: Overlap detection. For each file, verify no two resolved
        // edits' byte ranges (against the original content) intersect. Ranges
        // that merely touch (a.end == b.start) are allowed.
        for (path, edits) in &resolved_by_file {
            for a in 0..edits.len() {
                for b in (a + 1)..edits.len() {
                    let ea = &edits[a];
                    let eb = &edits[b];
                    if ea.start < eb.end && eb.start < ea.end {
                        bail!(
                            "Edits {} and {} overlap in {} (bytes {}-{} and {}-{}). \
                             Merge them into a single edit or remove one.",
                            ea.input_index,
                            eb.input_index,
                            path.display(),
                            ea.start,
                            ea.end,
                            eb.start,
                            eb.end
                        );
                    }
                }
            }
        }

        // Phase 4: Apply edits per file, highest end-offset first so that
        // earlier (lower-offset) edits' ranges remain valid against the
        // in-memory content as it grows or shrinks. This makes the JSON input
        // order irrelevant for non-overlapping edits.
        struct FileStats {
            edits: usize,
            added: usize,
            removed: usize,
        }
        let mut file_stats: HashMap<PathBuf, FileStats> = HashMap::new();
        let mut total_edits = 0usize;
        let mut total_added = 0usize;
        let mut total_removed = 0usize;

        for (path, mut edits) in resolved_by_file {
            // Sort end-to-start (descending by end, tie-break by start desc).
            edits.sort_by(|a, b| b.end.cmp(&a.end).then(b.start.cmp(&a.start)));

            let content = file_contents
                .get_mut(&path)
                .expect("file content must exist");

            for edit in &edits {
                *content = format!(
                    "{}{}{}",
                    &content[..edit.start],
                    edit.effective_new,
                    &content[edit.end..]
                );
            }

            let edits_count = edits.len();
            let added: usize = edits.iter().map(|e| e.new_lines).sum();
            let removed: usize = edits.iter().map(|e| e.old_lines).sum();

            file_stats.insert(
                path,
                FileStats {
                    edits: edits_count,
                    added,
                    removed,
                },
            );
            total_edits += edits_count;
            total_added += added;
            total_removed += removed;
        }

        // Phase 5: Write all modified files.
        for (path, content) in &file_contents {
            if file_stats.contains_key(path) {
                tokio::fs::write(path, content)
                    .await
                    .with_context(|| format!("Failed to write file: {}", path.display()))?;
            }
        }

        let file_count = file_stats.len();

        // Build per-file stats array sorted by path for stable display order.
        let mut sorted_paths: Vec<&PathBuf> = file_stats.keys().collect();
        sorted_paths.sort();
        let per_file: Vec<serde_json::Value> = sorted_paths
            .iter()
            .map(|p| {
                let s = &file_stats[*p];
                json!({
                    "path": p.to_string_lossy(),
                    "edits": s.edits,
                    "added": s.added,
                    "removed": s.removed,
                })
            })
            .collect();

        let summary = format!(
            "Applied {} edit{} across {} file{}",
            total_edits,
            if total_edits == 1 { "" } else { "s" },
            file_count,
            if file_count == 1 { "" } else { "s" },
        );

        Ok(ToolOutput {
            content: summary,
            metadata: Some(json!({
                "file_count": file_count,
                "edits": total_edits,
                "lines_added": total_added,
                "lines_removed": total_removed,
                "file_stats": per_file,
            })),
        })
    }
}

/// Format a [`FindDiag`] into a human-readable, actionable error string that
/// names the edit index, file path, the matching pass that failed, and — when
/// known — the 0-based line number of the closest near-match attempt
/// (WSPLAN M3-T4).
fn format_diag_error(diag: &FindDiag, edit_index: usize, path: &Path) -> String {
    let pass = diag.pass;
    match diag.kind {
        FindDiagKind::NotFound => {
            let line_hint = diag
                .closest_line
                .map(|l| format!(" (closest near-match attempt at line {})", l + 1))
                .unwrap_or_default();
            format!(
                "Edit {}: old_str not found in {} (last matching pass: `{}`). \
                 Make sure it matches exactly.{}",
                edit_index,
                path.display(),
                pass,
                line_hint
            )
        }
        FindDiagKind::MultipleMatches(n) => format!(
            "Edit {}: old_str found {} times in {} (last matching pass: `{}`). \
             It must match exactly once. Add more context to make it unique.",
            edit_index,
            n,
            path.display(),
            pass
        ),
    }
}

/// Resolves a path relative to the working directory, or returns it as-is if absolute.
fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}

// ── Unit tests ───────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_path_relative() {
        let p = resolve_path(Path::new("/work"), "src/main.rs");
        assert_eq!(p, PathBuf::from("/work/src/main.rs"));
    }

    #[test]
    fn resolve_path_absolute() {
        let p = resolve_path(Path::new("/work"), "/etc/hosts");
        assert_eq!(p, PathBuf::from("/etc/hosts"));
    }

    #[test]
    fn format_diag_error_not_found_includes_pass_and_line() {
        let diag = FindDiag::not_found("collapsed", Some(4));
        let msg = format_diag_error(&diag, 2, Path::new("/tmp/foo.rs"));
        assert!(msg.contains("Edit 2"));
        assert!(msg.contains("/tmp/foo.rs"));
        assert!(msg.contains("`collapsed`"));
        assert!(msg.contains("line 5"));
    }

    #[test]
    fn format_diag_error_multiple_includes_count() {
        let diag = FindDiag::multiple("exact", 3, None);
        let msg = format_diag_error(&diag, 1, Path::new("/tmp/bar.rs"));
        assert!(msg.contains("Edit 1"));
        assert!(msg.contains("3 times"));
        assert!(msg.contains("`exact`"));
    }
}
