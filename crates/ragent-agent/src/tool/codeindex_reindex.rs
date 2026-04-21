//! Codebase index re-index trigger tool.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Trigger a full re-index of the codebase.
pub struct CodeIndexReindexTool;

fn not_available() -> ToolOutput {
    ToolOutput {
        content: "Code index is not available. It may be disabled or not yet initialised. \
                  Use `/codeindex on` to enable it."
            .to_string(),
        metadata: Some(json!({
            "error": "codeindex_disabled",
            "fallback_tools": []
        })),
    }
}

#[async_trait::async_trait]
impl Tool for CodeIndexReindexTool {
    fn name(&self) -> &'static str {
        "codeindex_reindex"
    }

    fn description(&self) -> &'static str {
        "Trigger a full re-index of the codebase. \
         Scans all files, extracts symbols, and updates the search index. \
         Use after major file changes or when search results seem stale."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {},
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "codeindex:write"
    }

    async fn execute(&self, _input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let idx = match &ctx.code_index {
            Some(idx) => idx,
            None => return Ok(not_available()),
        };

        let result = idx.full_reindex()?;

        let output = format!(
            "Re-index complete: +{} ~{} -{} files, {} symbols in {}ms",
            result.files_added,
            result.files_updated,
            result.files_removed,
            result.symbols_extracted,
            result.elapsed_ms,
        );

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({
                "files_added": result.files_added,
                "files_updated": result.files_updated,
                "files_removed": result.files_removed,
                "symbols_extracted": result.symbols_extracted,
                "elapsed_ms": result.elapsed_ms,
            })),
        })
    }
}
