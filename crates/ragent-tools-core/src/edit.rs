//! Surgical text replacement tool for file editing.
//!
//! Provides [`EditTool`], the renewed single-file edit tool aligned with
//! Claude Code's `Edit` semantics (editrenewal spec). It replaces exactly one
//! occurrence of `old_string` with `new_string` in a file using **strict
//! exact-match** replacement — whitespace, indentation, and line endings must
//! match byte-for-byte.
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
//! # Stale-file detection (FR-003)
//!
//! When the session has recorded a read timestamp for `file_path` (via the
//! `read` tool), the edit tool compares the file's current mtime against that
//! recorded timestamp and rejects the edit if the file was modified after it
//! was read. When no timestamp has been recorded, the edit proceeds (no
//! baseline is available).
//!
//! # Result snippet (FR-008)
//!
//! On success the tool returns a `cat -n`-style snippet of the edited file
//! with at least four lines of context before and after the change, clamped to
//! the file boundaries.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::Path;
use std::time::SystemTime;

use super::path_util::resolve_path;
use super::replace::{FindError, find_exact_replacement_range};
use super::{Tool, ToolContext, ToolOutput};

/// Minimum lines of context to show before and after the edited region in the
/// result snippet (FR-008).
const SNIPPET_CONTEXT_LINES: usize = 4;

/// Replaces an exact, unique occurrence of `old_string` with `new_string` in a
/// file, matching Claude Code `Edit` semantics.
///
/// The search string must match exactly once; zero or multiple matches are
/// treated as errors to prevent ambiguous edits. Supports create, update, and
/// delete operations via empty `old_string` / `new_string` (FR-006).
#[allow(dead_code)]
pub struct EditTool;

#[async_trait::async_trait]
impl Tool for EditTool {
    fn name(&self) -> &'static str {
        "edit"
    }

    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &'static str {
        "Replace exactly one occurrence of old_string with new_string in a file. \
         old_string must match exactly once, byte-for-byte (including whitespace and \
         indentation). Use an empty old_string with a non-existent file to create it; \
         an empty new_string deletes the matched text. Include 3–5 lines of context \
         around the change point so the match is unique. The result includes a \
         line-numbered snippet of the edited region."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "file_path": {
                    "type": "string",
                    "description": "Absolute path to the file to edit"
                },
                "old_string": {
                    "type": "string",
                    "description": "Exact string to find and replace (must match exactly once, including whitespace). Empty string creates a new file."
                },
                "new_string": {
                    "type": "string",
                    "description": "Replacement string. Empty string deletes the matched text."
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
            "required": ["file_path", "old_string", "new_string"]
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
    /// - `old_string` is not found in the file (FR-004)
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

        let path = resolve_path(&ctx.working_dir, path_str);

        super::check_path_within_root(&path, &ctx.working_dir)?;

        // Acquire file lock to serialize concurrent edits to the same file.
        let _lock = super::file_lock::lock_file(&path).await;

        // ── Create operation (FR-006): empty old_string ───────────────────────
        if old_string.is_empty() {
            return create_file(&path, new_string, ctx, used_legacy_params).await;
        }

        // ── No-change rejection (FR-007) ──────────────────────────────────────
        if old_string == new_string {
            bail!(
                "old_string and new_string are identical in {}. \
                 No changes would be made; refusing the no-op edit.",
                path.display()
            );
        }

        // ── Read the file ─────────────────────────────────────────────────────
        let content = tokio::fs::read_to_string(&path).await.with_context(|| {
            format!(
                "Cannot read file '{}': file may not exist or is not accessible",
                path.display()
            )
        })?;

        // ── Stale-file detection (FR-003) ─────────────────────────────────────
        check_stale_file(&path, ctx)?;

        // ── Strict exact-match replacement (FR-004, FR-005) ───────────────────
        let (start, end, effective_new_str) =
            match find_exact_replacement_range(&content, old_string, new_string) {
                Ok(range) => range,
                Err(FindError::NotFound) => bail!(
                    "old_string not found in {}. \
                     Strict exact match requires the string to occur verbatim, including \
                     whitespace and indentation. Re-read the file and copy the exact text.",
                    path.display()
                ),
                Err(FindError::MultipleMatches(n)) => bail!(
                    "old_string found {} times in {}. \
                     It must match exactly once. Add more surrounding context to the \
                     old_string to make the match unique.",
                    n,
                    path.display()
                ),
            };

        // ── Apply the replacement ─────────────────────────────────────────────
        let new_content = format!(
            "{}{}{}",
            &content[..start],
            effective_new_str,
            &content[end..]
        );

        tokio::fs::write(&path, &new_content)
            .await
            .with_context(|| format!("Failed to write file: {}", path.display()))?;

        // Record the new mtime so a subsequent edit in the same session does not
        // trip the stale-file check on the file we just wrote.
        record_edit_timestamp(&path, ctx);

        // ── Build the result snippet (FR-008) ─────────────────────────────────
        let snippet = build_snippet(&new_content, start, start + effective_new_str.len());

        let old_lines = old_string.lines().count();
        let new_lines = effective_new_str.lines().count();
        let lines_changed = old_lines.max(new_lines);

        let mut metadata = json!({
            "path": path.display().to_string(),
            "old_lines": old_lines,
            "new_lines": new_lines,
            "lines": lines_changed,
            "snippet": snippet,
        });

        if used_legacy_params {
            metadata["deprecation_warning"] = json!(
                "Legacy parameter names (path/old_str/new_str) are deprecated. \
                 Use file_path/old_string/new_string instead."
            );
        }

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
) -> Result<ToolOutput> {
    if path.exists() {
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
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("Failed to create parent dirs for {}", path.display()))?;
    }

    tokio::fs::write(path, new_string)
        .await
        .with_context(|| format!("Failed to write new file: {}", path.display()))?;

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

    Ok(ToolOutput {
        content: snippet,
        metadata: Some(metadata),
    })
}

/// Check whether the file was modified after the session last read it
/// (FR-003). When a read timestamp has been recorded for `path`, compare the
/// current on-disk mtime against it and reject the edit if the file is newer.
///
/// When no timestamp has been recorded, the edit proceeds — no baseline is
/// available, so the stale-file check is a no-op. This keeps the tool usable
/// for one-shot edits while delivering the critical safety property for
/// sessions that use the `read` tool before editing.
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
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0)
        });

    let Some(current_millis) = current_millis else {
        return Ok(());
    };

    // Allow a small 1ms tolerance to avoid spurious rejections from filesystem
    // mtime granularity rounding when the read and the edit happen in the same
    // tick.
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
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        if let Ok(mut map) = ctx.read_timestamps.write() {
            map.insert(path.to_path_buf(), millis);
        }
    }
}

/// Build a `cat -n`-style line-numbered snippet of `content` centred on the
/// edited byte range `[change_start, change_end)` with at least
/// [`SNIPPET_CONTEXT_LINES`] lines of context before and after, clamped to the
/// file boundaries (FR-008).
pub(crate) fn build_snippet(content: &str, change_start: usize, change_end: usize) -> String {
    // Determine the 1-based line numbers of the edited region.
    let change_start_line = byte_offset_to_line(content, change_start);
    let change_end_line =
        byte_offset_to_line(content, change_end.saturating_sub(1).max(change_start));

    let total_lines = content.lines().count().max(1);

    let snippet_start = change_start_line
        .saturating_sub(SNIPPET_CONTEXT_LINES)
        .max(1);
    let snippet_end = (change_end_line + SNIPPET_CONTEXT_LINES).min(total_lines);

    let lines: Vec<&str> = content.lines().collect();
    let mut out = String::new();
    for (idx, line) in lines.iter().enumerate() {
        let line_no = idx + 1;
        if line_no < snippet_start || line_no > snippet_end {
            continue;
        }
        let marker = if line_no >= change_start_line && line_no <= change_end_line {
            ">"
        } else {
            " "
        };
        out.push_str(&format!("{:>4}{} {}\n", line_no, marker, line));
    }
    if out.is_empty() {
        // Edge case: empty file or empty new_string with no trailing newline.
        out.push_str("    1  \n");
    }
    out
}

/// Convert a byte offset into `content` to a 1-based line number.
pub(crate) fn byte_offset_to_line(content: &str, offset: usize) -> usize {
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
