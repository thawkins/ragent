//! Codebase index full-text search tool.

use anyhow::{Context, Result};
use serde_json::{Value, json};

use super::{Tool, ToolContext, ToolOutput};

/// Search the codebase index for symbols, functions, types, and documentation
/// using full-text search with optional structured filters.
pub struct CodeIndexSearchTool;

/// Build a "not available" response when the code index is disabled.
fn not_available() -> ToolOutput {
    ToolOutput {
        content: "Code index is not available. It may be disabled or not yet initialised. \
                  Use `grep` or `glob` as fallback tools for code search."
            .to_string(),
        metadata: Some(json!({
            "error": "codeindex_disabled",
            "fallback_tools": ["grep", "glob", "lsp_symbols", "lsp_references"]
        })),
    }
}

#[async_trait::async_trait]
impl Tool for CodeIndexSearchTool {
    fn name(&self) -> &'static str {
        "codeindex_search"
    }

    fn description(&self) -> &'static str {
        "Search the codebase index for symbols, functions, types, and documentation. \
         Uses full-text search with optional filters by kind, language, and file path. \
         USE THIS instead of `grep` or `search` when looking for named code entities \
         (functions, structs, enums, traits, variables). The index is faster, returns \
         structured results with file/line/signature, and understands symbol kinds. \
         Only use `grep`/`search` for arbitrary text patterns, comments, or non-symbol content."
    }

    fn parameters_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Search query — symbol name, keyword, or phrase to find in the codebase"
                },
                "kind": {
                    "type": "string",
                    "description": "Filter by symbol kind: function, struct, enum, trait, impl, const, static, type_alias, module, macro, field, variant, interface, class, method",
                    "enum": ["function", "struct", "enum", "trait", "impl", "const", "static", "type_alias", "module", "macro", "field", "variant", "interface", "class", "method"]
                },
                "language": {
                    "type": "string",
                    "description": "Filter by programming language (e.g. 'rust', 'python', 'typescript')"
                },
                "file_pattern": {
                    "type": "string",
                    "description": "Filter by file path substring (e.g. 'src/parser' or '.rs')"
                },
                "max_results": {
                    "type": "integer",
                    "description": "Maximum results to return (default: 20, max: 100)"
                }
            },
            "required": ["query"],
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

        let query_str = input["query"]
            .as_str()
            .context("Missing required 'query' parameter")?;

        let kind = input["kind"]
            .as_str()
            .and_then(|k| k.parse::<ragent_code::types::SymbolKind>().ok());
        let language = input["language"].as_str().map(String::from);
        let file_pattern = input["file_pattern"].as_str().map(String::from);
        let max_results = input["max_results"]
            .as_u64()
            .map(|n| n.min(100) as usize)
            .unwrap_or(20);

        let search_query = ragent_code::types::SearchQuery {
            query: query_str.to_string(),
            kind,
            language,
            file_pattern,
            max_results,
            include_body: false,
        };

        let results = idx.search(&search_query)?;

        if results.is_empty() {
            return Ok(ToolOutput {
                content: format!("No results found for '{query_str}'."),
                metadata: Some(json!({"total_results": 0})),
            });
        }

        let mut output = String::new();
        for (i, r) in results.iter().enumerate() {
            output.push_str(&format!(
                "{}. {} `{}` — {}:{}\n",
                i + 1,
                r.kind,
                r.symbol_name,
                r.file_path,
                r.line,
            ));
            if !r.signature.is_empty() {
                output.push_str(&format!("   {}\n", r.signature));
            }
            if !r.doc_snippet.is_empty() {
                let doc = if r.doc_snippet.len() > 120 {
                    format!("{}…", &r.doc_snippet[..120])
                } else {
                    r.doc_snippet.clone()
                };
                output.push_str(&format!("   /// {doc}\n"));
            }
        }

        Ok(ToolOutput {
            content: output,
            metadata: Some(json!({"total_results": results.len()})),
        })
    }
}
