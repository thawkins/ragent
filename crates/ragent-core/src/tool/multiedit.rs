//! Batch text replacement tool for editing multiple files.
//!
//! Provides [`MultiEditTool`], which applies multiple search-and-replace
//! operations across one or more files atomically. All edits are validated
//! before any files are written — if any match fails, no files are modified.

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

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

#[async_trait::async_trait]
impl Tool for MultiEditTool {
    fn name(&self) -> &str {
        "multiedit"
    }

    fn description(&self) -> &str {
        "Apply multiple edits to one or more files atomically. Each edit replaces \
         exactly one occurrence of old_str with new_str. All edits are validated \
         before any files are written — if any match fails, no files are modified."
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

    fn permission_category(&self) -> &str {
        "file:write"
    }

    /// Executes all edits atomically.
    ///
    /// # Errors
    ///
    /// Returns an error if the `edits` array is missing or empty, if any file
    /// cannot be read, or if any `old_str` does not match exactly once in its
    /// target file.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let edits_arr = input["edits"]
            .as_array()
            .context("Missing 'edits' array parameter")?;

        if edits_arr.is_empty() {
            bail!("edits array is empty");
        }

        // Parse all edit operations
        let mut ops: Vec<EditOp> = Vec::with_capacity(edits_arr.len());
        for (i, edit) in edits_arr.iter().enumerate() {
            let path_str = edit["path"]
                .as_str()
                .with_context(|| format!("Edit {}: missing 'path'", i))?;
            let old_str = edit["old_str"]
                .as_str()
                .with_context(|| format!("Edit {}: missing 'old_str'", i))?;
            let new_str = edit["new_str"]
                .as_str()
                .with_context(|| format!("Edit {}: missing 'new_str'", i))?;

            ops.push(EditOp {
                path: resolve_path(&ctx.working_dir, path_str),
                old_str: old_str.to_string(),
                new_str: new_str.to_string(),
            });
        }

        // Phase 1: Read all target files and validate every edit
        // Group edits by file path so we apply them sequentially to the same content.
        let mut file_contents: HashMap<PathBuf, String> = HashMap::new();
        for op in &ops {
            if !file_contents.contains_key(&op.path) {
                let content = tokio::fs::read_to_string(&op.path)
                    .await
                    .with_context(|| format!("Failed to read file: {}", op.path.display()))?;
                file_contents.insert(op.path.clone(), content);
            }
        }

        // Phase 2: Apply edits in order to in-memory content, validating each
        let mut files_modified: HashMap<PathBuf, usize> = HashMap::new();
        let mut total_edits = 0usize;
        let mut total_lines_changed = 0usize;

        for (i, op) in ops.iter().enumerate() {
            let content = file_contents
                .get_mut(&op.path)
                .expect("file content must exist");

            let count = content.matches(&op.old_str).count();
            if count == 0 {
                bail!(
                    "Edit {}: old_str not found in {}. Make sure it matches exactly.",
                    i,
                    op.path.display()
                );
            }
            if count > 1 {
                bail!(
                    "Edit {}: old_str found {} times in {}. It must match exactly once. \
                     Add more context to make it unique.",
                    i,
                    count,
                    op.path.display()
                );
            }

            *content = content.replacen(&op.old_str, &op.new_str, 1);
            *files_modified.entry(op.path.clone()).or_insert(0) += 1;
            total_edits += 1;
            total_lines_changed += op.new_str.lines().count();
        }

        // Phase 3: Write all modified files
        for (path, content) in &file_contents {
            if files_modified.contains_key(path) {
                tokio::fs::write(path, content)
                    .await
                    .with_context(|| format!("Failed to write file: {}", path.display()))?;
            }
        }

        let file_count = files_modified.len();
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
                "files": file_count,
                "edits": total_edits,
                "lines": total_lines_changed,
            })),
        })
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
