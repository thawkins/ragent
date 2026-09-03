//! Surgical text replacement tool for file editing.
//!
//! Provides [`EditTool`], the single-file edit tool. It replaces exactly one
//! occurrence of `old_string` with `new_string` in a file using the strict
//! exact-byte matcher shared with `replace.rs` (historically also used by the
//! legacy `memory_replace` tool).
//!
//! # Parameter names (FR-001)
//!
//! The canonical parameter names are `file_path`, `old_string`, and
//! `new_string`. For backward compatibility during the deprecation window
//! (FR-012), the legacy names `path`, `old_str`, and `new_str` are also
//! accepted and normalised to the canonical names before execution.
//!
//! # Operations (FR-006)
//!
//! - **Update**: `old_string` non-empty, `new_string` non-empty → replace the
//!   unique match.
//! - **Delete**: `old_string` non-empty, `new_string` empty → remove the
//!   matched text.
//! - **Create**: `old_string` empty and the file does not exist → write
//!   `new_string` to a new file. Rejected if the file already exists.
//!
//! # Matching (editplan P2 — fallback cascade)
//!
//! When `collapse_whitespace` is false (the default) the matcher runs the
//! progressive fallback cascade in [`super::replace::find_replacement_cascade`]:
//!
//! 1. **Exact** — `old_string` must match exactly once, byte-for-byte.
//! 2. **Flexible** — when exact matching fails with not-found, every run of
//!    whitespace in the needle is matched against any non-empty run of
//!    whitespace in the file. A unique match wins.
//! 3. **Indent-normalised** — when both lanes above fail, per-line comparison
//!    with leading whitespace stripped; the replacement re-applies the file's
//!    own indentation.
//!
//! When `collapse_whitespace` is true, only the flexible lane runs (this is
//! the opt-in mode). `new_string` is inserted verbatim — never re-indented
//! or line-ending-normalised, except by the explicit indent-reapplication
//! rule of the indent-normalised fallback lane.
//!
//! The winning lane is recorded in the edit-log `match_lane` field (P4.12)
//! so future analyses can quantify which lane rescues each edit.
//!
//! # Stale-file detection (FR-003, amended by P1.3)
//!
//! When the session has recorded a read timestamp for `file_path` (via the
//! `read` tool), the edit tool compares the file's current mtime against that
//! recorded timestamp and rejects the edit if the file was modified after it
//! was read. On rejection the tool refreshes the recorded read timestamp
//! once (P1.3 retry-once) so the model's next attempt starts from the live
//! content. When no timestamp has been recorded, the edit proceeds (no
//! baseline is available).
//!
//! # Result snippet (FR-008)
//!
//! On success the tool returns a `cat -n`-style snippet of the edited file
//! with at least four lines of context before and after the change, clamped to
//! the file boundaries.
//!
//! # Dry-run mode
//!
//! Pass `"dry_run": true` to resolve the match and preview the change without
//! writing to disk. The response includes the same snippet metadata and a
//! `dry_run` flag.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::Path;

use super::edit_common::{check_stale_file, record_edit_timestamp};
use super::edit_log::{EntryExtras, log_edit_operation, log_edit_operation_ex};
use super::path_util::resolve_path;
use super::replace::{
    CascadeFail, CascadeMatch, FindError, MatchLane, disambiguation_hint,
    find_flexible_replacement_range, find_replacement_cascade, format_match_failure, length_note,
};
use super::{
    Tool, ToolContext, ToolOutput, check_path_within_any_root, check_path_within_root_cached,
};

/// Minimum lines of context to show before and after the edited region in the
/// result snippet (FR-008).
const SNIPPET_CONTEXT_LINES: usize = 4;

/// Replaces an exact, unique occurrence of `old_string` with `new_string` in a
/// file using strict byte-for-byte matching.
///
/// The search string must match exactly once; zero or multiple matches are
/// treated as errors to prevent ambiguous edits. Supports create, update, and
/// delete operations via empty `old_string` / `new_string` (FR-006).
///
/// `#[allow(dead_code)]` — the type is registered and used by the lib target,
/// but it is never directly constructed by the external integration test target
/// that re-imports this source via `#[path]`.
#[allow(dead_code)]
pub struct EditTool;

#[async_trait::async_trait]
impl Tool for EditTool {
    fn name(&self) -> &'static str {
        "edit"
    }

    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &'static str {
        "Replace exactly one occurrence of `old_string` with `new_string` in a \
         single file. Required parameters: `file_path` (string), `old_string` \
         (string), and `new_string` (string). By default `old_string` must \
         match exactly once, byte-for-byte (indentation, whitespace, and line \
         endings must match precisely). If exact matching fails, a fallback \
         cascade retries with whitespace-flexible and indent-normalised \
         matching before erroring. Optional `collapse_whitespace` \
         (boolean, default false) relaxes matching: backslash escapes \
         (\\t, \\n, \\r, \\\\) in old_string are decoded and every run of \
         whitespace matches a non-empty run of whitespace in the file, so \
         collapsed indentation or alignment whitespace does not cause spurious \
         failures. Include 3–5 lines of context around \
         the change point so the match is unique; keep `old_string` under 20 \
         lines where possible. If your previous edit on this file succeeded, \
         treat your in-context copy as stale and re-read before composing the \
         next `old_string`. Use an empty `old_string` on \
         a non-existent file to create it; use an empty `new_string` to delete \
         the matched text. Optional: `dry_run` (boolean) previews the change \
         without writing. Legacy aliases `path`/`old_str`/`new_str` are accepted \
         but deprecated."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "REQUIRED. Absolute path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "REQUIRED. String to find and replace (must match exactly once in the file, byte-for-byte). Empty string creates a new file."
                },
                "new_string": {
                    "type": "string",
                    "description": "REQUIRED. Replacement string. Empty string deletes the matched text."
                },
                "dry_run": {
                    "type": "boolean",
                    "description": "If true, resolve the match and return a preview snippet without writing the file."
                },
                "collapse_whitespace": {
                    "type": "boolean",
                    "description": "If true, relax matching: backslash escapes (\\t, \\n, \\r, \\\\) in old_string are decoded and every whitespace run matches a non-empty whitespace run in the file. Default false (byte-for-byte exact).",
                    "default": false
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
        })
    }

    fn permission_category(&self) -> &'static str {
        "file:write"
    }

    /// Performs a surgical text replacement in a file.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The `file_path`, `old_string`, or `new_string` parameter is missing
    /// - The file cannot be read (file not found, permission denied, not UTF-8)
    /// - `old_string` is not found in the file (exact byte match, FR-004)
    /// - `old_string` matches multiple locations (FR-004, FR-005)
    /// - `old_string` and `new_string` are identical (FR-007)
    /// - A create is requested but the file already exists (FR-006)
    /// - The file was modified after it was read by the session (FR-003)
    /// - The file cannot be written after the edit
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        // ── Parameter extraction (canonical + legacy names) ───────────────────
        let path_str = input["file_path"]
            .as_str()
            .or_else(|| input["path"].as_str())
            .context("Missing required 'file_path' (or legacy 'path') parameter")?;
        let old_string = input["old_string"]
            .as_str()
            .or_else(|| input["old_str"].as_str())
            .context("Missing required 'old_string' (or legacy 'old_str') parameter")?;
        let new_string = input["new_string"]
            .as_str()
            .or_else(|| input["new_str"].as_str())
            .context("Missing required 'new_string' (or legacy 'new_str') parameter")?;

        let used_legacy_params =
            !input["path"].is_null() || !input["old_str"].is_null() || !input["new_str"].is_null();

        let dry_run = input
            .get("dry_run")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let collapse_whitespace = input
            .get("collapse_whitespace")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        let path = resolve_path(&ctx.working_dir, path_str);

        // C-002: edits must stay inside the allowed roots.
        // Use configured allowed_roots if available, otherwise fall back to working_dir.
        if ctx.allowed_roots.is_empty() {
            check_path_within_root_cached(&path, &ctx.working_dir, &ctx.canonical_cache)?;
        } else {
            let root_refs: Vec<&std::path::Path> =
                ctx.allowed_roots.iter().map(|p| p.as_path()).collect();
            check_path_within_any_root(&path, &root_refs)?;
        }

        // Acquire file lock to serialize concurrent edits to the same file.
        let _lock = super::file_lock::lock_file(&path).await;

        // ── Create operation (FR-006): empty old_string ───────────────────────
        if old_string.is_empty() {
            return create_file(&path, new_string, ctx, used_legacy_params, dry_run).await;
        }

        // ── No-change rejection (FR-007) ──────────────────────────────────────
        if old_string == new_string {
            let outcome = format!("no-change rejected in {}", path.display());
            log_edit_operation(
                &ctx.working_dir,
                "edit",
                &path,
                old_string,
                new_string,
                &outcome,
                dry_run,
            );
            bail!(
                "old_string and new_string are identical in {}. \
                 No changes would be made; refusing the no-op edit.",
                path.display()
            );
        }

        // ── Read the file ────────���────────────────────────────────────────────
        let content = tokio::fs::read_to_string(&path).await.with_context(|| {
            format!(
                "Cannot read file '{}': file may not exist or is not accessible",
                path.display()
            )
        });
        if let Err(ref e) = content {
            log_edit_operation(
                &ctx.working_dir,
                "edit",
                &path,
                old_string,
                new_string,
                &format!("read error: {e}"),
                dry_run,
            );
        }
        let content = content?;

        // ── Stale-file detection (FR-003) ─────────────────────────────────────
        if let Err(e) = check_stale_file(&path, ctx) {
            // P1.3: refresh the session's read timestamp once so the model's
            // next attempt starts from the live content instead of being
            // rejected again on the same file.
            record_edit_timestamp(&path, ctx);
            log_edit_operation_ex(
                &ctx.working_dir,
                &path,
                EntryExtras {
                    tool: "edit",
                    old_str: old_string,
                    new_str: new_string,
                    outcome: &format!("stale-file rejected: {e}"),
                    dry_run,
                    match_lane: None,
                    note: Some("stale retry-once: read timestamp refreshed"),
                },
            );
            bail!(
                "{}. The session's read timestamp has been refreshed — \
                 re-issue the edit against the live content (the file on \
                 disk is the new baseline).",
                e
            );
        }

        // ── Replacement (editplan P2: exact → flexible → indent-normalised) ──
        let (lane, start, end, new_str) = if collapse_whitespace {
            // Collapse-whitespace opt-in: run only the flexible lane.
            match find_flexible_replacement_range(&content, old_string, new_string) {
                Ok((s, e, ns)) => (MatchLane::Flexible, s, e, ns),
                Err(FindError::NotFound) => {
                    // P2.6: try the line-similarity hint before giving up.
                    let err =
                        super::replace::not_found_hint(&content, old_string, &path, None, true);
                    log_edit_operation_ex(
                        &ctx.working_dir,
                        &path,
                        EntryExtras {
                            tool: "edit",
                            old_str: old_string,
                            new_str: new_string,
                            outcome: &err,
                            dry_run,
                            match_lane: Some("not_found"),
                            note: length_note(old_string),
                        },
                    );
                    bail!(err);
                }
                Err(FindError::MultipleMatches(n)) => {
                    let decoded = super::replace::decode_escapes(old_string);
                    let starts: Vec<usize> = content
                        .match_indices(decoded.as_str())
                        .take(3)
                        .map(|(i, _)| i)
                        .collect();
                    let err = format!(
                        "{} (collapse_whitespace mode: escapes decoded, whitespace runs collapsed)\n{}",
                        format_match_failure(&super::replace::FindDiag::multiple(n), &path),
                        disambiguation_hint(&content, old_string, &starts)
                    );
                    log_edit_operation_ex(
                        &ctx.working_dir,
                        &path,
                        EntryExtras {
                            tool: "edit",
                            old_str: old_string,
                            new_str: new_string,
                            outcome: &err,
                            dry_run,
                            match_lane: Some("multiple"),
                            note: length_note(old_string),
                        },
                    );
                    bail!(err);
                }
            }
        } else {
            match find_replacement_cascade(&content, old_string, new_string) {
                CascadeMatch::Found {
                    lane,
                    start,
                    end,
                    new_str,
                } => (lane, start, end, new_str),
                CascadeMatch::Failed(CascadeFail::NotFound) => {
                    // P2.6: line-similarity hint so the next attempt lands closer.
                    let err =
                        super::replace::not_found_hint(&content, old_string, &path, None, false);
                    log_edit_operation_ex(
                        &ctx.working_dir,
                        &path,
                        EntryExtras {
                            tool: "edit",
                            old_str: old_string,
                            new_str: new_string,
                            outcome: &err,
                            dry_run,
                            match_lane: Some("not_found"),
                            note: length_note(old_string),
                        },
                    );
                    bail!(err);
                }
                CascadeMatch::Failed(CascadeFail::MultipleMatches {
                    lane: m_lane,
                    count,
                    starts,
                }) => {
                    // P2.7: show each candidate's location so the model can
                    // extend old_string on the next attempt.
                    let hint = disambiguation_hint(&content, old_string, &starts);
                    let err = format!(
                        "{}\n{}",
                        format_match_failure(&super::replace::FindDiag::multiple(count), &path),
                        hint
                    );
                    log_edit_operation_ex(
                        &ctx.working_dir,
                        &path,
                        EntryExtras {
                            tool: "edit",
                            old_str: old_string,
                            new_str: new_string,
                            outcome: &err,
                            dry_run,
                            match_lane: Some(m_lane.as_str()),
                            note: length_note(old_string),
                        },
                    );
                    bail!(err);
                }
            }
        };
        if dry_run {
            let snippet = build_snippet(&content, start, end);
            let old_lines = old_string.lines().count();
            let new_lines = new_str.lines().count();
            let path_str = path.display().to_string();
            log_edit_operation_ex(
                &ctx.working_dir,
                &path,
                EntryExtras {
                    tool: "edit",
                    old_str: old_string,
                    new_str: new_string,
                    outcome: "success (dry-run preview)",
                    dry_run,
                    match_lane: Some(lane.as_str()),
                    note: None,
                },
            );
            return Ok(ToolOutput {
                content: snippet.clone(),
                metadata: Some(json!({
                    "path": path_str,
                    "dry_run": true,
                    "match_lane": lane.as_str(),
                    "old_lines": old_lines,
                    "new_lines": new_lines,
                    "lines": old_lines.max(new_lines),
                    "snippet": snippet,
                })),
            });
        }

        // ── Apply the replacement ────────────────────────────────────────────────────────────────
        let new_content = format!("{}{}{}", &content[..start], new_str, &content[end..]);

        let write_result = tokio::fs::write(&path, &new_content)
            .await
            .with_context(|| format!("Failed to write file: {}", path.display()));
        if let Err(ref e) = write_result {
            log_edit_operation(
                &ctx.working_dir,
                "edit",
                &path,
                old_string,
                new_string,
                &format!("write error: {e}"),
                dry_run,
            );
        }
        write_result?;

        // Record the new mtime so a subsequent edit in the same session does not
        // trip the stale-file check on the file we just wrote. This also updates
        // the read baseline (P1.2) so the next edit sees the post-write content.
        record_edit_timestamp(&path, ctx);

        // ── Build the result snippet (FR-008) ───────────────���─────────────────
        let snippet = build_snippet(&new_content, start, start + new_str.len());

        // `old_string` may be the user-provided text; report the replaced byte span size.
        let old_lines = content[start..end].lines().count().max(1);
        let new_lines = new_str.lines().count();
        let lines_changed = old_lines.max(new_lines);

        let mut metadata = json!({
            "path": path.display().to_string(),
            "old_lines": old_lines,
            "new_lines": new_lines,
            "lines": lines_changed,
            "dry_run": false,
            "collapse_whitespace": collapse_whitespace,
            "match_lane": lane.as_str(),
            "buffer_note": "The on-disk file now differs from any copy in your context. Re-read before composing the next edit.",
            "snippet": snippet,
        });

        if used_legacy_params {
            metadata["deprecation_warning"] = json!(
                "Legacy parameter names (path/old_str/new_str) are deprecated. \
                 Use file_path/old_string/new_string instead."
            );
        }
        log_edit_operation_ex(
            &ctx.working_dir,
            &path,
            EntryExtras {
                tool: "edit",
                old_str: old_string,
                new_str: new_string,
                outcome: "success",
                dry_run,
                match_lane: Some(lane.as_str()),
                note: None,
            },
        );

        Ok(ToolOutput {
            content: snippet,
            metadata: Some(metadata),
        })
    }
}

/// Handle the create-file operation (FR-006): `old_string` is empty and the
/// file must not already exist. Writes `new_string` to a new file and returns
/// a snippet of the created content.
///
/// `#[allow(dead_code)]` — used by the lib build but not by the test target that
/// re-imports this source via `#[path]`.
#[allow(dead_code)]
async fn create_file(
    path: &Path,
    new_string: &str,
    ctx: &ToolContext,
    used_legacy_params: bool,
    dry_run: bool,
) -> Result<ToolOutput> {
    if path.exists() {
        log_edit_operation(
            &ctx.working_dir,
            "edit",
            path,
            "",
            new_string,
            "create rejected: file already exists",
            dry_run,
        );
        bail!(
            "Cannot create file '{}': it already exists. \
             To edit an existing file, provide a non-empty old_string that \
             matches exactly once.",
            path.display()
        );
    }

    if let Some(parent) = path.parent()
        && !parent.as_os_str().is_empty()
        && !parent.exists()
    {
        let parent_result = tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create parent dirs for {}", path.display()));
        if let Err(ref e) = parent_result {
            log_edit_operation(
                &ctx.working_dir,
                "edit",
                path,
                "",
                new_string,
                &format!("mkdir error: {e}"),
                dry_run,
            );
        }
        parent_result?;
    }

    let write_result = tokio::fs::write(path, new_string)
        .await
        .with_context(|| format!("Failed to write new file: {}", path.display()));
    if let Err(ref e) = write_result {
        log_edit_operation(
            &ctx.working_dir,
            "edit",
            path,
            "",
            new_string,
            &format!("write error: {e}"),
            dry_run,
        );
    }
    write_result?;

    record_edit_timestamp(path, ctx);

    let snippet = build_snippet(new_string, 0, new_string.len());

    let new_lines = new_string.lines().count();
    let mut metadata = json!({
        "path": path.display().to_string(),
        "old_lines": 0,
        "new_lines": new_lines,
        "lines": new_lines,
        "created": true,
        "snippet": snippet,
    });

    if used_legacy_params {
        metadata["deprecation_warning"] = json!(
            "Legacy parameter names (path/old_str/new_str) are deprecated. \
             Use file_path/old_string/new_string instead."
        );
    }
    log_edit_operation(
        &ctx.working_dir,
        "edit",
        path,
        "",
        new_string,
        "success",
        dry_run,
    );

    Ok(ToolOutput {
        content: snippet,
        metadata: Some(metadata),
    })
}

/// Build a `cat -n`-style line-numbered snippet of `content` centred on the
/// edited byte range `[change_start, change_end)` with at least
/// [`SNIPPET_CONTEXT_LINES`] lines of context before and after, clamped to the
/// file boundaries (FR-008).
#[must_use]
pub fn build_snippet(content: &str, change_start: usize, change_end: usize) -> String {
    // Determine the 1-based line numbers of the edited region.
    let change_start_line = byte_offset_to_line(content, change_start);
    let change_end_line =
        byte_offset_to_line(content, change_end.saturating_sub(1).max(change_start));

    let total_lines = content.lines().count().max(1);

    let snippet_start = change_start_line
        .saturating_sub(SNIPPET_CONTEXT_LINES)
        .max(1);
    let snippet_end = (change_end_line + SNIPPET_CONTEXT_LINES).min(total_lines);

    // Iterate lines directly instead of collecting into a Vec<&str>, which
    // avoids allocating a vector of line slices for the entire file. Break
    // early once we pass snippet_end.
    let mut out = String::new();
    for (idx, line) in content.lines().enumerate() {
        let line_no = idx + 1;
        if line_no > snippet_end {
            break;
        }
        if line_no < snippet_start {
            continue;
        }
        let marker = if line_no >= change_start_line && line_no <= change_end_line {
            ">"
        } else {
            " "
        };
        out.push_str(&format!("{line_no:>4}{marker} {line}\n"));
    }
    if out.is_empty() {
        // Edge case: empty file or empty new_string with no trailing newline.
        out.push_str("    1  \n");
    }
    out
}

/// Convert a byte offset into `content` to a 1-based line number.
#[must_use]
pub fn byte_offset_to_line(content: &str, offset: usize) -> usize {
    let mut line = 1;
    for (i, ch) in content.char_indices() {
        if i >= offset {
            break;
        }
        if ch == '\n' {
            line += 1;
        }
    }
    line
}

// ── Unit tests ─────────────────────────────────────────────────────────��─────
