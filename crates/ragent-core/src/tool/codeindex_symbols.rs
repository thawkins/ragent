//! Codebase index symbol query tool.

use anyhow::Result;
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Query structured symbol information from the code index.
///
/// Returns symbols (functions, structs, enums, traits, etc.) with their
/// location, visibility, and signature.
pub struct CodeIndexSymbolsTool;

fn not_available() -> ToolOutput {
    ToolOutput {
        content: "Code index is not available. It may be disabled or not yet initialised. \
                  Use `lsp_symbols` or `grep` as fallback tools."
            .to_string(),
        metadata: Some(json!({
            "error": "codeindex_disabled",
            "fallback_tools": ["lsp_symbols", "grep"]
        })),
    }
}

#[async_trait::async_trait]
impl Tool for CodeIndexSymbolsTool {
    fn name(&self) -> &'static str {
        "codeindex_symbols"
    }

    fn description(&self) -> &'static str {
        "Query symbols (functions, structs, enums, traits) from the codebase index. \
         Supports filtering by name, kind, file, language, and visibility. \
         Returns structured results with location, signature, and documentation."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "name": {
                    "type": "string",
                    "description": "Filter by symbol name (case-insensitive substring match)"
                },
                "kind": {
                    "type": "string",
                    "description": "Filter by symbol kind",
                    "enum": ["function", "struct", "enum", "trait", "impl", "const", "static", "type_alias", "module", "macro", "field", "variant", "interface", "class", "method"]
                },
                "file_path": {
                    "type": "string",
                    "description": "Filter by file path (substring match)"
                },
                "language": {
                    "type": "string",
                    "description": "Filter by language (e.g. 'rust')"
                },
                "visibility": {
                    "type": "string",
                    "description": "Filter by visibility",
                    "enum": ["public", "private", "crate"]
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum results (default: 50, max: 200)"
                }
            },
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

        let name = input["name"].as_str().map(String::from);
        let kind = input["kind"].as_str().and_then(|k| {
            k.parse::<ragent_code::types::SymbolKind>().ok()
        });
        let file_path = input["file_path"].as_str().map(String::from);
        let language = input["language"].as_str().map(String::from);
        let visibility = input["visibility"].as_str().and_then(|v| {
            v.parse::<ragent_code::types::Visibility>().ok()
        });
        let limit = input["limit"]
            .as_u64()
            .map(|n| n.min(200) as u32)
            .unwrap_or(50);

        let filter = ragent_code::types::SymbolFilter {
            name,
            kind,
            file_path,
            language,
            visibility,
            limit: Some(limit),
        };

        let symbols = idx.symbols(&filter)?;

        if symbols.is_empty() {
            return Ok(ToolOutput {
                content: "No symbols matched the filter.".to_string(),
                metadata: Some(json!({"total_results": 0})),
            });
        }

        // Group by file for cleaner output.
        let mut output = String::new();
        let mut current_file: Option<i64> = None;

        for sym in &symbols {
            if current_file != Some(sym.file_id) {
                current_file = Some(sym.file_id);
                output.push_str(&format!("\n── file_id:{} ──\n", sym.file_id));
            }
            let vis = &sym.visibility;
            output.push_str(&format!(
                "  [{vis}] {} `{}` (L{}-{})",
                sym.kind, sym.name, sym.start_line, sym.end_line
            ));
            if let Some(ref sig) = sym.signature {
                output.push_str(&format!("\n       {sig}"));
            }
            output.push('\n');
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({"total_results": symbols.len()})),
        })
    }
}
