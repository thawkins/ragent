//! Atomic batch text replacement tool for editing multiple files.
//!
//! Provides [`MultiEditTool`], which applies multiple search-and-replace
//! operations across one or more files atomically. All edits are validated
//! before any files are written — if any match fails, no files are modified.
//!
//! # Matching (editrenewal FR-004 / FR-009, amended)
//!
//! Each edit first attempts **strict exact-match** replacement
//! ([`find_exact_replacement_range`]). If the strict match fails with
//! `NotFound`, a controlled batch normalization fallback strips CRLF and
//! trailing whitespace per line and tries again. Indentation and internal
//! whitespace are preserved so batch edits remain deterministic and easy to
//! reason about. If the fallback also fails, the edit is rejected and no files
//! are modified.
//!
//! # Dry-run mode
//!
//! Pass `"dry_run": true` to validate every edit and preview the changes
//! without writing any files.
//!
//! # Parameter names (editrenewal FR-009)
//!
//! Each edit object accepts the canonical parameter names `file_path`,
//! `old_string`, and `new_string`. For backward compatibility during the
//! deprecation window, the legacy names `path`, `old_str`, and `new_str` are
//! still accepted and normalised to the canonical names before execution.
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
use std::time::SystemTime;

use super::path_util::resolve_path;
use super::replace::{
    FindDiag, FindError, find_batch_normalized_replacement_range, find_exact_replacement_range,
    format_match_failure,
};
use super::{Tool, ToolContext, ToolOutput};

/// Applies multiple search-and-replace edits across one or more files atomically.
///
/// Each edit specifies a file path, an exact search string, and its replacement.
/// All edits are validated first (each `old_string` must match exactly once in
/// its target file). Only after all validations pass are the files written. If
/// any edit fails validation, no files are modified.
///
/// `#[allow(dead_code)]` — the type is registered and used by the lib target,
/// but it is never directly constructed by the external integration test target
/// that re-imports this source via `#[path]`.
#[allow(dead_code)]
pub struct MultiEditTool;
// `MultiEditTool` is constructed and registered via
// `crates/ragent-tools-core/src/lib.rs`. The "never constructed" warning
// appears only because the integration test target re-imports this source
// file via `#[path]` and compiles a fresh copy that is not wired into a
// registry.
/// A single edit operation parsed from the input JSON.
///
/// `#[allow(dead_code)]` — used by the lib build but not by the test target that
/// re-imports this source via `#[path]`.
#[allow(dead_code)]
struct EditOp {
    path: PathBuf,
    old_str: String,
    new_str: String,
}

/// A resolved edit: the original input index, the byte range against the
/// original file content, and the effective replacement text (which may have
/// indentation re-applied by the shared matcher).
///
/// `#[allow(dead_code)]` — used by the lib build but not by the test target that
/// re-imports this source via `#[path]`.
#[allow(dead_code)]
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
        "multi_edit"
    }

    /// # Errors
    ///
    /// Returns an error if the `edits` array is missing, malformed, or empty.
    fn description(&self) -> &'static str {
        "Apply multiple surgical text edits to one or more files atomically. \
         Required parameter: `edits` (array of edit objects). Each edit object \
         must provide `file_path` (string), `old_string` (string), and \
         `new_string` (string). Every edit must match exactly once in its file; \
         if any single edit fails validation, no files are modified. Edits to \
         the same file are overlap-checked and applied highest-offset-first, so \
         input order does not matter. Legacy aliases `path`/`old_str`/`new_str` \
         are accepted inside each edit object but deprecated."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "edits": {
                    "type": "array",
                    "description": "REQUIRED. Array of edit operations to apply. Each edit is a single-instance exact replacement.",
                    "items": {
                        "type": "object",
                        "properties": {
                            "file_path": {
                                "type": "string",
                                "description": "REQUIRED. Absolute path to the file to edit"
                            },
                            "old_string": {
                                "type": "string",
                                "description": "REQUIRED. String to find (must match exactly once; CRLF and trailing-whitespace differences are tolerated after strict exact match fails)"
                            },
                            "new_string": {
                                "type": "string",
                                "description": "REQUIRED. Replacement string"
                            },
                            "path": {
                                "type": "string",
                                "description": "Legacy alias for file_path (deprecated)"
                            },
                            "old_str": {
                                "type": "string",
                                "description": "Legacy alias for old_string (deprecated)"
                            },
                            "new_str": {
                                "type": "string",
                                "description": "Legacy alias for new_string (deprecated)"
                            }
                        },
                        "required": ["file_path", "old_string", "new_string"],
                        "additionalProperties": false
                    }
                }
            },
            "required": ["edits"],
            "additionalProperties": false
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
        let dry_run = input
            .get("dry_run")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        let edits_arr = input["edits"]
            .as_array()
            .context("Missing required 'edits' array parameter")?;

        if edits_arr.is_empty() {
            bail!("The 'edits' array is empty. Provide at least one edit operation.");
        }

        // Parse all edit operations. Accept the canonical parameter names
        // (file_path, old_string, new_string) and the legacy names
        // (path, old_str, new_str) for backward compatibility (editrenewal
        // FR-009 / FR-012).
        let mut ops: Vec<EditOp> = Vec::with_capacity(edits_arr.len());
        for (i, edit) in edits_arr.iter().enumerate() {
            let path_str = edit["file_path"]
                .as_str()
                .or_else(|| edit["path"].as_str())
                .with_context(|| format!("Edit {i}: missing 'file_path' (or legacy 'path')"))?;
            let old_str = edit["old_string"]
                .as_str()
                .or_else(|| edit["old_str"].as_str())
                .with_context(|| format!("Edit {i}: missing 'old_string' (or legacy 'old_str')"))?;
            let new_str = edit["new_string"]
                .as_str()
                .or_else(|| edit["new_str"].as_str())
                .with_context(|| format!("Edit {i}: missing 'new_string' (or legacy 'new_str')"))?;

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

        // Phase 1b: Stale-file detection (editrenewal FR-003 / FR-009).
        // For every target file that the session has recorded a read
        // timestamp for, reject the batch if the file was modified after it
        // was read. Files with no recorded timestamp proceed (no baseline).
        for path in &unique_paths {
            if let Err(e) = check_stale_file(path, ctx) {
                bail!("{e}");
            }
        }
        // Phase 2: Resolve every edit against the original file content and
        // group resolved edits by file path. Uses the strict exact-match
        // matcher first, then a controlled CRLF/trailing-whitespace fallback
        // if the strict match returns NotFound.
        let mut resolved_by_file: HashMap<PathBuf, Vec<ResolvedEdit>> = HashMap::new();
        for (i, op) in ops.iter().enumerate() {
            let original = file_contents
                .get(&op.path)
                .expect("file content must exist for every op path");

            let (start, end, effective_new) =
                resolve_batch_edit(original, &op.old_str, &op.new_str).map_err(|diag| {
                    anyhow::anyhow!("Edit {}: {}", i, format_match_failure(&diag, &op.path))
                })?;

            let old_lines = original[start..end].lines().count().max(1);
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

        // Phase 5: Write all modified files (skipped in dry-run mode).
        if !dry_run {
            for (path, content) in &file_contents {
                if file_stats.contains_key(path) {
                    tokio::fs::write(path, content)
                        .await
                        .with_context(|| format!("Failed to write file: {}", path.display()))?;
                    // Refresh the read timestamp for this file so a follow-up
                    // edit in the same session does not trip the stale-file
                    // check on a file we just wrote (editrenewal FR-003).
                    record_edit_timestamp(path, ctx);
                }
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
            "{} {} edit{} across {} file{}",
            if dry_run { "Would apply" } else { "Applied" },
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
                "dry_run": dry_run,
                "lines_added": total_added,
                "lines_removed": total_removed,
                "file_stats": per_file,
            })),
        })
    }
}

/// Resolve a single batch edit against the original file content.
///
/// First tries a strict exact match. If that returns [`FindError::NotFound`],
/// falls back to the controlled batch normalization (CRLF → LF, trailing
/// whitespace stripped per line). On failure returns a [`FindDiag`] so the
/// caller can produce an actionable error message.
pub fn resolve_batch_edit(
    content: &str,
    old_str: &str,
    new_str: &str,
) -> Result<(usize, usize, String), FindDiag> {
    match find_exact_replacement_range(content, old_str, new_str) {
        Ok(range) => return Ok(range),
        Err(FindError::MultipleMatches(n)) => {
            return Err(FindDiag::multiple("exact", n, None));
        }
        Err(FindError::NotFound) => {}
    }

    match find_batch_normalized_replacement_range(content, old_str, new_str) {
        Ok(range) => Ok(range),
        Err(FindError::MultipleMatches(n)) => Err(FindDiag::multiple("batch-normalized", n, None)),
        Err(FindError::NotFound) => Err(FindDiag::not_found("batch-normalized", None)),
    }
}

/// Check whether the file was modified after the session last read it
/// (editrenewal FR-003 / FR-009). When a read timestamp has been recorded for
/// `path`, compare the current on-disk mtime against it and return an error if
/// the file is newer. When no timestamp has been recorded, the check is a
/// no-op (no baseline available).
///
/// `#[allow(dead_code)]` — used by the lib build but not by the test target that
/// re-imports this source via `#[path]`.
#[allow(dead_code)]
fn check_stale_file(path: &Path, ctx: &ToolContext) -> Result<()> {
    let recorded = ctx
        .read_timestamps
        .read()
        .ok()
        .and_then(|map| map.get(path).copied());

    let Some(recorded_millis) = recorded else {
        return Ok(());
    };

    let current_millis = std::fs::metadata(path)
        .ok()
        .and_then(|m| m.modified().ok())
        .map(|mtime| {
            mtime
                .duration_since(SystemTime::UNIX_EPOCH)
                .map_or(0, |d| d.as_millis() as u64)
        });

    let Some(current_millis) = current_millis else {
        return Ok(());
    };

    // 1ms tolerance for filesystem mtime granularity.
    if current_millis > recorded_millis.saturating_add(1) {
        bail!(
            "File '{}' was modified after it was last read by this session \
             (read mtime {}ms, current mtime {}ms). Re-read the file before \
             editing to avoid clobbering external changes.",
            path.display(),
            recorded_millis,
            current_millis
        );
    }

    Ok(())
}

/// Record (or refresh) the edit timestamp for `path` so a follow-up edit in
/// the same session does not trip the stale-file check on a file we just
/// wrote.
///
/// `#[allow(dead_code)]` — used by the lib build but not by the test target that
/// re-imports this source via `#[path]`.
#[allow(dead_code)]
fn record_edit_timestamp(path: &Path, ctx: &ToolContext) {
    if let Ok(meta) = std::fs::metadata(path)
        && let Ok(mtime) = meta.modified()
    {
        let millis = mtime
            .duration_since(SystemTime::UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        if let Ok(mut map) = ctx.read_timestamps.write() {
            map.insert(path.to_path_buf(), millis);
        }
    }
}

// ── Unit tests ───────────────────────────────────────────────────────────────
