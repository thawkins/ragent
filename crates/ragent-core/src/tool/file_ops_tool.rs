//! Concurrent file operations tool for batch reading and atomic editing.
//!
//! This tool provides `file_batch_read` and `file_batch_edit` operations for
//! efficiently reading multiple files in parallel and staging/committing edits
//! with conflict detection.

use super::{Tool, ToolContext, ToolOutput};
use crate::file_ops::{CommitResult, apply_batch_edits};
use anyhow::{Context, Result};
use serde_json::{Value, json};
use std::path::Path;

/// Tool that applies a batch of edits using the EditStaging flow.
///
/// Input JSON schema:
/// {
///   "edits": [{ "path": "path/to/file", "content": "new content" }, ...],
///   "concurrency": optional integer,
///   "dry_run": optional bool
/// }
pub struct FileOpsTool;

#[async_trait::async_trait]
impl Tool for FileOpsTool {
    /// # Errors
    ///
    /// Returns an error if the name string cannot be converted or returned.
    fn name(&self) -> &str {
        "file_ops"
    }

    /// # Errors
    ///
    /// Returns an error if the description string cannot be converted or returned.
    fn description(&self) -> &str {
        "Apply a batch of staged edits concurrently using the EditStaging APIs"
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "edits": {
                    "type": "array",
                    "items": { "type": "object", "properties": { "path": {"type":"string"}, "content": {"type":"string"}}, "required": ["path","content"] }
                },
                "concurrency": { "type": "integer" },
                "dry_run": { "type": "boolean" }
            },
            "required": ["edits"]
        })
    }

    /// # Errors
    ///
    /// Returns an error if the category string cannot be converted or returned.
    fn permission_category(&self) -> &str {
        "file:write"
    }

    /// # Errors
    ///
    /// Returns an error if the `edits` array is missing, if any edit is missing
    /// required fields, or if the batch edit operation fails.
    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let edits_arr = input["edits"].as_array().context("Missing 'edits' array")?;
        let concurrency = input["concurrency"]
            .as_i64()
            .map(|n| n as usize)
            .unwrap_or_else(|| num_cpus::get());
        let dry_run = input["dry_run"].as_bool().unwrap_or(false);

        let mut pairs = Vec::with_capacity(edits_arr.len());
        for e in edits_arr {
            let path = e["path"]
                .as_str()
                .context("edit.path missing or not string")?;
            let content = e["content"]
                .as_str()
                .context("edit.content missing or not string")?;
            let resolved = if std::path::Path::new(path).is_absolute() {
                Path::new(path).to_path_buf()
            } else {
                ctx.working_dir.join(path)
            };
            pairs.push((resolved, content.to_string()));
        }

        let res: CommitResult = apply_batch_edits(pairs, concurrency, dry_run).await?;

        let summary = format!(
            "applied={} conflicts={} errors={}",
            res.applied.len(),
            res.conflicts.len(),
            res.errors.len()
        );
        Ok(ToolOutput {
            content: summary,
            metadata: Some(
                json!({"applied": res.applied.len(), "conflicts": res.conflicts.len(), "errors": res.errors.len()}),
            ),
        })
    }
}
