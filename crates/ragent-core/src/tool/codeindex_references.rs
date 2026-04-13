//! Codebase index reference lookup tool.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Find all references to a named symbol across the codebase.
pub struct CodeIndexReferencesTool;

fn not_available() -> ToolOutput {
    ToolOutput {
        content: "Code index is not available. It may be disabled or not yet initialised. \
                  Use `lsp_references` or `grep` as fallback tools."
            .to_string(),
        metadata: Some(json!({
            "error": "codeindex_disabled",
            "fallback_tools": ["lsp_references", "grep"]
        })),
    }
}

#[async_trait::async_trait]
impl Tool for CodeIndexReferencesTool {
    fn name(&self) -> &'static str {
        "codeindex_references"
    }

    fn description(&self) -> &'static str {
        "Find all references to a symbol by name across the indexed codebase. \
         Returns file locations grouped by file, with reference kind (call, type, field_access)."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "symbol": {
                    "type": "string",
                    "description": "The symbol name to find references for"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum references to return (default: 50, max: 200)"
                }
            },
            "required": ["symbol"],
            "additionalProperties": false
        })
    }

    fn permission_category(&self) -> &'static str {
        "codeindex:read"
    }

    async fn execute(&self, input: Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let idx = match &ctx.code_index {
            Some(idx) => idx,
            None => return Ok(not_available()),
        };

        let symbol = input["symbol"]
            .as_str()
            .context("Missing required 'symbol' parameter")?;

        let limit = input["limit"]
            .as_u64()
            .map(|n| n.min(200) as usize)
            .unwrap_or(50);

        let refs = idx.references(symbol, limit)?;

        if refs.is_empty() {
            return Ok(ToolOutput {
                content: format!("No references found for '{symbol}'."),
                metadata: Some(json!({"total_results": 0})),
            });
        }

        // Group by file path for readable output.
        let mut output = String::new();
        let mut current_path: Option<String> = None;

        for r in &refs {
            let display_path = if r.file_path.is_empty() {
                format!("file_id:{}", r.file_id)
            } else {
                r.file_path.clone()
            };
            if current_path.as_deref() != Some(&display_path) {
                current_path = Some(display_path.clone());
                output.push_str(&format!("\n── {} ──\n", display_path));
            }
            output.push_str(&format!(
                "  L{}:{} — {} ({})\n",
                r.line, r.col, r.symbol_name, r.kind
            ));
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({"total_results": refs.len()})),
        })
    }
}
