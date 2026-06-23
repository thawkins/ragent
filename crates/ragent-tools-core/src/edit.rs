//! Surgical text replacement tool for file editing.
//!
//! Provides [`EditTool`], which replaces exactly one occurrence of a search
//! string with a replacement string in a file, ensuring precise edits.
//!
//! Matching is delegated to the shared seven-pass matcher in
//! [`super::replace`], which handles common LLM output quirks: exact match,
//! CRLF normalisation, trailing/leading whitespace stripping, collapsed
//! whitespace, blank-line normalisation, and final-newline normalisation.
//! See [`ragent_tools_core::replace`] for the full pass documentation.
//!
//! [`FindError`] and [`find_replacement_range`] are re-exported here so that
//! existing callers (e.g. `multiedit`) can keep importing them from `edit`
//! without churn.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

use super::replace::{FindError, find_replacement_range};
use super::{Tool, ToolContext, ToolOutput};

/// Replaces an exact, unique occurrence of `old_str` with `new_str` in a file.
///
/// The search string must match exactly once; zero or multiple matches are
/// treated as errors to prevent ambiguous edits.
pub struct EditTool;

#[async_trait::async_trait]
impl Tool for EditTool {
    fn name(&self) -> &'static str {
        "edit"
    }

    /// Returns a human-readable description of what the tool does.
    fn description(&self) -> &'static str {
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

    fn permission_category(&self) -> &'static str {
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

        super::check_path_within_root(&path, &ctx.working_dir)?;

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
            content: String::new(), // Empty on success; errors are returned as Err
            metadata: Some(json!({
                "path": path.display().to_string(),
                "old_lines": old_lines,
                "new_lines": new_lines,
                "lines": lines_changed,
            })),
        })
    }
}

fn resolve_path(working_dir: &Path, path_str: &str) -> PathBuf {
    let p = PathBuf::from(path_str);
    if p.is_absolute() {
        p
    } else {
        working_dir.join(p)
    }
}
